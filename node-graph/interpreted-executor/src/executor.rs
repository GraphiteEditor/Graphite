use std::error::Error;
use std::{collections::HashMap, sync::Arc};

use borrow_stack::{BorrowStack, FixedSizeStack};
use dyn_any::{DynAny, StaticType};
use graph_craft::document::value::{UpcastNode, Value};
use graph_craft::document::NodeId;
use graph_craft::proto::{ConstructionArgs, ProtoNode, ProtoNodeInput};
use graphene_core::value::ValueNode;
use graphene_core::Node;
use graphene_std::any::{Any, DynAnyNode, IntoTypeErasedNode, TypeErasedNode, TypeErasedPinned, TypeErasedPinnedRef};

use crate::node_registry::constrcut_node;
use graph_craft::{executor::Executor, proto::ProtoNetwork};

pub struct DynamicExecutor {
	stack: FixedSizeStack<()>,
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
	fn execute<'a>(&self, input: Any<'a>) -> Result<Any<'a>, Box<dyn Error>> {
		/*let result = unsafe { self.stack.get().last().unwrap().eval(input) };
		Ok(result)*/
		todo!()
	}
}

pub struct NodeContainer<'n> {
	node: TypeErasedPinned<'n>,
	// the dependencies are only kept to ensure that the nodes are not dropped while still in use
	_dependencies: Vec<Arc<NodeContainer<'static>>>,
}

impl<'a> NodeContainer<'a> {
	/// Return a static reference to the TypeErasedNode
	/// # Safety
	/// This is unsafe because the returned reference is only valid as long as the NodeContainer is alive
	pub unsafe fn erase_lifetime(self) -> NodeContainer<'static> {
		std::mem::transmute(self)
	}
}
impl NodeContainer<'static> {
	unsafe fn static_ref(&self) -> TypeErasedPinnedRef<'static> {
		let s = &*(self as *const Self);
		*(&s.node.as_ref() as *const TypeErasedPinnedRef<'static>)
	}
}

/*
impl<'a> AsRef<TypeErasedPinnedRef<'a>> for NodeContainer<'a> {
	fn as_ref(&self) -> &'a TypeErasedPinnedRef<'a> {
		self.node.as_ref()
	}
}*/

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
	fn node_refs(&self, nodes: &[NodeId]) -> Vec<TypeErasedPinnedRef<'static>> {
		self.node_deps(nodes).into_iter().map(|node| unsafe { node.as_ref().static_ref() }).collect()
	}
	fn node_deps(&self, nodes: &[NodeId]) -> Vec<Arc<NodeContainer<'static>>> {
		nodes.iter().map(|node| self.nodes.get(node).unwrap().clone()).collect()
	}

	fn store_node(&mut self, node: Arc<NodeContainer<'static>>, id: NodeId) -> Arc<NodeContainer<'static>> {
		self.nodes.insert(id, node.clone());
		node
	}

	pub fn get(&self, id: NodeId) -> Option<Arc<NodeContainer<'static>>> {
		self.nodes.get(&id).cloned()
	}

	pub fn eval<'i, I: StaticType + 'i, O: StaticType + 'i>(&self, id: NodeId, input: I) -> Option<O> {
		let node = self.nodes.get(&id).cloned()?;
		let output = node.node.eval(Box::new(input));
		dyn_any::downcast::<O>(output).ok().map(|o| *o)
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
				let upcasted = UpcastNode::new(value);
				let node = Box::pin(upcasted) as TypeErasedPinned<'_>;
				let node = NodeContainer { node, _dependencies: vec![] };
				let node = unsafe { node.erase_lifetime() };
				self.store_node(Arc::new(node), id);
			}
			ConstructionArgs::Nodes(ids) => {
				let construction_nodes = self.node_refs(&ids);
				let node = constrcut_node(identifier, construction_nodes);
				let node = NodeContainer {
					node,
					_dependencies: self.node_deps(&ids),
				};
				let node = unsafe { node.erase_lifetime() };
				self.store_node(Arc::new(node), id);
			}
		}
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
		assert_eq!(tree.eval(0, ()), Some(2u32));
	}
}
