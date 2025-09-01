use crate::node_registry;
use dyn_any::StaticType;
use graph_craft::Type;
use graph_craft::document::NodeId;
use graph_craft::document::value::{TaggedValue, UpcastAsRefNode, UpcastNode};
use graph_craft::graphene_compiler::Executor;
use graph_craft::proto::{ConstructionArgs, GraphError, LocalFuture, NodeContainer, ProtoNetwork, ProtoNode, SharedNodeContainer, TypeErasedBox, TypingContext};
use graph_craft::proto::{GraphErrorType, GraphErrors};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::Arc;

/// An executor of a node graph that does not require an online compilation server, and instead uses `Box<dyn ...>`.
#[derive(Clone)]
pub struct DynamicExecutor {
	output: NodeId,
	/// Stores all of the dynamic node structs.
	tree: BorrowTree,
	/// Stores the types of the proto nodes.
	typing_context: TypingContext,
	// This allows us to keep the nodes around for one more frame which is used for introspection
	orphaned_nodes: HashSet<NodeId>,
}

impl Default for DynamicExecutor {
	fn default() -> Self {
		Self {
			output: Default::default(),
			tree: Default::default(),
			typing_context: TypingContext::new(&node_registry::NODE_REGISTRY),
			orphaned_nodes: HashSet::new(),
		}
	}
}

#[derive(PartialEq, Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct NodeTypes {
	pub inputs: Vec<Type>,
	pub output: Type,
}

#[derive(PartialEq, Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ResolvedDocumentNodeTypes {
	pub types: HashMap<Vec<NodeId>, NodeTypes>,
}

type Path = Box<[NodeId]>;

#[derive(PartialEq, Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ResolvedDocumentNodeTypesDelta {
	pub add: Vec<(Path, NodeTypes)>,
	pub remove: Vec<Path>,
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
			orphaned_nodes: HashSet::new(),
		})
	}

	/// Updates the existing [`BorrowTree`] to reflect the new [`ProtoNetwork`], reusing nodes where possible.
	#[cfg_attr(debug_assertions, inline(never))]
	pub async fn update(&mut self, proto_network: ProtoNetwork) -> Result<ResolvedDocumentNodeTypesDelta, GraphErrors> {
		self.output = proto_network.output;
		self.typing_context.update(&proto_network)?;
		let (add, orphaned) = self.tree.update(proto_network, &self.typing_context).await?;
		let old_to_remove = core::mem::replace(&mut self.orphaned_nodes, orphaned);
		let mut remove = Vec::with_capacity(old_to_remove.len() - self.orphaned_nodes.len().min(old_to_remove.len()));
		for node_id in old_to_remove {
			if self.orphaned_nodes.contains(&node_id) {
				let path = self.tree.free_node(node_id);
				self.typing_context.remove_inference(node_id);
				if let Some(path) = path {
					remove.push(path);
				}
			}
		}
		let add = self.document_node_types(add.into_iter()).collect();
		Ok(ResolvedDocumentNodeTypesDelta { add, remove })
	}

	/// Calls the `Node::serialize` for that specific node, returning for example the cached value for a monitor node. The node path must match the document node path.
	pub fn introspect(&self, node_path: &[NodeId]) -> Result<Arc<dyn std::any::Any + Send + Sync + 'static>, IntrospectError> {
		self.tree.introspect(node_path)
	}

	pub fn input_type(&self) -> Option<Type> {
		self.typing_context.type_of(self.output).map(|node_io| node_io.call_argument.clone())
	}

	pub fn tree(&self) -> &BorrowTree {
		&self.tree
	}

	pub fn output(&self) -> NodeId {
		self.output
	}

	pub fn output_type(&self) -> Option<Type> {
		self.typing_context.type_of(self.output).map(|node_io| node_io.return_value.clone())
	}

	pub fn document_node_types<'a>(&'a self, nodes: impl Iterator<Item = Path> + 'a) -> impl Iterator<Item = (Path, NodeTypes)> + 'a {
		nodes.flat_map(|id| self.tree.source_map().get(&id).map(|(_, b)| (id, b.clone())))
		// TODO: https://github.com/GraphiteEditor/Graphite/issues/1767
		// TODO: Non exposed inputs are not added to the inputs_source_map, so they are not included in the resolved_document_node_types. The type is still available in the typing_context. This only affects the UI-only "Import" node.
	}
}

