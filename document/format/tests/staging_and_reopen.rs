// Staging a whole document after a recorded batch, and what a reopen must carry over.
#![cfg(feature = "conversion")]

use document_container::AnyContainer;
use document_container::backends::memory::MemoryBackend;
use document_format::{GddV1, GddV1Layout};
use document_graph_storage::{NoMetadata, PeerId, Registry, UserId};
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
	let mut gdd = GddV1::create_in(AnyContainer::Memory(MemoryBackend::new()), GddV1Layout, PeerId(21), UserId(21), 1, "ed".into(), "std".into()).unwrap();
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
		attributes: Default::default(),
	};
	futures::executor::block_on(async {
		let mut gdd = GddV1::create_in(AnyContainer::Memory(MemoryBackend::new()), GddV1Layout, PeerId(22), UserId(22), 1, "ed".into(), "std".into()).unwrap();
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

/// A reused sequence would let a peer's retired marks drop the new op.
#[test]
fn a_reopen_continues_the_hot_op_sequence() {
	use document_graph_storage::{AttributeDelta, RegistryDelta, Value};
	let set = |value: u32| RegistryDelta::ChangeDocumentAttribute {
		delta: AttributeDelta {
			key: "k".into(),
			value: Some(Value::from(serde_json::json!(value))),
		},
	};
	futures::executor::block_on(async {
		let mut gdd = GddV1::create_in(AnyContainer::Memory(MemoryBackend::new()), GddV1Layout, PeerId(23), UserId(23), 1, "ed".into(), "std".into()).unwrap();
		gdd.stage_ops([set(1)]).unwrap();
		let up_to = gdd.session().hot_log().last().unwrap().timestamp;
		gdd.retire(up_to).unwrap();
		let last = gdd.stage_ops([set(2)]).unwrap().last().unwrap().sequence;

		let (working, layout) = gdd.into_storage();
		let mut reopened = GddV1::open_in(working, layout).await.unwrap();
		assert_eq!(reopened.stage_ops([set(3)]).unwrap()[0].sequence.0, last.0 + 1);

		let up_to = reopened.session().hot_log().last().unwrap().timestamp;
		reopened.retire(up_to).unwrap();
		let last = reopened.session().last_hot_sequence();
		let clock = reopened.session().clock_counter();
		let (working, layout) = reopened.into_storage();
		let mut reopened = GddV1::open_in(working, layout).await.unwrap();
		assert_eq!(reopened.session().clock_counter(), clock, "the clock carries over");
		assert_eq!(reopened.stage_ops([set(4)]).unwrap()[0].sequence.0, last.0 + 1);
	});
}

#[test]
fn a_user_change_survives_a_reopen() {
	use document_graph_storage::{AttributeDelta, RegistryDelta, Value};
	let set = |key: &str| RegistryDelta::ChangeDocumentAttribute {
		delta: AttributeDelta {
			key: key.into(),
			value: Some(Value::from(serde_json::json!(1))),
		},
	};
	futures::executor::block_on(async {
		let mut gdd = GddV1::create_in(AnyContainer::Memory(MemoryBackend::new()), GddV1Layout, PeerId(24), UserId(240), 1, "ed".into(), "std".into()).unwrap();
		gdd.stage_ops([set("a")]).unwrap();
		gdd.retire_pending_interaction().unwrap();
		gdd.stage_ops([set("b")]).unwrap();

		gdd.set_user(UserId(250), PeerId(25)).unwrap();
		let clock = gdd.session().clock_counter();
		let (working, layout) = gdd.into_storage();
		let mut reopened = GddV1::open_in(working, layout).await.unwrap();
		assert_eq!(reopened.session().peer(), PeerId(25));
		assert_eq!(reopened.session().user(), UserId(250));
		assert_eq!(reopened.session().clock_counter(), clock);

		let staged = reopened.stage_ops([set("c")]).unwrap();
		assert!(staged.iter().all(|hot_op| hot_op.timestamp.peer == PeerId(25) && hot_op.timestamp.counter > clock));
		assert_eq!(staged[0].sequence.0, 1, "the new peer's run starts over");
		reopened.retire_pending_interaction().unwrap();

		let (working, layout) = reopened.into_storage();
		let reopened = GddV1::open_in(working, layout).await.unwrap();
		let session = reopened.session();
		assert!(session.hot_log().is_empty(), "the previous user's open step closed and retired");
		assert_eq!(session.user_of(PeerId(24)), Some(UserId(240)));
		assert_eq!(session.user_of(PeerId(25)), Some(UserId(250)));
		assert!(!session.is_mine(PeerId(24)));
		assert!(session.is_mine(PeerId(25)));
		let authors: Vec<PeerId> = session
			.history()
			.filter(|delta| matches!(delta.kind, RegistryDelta::ChangeDocumentAttribute { .. }))
			.map(|delta| delta.author)
			.collect();
		assert_eq!(authors, [PeerId(24), PeerId(24), PeerId(25)]);
	});
}

/// Large input values are staged as blobs, an edited one as a delta against its predecessor. A reopen and an archive
/// round trip both find them again, for the current state and an undone one.
#[test]
fn blobs_resolve_after_a_reopen_and_an_archive_round_trip() {
	use document_container::AsyncContainer;
	use document_format::export::{ExportFormat, ExportOptions};
	use document_format::layout::Layout;
	use document_graph_storage::{NodeInput, RegistryDelta, Value, blob::BLOB_THRESHOLD};
	use graph_craft::document::value::TaggedValue;

	let original = TaggedValue::String("a".repeat(BLOB_THRESHOLD));
	let edited = TaggedValue::String(format!("b{}", "a".repeat(BLOB_THRESHOLD)));
	let mut network = network_with_one_node();
	network.nodes.get_mut(&NodeId(1)).unwrap().inputs = vec![graph_craft::document::NodeInput::value(original.clone(), false)];

	futures::executor::block_on(async {
		let mut gdd = GddV1::create_in(AnyContainer::Memory(MemoryBackend::new()), GddV1Layout, PeerId(26), UserId(26), 1, "ed".into(), "std".into()).unwrap();
		let store = HashMapResourceStorage::new();
		gdd.commit_from_runtime(&network, &NoMetadata, &ResourceRegistry::new(), &store).unwrap();

		// An op built outside a conversion carries the value inline until staging moves it into a blob
		let id = *gdd.registry().node_instances.keys().next().unwrap();
		let value = Value::from(serde_json::to_value(&edited).unwrap());
		let new_input = NodeInput::Value { value, exposed: false };
		gdd.stage_ops([RegistryDelta::ChangeNodeInput { id, index: 0, new_input }]).unwrap();
		gdd.retire_pending_interaction().unwrap();

		let edited_blob = gdd.registry().node_instances[&id].inputs()[0].input.blob().expect("staging moves the value into a blob");
		let (working, layout) = gdd.into_storage();
		let record = working.read(&layout.blob_path(&edited_blob)).await.unwrap();
		assert_eq!(record.as_slice()[0], 1, "the edit is stored as a delta");
		assert!(record.as_slice().len() < 100, "the delta is small, got {} bytes", record.as_slice().len());

		let input_value = |gdd: &GddV1, declarations: &document_graph_storage::Declarations| {
			let (converted, _) = gdd.registry().to_runtime_with_metadata(declarations, gdd.blobs()).expect("to_runtime");
			let node = converted.nodes.values().next().expect("the node converts");
			let graph_craft::document::NodeInput::Value { tagged_value, .. } = &node.inputs[0] else {
				panic!("a value input")
			};
			(**tagged_value).clone()
		};

		let reopened = GddV1::open_in(working, layout).await.unwrap();
		let archive = reopened.export_to_bytes(ExportFormat::Zip, ExportOptions::default(), &store, None).await.unwrap();
		let unpacked = GddV1::open_from_archive(&archive, AnyContainer::Memory(MemoryBackend::new()), GddV1Layout).await.unwrap();

		let flat_options = ExportOptions {
			include_history: false,
			..Default::default()
		};
		let flat_archive = reopened.export_to_bytes(ExportFormat::Zip, flat_options, &store, None).await.unwrap();
		let flat = GddV1::open_from_archive(&flat_archive, AnyContainer::Memory(MemoryBackend::new()), GddV1Layout).await.unwrap();
		let declarations = flat.declarations(&store).await;
		assert_eq!(flat.blobs().len(), 1, "a copy without history leaves the undone value behind");
		assert_eq!(input_value(&flat, &declarations), edited);

		for mut gdd in [reopened, unpacked] {
			let declarations = gdd.declarations(&store).await;
			assert_eq!(gdd.blobs().len(), 2, "both values' blobs load, the undone one included");
			assert_eq!(input_value(&gdd, &declarations), edited);

			gdd.undo().unwrap();
			assert_eq!(input_value(&gdd, &declarations), original);
		}
	});
}
