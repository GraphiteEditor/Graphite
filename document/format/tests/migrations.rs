// Opening a document runs the format steps and node migrations the binary links, and persists what they changed.
#![cfg(feature = "migrations")]

use document_container::AnyContainer;
use document_container::backends::memory::MemoryBackend;
use document_format::{Error, GddV1, GddV1Layout, manifest};
use document_graph_storage::{EncodedDeclaration, Implementation, NoMetadata, PeerId};
use futures::executor::block_on;
use graph_craft::application_io::resource::HashMapResourceStorage;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeId, NodeNetwork};
use graph_craft::{ProtoNodeIdentifier, concrete};
use graphene_resource::ResourceRegistry;
use migration_core::rename;
use migration_formats as _;

rename!("test_nodes::FadeNode"@1 => "test_nodes::OpacityNode"@2);

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

fn create() -> GddV1 {
	GddV1::create_in(AnyContainer::Memory(MemoryBackend::new()), GddV1Layout, PeerId(7), 1, "editor".into(), "std".into()).unwrap()
}

fn implementations(gdd: &GddV1) -> Vec<Implementation> {
	let mut nodes: Vec<_> = gdd.registry().node_instances.iter().collect();
	nodes.sort_by_key(|(id, _)| **id);
	nodes.into_iter().map(|(_, node)| node.implementation().clone()).collect()
}

#[test]
fn opening_migrates_nodes_and_persists_the_result() {
	block_on(async {
		let mut gdd = create();
		gdd.commit_from_runtime(&network_of(&[(1, "test_nodes::FadeNode")]), &NoMetadata, &ResourceRegistry::new(), &HashMapResourceStorage::new())
			.unwrap();
		let (working, layout) = gdd.into_storage();

		let migrated = GddV1::open_in(working, layout).await.unwrap();
		let target = EncodedDeclaration::new("test_nodes::OpacityNode".to_string(), 2).unwrap();
		assert_eq!(implementations(&migrated), [Implementation::ProtoNode(target.id)]);
		assert!(migrated.registry().resources.contains_key(&target.id), "the target declaration is a registered resource");
		assert_eq!(migrated.declarations(&migrated).await[&target.id], target.proto, "its bytes are stored");
		assert_eq!(migrated.manifest().format_version, manifest::SUPPORTED_FORMAT_VERSION);

		// The rewritten history persisted, so a reopen has nothing left to migrate
		let registry = migrated.registry().clone();
		let (working, layout) = migrated.into_storage();
		let reopened = GddV1::open_in(working, layout).await.unwrap();
		assert!(reopened.registry().value_equal(&registry));
	});
}

#[test]
fn undo_and_redo_cross_migrated_history() {
	block_on(async {
		let store = HashMapResourceStorage::new();
		let mut gdd = create();
		gdd.commit_from_runtime(&network_of(&[(1, "test_nodes::FadeNode")]), &NoMetadata, &ResourceRegistry::new(), &store)
			.unwrap();
		gdd.commit_from_runtime(&network_of(&[(1, "test_nodes::FadeNode"), (2, "test_nodes::FadeNode")]), &NoMetadata, &ResourceRegistry::new(), &store)
			.unwrap();
		gdd.undo().unwrap();
		let (working, layout) = gdd.into_storage();

		let mut migrated = GddV1::open_in(working, layout).await.unwrap();
		let target = Implementation::ProtoNode(EncodedDeclaration::new("test_nodes::OpacityNode".to_string(), 2).unwrap().id);
		assert_eq!(implementations(&migrated), std::slice::from_ref(&target), "the undone node stays undone");

		migrated.redo().unwrap();
		assert_eq!(implementations(&migrated), [target.clone(), target], "redo brings it back migrated");
	});
}

#[test]
fn opening_steps_the_format_version() {
	block_on(async {
		let mut gdd = create();
		gdd.update_manifest(|manifest| manifest.format_version = 1).unwrap();
		let (working, layout) = gdd.into_storage();

		let migrated = GddV1::open_in(working, layout).await.unwrap();
		assert_eq!(migrated.manifest().format_version, 2);

		let (working, layout) = migrated.into_storage();
		assert_eq!(GddV1::open_in(working, layout).await.unwrap().manifest().format_version, 2, "the bump persisted");
	});
}

#[test]
fn a_missing_format_step_fails_the_open() {
	block_on(async {
		let mut gdd = create();
		gdd.update_manifest(|manifest| manifest.format_version = 0).unwrap();
		let (working, layout) = gdd.into_storage();

		let error = GddV1::open_in(working, layout).await.err().unwrap();
		assert!(matches!(error, Error::Migration(ref runner_error) if runner_error.to_string().contains("version 0")), "{error}");
	});
}
