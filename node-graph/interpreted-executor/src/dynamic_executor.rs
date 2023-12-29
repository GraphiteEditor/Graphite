use crate::node_registry;

use dyn_any::StaticType;
use graph_craft::document::value::{TaggedValue, UpcastNode};
use graph_craft::document::{NodeId, Source};
use graph_craft::graphene_compiler::Executor;
use graph_craft::proto::{ConstructionArgs, GraphError, LocalFuture, NodeContainer, ProtoNetwork, ProtoNode, SharedNodeContainer, TypeErasedBox, TypingContext};
use graph_craft::proto::{GraphErrorType, GraphErrors};
use graph_craft::Type;

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::Arc;

/// An executor of a node graph that does not require an online compilation server, and instead uses `Box<dyn ...>`.
pub struct DynamicExecutor {
	output: NodeId,
	/// Stores all of the dynamic node structs.
	tree: BorrowTree,
	/// Stores the types of the protonodes.
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

#[derive(PartialEq, Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ResolvedDocumentNodeTypes {
	pub inputs: HashMap<Source, Type>,
	pub outputs: HashMap<Source, Type>,
}

impl DynamicExecutor {
	pub async fn new(proto_network: ProtoNetwork) -> Result<Self, GraphErrors> {
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

	/// Updates the existing [`BorrowTree`] to reflect the new [`ProtoNetwork`], reusing nodes where possible.
	pub async fn update(&mut self, proto_network: ProtoNetwork) -> Result<(), GraphErrors> {
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

	/// Calls the `Node::serialize` for that specific node, returning for example the cached value for a monitor node. The node path must match the document node path.
	pub fn introspect(&self, node_path: &[NodeId]) -> Option<Option<Arc<dyn std::any::Any>>> {
		self.tree.introspect(node_path)
	}

	pub fn input_type(&self) -> Option<Type> {
		self.typing_context.type_of(self.output).map(|node_io| node_io.input.clone())
	}

	pub fn output_type(&self) -> Option<Type> {
		self.typing_context.type_of(self.output).map(|node_io| node_io.output.clone())
	}

	pub fn document_node_types(&self) -> ResolvedDocumentNodeTypes {
		let mut resolved_document_node_types = ResolvedDocumentNodeTypes::default();
		for (source, &(protonode_id, protonode_index)) in self.tree.inputs_source_map() {
			let Some(node_io) = self.typing_context.type_of(protonode_id) else { continue };
			let Some(ty) = [&node_io.input].into_iter().chain(&node_io.parameters).nth(protonode_index) else {
				continue;
			};
			resolved_document_node_types.inputs.insert(source.clone(), ty.clone());
		}
		for (source, &protonode_id) in self.tree.outputs_source_map() {
			let Some(node_io) = self.typing_context.type_of(protonode_id) else { continue };
			resolved_document_node_types.outputs.insert(source.clone(), node_io.output.clone());
		}
		resolved_document_node_types
	}
}

impl<'a, I: StaticType + 'a> Executor<I, TaggedValue> for &'a DynamicExecutor {
	fn execute(&self, input: I) -> LocalFuture<Result<TaggedValue, Box<dyn Error>>> {
		Box::pin(async move { self.tree.eval_tagged_value(self.output, input).await.map_err(|e| e.into()) })
	}
}

#[derive(Default)]
/// A store of the dynamically typed nodes and also the source map.
pub struct BorrowTree {
	/// A hashmap of node IDs and dynamically typed nodes.
	nodes: HashMap<NodeId, SharedNodeContainer>,
	/// A hashmap from the document path to the protonode ID.
	source_map: HashMap<Vec<NodeId>, NodeId>,
	/// Each document input source maps to one protonode input (however one protonode input may come from several sources)
	inputs_source_map: HashMap<Source, (NodeId, usize)>,
	/// A mapping of document input sources to the (single) protonode output
	outputs_source_map: HashMap<Source, NodeId>,
}

impl BorrowTree {
	pub async fn new(proto_network: ProtoNetwork, typing_context: &TypingContext) -> Result<BorrowTree, GraphErrors> {
		let mut nodes = BorrowTree::default();
		for (id, node) in proto_network.nodes {
			nodes.push_node(id, node, typing_context).await?
		}
		Ok(nodes)
	}

