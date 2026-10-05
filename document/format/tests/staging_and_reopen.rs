// Staging a whole document after a recorded batch.
#![cfg(feature = "conversion")]

use document_container::AnyContainer;
use document_container::backends::memory::MemoryBackend;
use document_format::{GddV1, GddV1Layout};
use document_graph_storage::{NoMetadata, PeerId, Registry};
use graph_craft::application_io::resource::HashMapResourceStorage;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeId, NodeNetwork};
use graph_craft::{ProtoNodeIdentifier, concrete};
use graphene_resource::ResourceRegistry;

fn network_with_one_node() -> NodeNetwork {
	let node = DocumentNode {
		implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::identity::IdentityNode")),
		call_argument: concrete!(()),
		..Default::default()
	};
	NodeNetwork {
		nodes: [(NodeId(1), node)].into_iter().collect(),
		..Default::default()
	}
}

/// A whole-document stage after a recorded batch stages nothing new: restating the batch would fail on the node it added.
#[test]
fn restaging_the_whole_document_after_a_batch_stages_nothing() {
	let mut gdd = GddV1::create_in(AnyContainer::Memory(MemoryBackend::new()), GddV1Layout, PeerId(21), 1, "ed".into(), "std".into()).unwrap();
	let store = HashMapResourceStorage::new();
	let resources = ResourceRegistry::new();
	gdd.stage_runtime_snapshot(&NodeNetwork::default(), &NoMetadata, &resources, &store)
		.expect("first whole-document stage");

	let target = Registry::convert_from_runtime(&network_with_one_node(), &NoMetadata, &resources, PeerId(21)).unwrap();
	let ops = document_graph_storage::delta::compute_deltas(gdd.registry(), &target.registry);
	gdd.stage_constructed_ops(ops, &target.declaration_bytes, &store).expect("recorded batch");
	let hot_before = gdd.session().hot_log().len();

	gdd.stage_runtime_snapshot(&network_with_one_node(), &NoMetadata, &resources, &store)
		.expect("the runtime already matches what was staged");
	assert_eq!(gdd.session().hot_log().len(), hot_before, "nothing new to stage");
}
