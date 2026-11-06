//! The latest frozen version reads and writes exactly what the live types do. `cargo run migration check` compares the
//! definitions; this compares the encodings, which a changed custom serde impl could break unnoticed.

use document_graph_storage::{
	AttributeDelta, Delta, HotOp, Implementation, Network, NetworkId, Node, NodeId, NodeInput, PeerId, Priority, ProtoNode, ROOT_NETWORK, Registry, RegistryDelta, ResourceEntry, ResourceHash,
	ResourceId, Session, SessionState, SourceKey, UserId, Value,
};
use migration_formats::latest;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;

/// Live value to postcard, through the frozen type, and back.
fn through_frozen_postcard<Live: Serialize + DeserializeOwned, Frozen: Serialize + DeserializeOwned>(live: &Live) -> (Vec<u8>, Live) {
	let frozen: Frozen = postcard::from_bytes(&postcard::to_stdvec(live).unwrap()).unwrap();
	let bytes = postcard::to_stdvec(&frozen).unwrap();
	let decoded = postcard::from_bytes(&bytes).unwrap();
	(bytes, decoded)
}

/// Live value to JSON text and back through the frozen type. Text, since a JSON value can't hold the `u128` in a `Rev`.
fn through_frozen_json<Live: Serialize, Frozen: Serialize + DeserializeOwned>(live: &Live) -> String {
	let frozen: Frozen = serde_json::from_str(&serde_json::to_string(live).unwrap()).unwrap();
	serde_json::to_string(&frozen).unwrap()
}

fn node(network: NetworkId, implementation: Implementation, inputs: Vec<NodeInput>) -> Node {
	let mut node = Node::new(network, implementation, inputs.len());
	for (slot, input) in node.inputs_mut().iter_mut().zip(inputs) {
		slot.input = input;
	}
	node
}

fn nested_value() -> Value {
	Value::Object(vec![
		("float".to_string(), Value::Float(1.5)),
		("int".to_string(), Value::Int(-7)),
		("list".to_string(), Value::Array(vec![Value::Bool(true), Value::None, Value::Str("text".to_string())])),
	])
}

/// A session whose history uses every op kind an edit can stage, with one op still hot.
fn session() -> Session {
	let mut session = Session::with_identity(PeerId(1), UserId(7));
	let mut commit = |ops: Vec<RegistryDelta>| {
		session.stage_computed_ops(ops).unwrap();
		session.retire_all().unwrap();
	};

	let (first, second, nested) = (NodeId(1), NodeId(2), NetworkId(5));
	let declaration = ResourceId::from(11);
	let image = ResourceId::from(12);
	let attribute = |key: &str| AttributeDelta {
		key: key.to_string(),
		value: Some(nested_value()),
	};
	let source_key = SourceKey {
		priority: Priority::new(0.5).unwrap(),
		peer: PeerId(1),
	};

	commit(vec![
		RegistryDelta::RegisterPeer { peer: PeerId(1), user: UserId(7) },
		RegistryDelta::AddNetwork {
			id: ROOT_NETWORK,
			network: Network::default(),
		},
		RegistryDelta::AddNetwork {
			id: nested,
			network: Network::default(),
		},
		RegistryDelta::AddResource {
			id: declaration,
			entry: ResourceEntry::default(),
		},
		RegistryDelta::AddResource {
			id: image,
			entry: ResourceEntry::default(),
		},
	]);
	commit(vec![
		RegistryDelta::AddNode {
			id: first,
			node: node(
				ROOT_NETWORK,
				Implementation::ProtoNode(declaration),
				vec![
					NodeInput::Value { value: nested_value(), exposed: true },
					NodeInput::Scope("editor-api".into()),
					NodeInput::Import { index: 0 },
					NodeInput::Reflection,
				],
			),
		},
		RegistryDelta::AddNode {
			id: second,
			node: node(ROOT_NETWORK, Implementation::Network(nested), vec![NodeInput::Node { id: first, index: 0 }]),
		},
	]);
	commit(vec![
		RegistryDelta::ChangeNodeInput {
			id: first,
			index: 0,
			new_input: NodeInput::Value { value: Value::Int(3), exposed: false },
		},
		RegistryDelta::ChangeNodeAttribute {
			id: first,
			delta: attribute("ui::name"),
		},
		RegistryDelta::ChangeNodeInputAttribute {
			id: first,
			index: 1,
			delta: attribute("ui::label"),
		},
		RegistryDelta::SetNetworkExport {
			id: ROOT_NETWORK,
			index: 0,
			export: Some(NodeInput::Node { id: second, index: 0 }),
		},
		RegistryDelta::ChangeNetworkAttribute {
			id: ROOT_NETWORK,
			delta: attribute("ui::navigation"),
		},
		RegistryDelta::ChangeDocumentAttribute { delta: attribute("ui::document") },
	]);
	commit(vec![
		RegistryDelta::SetResourceHash {
			id: image,
			hash: Some(ResourceHash::from(b"image".as_slice())),
		},
		RegistryDelta::AddSource {
			id: image,
			key: source_key,
			source: Value::Str("embedded".to_string()),
		},
	]);
	commit(vec![
		RegistryDelta::SetNodeImplementation {
			id: second,
			implementation: Implementation::ProtoNode(declaration),
		},
		RegistryDelta::SetNodeInputs {
			id: second,
			inputs: node(ROOT_NETWORK, Implementation::ProtoNode(declaration), vec![NodeInput::Other, NodeInput::Import { index: 1 }])
				.inputs()
				.to_vec(),
		},
		RegistryDelta::RemoveSource { id: image, key: source_key },
	]);
	commit(vec![
		RegistryDelta::RemoveNode {
			id: first,
			snapshot: node(ROOT_NETWORK, Implementation::ProtoNode(declaration), vec![]),
		},
		RegistryDelta::RemoveNetwork {
			id: nested,
			snapshot: Network::default(),
		},
		RegistryDelta::RemoveResource {
			id: image,
			snapshot: ResourceEntry::default(),
		},
	]);

	// Left hot, so the hot log has an op too
	session
		.stage_computed_ops(vec![RegistryDelta::SetNetworkExport {
			id: ROOT_NETWORK,
			index: 1,
			export: None,
		}])
		.unwrap();
	session
}

