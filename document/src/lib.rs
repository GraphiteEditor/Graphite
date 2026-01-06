#![allow(unused)]
use std::{
	borrow::Cow,
	collections::{BTreeMap, HashMap},
};

struct Registry {
	node_declarations: BTreeMap<DeclarationId, Implementation>,
	node_instances: BTreeMap<NodeId, Node>,
	networks: BTreeMap<NetworkId, Network>,
}
struct Document {
	registry: Registry,
	history: HashMap<DeltaId, Delta>,
	head: DeltaId,
}

type DeclarationId = u64; // content based hash
type NodeId = u64;
type NetworkId = u64;
type ProtoNodeId = String;
type Value = serde_json::Value;

type Attributes = HashMap<String, Value>;

struct Node {
	declaration: DeclarationId,
	inputs: Vec<NodeId>,
	inputs_attributes: Vec<Attributes>,
	attributes: Attributes,
	network: NetworkId,
}

struct NodeAttributes {
	name: Option<String>,
}

enum NodeInput {
	Node { node_id: NodeId, output_index: usize },
	Value { raw_value: &'static [u8], exposed: bool },
	Scope(Cow<'static, str>),
}

enum Implementation {
	ProtoNode(ProtoNode), // add source code?
	Network(NetworkId),
}

struct Network {
	exports: Vec<(NodeId, usize)>,
}

struct ProtoNode {
	identifier: ProtoNodeId,
	code: Option<String>,
	wasm: Option<Vec<u8>>,
}

type DeltaId = u64;

struct Delta {
	timestamp: u64,
	predecessor: Option<DeltaId>,
	id: DeltaId,
	delta_type: RegistryDelta,
}

enum RegistryDelta {
	AddNode { network: NetworkId, node_id: NodeId, node: Node },
	RemoveNode { node_id: NodeId },
	ChangeNodeInput { node_id: NodeId, input_idx: usize, new_node: NodeInput },
	ChangeNodeAttribute { node_id: NodeId, delta: AttributeDelta },
	ChangeNodeInputAttribute { node_id: NodeId, input_idx: usize, delta: AttributeDelta },
}

enum AttributeDelta {
	Set { key: String, value: Value },
	Remove { key: String },
}
