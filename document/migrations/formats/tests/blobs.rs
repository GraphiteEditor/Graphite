//! The step to version 4 moves large node input values into the same blobs that live staging makes.

use document_graph_storage::blob::{BLOB_THRESHOLD, BlobBytes, decode_blob};
use document_graph_storage::{Delta, HotOp, Implementation, Network, Node, NodeId, NodeInput, PeerId, ROOT_NETWORK, Registry, RegistryDelta, ResourceId, Session, UserId, Value};
use migration_core::{DocumentPayloads, Payload, PayloadCodec};

// Format steps register when linked
use migration_formats as _;

fn large_input(prefix: &str) -> NodeInput {
	NodeInput::Value {
		value: Value::Str(format!("{prefix}{}", "a".repeat(BLOB_THRESHOLD))),
		exposed: false,
	}
}

fn postcard_payload(value: &impl serde::Serialize) -> Payload {
	Payload::encode(value, PayloadCodec::Postcard).unwrap()
}

#[test]
fn version_four_moves_large_values_into_blobs_based_on_their_slot() {
	// `Value::Blob` is the only change in version 4, so a version 3 document is a live one without blobs
	let id = NodeId(1);
	let mut session = Session::with_identity(PeerId(1), UserId(1));
	let mut node = Node::new(ROOT_NETWORK, Implementation::ProtoNode(ResourceId::from(1)), 1);
	node.inputs_mut()[0].input = large_input("");
	let network = RegistryDelta::AddNetwork {
		id: ROOT_NETWORK,
		network: Network::default(),
	};
	session.stage_computed_ops(vec![network, RegistryDelta::AddNode { id, node }]).unwrap();
	session
		.stage_computed_ops(vec![RegistryDelta::ChangeNodeInput {
			id,
			index: 0,
			new_input: large_input("b"),
		}])
		.unwrap();
	session.retire_all().unwrap();
	session
		.stage_computed_ops(vec![RegistryDelta::ChangeNodeInput {
			id,
			index: 0,
			new_input: large_input("c"),
		}])
		.unwrap();

	let mut document = DocumentPayloads {
		format_version: 3,
		registry: Some(postcard_payload(session.retired_registry())),
		history: session.history().map(postcard_payload).collect(),
		hot_log: session.hot_log().iter().map(postcard_payload).collect(),
		..Default::default()
	};
	let steps = migration_core::registered_format_migrations();
	let step = steps.iter().find(|step| step.migrates_from() == 3).expect("the step into version 4");
	document.registry = Some(step.migrate_registry(document.registry.as_ref().unwrap()).unwrap());
	document.history = document.history.iter().map(|record| step.migrate_delta(record).unwrap()).collect();
	document.hot_log = document.hot_log.iter().map(|record| step.migrate_hot_op(record).unwrap()).collect();
	step.migrate_document(&mut document).unwrap();

	// Both paths hash the same bytes, so the migrated blob equals the staged one
	let staged_blob = |input: NodeInput| {
		let mut input = input;
		input.move_into_blob(&mut BlobBytes::new());
		input.blob().unwrap()
	};
	let [first, second, third] = [large_input(""), large_input("b"), large_input("c")].map(staged_blob);

	let history: Vec<Delta> = document.history.iter().map(|record| record.decode().unwrap()).collect();
	let hot_log: Vec<HotOp> = document.hot_log.iter().map(|record| record.decode().unwrap()).collect();
	let registry: Registry = document.registry.as_ref().unwrap().decode().unwrap();
	let written = |op: &RegistryDelta| match op {
		RegistryDelta::AddNode { node, .. } => node.inputs()[0].input.blob(),
		RegistryDelta::ChangeNodeInput { new_input, .. } => new_input.blob(),
		_ => None,
	};
	let retired: Vec<_> = history.iter().filter_map(|delta| written(&delta.kind)).collect();
	let hot: Vec<_> = hot_log.iter().filter_map(|hot_op| written(&hot_op.op)).collect();
	assert_eq!(retired, [first, second]);
	assert_eq!(hot, [third]);
	assert_eq!(registry.node_instances[&id].inputs()[0].input.blob(), Some(second));

	assert_eq!(document.blobs.len(), 3);
	let [first, second, third] = [first, second, third].map(|hash| hash.0);
	assert_eq!(decode_blob(&document.blobs[&third]).unwrap(), Value::Str(format!("c{}", "a".repeat(BLOB_THRESHOLD))));
	assert_eq!(document.blob_bases.get(&first), None, "the first value of a slot has nothing to be based on");
	assert_eq!(document.blob_bases.get(&second), Some(&first));
	assert_eq!(document.blob_bases.get(&third), Some(&second));
}