#[test]
fn history_encodes_identically() {
	let session = session();
	let mut history: Vec<Delta> = session.history().cloned().collect();
	assert!(history.len() > 5);

	// The op kinds no edit stages
	history.push(Delta::merge([history[0].id, history[1].id], PeerId(1), Default::default()));
	history.push(Delta::new(None, PeerId(1), Default::default(), RegistryDelta::Meta, Default::default()));
	history.push(Delta::new(None, PeerId(1), Default::default(), RegistryDelta::Other(Value::None), Default::default()));

	for delta in &history {
		// Deltas hold no hash maps, so their bytes are deterministic
		let (bytes, _) = through_frozen_postcard::<Delta, latest::Delta>(delta);
		assert_eq!(bytes, postcard::to_stdvec(delta).unwrap(), "delta {:?} changed through the frozen types", delta.id);
	}
}

#[test]
fn registry_decodes_identically() {
	let session = session();

	// Hash map order differs between encodings, so compare decoded values
	for registry in [session.registry(), session.retired_registry()] {
		let (_, decoded) = through_frozen_postcard::<Registry, latest::Registry>(registry);
		assert_eq!(&decoded, registry);
	}
}

#[test]
fn hot_log_encodes_identically() {
	let session = session();
	assert!(!session.hot_log().is_empty());

	for hot_op in session.hot_log() {
		let (bytes, _) = through_frozen_postcard::<HotOp, latest::HotOp>(hot_op);
		assert_eq!(bytes, postcard::to_stdvec(hot_op).unwrap());
	}
}

#[test]
fn session_state_encodes_identically() {
	let session = session();
	let state = SessionState {
		peer_id: session.peer(),
		user_id: session.user(),
		head_rev: session.head_rev(),
		last_broadcast_rev: session.head_rev(),
		redo_stack: session.history().map(|delta| delta.id).take(2).collect(),
		next_node_counter: session.next_node_counter(),
		last_hot_sequence: session.last_hot_sequence(),
		view_settings: BTreeMap::from([("ui::doc::zoom".to_string(), nested_value())]),
		network_view_settings: BTreeMap::from([(ROOT_NETWORK, BTreeMap::from([("ui::nav".to_string(), Value::Float(2.))]))]),
		retired_marks: session.retired_marks().clone(),
		clock_counter: session.clock_counter(),
		shared: false,
	};

	assert_eq!(through_frozen_json::<SessionState, latest::SessionState>(&state), serde_json::to_string(&state).unwrap());
}

#[test]
fn declarations_encode_identically() {
	let mut declaration = ProtoNode {
		identifier: "math_nodes::add".to_string(),
		version: 2,
		attributes: Default::default(),
	};
	declaration.attributes.set("ui::category", nested_value(), Default::default());

	assert_eq!(through_frozen_json::<ProtoNode, latest::ProtoNode>(&declaration), serde_json::to_string(&declaration).unwrap());
}
