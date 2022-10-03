use std::collections::HashMap;
use std::fmt::Display;

type NodeId = u64;

fn gen_node_id() -> NodeId {
	todo!()
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
	edges: Vec<Connection>,
	nodes: HashMap<NodeId, DocumentNode>,
}

#[derive(Debug, Clone)]
pub struct Connection {
	from: NodeId,
	to: NodeId,
}

#[derive(Debug, Clone)]
pub struct ProtoNode {
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
		Self { name: "id".into() }
	}
	pub fn new(name: String) -> Self {
		Self { name }
	}
}

impl NodeNetwork {
	pub fn map_ids(&mut self, f: impl Fn(NodeId) -> NodeId + Copy) {
		self.inputs.iter_mut().for_each(|id| *id = f(*id));
		self.edges.iter_mut().for_each(|conn| *conn = Connection { from: f(conn.from), to: f(conn.to) });
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

	/// Recursively dissolve non primitive document nodes and return a single flattened network of nodes.
	pub fn flatten(&self, node: NodeId, network: &mut NodeNetwork, nodes: &mut HashMap<NodeId, DocumentNode>) {
		let (id, mut node) = nodes.remove_entry(&node).unwrap();

		match node.implementation {
			DocumentNodeImplementation::Network(mut inner_network) => {
				// Connect all network inputs to either the parent network nodes, or newly created value nodes.
				inner_network.map_ids(|_| gen_node_id());
				network.edges.extend(inner_network.edges);
				for (document_input, network_input) in node.inputs.iter().zip(network.inputs.clone().iter()) {
					match document_input {
						NodeInput::Node(node) => network.edges.push(Connection { from: *node, to: *network_input }),
						NodeInput::Value(widget) => {
							let new_id = gen_node_id();
							let value_node = DocumentNode {
								name: "value".into(),
								id: new_id,
								inputs: Vec::new(),
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNode::new(format!("{}-{}", node.name, widget))),
							};
							nodes.insert(new_id, value_node);

							network.edges.push(Connection { from: new_id, to: *network_input });
						}
					}
				}
				// Copy nodes from the inner network into the parent network
				for node_id in inner_network.nodes.keys().cloned().collect::<Vec<_>>() {
					let (_old_id, mut node) = inner_network.nodes.remove_entry(&node_id).unwrap();
					node.inputs = network
						.edges
						.iter()
						.filter(|Connection { to, .. }| *to == node_id)
						.map(|Connection { from, .. }| NodeInput::Node(*from))
						.collect();
					assert_eq!(node.inputs.len(), 1);
					nodes.insert(node.id, node);
				}
				node.implementation = DocumentNodeImplementation::ProtoNode(ProtoNode::id());
			}
			DocumentNodeImplementation::ProtoNode(proto_node) => {
				node.implementation = DocumentNodeImplementation::ProtoNode(proto_node);
			}
		}
		nodes.insert(id, node);
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

/*
use core::marker::PhantomData;

use graphene_core::{structural::After, structural::ComposeNode, value::ValueNode, Node, RefNode};

use crate::any::Any;
use crate::any::DynAnyNode;

pub trait DocumentNode<I>: RefNode<I> {
	fn input_hints(&self) -> &'static [&'static str];
	fn input_types(&self) -> &'static [&'static str];
	fn inputs(&self) -> Vec<String> {
		self.input_hints().iter().zip(self.input_types()).map(|(a, b)| format!("{}{}", a, b)).collect()
	}
}

struct InjectNode<N: Node<I> + Copy, I>(N, PhantomData<I>);

impl<'n, N: Node<I> + Copy, I> Node<&'n [&'n AnyNode<'n>]> for &'n InjectNode<N, I> {
	type Output = Box<dyn RefNode<Any<'n>, Output = Any<'n>> + 'n>;
	fn eval(self, input: &'n [&'n AnyNode<'n>]) -> Self::Output {
		assert_eq!(input.len(), 1);
		Box::new(ComposeNode::new(&DynAnyNode(input[0]), &DynAnyNode(self.0)))
	}
}

impl<N: Node<I> + Copy, I> InjectNode<N, I> {
	const TYPES: &'static [&'static str] = &[core::any::type_name::<I>()];
	const HINTS: &'static [&'static str] = &["input: "];
}
impl<'n, N: Node<I> + Copy, I> DocumentNode<&'n [&'n AnyNode<'n>]> for &'n InjectNode<N, I> {
	fn input_hints(&self) -> &'static [&'static str] {
		InjectNode::<N, I>::HINTS
	}
	fn input_types(&self) -> &'static [&'static str] {
		InjectNode::<N, I>::TYPES
	}
}

pub type NodeId = u32;

type AnyNode<'n> = dyn RefNode<Any<'n>, Output = Any<'n>>;

pub struct DocumentGraphNode<'n> {
	pub id: NodeId,
	pub inputs: Vec<NodeInput>,
	pub node: NodeWrapper<'n>,
}

impl<'n> DocumentGraphNode<'n> {
	pub fn new(id: NodeId, inputs: Vec<NodeInput>, node: NodeWrapper<'n>) -> Self {
		Self { id, inputs, node }
	}
}

pub struct NodeWrapper<'n> {
	pub node: &'n (dyn Node<Any<'n>, Output = Any<'n>> + 'n),

	pub path: &'static str,
}

impl<'n> NodeWrapper<'n> {
	pub fn new(node: &'n (dyn Node<Any<'n>, Output = Any<'n>> + 'n), path: &'static str) -> Self {
		Self { node, path }
	}
}

pub enum NodeInput {
	Node(NodeId),
	Default(ValueNode<Any<'static>>),
}

#[cfg(test)]
mod test {
	use crate::any::DynAnyNode;

	use super::*;
	use graphene_core::value::ValueNode;

	#[test]
	fn inject_node() {
		let inject_node = InjectNode(&ValueNode(4u32), PhantomData);
		use super::DocumentNode;
		/*assert_eq!(
			(&inject_node as &dyn DocumentNode<&[&AnyNode], Output = ComposeNode<&AnyNode, ValueNode<u32>, ()>>).inputs(),
			vec!["input: ()"]
		);*/
		let any_inject = DynAnyNode(&inject_node, PhantomData);
		let any_inject = Box::leak(Box::new(any_inject));
		let wrapped = NodeWrapper::new((&any_inject) as &(dyn Node<&[&AnyNode], Output = Any>), "grahpene_core::document::InjectNode");
		let document_node = DocumentGraphNode::new(0, vec![], wrapped);
	}
}
*/
