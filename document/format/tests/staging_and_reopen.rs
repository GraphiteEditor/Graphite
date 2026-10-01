// Staging a whole document after a recorded batch, and what a reopen must carry over.
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
fn a_whole_document_stage_after_a_recorded_batch_stages_nothing_new() {
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

/// `registry.bin` is the retired snapshot the hot log replays onto, so an op still hot at save time must stay out of it.
#[test]
fn a_reopen_keeps_unretired_hot_ops_out_of_the_retired_snapshot() {
	use document_graph_storage::{AttributeDelta, HotOp, HotSequence, RegistryDelta, TimeStamp, Value};
	let set = |value: u32, counter: u64| HotOp {
		op: RegistryDelta::ChangeDocumentAttribute {
			delta: AttributeDelta {
				key: "k".into(),
				value: Some(Value::from(serde_json::json!(value))),
			},
		},
		timestamp: TimeStamp { counter, peer: PeerId(22) },
		sequence: HotSequence(counter),
	};
	futures::executor::block_on(async {
		let mut gdd = GddV1::create_in(AnyContainer::Memory(MemoryBackend::new()), GddV1Layout, PeerId(22), 1, "ed".into(), "std".into()).unwrap();
		gdd.apply_hot_op(set(1, 1)).unwrap();
		gdd.apply_hot_op(set(2, 2)).unwrap();
		gdd.retire(TimeStamp { counter: 1, peer: PeerId(22) }).unwrap();

		let retired = gdd.session().retired_registry().clone();
		let (working, layout) = gdd.into_storage();
		let reopened = GddV1::open_in(working, layout).await.unwrap();
		assert!(reopened.session().retired_registry().value_equal(&retired), "the hot op leaked into the retired snapshot");
		assert_eq!(reopened.session().hot_log().len(), 1);
	});
}

/// A reopen continues this peer's clock and run of hot op sequences, with ops still hot and after they all retired,
/// since a reused sequence would let a peer's settled marks drop the new op.
#[test]
fn a_reopen_continues_the_hot_op_sequence() {
	use document_graph_storage::{AttributeDelta, RegistryDelta, Value};
	let set = |value: u32| RegistryDelta::ChangeDocumentAttribute {
		delta: AttributeDelta {
			key: "k".into(),
			value: Some(Value::from(serde_json::json!(value))),
		},
	};
	// Stages one op and returns the sequence it got.
	fn stage(gdd: &mut GddV1, op: RegistryDelta) -> u64 {
		gdd.stage_constructed_ops(vec![op], &Default::default(), &HashMapResourceStorage::new()).unwrap();
		gdd.session().hot_log().last().unwrap().sequence.0
	}
	futures::executor::block_on(async {
		let mut gdd = GddV1::create_in(AnyContainer::Memory(MemoryBackend::new()), GddV1Layout, PeerId(23), 1, "ed".into(), "std".into()).unwrap();
		stage(&mut gdd, set(1));
		let up_to = gdd.session().hot_log().last().unwrap().timestamp;
		gdd.retire(up_to).unwrap();
		let last = stage(&mut gdd, set(2));

		let (working, layout) = gdd.into_storage();
		let mut reopened = GddV1::open_in(working, layout).await.unwrap();
		assert_eq!(stage(&mut reopened, set(3)), last + 1);

		let up_to = reopened.session().hot_log().last().unwrap().timestamp;
		reopened.retire(up_to).unwrap();
		let last = reopened.session().last_hot_sequence().0;
		let clock = reopened.session().clock_counter();
		let (working, layout) = reopened.into_storage();
		let mut reopened = GddV1::open_in(working, layout).await.unwrap();
		assert_eq!(reopened.session().clock_counter(), clock, "the clock carries over");
		assert_eq!(stage(&mut reopened, set(4)), last + 1);
	});
}
