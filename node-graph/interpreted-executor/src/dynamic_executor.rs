use std::collections::{HashMap, HashSet};
use std::error::Error;

use std::sync::Arc;

use dyn_any::StaticType;
use graph_craft::document::value::{TaggedValue, UpcastNode};
use graph_craft::document::NodeId;
use graph_craft::graphene_compiler::Executor;
use graph_craft::proto::{ConstructionArgs, LocalFuture, NodeContainer, ProtoNetwork, ProtoNode, TypeErasedBox, TypingContext};
use graph_craft::Type;

use crate::node_registry;

pub struct DynamicExecutor {
	output: NodeId,
	tree: BorrowTree,
	typing_context: TypingContext,
	// This allows us to keep the nodes around for one more frame which is used for introspection
	orphaned_nodes: Vec<NodeId>,
}

impl Default for DynamicExecutor {
	fn default() -> Self {
		Self {
			output: Default::default(),
			tree: Default::default(),
			typing_context: TypingContext::new(&node_registry::NODE_REGISTRY),
			orphaned_nodes: Vec::new(),
		}
	}
}

impl DynamicExecutor {
	pub async fn new(proto_network: ProtoNetwork) -> Result<Self, String> {
		let mut typing_context = TypingContext::new(&node_registry::NODE_REGISTRY);
		typing_context.update(&proto_network)?;
		let output = proto_network.output;
		let tree = BorrowTree::new(proto_network, &typing_context).await?;

		Ok(Self {
			tree,
			output,
			typing_context,
			orphaned_nodes: Vec::new(),
		})
	}

	pub async fn update(&mut self, proto_network: ProtoNetwork) -> Result<(), String> {
		self.output = proto_network.output;
		self.typing_context.update(&proto_network)?;
		let mut orphans = self.tree.update(proto_network, &self.typing_context).await?;
		core::mem::swap(&mut self.orphaned_nodes, &mut orphans);
		for node_id in orphans {
			if self.orphaned_nodes.contains(&node_id) {
				self.tree.free_node(node_id)
			}
		}
		Ok(())
	}

	pub fn introspect(&self, node_path: &[NodeId]) -> Option<Option<Arc<dyn std::any::Any>>> {
		self.tree.introspect(node_path)
	}

	pub fn input_type(&self) -> Option<Type> {
		self.typing_context.type_of(self.output).map(|node_io| node_io.input.clone())
	}

	pub fn output_type(&self) -> Option<Type> {
		self.typing_context.type_of(self.output).map(|node_io| node_io.output.clone())
	}
}

impl<'a, I: StaticType + 'a> Executor<I, TaggedValue> for &'a DynamicExecutor {
	fn execute(&self, input: I) -> LocalFuture<Result<TaggedValue, Box<dyn Error>>> {
		Box::pin(async move { self.tree.eval_tagged_value(self.output, input).await.map_err(|e| e.into()) })
	}
}

#[derive(Default)]
pub struct BorrowTree {
	nodes: HashMap<NodeId, Arc<NodeContainer>>,
	source_map: HashMap<Vec<NodeId>, NodeId>,
}

impl BorrowTree {
	pub async fn new(proto_network: ProtoNetwork, typing_context: &TypingContext) -> Result<BorrowTree, String> {
		let mut nodes = BorrowTree::default();
		for (id, node) in proto_network.nodes {
			nodes.push_node(id, node, typing_context).await?
		}
		Ok(nodes)
	}

	/// Pushes new nodes into the tree and return orphaned nodes
	pub async fn update(&mut self, proto_network: ProtoNetwork, typing_context: &TypingContext) -> Result<Vec<NodeId>, String> {
		let mut old_nodes: HashSet<_> = self.nodes.keys().copied().collect();
		for (id, node) in proto_network.nodes {
			if !self.nodes.contains_key(&id) {
				self.push_node(id, node, typing_context).await?;
			} else {
				let Some(node_container) = self.nodes.get_mut(&id) else { continue };
				node_container.reset();
			}
			old_nodes.remove(&id);
		}
		self.source_map.retain(|_, nid| !old_nodes.contains(nid));
		Ok(old_nodes.into_iter().collect())
	}

	fn node_deps(&self, nodes: &[NodeId]) -> Vec<Arc<NodeContainer>> {
		nodes.iter().map(|node| self.nodes.get(node).unwrap().clone()).collect()
	}

	fn store_node(&mut self, node: Arc<NodeContainer>, id: NodeId) {
		self.nodes.insert(id, node);
	}

	pub fn introspect(&self, node_path: &[NodeId]) -> Option<Option<Arc<dyn std::any::Any>>> {
		let id = self.source_map.get(node_path)?;
		let node = self.nodes.get(id)?;
		Some(node.serialize())
	}

	pub fn get(&self, id: NodeId) -> Option<Arc<NodeContainer>> {
		self.nodes.get(&id).cloned()
	}

	pub async fn eval<'i, I: StaticType + 'i, O: StaticType + 'i>(&'i self, id: NodeId, input: I) -> Option<O> {
		let node = self.nodes.get(&id).cloned()?;
		let output = node.eval(Box::new(input));
		dyn_any::downcast::<O>(output.await).ok().map(|o| *o)
	}
	pub async fn eval_tagged_value<'i, I: StaticType + 'i>(&'i self, id: NodeId, input: I) -> Result<TaggedValue, String> {
		let node = self.nodes.get(&id).cloned().ok_or("Output node not found in executor")?;
		let output = node.eval(Box::new(input));
		TaggedValue::try_from_any(output.await)
	}

	pub fn free_node(&mut self, id: NodeId) {
		self.nodes.remove(&id);
	}

	pub async fn push_node(&mut self, id: NodeId, proto_node: ProtoNode, typing_context: &TypingContext) -> Result<(), String> {
		let ProtoNode {
			construction_args,
			identifier,
			document_node_path,
			..
		} = proto_node;
		self.source_map.insert(document_node_path, id);

		match construction_args {
			ConstructionArgs::Value(value) => {
				let upcasted = UpcastNode::new(value);
				let node = Box::new(upcasted) as TypeErasedBox<'_>;
				let node = NodeContainer::new(node);
				self.store_node(node.into(), id);
			}
			ConstructionArgs::Inline(_) => unimplemented!("Inline nodes are not supported yet"),
			ConstructionArgs::Nodes(ids) => {
				let ids: Vec<_> = ids.iter().map(|(id, _)| *id).collect();
				let construction_nodes = self.node_deps(&ids);
				let constructor = typing_context.constructor(id).ok_or(format!("No constructor found for node {:?}", identifier))?;
				let node = constructor(construction_nodes).await;
				let node = NodeContainer::new(node);
				self.store_node(node.into(), id);
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
	fn push_node_sync() {
		let mut tree = BorrowTree::default();
		let val_1_protonode = ProtoNode::value(ConstructionArgs::Value(TaggedValue::U32(2u32)), vec![]);
		let context = TypingContext::default();
		let future = tree.push_node(0, val_1_protonode, &context); //.await.unwrap();
		futures::executor::block_on(future).unwrap();
		let _node = tree.get(0).unwrap();
		let result = futures::executor::block_on(tree.eval(0, ()));
		assert_eq!(result, Some(2u32));
	}
}
