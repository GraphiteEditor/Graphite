use std::collections::HashMap;
use std::fmt::Display;
use std::sync::Mutex;

use rand_chacha::{
	rand_core::{RngCore, SeedableRng},
	ChaCha20Rng,
};

type NodeId = u64;
static RNG: Mutex<Option<ChaCha20Rng>> = Mutex::new(None);

pub fn generate_uuid() -> u64 {
	let mut lock = RNG.lock().expect("uuid mutex poisoned");
	if lock.is_none() {
		*lock = Some(ChaCha20Rng::seed_from_u64(0));
	}
	lock.as_mut().map(ChaCha20Rng::next_u64).unwrap()
}

fn gen_node_id() -> NodeId {
	static mut NODE_ID: NodeId = 0;
	unsafe {
		NODE_ID += 1;
		NODE_ID
	}
}

fn merge_ids(a: u64, b: u64) -> u64 {
	use std::hash::{Hash, Hasher};
	let mut hasher = std::collections::hash_map::DefaultHasher::new();
	a.hash(&mut hasher);
	b.hash(&mut hasher);
	hasher.finish()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentNode {
	name: String,
	inputs: Vec<NodeInput>,
	implementation: DocumentNodeImplementation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeInput {
	Node(NodeId),
	Value(InputWidget),
	Network,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputWidget;

impl Display for InputWidget {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "InputWidget")
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentNodeImplementation {
	Network(NodeNetwork),
	ProtoNode(ProtoNode),
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct NodeNetwork {
	inputs: Vec<NodeId>,
	output: NodeId,
	nodes: HashMap<NodeId, DocumentNode>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ConstructionArgs {
	#[default]
	None,
	Unresolved,
	Value,
	Nodes(Vec<NodeId>),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProtoNode {
	construction_args: ConstructionArgs,
	input: Option<NodeId>,
	name: String,
}

impl NodeInput {
	fn map_ids(&mut self, f: impl Fn(NodeId) -> NodeId) {
		match self {
			NodeInput::Node(id) => *self = NodeInput::Node(f(*id)),
			_ => (),
		}
	}
}

impl ProtoNode {
	pub fn id(id: NodeId) -> Self {
		Self {
			name: "id".into(),
			input: Some(id),
			..Default::default()
		}
	}
	pub fn value(name: String, value: ConstructionArgs) -> Self {
		Self {
			name,
			construction_args: value,
			..Default::default()
		}
	}
}

impl NodeNetwork {
	pub fn map_ids(&mut self, f: impl Fn(NodeId) -> NodeId + Copy) {
		self.inputs.iter_mut().for_each(|id| *id = f(*id));
		self.output = f(self.output);
		self.nodes = self
			.nodes
			.iter()
			.map(|(id, node)| {
				let mut node = node.clone();
				node.inputs.iter_mut().for_each(|input| input.map_ids(f));
				(f(*id), node)
			})
			.collect();
	}

	pub fn flatten(&mut self, node: NodeId) {
		self.flatten_with_map_fn(node, merge_ids)
	}

	/// Recursively dissolve non primitive document nodes and return a single flattened network of nodes.
	pub fn flatten_with_map_fn(&mut self, node: NodeId, map_ids: impl Fn(NodeId, NodeId) -> NodeId + Copy) {
		let (id, mut node) = self.nodes.remove_entry(&node).expect("The node which was supposed to be flattened does not exist in the network");

		match node.implementation {
			DocumentNodeImplementation::Network(mut inner_network) => {
				// Connect all network inputs to either the parent network nodes, or newly created value nodes.
				inner_network.map_ids(|inner_id| map_ids(id, inner_id));
				let new_nodes = inner_network.nodes.keys().cloned().collect::<Vec<_>>();
				// Copy nodes from the inner network into the parent network
				self.nodes.extend(inner_network.nodes);

				for (document_input, network_input) in node.inputs.iter().zip(self.inputs.clone().iter()) {
					match document_input {
						NodeInput::Node(node) => {
							let network_input = self.nodes.get_mut(network_input).unwrap();
							network_input.inputs.insert(0, NodeInput::Node(*node));
						}
						NodeInput::Value(widget) => {
							let new_id = gen_node_id();
							let value_node = DocumentNode {
								name: "value".into(),
								inputs: Vec::new(),
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNode::value(format!("{}-{}", node.name, widget), ConstructionArgs::Value)),
							};
							assert!(!self.nodes.contains_key(&new_id));
							self.nodes.insert(new_id, value_node);
							let network_input = self.nodes.get_mut(network_input).unwrap();
							network_input.inputs.push(NodeInput::Node(new_id));
						}
						NodeInput::Network => todo!(),
					}
				}
				node.implementation = DocumentNodeImplementation::ProtoNode(ProtoNode::id(inner_network.output));
				self.inputs = vec![inner_network.output];
				for node_id in new_nodes {
					self.flatten_with_map_fn(node_id, map_ids);
				}
			}
			DocumentNodeImplementation::ProtoNode(proto_node) => {
				node.implementation = DocumentNodeImplementation::ProtoNode(proto_node);
			}
		}
		assert!(!self.nodes.contains_key(&id), "Trying to insert a node into the network caused an id conflict");
		self.nodes.insert(id, node);
	}
}

struct Map<I, O>(core::marker::PhantomData<(I, O)>);

impl<O> Display for Map<(), O> {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
		write!(f, "Map")
	}
}

impl Display for Map<i32, String> {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
		write!(f, "Map<String>")
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn map_ids() {
		let mut network = add_network();
		network.map_ids(|id| id + 1);
		let maped_add = NodeNetwork {
			inputs: vec![1],
			output: 2,
			nodes: [
				(
					1,
					DocumentNode {
						name: "cons".into(),
						inputs: vec![NodeInput::Value(InputWidget)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNode::value("cons".into(), ConstructionArgs::Unresolved)),
					},
				),
				(
					2,
					DocumentNode {
						name: "add".into(),
						inputs: vec![NodeInput::Node(1)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNode::value("add".into(), ConstructionArgs::Unresolved)),
					},
				),
			]
			.iter()
			.cloned()
			.collect(),
		};
		assert_eq!(network, maped_add);
	}

	fn add_network() -> NodeNetwork {
		NodeNetwork {
			inputs: vec![0],
			output: 1,
			nodes: [
				(
					0,
					DocumentNode {
						name: "cons".into(),
						inputs: vec![NodeInput::Value(InputWidget)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNode::value("cons".into(), ConstructionArgs::Unresolved)),
					},
				),
				(
					1,
					DocumentNode {
						name: "add".into(),
						inputs: vec![NodeInput::Node(0)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNode::value("add".into(), ConstructionArgs::Unresolved)),
					},
				),
			]
			.iter()
			.cloned()
			.collect(),
		}
	}
	#[test]
	fn flatten_add() {
		let mut network = NodeNetwork {
			inputs: vec![1],
			output: 1,
			nodes: [(
				1,
				DocumentNode {
					name: "Inc".into(),
					inputs: vec![],
					implementation: DocumentNodeImplementation::Network(add_network()),
				},
			)]
			.iter()
			.cloned()
			.collect(),
		};
		network.flatten_with_map_fn(1, |self_id, inner_id| self_id * 10 + inner_id);
		let flat_network = NodeNetwork {
			inputs: vec![10],
			output: 1,
			nodes: [
				(
					1,
					DocumentNode {
						name: "Inc".into(),
						inputs: vec![NodeInput::Node(11)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNode::id(11)),
					},
				),
				(
					10,
					DocumentNode {
						name: "cons".into(),
						inputs: vec![NodeInput::Network],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNode::value("cons".into(), ConstructionArgs::Nodes(vec![]))),
					},
				),
				(
					12,
					DocumentNode {
						name: "value".into(),
						inputs: vec![],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNode::value("value".into(), ConstructionArgs::Value)),
					},
				),
				(
					11,
					DocumentNode {
						name: "add".into(),
						inputs: vec![NodeInput::Node(10)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNode::value("add".into(), ConstructionArgs::None)),
					},
				),
			]
			.iter()
			.cloned()
			.collect(),
		};
		// for debuging purposes
		println!("{:#?}", network);
		assert_eq!(flat_network, network);
	}
}
