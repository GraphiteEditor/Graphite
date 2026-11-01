// Migrations asking the compiler run as part of opening, in order, and cost nothing to documents they don't apply to
#![cfg(feature = "migrations")]

use document_container::AnyContainer;
use document_container::backends::memory::MemoryBackend;
use document_format::{GddV1, GddV1Layout};
use document_graph_storage::{EncodedDeclaration, Implementation, NoMetadata, PeerId, Registry, UserId};
use futures::executor::block_on;
use graph_craft::application_io::resource::HashMapResourceStorage;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeId, NodeNetwork};
use graph_craft::{ProtoNodeIdentifier, concrete};
use graphene_resource::{LoadResource, ResourceFuture, ResourceHash, ResourceRegistry, ResourceStorage};
use migration_core::{Compiler, DeclarationResolver, MigrationHost, Node, Target, ValueType, decl, migration};
use migration_formats as _;
use std::sync::atomic::{AtomicUsize, Ordering};

struct IsVector;

impl ValueType for IsVector {
	fn element_name(&self) -> Option<&str> {
		Some(std::any::type_name::<IsVector>())
	}
}

struct FakeCompiler;

impl Compiler for FakeCompiler {
	fn input_type(&self, _node: &Node, _slot: usize) -> Option<Box<dyn ValueType>> {
		Some(Box::new(IsVector))
	}
}

/// Picks by what the compiler says about the state.
#[migration("test_nodes::CopyNode"@1 => "test_nodes::FallbackNode"@2)]
pub fn by_compiler(node: &Node, compiler: Option<&dyn Compiler>, target: &mut Target) {
	if compiler.and_then(|compiler| compiler.input_type(node, 0)).is_some_and(|value_type| value_type.element_is::<IsVector>()) {
		target.set(decl!("test_nodes::VectorNode"@1));
	}
}

/// Records that it was asked, and what the resolver knew when it was.
#[derive(Default)]
struct Host {
	asked: AtomicUsize,
	missing_declarations: AtomicUsize,
}

impl MigrationHost for Host {
	fn compiler(&self, state: &Registry, resolver: &dyn DeclarationResolver) -> Option<Box<dyn Compiler + '_>> {
		self.asked.fetch_add(1, Ordering::Relaxed);

		for node in state.node_instances.values() {
			if let Implementation::ProtoNode(resource) = node.implementation()
				&& resolver.proto(*resource).is_none()
			{
				self.missing_declarations.fetch_add(1, Ordering::Relaxed);
			}
		}
		Some(Box::new(FakeCompiler))
	}
}

/// A byte store that has nothing, like a device's cache that has never seen this document, and counts what it's asked for.
#[derive(Default)]
struct EmptyStore {
	loads: AtomicUsize,
}

impl LoadResource for EmptyStore {
	fn load(&self, _hash: ResourceHash) -> ResourceFuture<'_> {
		self.loads.fetch_add(1, Ordering::Relaxed);
		Box::pin(async { None })
	}
}

fn network_of(nodes: &[(u64, &'static str)]) -> NodeNetwork {
	let node = |identifier: &'static str| DocumentNode {
		implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new(identifier)),
		call_argument: concrete!(()),
		..Default::default()
	};
	NodeNetwork {
		nodes: nodes.iter().map(|(id, identifier)| (NodeId(*id), node(identifier))).collect(),
		..Default::default()
	}
}

