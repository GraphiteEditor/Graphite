use crate::node_registry;
use core_types::arena::Arena;
use core_types::context::{ContextImpl, DynSlot, EvalScope, VarArg, VarArgLink, VarArgSlots};
use core_types::gnode::GNode;
use core_types::gpoll::GPoll;
use core_types::registry::{EdgeHandle, ErasedGNode};
use core_types::runtime::{DynGraphRuntime, DynSpawner, GraphRuntime, NoopSpawner};
use graph_craft::Type;
use graph_craft::document::NodeId;
use graph_craft::document::value::TaggedValue;
use graph_craft::graphene_compiler::Executor;
use graph_craft::proto::{ConstructionArgs, GraphError, LocalFuture, ProtoNetwork, ProtoNode, TypingContext};
use graph_craft::proto::{GraphErrorType, GraphErrors};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::{Arc, Mutex, PoisonError};

const ARENA_CAPACITY: usize = 1 << 20;

/// An executor of a node graph that does not require an online compilation server, and instead uses `Box<dyn ...>`.
pub struct DynamicExecutor {
	output: NodeId,
	/// Stores all of the dynamic node structs.
	tree: BorrowTree,
	/// Stores the types of the proto nodes.
	typing_context: TypingContext,
	// This allows us to keep the nodes around for one more frame which is used for introspection
	orphaned_nodes: HashSet<NodeId>,
	arena: Mutex<Arena>,
	runtime: Arc<DynGraphRuntime>,
	live_sources: Vec<core_types::SourceId>,
}

fn noop_runtime() -> Arc<DynGraphRuntime> {
	Arc::new(GraphRuntime::new(Box::new(NoopSpawner) as Box<DynSpawner>))
}