impl<I> Executor<I, TaggedValue> for &DynamicExecutor
where
	I: StaticType + 'static + Send + Sync + std::panic::UnwindSafe,
{
	fn execute(&self, input: I) -> LocalFuture<'_, Result<TaggedValue, Box<dyn Error>>> {
		Box::pin(async move {
			use futures::FutureExt;

			let result = self.tree.eval_tagged_value(self.output, input);
			let wrapped_result = std::panic::AssertUnwindSafe(result).catch_unwind().await;

			match wrapped_result {
				Ok(result) => result.map_err(|e| e.into()),
				Err(e) => {
					Box::leak(e);
					Err("Node graph execution panicked".into())
				}
			}
		})
	}
}
pub struct InputMapping {}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IntrospectError {
	PathNotFound(Vec<NodeId>),
	ProtoNodeNotFound(NodeId),
	NoData,
	RuntimeNotReady,
}

impl std::fmt::Display for IntrospectError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			IntrospectError::PathNotFound(path) => write!(f, "Path not found: {path:?}"),
			IntrospectError::ProtoNodeNotFound(id) => write!(f, "ProtoNode not found: {id:?}"),
			IntrospectError::NoData => write!(f, "No data found for this node"),
			IntrospectError::RuntimeNotReady => write!(f, "Node runtime is not ready"),
		}
	}
}

/// A store of dynamically typed nodes and their associated source map.
///
/// [`BorrowTree`] maintains two main data structures:
/// 1. A map of [`NodeId`]s to their corresponding nodes and paths.
/// 2. A source map that links document paths to node IDs and their types.
///
/// This structure is central to managing the graph of nodes in the interpreter,
/// allowing for efficient access and manipulation of nodes based on their IDs or paths.
///
/// # Fields
///
/// * `nodes`: A [`HashMap`] of [`NodeId`]s to tuples of [`SharedNodeContainer`] and [`Path`].
///   This stores the actual node instances and their associated paths.
///
/// * `source_map`: A [`HashMap`] from [`Path`] to tuples of [`NodeId`] and [`NodeTypes`].
///   This maps document paths to node IDs and their associated type information.
///
/// A store of the dynamically typed nodes and also the source map.
#[derive(Default, Clone)]
pub struct BorrowTree {
	/// A hashmap of node IDs and dynamically typed nodes.
	nodes: HashMap<NodeId, (SharedNodeContainer, Path)>,
	/// A hashmap from the document path to the proto node ID.
	source_map: HashMap<Path, (NodeId, NodeTypes)>,
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
	pub async fn update(&mut self, proto_network: ProtoNetwork, typing_context: &TypingContext) -> Result<(Vec<Path>, HashSet<NodeId>), GraphErrors> {
		let mut old_nodes: HashSet<_> = self.nodes.keys().copied().collect();
		let mut new_nodes: Vec<_> = Vec::new();
		// TODO: Problem: When an identity node is connected directly to an export the first input to identity node is not added to the proto network, while the second input is. This means the primary input does not have a type.
		for (id, node) in proto_network.nodes {
			if !self.nodes.contains_key(&id) {
				new_nodes.push(node.original_location.path.clone().unwrap_or_default().into());
				self.push_node(id, node, typing_context).await?;
			} else if self.update_source_map(id, typing_context, &node) {
				new_nodes.push(node.original_location.path.clone().unwrap_or_default().into());
			}
			old_nodes.remove(&id);
		}
		Ok((new_nodes, old_nodes))
	}

	fn node_deps(&self, nodes: &[NodeId]) -> Vec<SharedNodeContainer> {
		nodes.iter().map(|node| self.nodes.get(node).unwrap().0.clone()).collect()
	}

	fn store_node(&mut self, node: SharedNodeContainer, id: NodeId, path: Path) {
		self.nodes.insert(id, (node, path));
	}

