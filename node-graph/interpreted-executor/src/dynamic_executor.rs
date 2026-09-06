use crate::node_registry;
use core_types::arena::Arena;
use core_types::context::{ContextImpl, DynSlot, EvalScope, ExtractAnimationTime, ExtractPointerPosition, ExtractRealTime, VarArg, VarArgLink, VarArgSlots};
use core_types::gpoll::GPoll;
use core_types::registry::EdgeHandle;
use core_types::runtime::{DynGraphRuntime, DynSpawner, GraphRuntime, NoopSpawner};
use graph_craft::Type;
use graph_craft::document::NodeId;
use graph_craft::document::value::TaggedValue;
use graph_craft::graphene_compiler::Executor;
use graph_craft::proto::{ConstructionArgs, GraphError, ProtoNetwork, ProtoNode, TypingContext};
use graph_craft::proto::{GraphErrorType, GraphErrors};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::{Arc, Mutex, PoisonError};

const ARENA_CAPACITY: usize = 1 << 27;

const PERSISTENT_CAPACITY: usize = 1 << 26;

/// The heap the persistent region's parked payloads may own before a flush.
/// Occupancy cannot stand in for it: a park costs one pointer in the region
/// and owns its content outside it, so the two diverge by orders of magnitude.
#[cfg(not(target_family = "wasm"))]
const PERSISTENT_HEAP_BUDGET: usize = 1 << 29;

#[cfg(target_family = "wasm")]
const PERSISTENT_HEAP_BUDGET: usize = 1 << 28;

/// The share of a budget that triggers a flush at the next boundary, chosen so
/// the region is emptied before a promote is refused mid-evaluation.
fn over_budget(used: usize, budget: usize) -> bool {
	used >= budget / 8 * 7
}