impl Default for DynamicExecutor {
	fn default() -> Self {
		Self {
			output: Default::default(),
			tree: Default::default(),
			typing_context: TypingContext::new(&node_registry::NODE_REGISTRY),
			orphaned_nodes: HashSet::new(),
			arena: Mutex::new(Arena::new(ARENA_CAPACITY)),
			runtime: noop_runtime(),
			live_sources: Vec::new(),
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
	pub async fn new(proto_network: ProtoNetwork) -> Result<Self, GraphErrors> {
		let mut typing_context = TypingContext::new(&node_registry::NODE_REGISTRY);
		typing_context.update(&proto_network)?;
		let output = proto_network.output;
		let sources = proto_network.source_ids();
		let tree = BorrowTree::new(proto_network, &typing_context).await?;
		let runtime = noop_runtime();
		runtime.retain_sources(&sources);

		Ok(Self {
			tree,
			output,
			typing_context,
			orphaned_nodes: HashSet::new(),
			arena: Mutex::new(Arena::new(ARENA_CAPACITY)),
			runtime,
			live_sources: sources,
		})
	}

	pub fn set_runtime(&mut self, runtime: Arc<DynGraphRuntime>) {
		runtime.retain_sources(&self.live_sources);
		self.runtime = runtime;
	}

	pub fn take_dirty(&self) -> bool {
		self.runtime.take_dirty()
	}

	/// Updates the existing [`BorrowTree`] to reflect the new [`ProtoNetwork`], reusing nodes where possible.
	#[cfg_attr(debug_assertions, inline(never))]
	pub async fn update(&mut self, proto_network: ProtoNetwork) -> Result<ResolvedDocumentNodeTypesDelta, (ResolvedDocumentNodeTypesDelta, GraphErrors)> {
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

		let sources = proto_network.source_ids();
		let (add, orphaned) = self
			.tree
			.update(proto_network, &self.typing_context)
			.await
			.map_err(|e| (ResolvedDocumentNodeTypesDelta::default(), e))?;
		self.runtime.retain_sources(&sources);
		self.live_sources = sources;
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

impl<I> Executor<I, GPoll<TaggedValue>> for &DynamicExecutor
where
	I: VarArg + Send + Sync + std::panic::RefUnwindSafe,
{
	fn execute(&self, input: I) -> LocalFuture<'_, Result<GPoll<TaggedValue>, Box<dyn Error>>> {
		Box::pin(async move {
			let Some(handle) = self.tree.get(self.output) else {
				return Err("Output node not found in executor".into());
			};
			let mut arena = self.arena.lock().unwrap_or_else(PoisonError::into_inner);
			let result = eval_root(&mut arena, &self.runtime, &input, |ctx| match TaggedValue::from_edge(handle.duplicate(), ctx) {
				Ok(poll) => poll.map(Ok),
				Err(error) => GPoll::Final(Err(error)),
			});
			match result {
				GPoll::Final(value) => Ok(GPoll::Final(value?)),
				GPoll::Partial(value) => Ok(GPoll::Partial(value?)),
				GPoll::Fallback(boxed) => {
					let (value, error) = *boxed;
					Ok(GPoll::Fallback(Box::new((value?, error))))
				}
				GPoll::Pending => Ok(GPoll::Pending),
				GPoll::Error(error) => Ok(GPoll::Error(error)),
			}
		})
	}
}
pub fn eval_root<S, T>(arena: &mut Arena, runtime: &GraphRuntime<S>, call_argument: DynSlot, eval: impl FnOnce(&ContextImpl) -> GPoll<T>) -> GPoll<T> {
	arena.reset();
	let generations = runtime.snapshot();
	let scope = EvalScope::new(None, None, None, &generations, arena);
	let root = ContextImpl::root(&scope);
	let link = VarArgLink {
		args: VarArgSlots::Single(call_argument),
		outer: None,
	};
	let ctx = root.with_varargs(&link);
	match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| eval(&ctx))) {
		Ok(result) => result,
		Err(_) => {
			arena.reset();
			GPoll::panicked()
		}
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
/// * `nodes`: A [`HashMap`] of [`NodeId`]s to tuples of [`EdgeHandle`] and [`Path`].
///   This stores the actual node instances and their associated paths.
///
/// * `source_map`: A [`HashMap`] from [`Path`] to tuples of [`NodeId`] and [`NodeTypes`].
///   This maps document paths to node IDs and their associated type information.
///
/// A store of the dynamically typed nodes and also the source map.
#[derive(Default)]
pub struct BorrowTree {
	/// A hashmap of node IDs and dynamically typed nodes.
	nodes: HashMap<NodeId, (EdgeHandle, Path)>,
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
		// TODO: Problem: When a passthrough node is connected directly to an export the first input to the passthrough node is not added to the proto network, while the second input is. This means the primary input does not have a type.
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

	fn node_deps(&self, nodes: &[NodeId]) -> Vec<EdgeHandle> {
		nodes.iter().map(|node| self.nodes.get(node).unwrap().0.duplicate()).collect()
	}

	fn store_node(&mut self, node: EdgeHandle, id: NodeId, path: Path) {
		self.nodes.insert(id, (node, path));
	}

	/// Calls the `GNode::serialize` for that specific node, returning for example the captured io record for a monitor node. The node path must match the document node path.
	pub fn introspect(&self, node_path: &[NodeId]) -> Result<Arc<dyn std::any::Any + Send + Sync + 'static>, IntrospectError> {
		let (id, _) = self.source_map.get(node_path).ok_or_else(|| IntrospectError::PathNotFound(node_path.to_vec()))?;
		let (node, _path) = self.nodes.get(id).ok_or(IntrospectError::ProtoNodeNotFound(*id))?;
		node.serialize().ok_or(IntrospectError::NoData)
	}

	pub fn get(&self, id: NodeId) -> Option<EdgeHandle> {
		self.nodes.get(&id).map(|(node, _)| node.duplicate())
	}

	/// Evaluate a node of the [`BorrowTree`], downcasting its edge to the expected output type.
	pub fn eval<I, T: 'static>(&self, id: NodeId, input: &I) -> Option<GPoll<T>>
	where
		ErasedGNode<T>: GNode<I, Output = T>,
	{
		let (node, _path) = self.nodes.get(&id)?;
		let edge = node.duplicate().downcast::<T>().ok()?;
		Some(edge.eval(input))
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
	///     let typing_context = TypingContext::default();
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
				let node = (**value)
					.clone()
					.to_edge()
					.map_err(|error| vec![GraphError::new(&proto_node, GraphErrorType::ConstructionFailed(error))])?;
				self.store_node(node, id, path.into());
			}
			ConstructionArgs::Inline(_) => unimplemented!("Inline nodes are not supported yet"),
			ConstructionArgs::Nodes(ids) => {
				let construction_nodes = self.node_deps(ids);
				let constructor = typing_context.constructor(id).ok_or_else(|| vec![GraphError::new(&proto_node, GraphErrorType::NoConstructor)])?;
				let node = constructor(construction_nodes).map_err(|error| vec![GraphError::new(&proto_node, GraphErrorType::ConstructionFailed(format!("{error:?}")))])?;
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
	use core_types::arena::ArenaCell;
	use core_types::context::{ExtractFootprint, ExtractVarArgs};
	use core_types::runtime::{SourceFuture, Spawner};
	use graph_craft::document::value::TaggedValue;

	struct InertSpawner;

	impl Spawner for InertSpawner {
		fn spawn(&self, _task: SourceFuture) {}
	}

	#[test]
	fn eval_root_builds_the_bare_root_with_the_call_argument_as_vararg_0() {
		let mut arena = Arena::new(64);
		let runtime = GraphRuntime::new(InertSpawner);
		let argument = 21.5f64;
		let result = eval_root(&mut arena, &runtime, &argument, |ctx| {
			assert!(ctx.try_footprint().is_none(), "the bare root carries no axes");
			GPoll::Final(ctx.vararg(0).ok().and_then(|slot| slot.downcast_ref::<f64>()).copied().unwrap_or(0.))
		});
		assert_eq!(result, GPoll::Final(21.5));
	}

	#[test]
	fn eval_root_resets_the_arena_at_eval_start() {
		let mut arena = Arena::new(64);
		let runtime = GraphRuntime::new(InertSpawner);
		let cell = ArenaCell::new();
		eval_root(&mut arena, &runtime, &(), |ctx| {
			let (_, weak) = ctx.scope().arena().alloc(5u32).unwrap();
			cell.store(weak);
			GPoll::Final(())
		});
		assert!(cell.load(&arena).is_some(), "the introspection window spans until the next eval");
		eval_root(&mut arena, &runtime, &(), |ctx| {
			assert!(cell.load(ctx.scope().arena()).is_none(), "the reset at eval start reclaims the previous frame");
			GPoll::Final(())
		});
	}

	#[test]
	fn a_panicking_eval_reports_the_error_and_resets_the_arena() {
		let mut arena = Arena::new(64);
		let runtime = GraphRuntime::new(InertSpawner);
		let cell = ArenaCell::new();
		let result: GPoll<()> = eval_root(&mut arena, &runtime, &(), |ctx| {
			let (_, weak) = ctx.scope().arena().alloc(5u32).unwrap();
			cell.store(weak);
			panic!("mid-eval");
		});
		assert_eq!(result, GPoll::panicked());
		assert!(cell.load(&arena).is_none(), "reset-on-panic leaves no stale records");
		assert_eq!(eval_root(&mut arena, &runtime, &(), |_| GPoll::Final(7u32)), GPoll::Final(7));
	}

	#[test]
	fn push_node_sync() {
		let mut tree = BorrowTree::default();
		let val_1_protonode = ProtoNode::value(ConstructionArgs::Value(TaggedValue::U32(2u32).into()), vec![]);
		let context = TypingContext::default();
		let future = tree.push_node(NodeId(0), val_1_protonode, &context);
		futures::executor::block_on(future).unwrap();
		let _node = tree.get(NodeId(0)).unwrap();

		let arena = Arena::new(64);
		let generations = [];
		let scope = EvalScope::new(None, None, None, &generations, &arena);
		let ctx = ContextImpl::root(&scope);
		let result: Option<GPoll<u32>> = tree.eval(NodeId(0), &ctx);
		assert_eq!(result, Some(GPoll::Final(2)));
	}
}
