use std::collections::HashSet;
use std::error::Error;
use std::{collections::HashMap, sync::Arc};

use dyn_any::StaticType;
use graph_craft::document::value::UpcastNode;
use graph_craft::document::NodeId;
use graph_craft::executor::Executor;
use graph_craft::proto::{ConstructionArgs, ProtoNetwork, ProtoNode, TypingContext};
use graphene_std::any::{Any, TypeErasedPinned, TypeErasedPinnedRef};

use crate::node_registry;

#[derive(Debug, Clone)]
pub struct DynamicExecutor {
	output: NodeId,
	tree: BorrowTree,
	typing_context: TypingContext,
}

impl Default for DynamicExecutor {
	fn default() -> Self {
		Self {
			output: Default::default(),
			tree: Default::default(),
			typing_context: TypingContext::new(&node_registry::NODE_REGISTRY),
		}
	}
}

impl DynamicExecutor {
	pub fn new(proto_network: ProtoNetwork) -> Result<Self, String> {
		let mut typing_context = TypingContext::new(&node_registry::NODE_REGISTRY);
		typing_context.update(&proto_network)?;
		let output = proto_network.output;
		let tree = BorrowTree::new(proto_network, &typing_context)?;

		Ok(Self { tree, output, typing_context })
	}

	pub fn update(&mut self, proto_network: ProtoNetwork) -> Result<(), String> {
		self.output = proto_network.output;
		self.typing_context.update(&proto_network)?;
		trace!("setting output to {}", self.output);
		self.tree.update(proto_network, &self.typing_context)?;
		Ok(())
	}
}

impl Executor for DynamicExecutor {
	fn execute<'a, 's: 'a>(&'s self, input: Any<'a>) -> Result<Any<'a>, Box<dyn Error>> {
		self.tree.eval_any(self.output, input).ok_or_else(|| "Failed to execute".into())
	}
}

pub struct NodeContainer<'n> {
	pub node: TypeErasedPinned<'n>,
	// the dependencies are only kept to ensure that the nodes are not dropped while still in use
	_dependencies: Vec<Arc<NodeContainer<'static>>>,
}

impl<'a> core::fmt::Debug for NodeContainer<'a> {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("NodeContainer").finish()
	}
}

impl<'a> NodeContainer<'a> {
	pub fn new(node: TypeErasedPinned<'a>, _dependencies: Vec<Arc<NodeContainer<'static>>>) -> Self {
		Self { node, _dependencies }
	}

	/// Return a static reference to the TypeErasedNode
	/// # Safety
	/// This is unsafe because the returned reference is only valid as long as the NodeContainer is alive
	pub unsafe fn erase_lifetime(self) -> NodeContainer<'static> {
		std::mem::transmute(self)
	}
}
impl NodeContainer<'static> {
	pub unsafe fn static_ref(&self) -> TypeErasedPinnedRef<'static> {
		let s = &*(self as *const Self);
		*(&s.node.as_ref() as *const TypeErasedPinnedRef<'static>)
	}
}

#[derive(Default, Debug, Clone)]
pub struct BorrowTree {
	nodes: HashMap<NodeId, Arc<NodeContainer<'static>>>,
}

impl BorrowTree {
	pub fn new(proto_network: ProtoNetwork, typing_context: &TypingContext) -> Result<Self, String> {
		let mut nodes = BorrowTree::default();
		for (id, node) in proto_network.nodes {
			nodes.push_node(id, node, typing_context)?
		}
		Ok(nodes)
	}

	/// Pushes new nodes into the tree and return orphaned nodes
	pub fn update(&mut self, proto_network: ProtoNetwork, typing_context: &TypingContext) -> Result<Vec<NodeId>, String> {
		let mut old_nodes: HashSet<_> = self.nodes.keys().copied().collect();
		for (id, node) in proto_network.nodes {
			if !self.nodes.contains_key(&id) {
				self.push_node(id, node, typing_context)?;
				old_nodes.remove(&id);
			}
		}
		Ok(old_nodes.into_iter().collect())
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

	pub fn push_node(&mut self, id: NodeId, proto_node: ProtoNode, typing_context: &TypingContext) -> Result<(), String> {
		let ProtoNode { construction_args, identifier, .. } = proto_node;

		match construction_args {
			ConstructionArgs::Value(value) => {
				let upcasted = UpcastNode::new(value);
				let node = Box::pin(upcasted) as TypeErasedPinned<'_>;
				let node = NodeContainer { node, _dependencies: vec![] };
				let node = unsafe { node.erase_lifetime() };
				self.store_node(Arc::new(node), id);
			}
			ConstructionArgs::Nodes(ids) => {
				let ids: Vec<_> = ids.iter().map(|(id, _)| *id).collect();
				let construction_nodes = self.node_refs(&ids);
				let constructor = typing_context.constructor(id).ok_or(format!("No constructor found for node {:?}", identifier))?;
				let node = constructor(construction_nodes);
				let node = NodeContainer {
					node,
					_dependencies: self.node_deps(&ids),
				};
				let node = unsafe { node.erase_lifetime() };
				self.store_node(Arc::new(node), id);
			}
		};
		Ok(())
	}
}

#[cfg(test)]
mod test {
	use graph_craft::document::value::TaggedValue;

	use super::*;

	#[test]
	fn push_node() {
		let mut tree = BorrowTree::default();
		let val_1_protonode = ProtoNode::value(ConstructionArgs::Value(TaggedValue::U32(2u32)));
		tree.push_node(0, val_1_protonode, &TypingContext::default()).unwrap();
		let _node = tree.get(0).unwrap();
		assert_eq!(tree.eval(0, ()), Some(2u32));
	}
}
