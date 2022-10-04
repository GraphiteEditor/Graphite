use std::collections::HashMap;
use std::fmt::Display;

type NodeId = u64;

fn gen_node_id() -> NodeId {
	todo!()
}

fn merge_ids(a: u64, b: u64) -> u64 {
	use std::hash::{Hash, Hasher};
	let mut hasher = std::collections::hash_map::DefaultHasher::new();
	a.hash(&mut hasher);
	b.hash(&mut hasher);
	hasher.finish()
}

#[derive(Debug, Clone)]
pub struct DocumentNode {
	name: String,
	id: NodeId,
	inputs: Vec<NodeInput>,
	implementation: DocumentNodeImplementation,
}

#[derive(Debug, Clone)]
pub enum NodeInput {
	Node(NodeId),
	Value(InputWidget),
}

#[derive(Debug, Clone)]
pub struct InputWidget;

impl Display for InputWidget {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "InputWidget")
	}
}

#[derive(Debug, Clone)]
pub enum DocumentNodeImplementation {
	Network(NodeNetwork),
	ProtoNode(ProtoNode),
}

#[derive(Debug, Default, Clone)]
pub struct NodeNetwork {
	inputs: Vec<NodeId>,
	output: NodeId,
	nodes: HashMap<NodeId, DocumentNode>,
}

#[derive(Debug, Clone, Default)]
pub enum ConstructionArgs {
	#[default]
	None,
	Value,
	Nodes(Vec<NodeId>),
}

#[derive(Debug, Clone, Default)]
pub struct ProtoNode {
	construction_args: ConstructionArgs,
	name: String,
}

impl NodeInput {
	fn map_ids(&mut self, f: impl Fn(NodeId) -> NodeId) {
		match self {
			NodeInput::Node(id) => *self = NodeInput::Node(f(*id)),
			NodeInput::Value(_) => (),
		}
	}
}

impl ProtoNode {
	pub fn id() -> Self {
		Self {
			name: "id".into(),
			..Default::default()
		}
	}
	pub fn new(name: String) -> Self {
		Self { name, ..Default::default() }
	}
}

impl NodeNetwork {
	pub fn map_ids(&mut self, f: impl Fn(NodeId) -> NodeId + Copy) {
		self.inputs.iter_mut().for_each(|id| *id = f(*id));
		self.nodes = self
			.nodes
			.iter()
			.map(|(id, node)| {
				let mut node = node.clone();
				node.inputs.iter_mut().for_each(|input| input.map_ids(f));
				node.id = f(node.id);
				(f(*id), node)
			})
			.collect();
	}

	/// Recursively dissolve non primitive document nodes and return a single flattened network of nodes.
	pub fn flatten(&mut self, node: NodeId) {
		let (id, mut node) = self.nodes.remove_entry(&node).unwrap();

		match node.implementation {
			DocumentNodeImplementation::Network(mut inner_network) => {
				// Connect all network inputs to either the parent network nodes, or newly created value nodes.
				inner_network.map_ids(|inner_id| merge_ids(id, inner_id));
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
								id: new_id,
								inputs: Vec::new(),
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNode::new(format!("{}-{}", node.name, widget))),
							};
							self.nodes.insert(new_id, value_node);
							let network_input = self.nodes.get_mut(network_input).unwrap();
							network_input.inputs.push(NodeInput::Node(new_id));
						}
					}
				}
				let new_nodes = inner_network.nodes.keys().cloned().collect::<Vec<_>>();
				// Copy nodes from the inner network into the parent network
				self.nodes.extend(inner_network.nodes);
				node.implementation = DocumentNodeImplementation::ProtoNode(ProtoNode::id());
				self.inputs = vec![inner_network.output];
				for node_id in new_nodes {
					self.flatten(node_id);
				}
			}
			DocumentNodeImplementation::ProtoNode(proto_node) => {
				node.implementation = DocumentNodeImplementation::ProtoNode(proto_node);
			}
		}
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
		let mut network = NodeNetwork {
			inputs: vec![0],
			output: 1,
			nodes: [
				(
					0,
					DocumentNode {
						name: "cons".into(),
						id: 0,
						inputs: vec![NodeInput::Value(InputWidget)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNode::new("cons".into())),
					},
				),
				(
					1,
					DocumentNode {
						name: "add".into(),
						id: 1,
						inputs: vec![NodeInput::Node(0)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNode::new("add".into())),
					},
				),
			]
			.iter()
			.cloned()
			.collect(),
		};
		network.map_ids(|id| id + 1);
		panic!("{:#?}", network);
	}
}
