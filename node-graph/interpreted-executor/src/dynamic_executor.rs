use crate::node_registry;
use core_types::{NodeError, Poll, SourceId, WasmNotSend};
use graph_craft::Type;
use graph_craft::document::NodeId;
use graph_craft::document::value::{TaggedValue, UpcastNode};
use graph_craft::graphene_compiler::Executor;
use graph_craft::proto::{Any, AnyInput, ConstructionArgs, GraphError, NodeContainer, ProtoNetwork, ProtoNode, SharedNode, SharedNodeContainer, TypeErasedBox, TypeErasedNode, TypingContext};
use graph_craft::proto::{GraphErrorType, GraphErrors};
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::{Arc, Mutex};

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
	sources: Vec<SourceId>,
}

impl Default for DynamicExecutor {
	fn default() -> Self {
		Self {
			output: Default::default(),
			tree: Default::default(),
			typing_context: TypingContext::new(&node_registry::NODE_REGISTRY),
			orphaned_nodes: HashSet::new(),
			sources: Vec::new(),
		}
	}
}

#[derive(PartialEq, Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct NodeTypes {
	pub inputs: Vec<Type>,
	pub output: Type,
}

type Path = Box<[NodeId]>;

#[derive(PartialEq, Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ResolvedDocumentNodeTypesDelta {
	pub add: Vec<(Path, NodeTypes)>,
	pub remove: Vec<Path>,
}

impl DynamicExecutor {
	pub fn new(proto_network: ProtoNetwork) -> Result<Self, GraphErrors> {
		let mut typing_context = TypingContext::new(&node_registry::NODE_REGISTRY);
		typing_context.update(&proto_network)?;
		let output = proto_network.output;
		let sources = proto_network.source_ids().copied().collect();
		let tree = BorrowTree::new(proto_network, &typing_context)?;

		Ok(Self {
			tree,
			output,
			typing_context,
			orphaned_nodes: HashSet::new(),
			sources,
		})
	}

	pub fn sources(&self) -> &[SourceId] {
		&self.sources
	}

