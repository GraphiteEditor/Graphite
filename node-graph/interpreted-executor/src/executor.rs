use std::collections::HashSet;
use std::error::Error;
use std::{collections::HashMap, sync::Arc};

use dyn_any::StaticType;
use graph_craft::document::value::UpcastNode;
use graph_craft::document::NodeId;
use graph_craft::proto::{ConstructionArgs, ProtoNode, ProtoNodeInput};
use graphene_std::any::{Any, TypeErasedPinned, TypeErasedPinnedRef};

use crate::node_registry::constrcut_node;
use graph_craft::{executor::Executor, proto::ProtoNetwork};

pub struct DynamicExecutor {
	output: NodeId,
	tree: BorrowTree,
}

impl DynamicExecutor {
	pub fn new(proto_network: ProtoNetwork) -> Self {
		let output = proto_network.output;
		let tree = BorrowTree::new(proto_network);
		Self { tree, output }
	}

	pub fn update(&mut self, proto_network: ProtoNetwork) {
		self.output = proto_network.output;
		self.tree.update(proto_network);
	}
}

impl Executor for DynamicExecutor {
	fn execute<'a, 's: 'a>(&'s self, input: Any<'a>) -> Result<Any<'a>, Box<dyn Error>> {
		self.tree.eval_any(self.output, input).ok_or_else(|| "Failed to execute".into())
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

	/// Pushes new nodes into the tree and return orphaned nodes
	pub fn update(&mut self, proto_network: ProtoNetwork) -> Vec<NodeId> {
		let mut old_nodes: HashSet<_> = self.nodes.keys().copied().collect();
		for (id, node) in proto_network.nodes {
			if !self.nodes.contains_key(&id) {
				self.push_node(id, node);
				old_nodes.remove(&id);
			}
		}
		old_nodes.into_iter().collect()
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
	pub fn eval_any<'i, 's: 'i>(&'s self, id: NodeId, input: Any<'i>) -> Option<Any<'i>> {
		let node = self.nodes.get(&id)?;
		Some(node.node.eval(input))
	}

	pub fn free_node(&mut self, id: NodeId) {
		self.nodes.remove(&id);
	}

	pub fn push_node(&mut self, id: NodeId, proto_node: ProtoNode) {
		let ProtoNode { input, construction_args, identifier } = proto_node;

		assert!(
			!matches!(input, ProtoNodeInput::Node(_)),
			"Only nodes without inputs are supported. Any inputs should already be resolved by placing ComposeNodes {:?}, {:?}",
			identifier,
			construction_args
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