fn new_arena(capacity: usize) -> Arena {
	Arena::new(capacity).unwrap_or_else(|| {
		log::error!("arena generations exhausted; continuing without frame caching");
		Arena::parked()
	})
}

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
	/// The region memo levels are promoted into: reset only between
	/// evaluations, and only once a promote has been refused.
	persistent: Mutex<Arena>,
	/// The record frame space, grow-only across evaluations and lent to the
	/// root by `&mut`.
	frames: Mutex<core_types::record::FrameArena>,
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
			arena: Mutex::new(new_arena(ARENA_CAPACITY)),
			persistent: Mutex::new(new_arena(PERSISTENT_CAPACITY)),
			frames: Mutex::new(core_types::record::FrameArena::new()),
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
	pub fn new(mut proto_network: ProtoNetwork) -> Result<Self, GraphErrors> {
		let mut typing_context = TypingContext::new(&node_registry::NODE_REGISTRY);
		typing_context.update(&mut proto_network)?;
		let output = proto_network.output;
		let sources = proto_network.source_ids();
		let tree = BorrowTree::new(proto_network, &typing_context)?;
		let runtime = noop_runtime();
		runtime.retain_sources(&sources);

		Ok(Self {
			tree,
			output,
			typing_context,
			orphaned_nodes: HashSet::new(),
			arena: Mutex::new(new_arena(ARENA_CAPACITY)),
			persistent: Mutex::new(new_arena(PERSISTENT_CAPACITY)),
			frames: Mutex::new(core_types::record::FrameArena::new()),
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
	pub fn update(&mut self, mut proto_network: ProtoNetwork) -> Result<ResolvedDocumentNodeTypesDelta, (ResolvedDocumentNodeTypesDelta, GraphErrors)> {
		self.output = proto_network.output;
		self.typing_context.update(&mut proto_network).map_err(|e| {
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
		let (add, orphaned) = self.tree.update(proto_network, &self.typing_context).map_err(|e| (ResolvedDocumentNodeTypesDelta::default(), e))?;
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

	/// Calls the `Node::serialize` for that specific node. A monitor serializes
	/// its stored context snapshot, and this entry recreates the monitored
	/// value from it as the legacy value the editor's downcasts expect: a
	/// rank-0 input yields its element and a leveled input its legacy list.
	pub fn introspect(&self, node_path: &[NodeId]) -> Result<Arc<dyn std::any::Any + Send + Sync + 'static>, IntrospectError> {
		let result = self.tree.introspect(node_path)?;
		if result.downcast_ref::<core_types::context::CtxSnapshot>().is_some() {
			return self
				.introspect_with(node_path, graphic_types::boundary::batch_to_legacy)
				.map(Arc::from);
		}
		Ok(result)
	}

	/// Re-evaluates the monitored input at `node_path` with its stored context
	/// snapshot and hands the resulting resident batch to `read`, inside the
	/// introspection window. The monitor stores only the context; the value is
	/// recreated against current source data, so a read right after an
	/// execution serves out of the warm memo entries.
	pub fn introspect_with<R>(&self, node_path: &[NodeId], read: impl FnOnce(&core_types::record::Layout, core_types::node::RecordBatch<'_>, &Arena) -> Option<R>) -> Result<R, IntrospectError> {
		let serialized = self.tree.introspect(node_path)?;
		let Some(snapshot) = serialized.downcast_ref::<core_types::context::CtxSnapshot>() else {
			return Err(IntrospectError::NoData);
		};
		let edge = self
			.tree
			.get_by_path(node_path)
			.and_then(EdgeHandle::record_edge)
			.ok_or_else(|| IntrospectError::PathNotFound(node_path.to_vec()))?;
		let arena = self.arena.lock().unwrap_or_else(PoisonError::into_inner);
		let persistent = self.persistent.lock().unwrap_or_else(PoisonError::into_inner);
		let mut buffer = self.frames.lock().unwrap_or_else(PoisonError::into_inner);
		buffer.reserve(self.tree.stack_need());
		let frames = buffer.frames();
		let generations = self.runtime.snapshot();
		let scope = EvalScope::new(snapshot.try_real_time(), snapshot.try_animation_time(), snapshot.try_pointer_position(), &generations, &arena).with_persistent(&persistent);
		let Some(ctx) = snapshot.rehydrate(&scope) else {
			return Err(IntrospectError::NoData);
		};
		let layout = core_types::node::Node::<ContextImpl>::layout(&edge);
		// The batch borrows the frames the read closure is handed, so the read
		// cannot outlive the scope that owns them.
		let frames = frames.scope();
		let result = if layout.depth > 0 {
			match core_types::record::materialize_level(&edge, &ctx, &arena, &frames) {
				core_types::record::LevelStatus::Batch(batch, _) => read(layout, batch, &arena),
				_ => None,
			}
		} else {
			match core_types::record::serve_input(&edge, &ctx, &frames) {
				GPoll::Final(value) | GPoll::Partial(value) => {
					let rec = layout.rec(&value);
					// SAFETY: the serve produced one live record of the input's layout.
					let batch = unsafe { core_types::node::RecordBatch::new(rec.ptr(), 1, layout) };
					read(layout, batch, &arena)
				}
				GPoll::Fallback(boxed) => {
					let (value, _) = *boxed;
					let rec = layout.rec(&value);
					// SAFETY: as for the final arm.
					let batch = unsafe { core_types::node::RecordBatch::new(rec.ptr(), 1, layout) };
					read(layout, batch, &arena)
				}
				_ => None,
			}
		};
		result.ok_or(IntrospectError::NoData)
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
	fn execute(&self, input: I) -> Result<GPoll<TaggedValue>, Box<dyn Error>> {
		let Some(handle) = self.tree.get(self.output) else {
			return Err("Output node not found in executor".into());
		};
		let mut arena = self.arena.lock().unwrap_or_else(PoisonError::into_inner);
		let mut persistent = self.persistent.lock().unwrap_or_else(PoisonError::into_inner);
		// The region is flushed whole between evaluations and the memos
		// re-promote: on a refusal, which leaves no room for the next promote,
		// and ahead of one where either the region itself or the heap its
		// parked payloads own is close to full.
		if persistent.exhausted() || over_budget(persistent.occupancy(), persistent.capacity()) || over_budget(persistent.retained_heap(), PERSISTENT_HEAP_BUDGET) {
			persistent.reset();
		}
		let mut buffer = self.frames.lock().unwrap_or_else(PoisonError::into_inner);
		buffer.reserve(self.tree.stack_need());
		let result = eval_root(&mut arena, &persistent, &mut buffer, &self.runtime, &input, |ctx, frames| {
			match TaggedValue::from_edge(handle.duplicate(), ctx, frames) {
				Ok(poll) => poll.map(Ok),
				Err(error) => GPoll::Final(Err(error)),
			}
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
	}
}
/// One evaluation over the arena and the frame buffer, which the caller sized
/// to the graph's frame need: both are the evaluation's lifetime, so a record
/// served anywhere in the cone lives exactly as long as the arena it may
/// reference. The persistent region is borrowed shared for the same span, so
/// no flush can land while a value promoted into it is readable.
pub fn eval_root<S, T>(
	arena: &mut Arena,
	persistent: &Arena,
	buffer: &mut core_types::record::FrameArena,
	runtime: &GraphRuntime<S>,
	call_argument: DynSlot,
	eval: impl for<'e> FnOnce(&ContextImpl<'e>, &core_types::record::Frames<'e>) -> GPoll<T>,
) -> GPoll<T> {
	arena.reset();
	let generations = runtime.snapshot();
	let scope = EvalScope::new(None, None, None, &generations, arena).with_persistent(persistent);
	let root = ContextImpl::root(&scope);
	let link = VarArgLink {
		args: VarArgSlots::Single(call_argument),
		outer: None,
	};
	let ctx = root.with_varargs(&link);
	let frames = buffer.frames();
	match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| eval(&ctx, &frames))) {
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
	/// The record-stack reserve, folded from the graph at construction.
	stack_need: usize,
}

impl BorrowTree {
	pub fn new(proto_network: ProtoNetwork, typing_context: &TypingContext) -> Result<BorrowTree, GraphErrors> {
		let mut nodes = BorrowTree::default();
		let stack_need = proto_network.stack_need;
		for (id, node) in proto_network.nodes {
			nodes.push_node(id, node, typing_context)?
		}
		nodes.stack_need = stack_need;
		Ok(nodes)
	}

	/// Pushes new nodes into the tree and return orphaned nodes
	pub fn update(&mut self, proto_network: ProtoNetwork, typing_context: &TypingContext) -> Result<(Vec<Path>, HashSet<NodeId>), GraphErrors> {
		let stack_need = proto_network.stack_need;
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
		self.stack_need = stack_need;
		Ok((new_nodes, old_nodes))
	}

	fn node_deps(&self, nodes: &[NodeId]) -> Vec<EdgeHandle> {
		nodes.iter().map(|node| self.nodes.get(node).unwrap().0.duplicate()).collect()
	}

	fn store_node(&mut self, node: EdgeHandle, id: NodeId, path: Path) {
		self.nodes.insert(id, (node, path));
	}

	/// Calls the `Node::serialize` for that specific node, returning for example the captured context snapshot for a monitor node. The node path must match the document node path.
	pub fn introspect(&self, node_path: &[NodeId]) -> Result<Arc<dyn std::any::Any + Send + Sync + 'static>, IntrospectError> {
		let (id, _) = self.source_map.get(node_path).ok_or_else(|| IntrospectError::PathNotFound(node_path.to_vec()))?;
		let (node, _path) = self.nodes.get(id).ok_or(IntrospectError::ProtoNodeNotFound(*id))?;
		node.serialize().ok_or(IntrospectError::NoData)
	}

	pub fn get(&self, id: NodeId) -> Option<EdgeHandle> {
		self.nodes.get(&id).map(|(node, _)| node.duplicate())
	}

	/// The edge handle for the node at a document path.
	pub fn get_by_path(&self, node_path: &[NodeId]) -> Option<EdgeHandle> {
		let (id, _) = self.source_map.get(node_path)?;
		self.get(*id)
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
	///     let typing_context = TypingContext::default();
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
	fn push_node(&mut self, id: NodeId, proto_node: ProtoNode, typing_context: &TypingContext) -> Result<(), GraphErrors> {
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
				let mut node = constructor(construction_nodes).map_err(|error| vec![GraphError::new(&proto_node, GraphErrorType::ConstructionFailed(format!("{error:?}")))])?;
				if let Some(layout) = proto_node.resolved_layout() {
					node.set_layout(layout.clone());
				}
				self.store_node(node, id, path.into());
			}
		};
		Ok(())
	}

	/// Returns the source map of the borrow tree
	pub fn source_map(&self) -> &HashMap<Path, (NodeId, NodeTypes)> {
		&self.source_map
	}

	/// The record-stack reserve of an evaluation, folded from the resolved layouts by the layout pass.
	pub fn stack_need(&self) -> usize {
		self.stack_need
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
		fn spawn(&self, _task: SourceFuture) -> bool {
			false
		}
	}

	#[test]
	fn the_flush_trigger_fires_on_occupancy_and_on_retained_heap() {
		assert!(!over_budget(0, 1 << 10));
		assert!(!over_budget(1 << 10 >> 1, 1 << 10), "half full stays");
		assert!(over_budget(1 << 10 >> 3 << 3, 1 << 10), "a full region flushes");
		assert!(over_budget((1 << 10) / 8 * 7, 1 << 10), "seven eighths is the trigger");

		let arena = Arena::new(1024).unwrap();
		assert!(!over_budget(arena.retained_heap(), PERSISTENT_HEAP_BUDGET), "an empty region is under every budget");
		let owned = vec![0u8; 4096];
		let length = owned.len();
		arena.alloc_sized(owned, length).unwrap();
		assert_eq!(arena.retained_heap(), length);
		assert!(over_budget(arena.retained_heap(), 4096), "the heap hint alone can trigger a flush");
		assert!(
			!over_budget(arena.occupancy(), arena.capacity()),
			"occupancy stays low while the retained heap is large, which is why the trigger reads both"
		);
	}

	#[test]
	fn eval_root_builds_the_bare_root_with_the_call_argument_as_vararg_0() {
		let mut arena = Arena::new(64).unwrap();
		let persistent = Arena::new(64).unwrap();
		let mut buffer = core_types::record::FrameArena::new();
		let runtime = GraphRuntime::new(InertSpawner);
		let argument = 21.5f64;
		let result = eval_root(&mut arena, &persistent, &mut buffer, &runtime, &argument, |ctx, _frames| {
			assert!(ctx.try_footprint().is_none(), "the bare root carries no axes");
			GPoll::Final(ctx.vararg(0).ok().and_then(|slot| slot.downcast_ref::<f64>()).copied().unwrap_or(0.))
		});
		assert_eq!(result, GPoll::Final(21.5));
	}

	#[test]
	fn eval_root_resets_the_arena_at_eval_start() {
		let mut arena = Arena::new(64).unwrap();
		let persistent = Arena::new(64).unwrap();
		let mut buffer = core_types::record::FrameArena::new();
		let runtime = GraphRuntime::new(InertSpawner);
		let cell = ArenaCell::new();
		eval_root(&mut arena, &persistent, &mut buffer, &runtime, &(), |ctx, _frames| {
			let (_, weak) = ctx.scope().arena().alloc(5u32).unwrap();
			cell.store(weak);
			GPoll::Final(())
		});
		assert!(cell.load(&arena).is_some(), "the introspection window spans until the next eval");
		eval_root(&mut arena, &persistent, &mut buffer, &runtime, &(), |ctx, _frames| {
			assert!(cell.load(ctx.scope().arena()).is_none(), "the reset at eval start reclaims the previous frame");
			GPoll::Final(())
		});
	}

	#[test]
	fn a_panicking_eval_reports_the_error_and_resets_the_arena() {
		let mut arena = Arena::new(64).unwrap();
		let persistent = Arena::new(64).unwrap();
		let mut buffer = core_types::record::FrameArena::new();
		let runtime = GraphRuntime::new(InertSpawner);
		let cell = ArenaCell::new();
		let result: GPoll<()> = eval_root(&mut arena, &persistent, &mut buffer, &runtime, &(), |ctx, _frames| {
			let (_, weak) = ctx.scope().arena().alloc(5u32).unwrap();
			cell.store(weak);
			panic!("mid-eval");
		});
		assert_eq!(result, GPoll::panicked());
		assert!(cell.load(&arena).is_none(), "reset-on-panic leaves no stale records");
		assert_eq!(eval_root(&mut arena, &persistent, &mut buffer, &runtime, &(), |_, _| GPoll::Final(7u32)), GPoll::Final(7));
	}

	#[test]
	fn push_node_sync() {
		let mut tree = BorrowTree::default();
		let val_1_protonode = ProtoNode::value(ConstructionArgs::Value(TaggedValue::U32(2u32).into()), vec![]);
		let context = TypingContext::default();
		tree.push_node(NodeId(0), val_1_protonode, &context).unwrap();
		let handle = tree.get(NodeId(0)).unwrap();
		let layout = handle.layout().clone();
		let edge = handle.duplicate().downcast_record::<u32>().unwrap();

		let arena = Arena::new(64).unwrap();
		let generations = [];
		let scope = EvalScope::new(None, None, None, &generations, &arena);
		let ctx = ContextImpl::root(&scope);
		let frames = core_types::record::test_frames(layout.frame_bytes());
		let GPoll::Final(value) = core_types::record::serve_input(&edge, &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(unsafe { core_types::record::read_element::<u32>(layout.rec(&value)) }, 2);
	}

	fn build_executor(mut network: ProtoNetwork) -> DynamicExecutor {
		network.resolve_types(&node_registry::NODE_REGISTRY).unwrap();
		network.compute_layouts();
		DynamicExecutor::new(network).unwrap()
	}

	fn proto_node(identifier: &'static str, args: Vec<NodeId>) -> ProtoNode {
		let mut node = ProtoNode::default();
		node.identifier = graph_craft::ProtoNodeIdentifier::new(identifier);
		node.call_argument = core_types::concrete!(core_types::context::Context);
		node.construction_args = ConstructionArgs::Nodes(args);
		node
	}

	fn string_value(value: &str) -> ProtoNode {
		ProtoNode::value(ConstructionArgs::Value(TaggedValue::String(value.to_string()).into()), vec![])
	}

	#[test]
	fn the_clone_node_clones_the_element_out_of_its_record_wire() {
		let network = ProtoNetwork {
			stack_need: 0,
			inputs: vec![],
			output: NodeId(1),
			nodes: vec![
				(NodeId(0), ProtoNode::value(ConstructionArgs::Value(TaggedValue::F64Array(vec![7.]).into()), vec![])),
				(NodeId(1), proto_node("graphene_core::debug::CloneNode", vec![NodeId(0)])),
			],
		};

		let executor = build_executor(network);
		let arena = Arena::new(1 << 20).unwrap();
		let generations = [];
		let scope = EvalScope::new(None, None, None, &generations, &arena);
		let ctx = ContextImpl::root(&scope);
		let handle = executor.tree().get(NodeId(1)).unwrap();
		let layout = handle.layout().clone();
		let edge = handle.duplicate().downcast_record::<f64>().unwrap();
		let frames = core_types::record::test_frames(executor.tree().stack_need());
		let GPoll::Final(value) = core_types::record::serve_input(&edge, &ctx, &frames) else {
			panic!("the flipped clone must evaluate over record wires, got a non-final poll");
		};
		assert_eq!(unsafe { core_types::record::read_element::<f64>(layout.rec(&value)) }, 7.);
	}

	#[test]
	fn the_palette_folds_its_record_wire_to_a_color_level() {
		let raster_list = TaggedValue::from_type(&core_types::concrete!(graphene_std::list::List<graphene_std::raster_types::Raster<graphene_std::raster_types::CPU>>)).unwrap();
		let network = ProtoNetwork {
			stack_need: 0,
			inputs: vec![],
			output: NodeId(2),
			nodes: vec![
				(NodeId(0), ProtoNode::value(ConstructionArgs::Value(raster_list.into()), vec![])),
				(NodeId(1), ProtoNode::value(ConstructionArgs::Value(TaggedValue::U32(4).into()), vec![])),
				(NodeId(2), proto_node("raster_nodes::image_color_palette::ImageColorPaletteNode", vec![NodeId(0), NodeId(1)])),
			],
		};

		let executor = build_executor(network);
		let arena = Arena::new(1 << 12).unwrap();
		let generations = [];
		let scope = EvalScope::new(None, None, None, &generations, &arena);
		let ctx = ContextImpl::root(&scope);
		let edge = executor.tree().get(NodeId(2)).unwrap().downcast_record::<graphene_std::raster::color::Color>().unwrap();
		let frames = core_types::record::test_frames(executor.tree().stack_need());
		let result = core_types::record::serve_input(&edge, &ctx, &frames);
		// The empty raster level folds to an empty palette: past-end at lane 0.
		assert!(
			matches!(&result, GPoll::Error(error) if error.kind == core_types::gpoll::ErrorKind::PastEnd),
			"the empty palette level must serve the past-end signal at lane 0"
		);
	}

	#[test]
	fn a_value_edge_is_a_record_wire_end_to_end() {
		let network = ProtoNetwork {
			stack_need: 0,
			inputs: vec![],
			output: NodeId(0),
			nodes: vec![(NodeId(0), ProtoNode::value(ConstructionArgs::Value(TaggedValue::F64(7.).into()), vec![]))],
		};

		let executor = build_executor(network);
		let value = executor.tree().get(NodeId(0)).unwrap();
		assert_eq!(value.ty(), &core_types::registry::record_edge_type::<f64>());
		assert_eq!(value.layout().depth, 0);
		assert_eq!((&executor).execute(()).unwrap(), GPoll::Final(TaggedValue::F64(7.)));
	}

	#[test]
	fn a_record_monitor_row_forwards_and_introspects_through_the_executor() {
		let mut monitor = proto_node("graphene_core::memo::MonitorNode", vec![NodeId(0)]);
		monitor.original_location.path = Some(vec![NodeId(9)]);
		let network = ProtoNetwork {
			stack_need: 0,
			inputs: vec![],
			output: NodeId(1),
			nodes: vec![(NodeId(0), ProtoNode::value(ConstructionArgs::Value(TaggedValue::F64(7.).into()), vec![])), (NodeId(1), monitor)],
		};

		let executor = build_executor(network);
		assert_eq!((&executor).execute(()).unwrap(), GPoll::Final(TaggedValue::F64(7.)));
		let element = executor.introspect(&[NodeId(9)]).unwrap();
		let element = element.downcast_ref::<f64>().expect("a record capture materializes to its element");
		assert_eq!(*element, 7.);
	}

	#[test]
	fn a_memoize_row_wires_generically_and_replays_over_record_wires() {
		let network = ProtoNetwork {
			stack_need: 0,
			inputs: vec![],
			output: NodeId(1),
			nodes: vec![
				(NodeId(0), ProtoNode::value(ConstructionArgs::Value(TaggedValue::String(String::from("cached")).into()), vec![])),
				(NodeId(1), proto_node("graphene_core::memo::MemoizeNode", vec![NodeId(0)])),
			],
		};

		let executor = build_executor(network);
		assert_eq!((&executor).execute(()).unwrap(), GPoll::Final(TaggedValue::String(String::from("cached"))));
		assert_eq!(
			(&executor).execute(()).unwrap(),
			GPoll::Final(TaggedValue::String(String::from("cached"))),
			"the second execution replays the deep copy against a reset arena"
		);
	}

	fn modification_value() -> ProtoNode {
		let modification = core_types::ContextModification::from_sources(core_types::context::ContextFeatures::all(), &[]);
		ProtoNode::value(ConstructionArgs::Value(TaggedValue::ContextModification(modification).into()), vec![])
	}

	#[test]
	fn a_context_modification_row_wires_over_a_value_wire() {
		let network = ProtoNetwork {
			stack_need: 0,
			inputs: vec![],
			output: NodeId(2),
			nodes: vec![
				(NodeId(0), ProtoNode::value(ConstructionArgs::Value(TaggedValue::F64(7.).into()), vec![])),
				(NodeId(1), modification_value()),
				(NodeId(2), proto_node("graphene_core::context_modification::ContextModificationNode", vec![NodeId(0), NodeId(1)])),
			],
		};

		let executor = build_executor(network);
		assert_eq!((&executor).execute(()).unwrap(), GPoll::Final(TaggedValue::F64(7.)));
	}

	#[test]
	fn nested_context_modifications_forward_the_layout() {
		let network = ProtoNetwork {
			stack_need: 0,
			inputs: vec![],
			output: NodeId(4),
			nodes: vec![
				(NodeId(0), ProtoNode::value(ConstructionArgs::Value(TaggedValue::F64(7.).into()), vec![])),
				(NodeId(1), modification_value()),
				(NodeId(2), proto_node("graphene_core::context_modification::ContextModificationNode", vec![NodeId(0), NodeId(1)])),
				(NodeId(3), modification_value()),
				(NodeId(4), proto_node("graphene_core::context_modification::ContextModificationNode", vec![NodeId(2), NodeId(3)])),
			],
		};

		let executor = build_executor(network);
		assert_eq!((&executor).execute(()).unwrap(), GPoll::Final(TaggedValue::F64(7.)));
	}

	#[test]
	fn stacked_frame_memos_replay_over_record_wires() {
		let network = ProtoNetwork {
			stack_need: 0,
			inputs: vec![],
			output: NodeId(2),
			nodes: vec![
				(NodeId(0), string_value("memoized")),
				(NodeId(1), proto_node("graphene_core::memo::FrameMemoNode", vec![NodeId(0)])),
				(NodeId(2), proto_node("graphene_core::memo::FrameMemoNode", vec![NodeId(1)])),
			],
		};

		let executor = build_executor(network);
		assert_eq!((&executor).execute(()).unwrap(), GPoll::Final(TaggedValue::String("memoized".to_string())));
		assert_eq!(
			(&executor).execute(()).unwrap(),
			GPoll::Final(TaggedValue::String("memoized".to_string())),
			"the frame memo must replay across evaluations"
		);
	}
}