	/// Updates the existing [`BorrowTree`] to reflect the new [`ProtoNetwork`], reusing nodes where possible.
	#[cfg_attr(debug_assertions, inline(never))]
	pub fn update(&mut self, proto_network: ProtoNetwork) -> Result<ResolvedDocumentNodeTypesDelta, (ResolvedDocumentNodeTypesDelta, GraphErrors)> {
		self.output = proto_network.output;
		self.typing_context.update(&proto_network).map_err(|e| {
			// If there is an error then get types that have been resolved before the error
			let add = proto_network
				.nodes
				.iter()
				.filter_map(|(id, node)| node.original_location.path.as_ref().map(|path| (path.clone().into_boxed_slice(), self.typing_context.infer(*id, node))))
				.take_while(|(_, r)| r.is_ok())
				.map(|(path, r)| {
					let r = r.unwrap();
					(
						path,
						NodeTypes {
							inputs: r.inputs,
							output: r.return_value,
						},
					)
				})
				.collect::<Vec<_>>();
			(ResolvedDocumentNodeTypesDelta { add, remove: Vec::new() }, e)
		})?;

		self.sources = proto_network.source_ids().copied().collect();
		let (add, orphaned) = self.tree.update(proto_network, &self.typing_context).map_err(|e| (ResolvedDocumentNodeTypesDelta::default(), e))?;
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

	// TODO(Dennis and Timon): Remove this once rank promotion is fixed.
	// Only really needed due to the runtime not knowing about rank adapters.
	// Once we reworked that we should be able to move this resolution into the runtime.
	// Also remember to remove `BorrowTree::nodes.3` at that point
	pub fn find_origin(&self, mut node: NodeId, error: &NodeError) -> Option<NodeId> {
		let mut trace = error.trace().iter().rev();
		while let Some(&index) = trace.next() {
			// Hack to undo rank promotion while traversing the graph
			if self.typing_context.promotions(node).is_some_and(|promotions| promotions.iter().any(|(argument, _)| *argument == index)) {
				trace.next();
			}

			node = *self.tree.nodes.get(&node).map(|(_, _, inputs)| &**inputs)?.get(index)?;
		}
		Some(node)
	}
}

impl<I> Executor<I, Poll<TaggedValue>> for &DynamicExecutor
where
	I: 'static + WasmNotSend,
{
	fn execute(&self, input: I) -> Result<Poll<TaggedValue>, Box<dyn Error>> {
		self.tree.eval_tagged_value(self.output, &input).map_err(|error| error.into())
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

/// A node in the borrow tree, which keeps the last context it was evaluated with in the graph for evaluating it again outside of it.
struct ObservedNode {
	container: NodeContainer,
	last_context: Mutex<Option<core_types::Context<'static>>>,
}

type SharedObservedNode = Arc<ObservedNode>;

thread_local! {
	/// Whether evaluated nodes keep their context, which they don't while a node is evaluated again outside of the graph.
	static KEEP_CONTEXTS: Cell<bool> = const { Cell::new(true) };
}

impl ObservedNode {
	fn new(node: TypeErasedBox<'static>) -> SharedObservedNode {
		Arc::new(Self {
			container: NodeContainer::new(node),
			last_context: Mutex::default(),
		})
	}

	/// Evaluates the node without it or the nodes upstream of it keeping the context.
	fn eval_unrecorded(&self, input: &AnyInput) -> Poll<Any> {
		// Restores the flag on drop, so it's restored even if the evaluation panics
		struct RestoreKeepContexts(bool);
		impl Drop for RestoreKeepContexts {
			fn drop(&mut self) {
				KEEP_CONTEXTS.set(self.0);
			}
		}

		let _restore = RestoreKeepContexts(KEEP_CONTEXTS.replace(false));
		self.container.eval(input)
	}

	fn last_context(&self) -> Option<core_types::Context<'static>> {
		self.last_context.lock().unwrap().clone()
	}
}

impl SharedNode for ObservedNode {
	#[inline]
	fn eval(&self, input: &AnyInput) -> Poll<Any> {
		if KEEP_CONTEXTS.get()
			&& let Some(context) = input.downcast_ref::<core_types::Context<'static>>()
		{
			*self.last_context.lock().unwrap() = Some(context.clone());
		}
		self.container.eval(input)
	}

	fn node(&self) -> &TypeErasedNode<'static> {
		self.container.node()
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
/// * `nodes`: A [`HashMap`] of [`NodeId`]s to tuples of [`ObservedNode`]s, [`Path`], and input [`NodeId`]s.
///   This stores the actual node instances, their associated paths, and the nodes connected to their inputs.
///
/// * `source_map`: A [`HashMap`] from [`Path`] to tuples of [`NodeId`] and [`NodeTypes`].
///   This maps document paths to node IDs and their associated type information.
///
/// A store of the dynamically typed nodes and also the source map.
#[derive(Default, Clone)]
pub struct BorrowTree {
	/// A hashmap of node IDs and dynamically typed nodes, with their paths and input node IDs.
	nodes: HashMap<NodeId, (SharedObservedNode, Path, Box<[NodeId]>)>,
	/// A hashmap from the document path to the proto node ID.
	source_map: HashMap<Path, (NodeId, NodeTypes)>,
	/// The context features each node's output depends on.
	context_features: HashMap<NodeId, core_types::ContextFeatures>,
}

impl BorrowTree {
	pub fn new(proto_network: ProtoNetwork, typing_context: &TypingContext) -> Result<BorrowTree, GraphErrors> {
		let mut nodes = BorrowTree::default();
		for (id, node) in proto_network.nodes {
			nodes.push_node(id, node, typing_context)?
		}
		Ok(nodes)
	}

	/// Pushes new nodes into the tree and return orphaned nodes
	pub fn update(&mut self, proto_network: ProtoNetwork, typing_context: &TypingContext) -> Result<(Vec<Path>, HashSet<NodeId>), GraphErrors> {
		let mut old_nodes: HashSet<_> = self.nodes.keys().copied().collect();
		let mut new_nodes: Vec<_> = Vec::new();
		// TODO: Problem: When a passthrough node is connected directly to an export the first input to the passthrough node is not added to the proto network, while the second input is. This means the primary input does not have a type.
		for (id, node) in proto_network.nodes {
			if !self.nodes.contains_key(&id) {
				new_nodes.push(node.original_location.path.clone().unwrap_or_default().into());
				self.push_node(id, node, typing_context)?;
			} else if self.update_source_map(id, typing_context, &node) {
				new_nodes.push(node.original_location.path.clone().unwrap_or_default().into());
			}
			old_nodes.remove(&id);
		}
		Ok((new_nodes, old_nodes))
	}

	fn node_deps(&self, nodes: &[NodeId]) -> Vec<SharedNodeContainer> {
		nodes.iter().map(|node| self.nodes.get(node).unwrap().0.clone() as SharedNodeContainer).collect()
	}

	fn store_node(&mut self, node: SharedObservedNode, id: NodeId, path: Path, inputs: Box<[NodeId]>) {
		self.nodes.insert(id, (node, path, inputs));
	}

	/// Calls the `Node::serialize` for that specific node, returning for example the cached value for a monitor node. The node path must match the document node path.
	pub fn introspect(&self, node_path: &[NodeId]) -> Result<Arc<dyn std::any::Any + Send + Sync + 'static>, IntrospectError> {
		let (id, _) = self.source_map.get(node_path).ok_or_else(|| IntrospectError::PathNotFound(node_path.to_vec()))?;
		let (node, _path, _) = self.nodes.get(id).ok_or(IntrospectError::ProtoNodeNotFound(*id))?;
		node.node().serialize().ok_or(IntrospectError::NoData)
	}

	pub fn get(&self, id: NodeId) -> Option<SharedNodeContainer> {
		self.nodes.get(&id).map(|(node, _, _)| node.clone() as SharedNodeContainer)
	}

	/// The context features this node's output depends on.
	pub fn context_features(&self, id: NodeId) -> core_types::ContextFeatures {
		self.context_features.get(&id).copied().unwrap_or_default()
	}

	pub fn input_node(&self, node_path: &[NodeId], input_index: usize) -> Result<NodeId, IntrospectError> {
		let (id, _) = self.source_map.get(node_path).ok_or_else(|| IntrospectError::PathNotFound(node_path.to_vec()))?;
		let (_, _, inputs) = self.nodes.get(id).ok_or(IntrospectError::ProtoNodeNotFound(*id))?;
		inputs.get(input_index).copied().ok_or(IntrospectError::NoData)
	}

	/// The context this node was last evaluated with in the graph.
	pub fn last_context(&self, id: NodeId) -> Result<Option<core_types::Context<'static>>, IntrospectError> {
		Ok(self.container(id)?.last_context())
	}

	/// Evaluates this node with the given context outside of a graph evaluation, without it or the nodes upstream of it keeping the context.
	pub fn evaluate(&self, id: NodeId, context: &core_types::Context<'static>) -> Result<Poll<core_types::registry::Any>, IntrospectError> {
		Ok(self.container(id)?.eval_unrecorded(context))
	}

	fn container(&self, id: NodeId) -> Result<&ObservedNode, IntrospectError> {
		self.nodes.get(&id).map(|(node, _, _)| &**node).ok_or(IntrospectError::ProtoNodeNotFound(id))
	}

	/// Evaluate the output node of the [`BorrowTree`].
	pub fn eval<I, O>(&self, id: NodeId, input: &I) -> Option<O>
	where
		I: 'static + WasmNotSend,
		O: 'static,
	{
		let (node, _path, _) = self.nodes.get(&id)?;
		node.eval(input).value().and_then(|output| output.downcast::<O>().ok().map(|o| *o))
	}

	/// Evaluate the output node of the [`BorrowTree`] and cast it to a tagged value.
	/// This ensures that no borrowed data can escape the node graph.
	pub fn eval_tagged_value<I>(&self, id: NodeId, input: &I) -> Result<Poll<TaggedValue>, String>
	where
		I: 'static + WasmNotSend,
	{
		let (node, _path, _) = self.nodes.get(&id).ok_or("Output node not found in executor")?;
		Ok(match node.eval(input) {
			Poll::Pending => Poll::Pending,
			Poll::Final(output) => Poll::Final(TaggedValue::try_from_any(output)?),
			Poll::Partial(output) => Poll::Partial(TaggedValue::try_from_any(output)?),
			Poll::Fallback(boxed) => {
				let (output, error) = *boxed;
				Poll::Fallback(Box::new((TaggedValue::try_from_any(output)?, error)))
			}
			Poll::Error(error) => Poll::Error(error),
		})
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
	/// fn example() -> Result<(), GraphErrors> {
	///     let (proto_network, node_id, proto_node) = ProtoNetwork::example();
	///     let typing_context = TypingContext::new(&node_registry::NODE_REGISTRY);
	///     let mut borrow_tree = BorrowTree::new(proto_network, &typing_context)?;
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
		self.context_features.remove(&id);
		let (_, path, _) = self.nodes.remove(&id)?;
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
	fn push_node(&mut self, id: NodeId, proto_node: ProtoNode, typing_context: &TypingContext) -> Result<(), GraphErrors> {
		self.update_source_map(id, typing_context, &proto_node);
		self.context_features.insert(id, proto_node.upstream_context_features);
		let path = proto_node.original_location.path.clone().unwrap_or_default();

		match &proto_node.construction_args {
			ConstructionArgs::Value(value) => {
				let upcasted = UpcastNode::new(value.to_owned());
				let node = ObservedNode::new(Box::new(upcasted) as TypeErasedBox<'_>);
				self.store_node(node, id, path.into(), Box::default());
			}
			ConstructionArgs::Inline(_) => unimplemented!("Inline nodes are not supported yet"),
			ConstructionArgs::Nodes(ids) => {
				let ids = ids.to_vec();
				let mut construction_nodes = self.node_deps(&ids);

				// Wrap arguments the typing pass marked for rank promotion with their adapter node
				if let Some(promotions) = typing_context.promotions(id) {
					for (argument_index, promotion) in promotions {
						let adapter_constructor = typing_context
							.adapter_constructor(&promotion.adapter_identifier())
							.ok_or_else(|| vec![GraphError::new(&proto_node, GraphErrorType::NoConstructor)])?;
						let adapter = adapter_constructor(vec![construction_nodes[*argument_index].clone()]);
						construction_nodes[*argument_index] = Arc::new(NodeContainer::new(adapter));
					}
				}

				let constructor = typing_context.constructor(id).ok_or_else(|| vec![GraphError::new(&proto_node, GraphErrorType::NoConstructor)])?;
				let node = constructor(construction_nodes);
				self.store_node(ObservedNode::new(node), id, path.into(), ids.into());
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
mod test;
