use std::error::Error;
use std::{collections::HashMap, sync::Arc};

use borrow_stack::{BorrowStack, FixedSizeStack};
use graph_craft::document::value::Value;
use graph_craft::document::NodeId;
use graph_craft::proto::{ConstructionArgs, ProtoNode, ProtoNodeInput};
use graphene_core::Node;
use graphene_std::any::{Any, IntoTypeErasedNode, TypeErasedNode};

use crate::node_registry::constrcut_node;
use graph_craft::{executor::Executor, proto::ProtoNetwork};

pub struct DynamicExecutor {
	stack: FixedSizeStack<TypeErasedNode<'static>>,
}

impl DynamicExecutor {
	pub fn new(proto_network: ProtoNetwork) -> Self {
		assert_eq!(proto_network.inputs.len(), 1);
		let node_count = proto_network.nodes.len();
		let stack = FixedSizeStack::new(node_count);
		for (_id, node) in proto_network.nodes {
			//constrcut_node(node, &stack);
		}
		Self { stack }
	}
}

impl Executor for DynamicExecutor {
	fn execute(&self, input: Any<'static>) -> Result<Any<'static>, Box<dyn Error>> {
		let result = unsafe { self.stack.get().last().unwrap().eval(input) };
		Ok(result)
	}
}

pub struct NodeContainer<'n> {
	node: TypeErasedNode<'n>,
	dependencies: Vec<Arc<NodeContainer<'n>>>,
}

impl<'a> NodeContainer<'a> {
	pub unsafe fn static_ref(&self) -> &'static TypeErasedNode<'a> {
		&*(&self.node as *const TypeErasedNode<'a>)
	}
}

impl<'a> AsRef<TypeErasedNode<'a>> for NodeContainer<'a> {
	fn as_ref(&self) -> &TypeErasedNode<'a> {
		&self.node
	}
}

#[derive(Default)]
pub struct BorrowTree {
	nodes: HashMap<NodeId, Arc<NodeContainer<'static>>>,
}

impl BorrowTree {
	pub fn new(proto_network: ProtoNetwork) -> Self {
		let mut nodes = BorrowTree::default();
		for (id, node) in proto_network.nodes {
			nodes.push_node(id, node)
		}
		nodes
	}
	fn node_refs(&self, nodes: &[NodeId]) -> Vec<&'static TypeErasedNode<'static>> {
		nodes
			.iter()
			.map(|node| unsafe { &*((&self.nodes.get(node).unwrap().as_ref().node) as *const TypeErasedNode<'static>) as &'static TypeErasedNode<'static> })
			.collect()
	}
	fn node_deps(&self, nodes: &[NodeId]) -> Vec<Arc<NodeContainer<'static>>> {
		nodes.iter().map(|node| self.nodes.get(node).unwrap().clone()).collect()
	}

	fn store_node(&mut self, node: TypeErasedNode<'static>, id: NodeId, dependencies: Vec<Arc<NodeContainer<'static>>>) -> Arc<NodeContainer<'static>> {
		let node = Arc::new(NodeContainer { node, dependencies });
		self.nodes.insert(id, node.clone());
		node
	}

	pub fn get(&self, id: NodeId) -> Option<Arc<NodeContainer<'static>>> {
		self.nodes.get(&id).cloned()
	}

	fn free_node(&mut self, id: NodeId) {
		self.nodes.remove(&id);
	}

	pub fn push_node(&mut self, id: NodeId, proto_node: ProtoNode) {
		let ProtoNode { input, construction_args, identifier } = proto_node;

		assert_eq!(
			input,
			ProtoNodeInput::None,
			"Only nodes without inputs are supported. Any inputs should already be resolved by placing ComposeNodes"
		);

		match construction_args {
			ConstructionArgs::Value(value) => {
				let node = graphene_core::generic::FnNode::new(move |_| value.clone().up_box() as Any<'static>);

				let node = node.into_type_erased();
				self.store_node(node, id, vec![]);
			}
			ConstructionArgs::Nodes(ids) => {
				let construction_nodes = self.node_refs(ids.as_slice());
				let node = constrcut_node(identifier, construction_nodes);
				self.store_node(node, id, self.node_deps(ids.as_slice()));
			}
		}

		/*
		if let Some((_id, f)) = self.nodes.get(proto_node.identifier) {
			f(proto_node, stack);
		} else {
			let other_types = NODE_REGISTRY
				.iter()
				.map(|(id, _)| id)
				.filter(|id| id.name.as_ref() == proto_node.identifier.name.as_ref())
				.collect::<Vec<_>>();
			panic!(
				"NodeImplementation: {:?} not found in Registry. Types for which the node is implemented:\n {:#?}",
				proto_node.identifier, other_types
			);
		}*/
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn push_node() {
		let mut tree = BorrowTree::default();
		let val_1_protonode = ProtoNode::value(ConstructionArgs::Value(Box::new(2u32)));
		tree.push_node(0, val_1_protonode);
		let node = tree.get(0).unwrap();
		let node = unsafe { node.static_ref() };
		let value = node.eval(().into());
		assert_eq!(*dyn_any::downcast::<u32>(value).unwrap(), 2u32);
	}
}