	/// Pushes new nodes into the tree and return orphaned nodes
	pub async fn update(&mut self, proto_network: ProtoNetwork, typing_context: &TypingContext) -> Result<Vec<NodeId>, GraphErrors> {
		let mut old_nodes: HashSet<_> = self.nodes.keys().copied().collect();
		for (id, node) in proto_network.nodes {
			if !self.nodes.contains_key(&id) {
				self.push_node(id, node, typing_context).await?;
			}
			old_nodes.remove(&id);
		}
		self.source_map.retain(|_, nid| !old_nodes.contains(nid));
		self.inputs_source_map.retain(|_, (nid, _)| !old_nodes.contains(nid));
		self.outputs_source_map.retain(|_, nid| !old_nodes.contains(nid));
		self.nodes.retain(|nid, _| !old_nodes.contains(nid));
		Ok(old_nodes.into_iter().collect())
	}

	fn node_deps(&self, nodes: &[NodeId]) -> Vec<SharedNodeContainer> {
		nodes.iter().map(|node| self.nodes.get(node).unwrap().clone()).collect()
	}

	fn store_node(&mut self, node: SharedNodeContainer, id: NodeId) {
		self.nodes.insert(id, node);
	}

	/// Calls the `Node::serialize` for that specific node, returning for example the cached value for a monitor node. The node path must match the document node path.
	pub fn introspect(&self, node_path: &[NodeId]) -> Option<Option<Arc<dyn std::any::Any>>> {
		let id = self.source_map.get(node_path)?;
		let node = self.nodes.get(id)?;
		Some(node.serialize())
	}

	pub fn get(&self, id: NodeId) -> Option<SharedNodeContainer> {
		self.nodes.get(&id).cloned()
	}

	/// Evaluate the output node of the [`BorrowTree`].
	pub async fn eval<'i, I: StaticType + 'i, O: StaticType + 'i>(&'i self, id: NodeId, input: I) -> Option<O> {
		let node = self.nodes.get(&id).cloned()?;
		let output = node.eval(Box::new(input));
		dyn_any::downcast::<O>(output.await).ok().map(|o| *o)
	}
	/// Evaluate the output node of the [`BorrowTree`] and cast it to a tagged value.
	/// This ensures that no borrowed data can escape the node graph.
	pub async fn eval_tagged_value<'i, I: StaticType + 'i>(&'i self, id: NodeId, input: I) -> Result<TaggedValue, String> {
		let node = self.nodes.get(&id).cloned().ok_or("Output node not found in executor")?;
		let output = node.eval(Box::new(input));
		TaggedValue::try_from_any(output.await)
	}

	pub fn free_node(&mut self, id: NodeId) {
		self.nodes.remove(&id);
	}

	/// Insert a new node into the borrow tree, calling the constructor function from `node_registry.rs`.
	pub async fn push_node(&mut self, id: NodeId, proto_node: ProtoNode, typing_context: &TypingContext) -> Result<(), GraphErrors> {
		self.source_map.insert(proto_node.original_location.path.clone().unwrap_or_default(), id);

		let params = match &proto_node.construction_args {
			ConstructionArgs::Nodes(nodes) => nodes.len() + 1,
			_ => 2,
		};
		self.inputs_source_map
			.extend((0..params).flat_map(|i| proto_node.original_location.inputs(i).map(move |source| (source, (id, i)))));
		self.outputs_source_map.extend(proto_node.original_location.outputs(0).map(|source| (source, id)));
		for x in proto_node.original_location.outputs_source.values() {
			assert_eq!(*x, 0, "protonodes should refer to output index 0");
		}

		match &proto_node.construction_args {
			ConstructionArgs::Value(value) => {
				let upcasted = UpcastNode::new(value.to_owned());
				let node = Box::new(upcasted) as TypeErasedBox<'_>;
				let node = NodeContainer::new(node);
				self.store_node(node, id);
			}
			ConstructionArgs::Inline(_) => unimplemented!("Inline nodes are not supported yet"),
			ConstructionArgs::Nodes(ids) => {
				let ids: Vec<_> = ids.iter().map(|(id, _)| *id).collect();
				let construction_nodes = self.node_deps(&ids);
				let constructor = typing_context.constructor(id).ok_or_else(|| vec![GraphError::new(&proto_node, GraphErrorType::NoConstructor)])?;
				let node = constructor(construction_nodes).await;
				let node = NodeContainer::new(node);
				self.store_node(node, id);
			}
		};
		Ok(())
	}

	pub fn inputs_source_map(&self) -> impl Iterator<Item = (&Source, &(NodeId, usize))> {
		self.inputs_source_map.iter()
	}

	pub fn outputs_source_map(&self) -> impl Iterator<Item = (&Source, &NodeId)> {
		self.outputs_source_map.iter()
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
		let future = tree.push_node(NodeId(0), val_1_protonode, &context); //.await.unwrap();
		futures::executor::block_on(future).unwrap();
		let _node = tree.get(NodeId(0)).unwrap();
		let result = futures::executor::block_on(tree.eval(NodeId(0), ()));
		assert_eq!(result, Some(2u32));
	}
}