	/// Calls the `Node::serialize` for that specific node, returning for example the cached value for a monitor node. The node path must match the document node path.
	pub fn introspect(&self, node_path: &[NodeId]) -> Result<Arc<dyn std::any::Any + Send + Sync + 'static>, IntrospectError> {
		let (id, _) = self.source_map.get(node_path).ok_or_else(|| IntrospectError::PathNotFound(node_path.to_vec()))?;
		let (node, _path) = self.nodes.get(id).ok_or(IntrospectError::ProtoNodeNotFound(*id))?;
		node.serialize().ok_or(IntrospectError::NoData)
	}

	pub fn get(&self, id: NodeId) -> Option<SharedNodeContainer> {
		self.nodes.get(&id).map(|(node, _)| node.clone())
	}

	/// Evaluate the output node of the [`BorrowTree`].
	pub async fn eval<'i, I, O>(&'i self, id: NodeId, input: I) -> Option<O>
	where
		I: StaticType + 'i + Send + Sync,
		O: StaticType + 'i,
	{
		let (node, _path) = self.nodes.get(&id).cloned()?;
		let output = node.eval(Box::new(input));
		dyn_any::downcast::<O>(output.await).ok().map(|o| *o)
	}
	/// Evaluate the output node of the [`BorrowTree`] and cast it to a tagged value.
	/// This ensures that no borrowed data can escape the node graph.
	pub async fn eval_tagged_value<I>(&self, id: NodeId, input: I) -> Result<TaggedValue, String>
	where
		I: StaticType + 'static + Send + Sync,
	{
		let (node, _path) = self.nodes.get(&id).cloned().ok_or("Output node not found in executor")?;
		let output = node.eval(Box::new(input));
		TaggedValue::try_from_any(output.await)
	}

	/// Removes a node from the [`BorrowTree`] and returns its associated path.
	///
	/// This method removes the specified node from both the `nodes` HashMap and,
	/// if applicable, the `source_map` HashMap.
	///
	/// # Arguments
	///
	/// * `self` - Mutable reference to the [`BorrowTree`].
	/// * `id` - The `NodeId` of the node to be removed.
	///
	/// # Returns
	///
	/// [`Option<Path>`] - The path associated with the removed node, or `None` if the node wasn't found.
	///
	/// # Example
	///
	/// ```rust
	/// use std::collections::HashMap;
	/// use graph_craft::document::*;
	/// use graph_craft::proto::*;
	/// use interpreted_executor::dynamic_executor::BorrowTree;
	/// use interpreted_executor::node_registry;
	///
	///
	/// async fn example() -> Result<(), GraphErrors> {
	///     let (proto_network, node_id, proto_node) = ProtoNetwork::example();
	///     let typing_context = TypingContext::new(&node_registry::NODE_REGISTRY);
	///     let mut borrow_tree = BorrowTree::new(proto_network, &typing_context).await?;
	///
	///     // Assert that the node exists in the BorrowTree
	///     assert!(borrow_tree.get(node_id).is_some(), "Node should exist before removal");
	///
	///     // Remove the node
	///     let removed_path = borrow_tree.free_node(node_id);
	///
	///     // Assert that the node was successfully removed
	///     assert!(removed_path.is_some(), "Node removal should return a path");
	///     assert!(borrow_tree.get(node_id).is_none(), "Node should not exist after removal");
	///
	///     // Try to remove the same node again
	///     let second_removal = borrow_tree.free_node(node_id);
	///
	///     assert_eq!(second_removal, None, "Second removal should return None");
	///
	///     println!("All assertions passed. free_node function works as expected.");
	///
	///     Ok(())
	/// }
	/// ```
	///
	/// # Notes
	///
	/// - Removes the node from `nodes` HashMap.
	/// - If the node is the primary node for its path in the `source_map`, it's also removed from there.
	/// - Returns `None` if the node is not found in the `nodes` HashMap.
	pub fn free_node(&mut self, id: NodeId) -> Option<Path> {
		let (_, path) = self.nodes.remove(&id)?;
		if self.source_map.get(&path)?.0 == id {
			self.source_map.remove(&path);
			return Some(path);
		}
		None
	}

	/// Updates the source map for a given node in the [`BorrowTree`].
	///
	/// This method updates or inserts an entry in the `source_map` HashMap for the specified node,
	/// using type information from the provided [`TypingContext`] and [`ProtoNode`].
	///
	/// # Arguments
	///
	/// * `self` - Mutable reference to the [`BorrowTree`].
	/// * `id` - The `NodeId` of the node to update in the source map.
	/// * `typing_context` - A reference to the [`TypingContext`] containing type information.
	/// * `proto_node` - A reference to the [`ProtoNode`] containing original location information.
	///
	/// # Returns
	///
	/// `bool` - `true` if a new entry was inserted, `false` if an existing entry was updated.
	///
	/// # Notes
	///
	/// - Updates or inserts an entry in the `source_map` HashMap.
	/// - Uses the `ProtoNode`'s original location path as the key for the source map.
	/// - Collects input types from both the main input and parameters.
	/// - Returns `false` and logs a warning if the node's type information is not found in the typing context.
	fn update_source_map(&mut self, id: NodeId, typing_context: &TypingContext, proto_node: &ProtoNode) -> bool {
		let Some(node_io) = typing_context.type_of(id) else {
			log::warn!("did not find type");
			return false;
		};
		let inputs = [&node_io.call_argument].into_iter().chain(&node_io.inputs).cloned().collect();

		let node_path = &proto_node.original_location.path.as_ref().unwrap_or(const { &vec![] });

		let entry = self.source_map.entry(node_path.to_vec().into()).or_default();

		let update = (
			id,
			NodeTypes {
				inputs,
				output: node_io.return_value.clone(),
			},
		);
		let modified = *entry != update;
		*entry = update;
		modified
	}

	/// Inserts a new node into the [`BorrowTree`], calling the constructor function from `node_registry.rs`.
	///
	/// This method creates a new node container based on the provided `ProtoNode`, updates the source map,
	/// and stores the node container in the `BorrowTree`.
	///
	///
	/// # Notes
	///
	/// - Updates the source map using [`update_source_map`](BorrowTree::update_source_map) before inserting the node.
	/// - Handles different types of construction arguments:
	///   - `Value`: Creates a node from a `TaggedValue`, with special handling for `EditorApi` values.
	///   - `Inline`: Currently unimplemented. Only used for `rust-gpu` support.
	///   - `Nodes`: Constructs a node using other nodes as dependencies.
	/// - Uses the constructor function from the `typing_context` for `Nodes` construction arguments.
	/// - Returns an error if no constructor is found for the given node ID.
	async fn push_node(&mut self, id: NodeId, proto_node: ProtoNode, typing_context: &TypingContext) -> Result<(), GraphErrors> {
		self.update_source_map(id, typing_context, &proto_node);
		let path = proto_node.original_location.path.clone().unwrap_or_default();

		match &proto_node.construction_args {
			ConstructionArgs::Value(value) => {
				let node = if let TaggedValue::EditorApi(api) = &**value {
					let editor_api = UpcastAsRefNode::new(api.clone());
					let node = Box::new(editor_api) as TypeErasedBox<'_>;
					NodeContainer::new(node)
				} else {
					let upcasted = UpcastNode::new(value.to_owned());
					let node = Box::new(upcasted) as TypeErasedBox<'_>;
					NodeContainer::new(node)
				};
				self.store_node(node, id, path.into());
			}
			ConstructionArgs::Inline(_) => unimplemented!("Inline nodes are not supported yet"),
			ConstructionArgs::Nodes(ids) => {
				let ids = ids.to_vec();
				let construction_nodes = self.node_deps(&ids);
				let constructor = typing_context.constructor(id).ok_or_else(|| vec![GraphError::new(&proto_node, GraphErrorType::NoConstructor)])?;
				let node = constructor(construction_nodes).await;
				let node = NodeContainer::new(node);
				self.store_node(node, id, path.into());
			}
		};
		Ok(())
	}

	/// Returns the source map of the borrow tree
	pub fn source_map(&self) -> &HashMap<Path, (NodeId, NodeTypes)> {
		&self.source_map
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use graph_craft::document::value::TaggedValue;

	#[test]
	fn push_node_sync() {
		let mut tree = BorrowTree::default();
		let val_1_protonode = ProtoNode::value(ConstructionArgs::Value(TaggedValue::U32(2u32).into()), vec![]);
		let context = TypingContext::default();
		let future = tree.push_node(NodeId(0), val_1_protonode, &context);
		futures::executor::block_on(future).unwrap();
		let _node = tree.get(NodeId(0)).unwrap();
		let result = futures::executor::block_on(tree.eval(NodeId(0), ()));
		assert_eq!(result, Some(2u32));
	}
}