fn stored(nodes: &[(u64, &'static str)]) -> AnyContainer {
	stored_with_bytes_in(nodes, &HashMapResourceStorage::new())
}

/// A document whose declaration bytes went to `store`, as the editor sends them to its global cache.
fn stored_with_bytes_in(nodes: &[(u64, &'static str)], store: &HashMapResourceStorage) -> AnyContainer {
	let mut gdd = GddV1::create_in(AnyContainer::Memory(MemoryBackend::new()), GddV1Layout, PeerId(7), UserId(7), 1, "editor".into(), "std".into()).unwrap();
	gdd.commit_from_runtime(&network_of(nodes), &NoMetadata, &ResourceRegistry::new(), store).unwrap();
	gdd.into_storage().0
}

fn implementation_of(identifier: &str, version: u32) -> Implementation {
	Implementation::ProtoNode(EncodedDeclaration::new(identifier.to_string(), version).unwrap().id)
}

#[test]
fn a_document_without_the_node_pays_nothing() {
	block_on(async {
		let (host, store) = (Host::default(), EmptyStore::default());
		let working = stored(&[(1, "test_nodes::OtherNode")]);

		let gdd = GddV1::open_in_with(working, GddV1Layout, Some(&store), Some(&host)).await.unwrap();

		assert!(!gdd.migrated());
		assert_eq!(host.asked.load(Ordering::Relaxed), 0, "the compiler is never built");
		assert_eq!(store.loads.load(Ordering::Relaxed), 0, "no declaration is loaded");
	});
}

#[test]
fn opening_runs_it_in_one_pass_and_the_result_persists() {
	block_on(async {
		let host = Host::default();
		let working = stored(&[(1, "test_nodes::CopyNode")]);

		let gdd = GddV1::open_in_with(working, GddV1Layout, None, Some(&host)).await.unwrap();
		let anchor = *gdd.registry().node_instances.keys().next().unwrap();
		assert_eq!(*gdd.registry().node_instances[&anchor].implementation(), implementation_of("test_nodes::VectorNode", 1));
		assert!(gdd.migrated());

		let target = EncodedDeclaration::new("test_nodes::VectorNode".to_string(), 1).unwrap();
		assert_eq!(gdd.declarations(&gdd).await[&target.id], target.proto, "the chosen declaration's bytes are stored");

		let registry = gdd.registry().clone();
		let (working, layout) = gdd.into_storage();
		let reopened = GddV1::open_in(working, layout).await.unwrap();
		assert!(reopened.registry().value_equal(&registry));
		assert!(!reopened.migrated());
	});
}

#[test]
fn declarations_come_from_the_container_when_the_cache_lacks_them() {
	block_on(async {
		let (host, store) = (Host::default(), EmptyStore::default());
		let nodes = [(1, "test_nodes::CopyNode"), (2, "test_nodes::OtherNode")];

		// An archive carries its declarations in the container, which a device's cache hasn't seen yet
		let mut gdd = GddV1::create_in(AnyContainer::Memory(MemoryBackend::new()), GddV1Layout, PeerId(7), UserId(7), 1, "editor".into(), "std".into()).unwrap();
		gdd.commit_from_runtime(&network_of(&nodes), &NoMetadata, &ResourceRegistry::new(), &HashMapResourceStorage::new())
			.unwrap();
		for (_, identifier) in nodes {
			gdd.store(&EncodedDeclaration::new(identifier.to_string(), 1).unwrap().bytes);
		}
		let working = gdd.into_storage().0;

		GddV1::open_in_with(working, GddV1Layout, Some(&store), Some(&host)).await.unwrap();

		assert!(store.loads.load(Ordering::Relaxed) > 0, "the cache is asked first");
		assert!(host.asked.load(Ordering::Relaxed) > 0);
		assert_eq!(host.missing_declarations.load(Ordering::Relaxed), 0, "the container fills in what the cache lacks");
	});
}

#[test]
fn declarations_come_from_the_cache_when_the_container_lacks_them() {
	block_on(async {
		let (host, store) = (Host::default(), HashMapResourceStorage::new());
		let working = stored_with_bytes_in(&[(1, "test_nodes::CopyNode"), (2, "test_nodes::OtherNode")], &store);

		GddV1::open_in_with(working, GddV1Layout, Some(&store), Some(&host)).await.unwrap();

		assert!(host.asked.load(Ordering::Relaxed) > 0);
		assert_eq!(host.missing_declarations.load(Ordering::Relaxed), 0);
	});
}

#[test]
fn opening_without_a_host_takes_the_fallback() {
	block_on(async {
		let working = stored(&[(1, "test_nodes::CopyNode")]);

		let gdd = GddV1::open_in(working, GddV1Layout).await.unwrap();

		let node = gdd.registry().node_instances.values().next().unwrap();
		assert_eq!(*node.implementation(), implementation_of("test_nodes::FallbackNode", 2));
	});
}
