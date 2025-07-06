use crate::node_registry::{MONITOR_NODES, NODE_REGISTRY};
use dyn_any::StaticType;
use glam::DAffine2;
use graph_craft::document::value::{TaggedValue, UpcastAsRefNode, UpcastNode};
use graph_craft::proto::{ConstructionArgs, GraphError, LocalFuture, NodeContainer, ProtoNode, SharedNodeContainer, TypeErasedBox, TypingContext, downcast_node};
use graph_craft::proto::{GraphErrorType, GraphErrors};
use graph_craft::{Type, concrete};
use graphene_std::application_io::{ExportFormat, RenderConfig, TimingInformation};
use graphene_std::memo::{IntrospectMode, MonitorNode};
use graphene_std::transform::Footprint;
use graphene_std::uuid::{CompiledProtonodeInput, NodeId, SNI};
use graphene_std::{NodeIOTypes, OwnedContextImpl};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::Arc;

/// An executor of a node graph that does not require an online compilation server, and instead uses `Box<dyn ...>`.
#[derive(Clone)]
pub struct DynamicExecutor {
	output: Option<SNI>,
	/// Stores all of the dynamic node structs.
	tree: BorrowTree,
	/// Stores the types of the proto nodes.
	typing_context: TypingContext,
	// TODO: Add lifetime for removed nodes so that if a SNI changes, then changes back to its previous SNI, the node does
	// not have to be reinserted
	// lifetime: HashSet<(SNI, usize)>,
}

impl Default for DynamicExecutor {
	fn default() -> Self {
		Self {
			output: None,
			tree: Default::default(),
			typing_context: TypingContext::new(&NODE_REGISTRY, &MONITOR_NODES),
		}
	}
}

impl DynamicExecutor {
	pub async fn new(proto_network: Vec<ProtoNode>) -> Result<Self, GraphErrors> {
		let mut typing_context = TypingContext::default();
		typing_context.update(&proto_network)?;
		let output = proto_network.get(0).map(|protonode| protonode.stable_node_id);
		let tree = BorrowTree::new(proto_network, &typing_context).await?;

		Ok(Self { tree, output, typing_context })
	}

	/// Updates the existing [`BorrowTree`] to reflect the new [`ProtoNetwork`], reusing nodes where possible.
	#[cfg_attr(debug_assertions, inline(never))]
	pub async fn update(mut self, proto_network: Vec<ProtoNode>) -> Result<(Vec<(SNI, Vec<Type>)>, Vec<(SNI, usize)>), GraphErrors> {
		self.output = proto_network.get(0).map(|protonode| protonode.stable_node_id);
		self.typing_context.update(&proto_network)?;
		// A protonode id can change while having the same document path, and the path can change while having the same stable node id.
		// Either way, the mapping of paths to ids and ids to paths has to be kept in sync.
		// The mapping of monitor node paths has to kept in sync as well.
		let (add, orphaned_proto_nodes) = self.tree.update(proto_network, &self.typing_context).await?;
		let mut remove = Vec::new();
		for sni in orphaned_proto_nodes {
			let Some(types) = self.typing_context.type_of(sni) else {
				log::error!("Could not get type for protonode {sni} when removing");
				continue;
			};
			remove.push((sni, types.inputs.len()));
			self.tree.free_node(&sni, types.inputs.len());
			self.typing_context.remove_inference(&sni);
		}

		let add_with_types = add
			.into_iter()
			.filter_map(|sni| {
				let Some(types) = self.typing_context.type_of(sni) else {
					log::debug!("Could not get type for added node: {sni}");
					return None;
				};
				Some((sni, types.inputs.clone()))
			})
			.collect();

		Ok((add_with_types, remove))
	}

	/// Intospect the value for that specific protonode input, returning for example the cached value for a monitor node.
	pub fn introspect(&self, protonode_input: CompiledProtonodeInput, introspect_mode: IntrospectMode) -> Result<Box<dyn std::any::Any + Send + Sync>, IntrospectError> {
		let node = self.get_monitor_node_container(protonode_input)?;
		node.introspect(introspect_mode).ok_or(IntrospectError::IntrospectNotImplemented)
	}

	pub fn set_introspect(&self, protonode_input: CompiledProtonodeInput, introspect_mode: IntrospectMode) {
		let Ok(node) = self.get_monitor_node_container(protonode_input) else {
			log::error!("Could not get monitor node for input: {:?}", protonode_input);
			return;
		};
		node.set_introspect(introspect_mode);
	}

	pub fn get_monitor_node_container(&self, protonode_input: CompiledProtonodeInput) -> Result<SharedNodeContainer, IntrospectError> {
		// The SNI of the monitor nodes are the ids of the protonode + input index
		let monitor_node_id = NodeId(protonode_input.0.0 + protonode_input.1 as u64 + 1);
		let inserted_node = self.tree.nodes.get(&monitor_node_id).ok_or(IntrospectError::ProtoNodeNotFound(monitor_node_id))?;
		Ok(inserted_node.clone())
	}

	pub fn input_type(&self) -> Option<Type> {
		self.output.and_then(|output| self.typing_context.type_of(output).map(|node_io| node_io.call_argument.clone()))
	}

	pub fn tree(&self) -> &BorrowTree {
		&self.tree
	}

	pub fn output(&self) -> Option<SNI> {
		self.output
	}

	pub fn output_type(&self) -> Option<Type> {
		self.output.and_then(|output| self.typing_context.type_of(output).map(|node_io| node_io.return_value.clone()))
	}

	pub fn execute<I>(&self, input: I) -> LocalFuture<'_, Result<TaggedValue, Box<dyn Error>>>
	where
		I: dyn_any::StaticType + 'static + Send + Sync + std::panic::UnwindSafe,
	{
		Box::pin(async move {
			use futures::FutureExt;
			let output_node = self.output.ok_or("Could not execute network before compilation")?;

			let result = self.tree.eval_tagged_value(output_node, input);
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

	// If node to evaluate is None then the most downstream node is used
	// pub async fn evaluate_from_node(&self, editor_context: EditorContext, node_to_evaluate: Option<SNI>) -> Result<TaggedValue, String> {
	// 	let node_to_evaluate: NodeId = node_to_evaluate
	// 		.or_else(|| self.output)
	// 		.ok_or("Could not find output node when evaluating network. Has the network been compiled?")?;
	// 	let input_type = self
	// 		.typing_context
	// 		.type_of(node_to_evaluate)
	// 		.map(|node_io| node_io.call_argument.clone())
	// 		.ok_or("Could not get input type of network to execute".to_string())?;
	// 	let result = match input_type {
	// 		t if t == concrete!(EditorContext) => self.execute(editor_context, node_to_evaluate).await.map_err(|e| e.to_string()),
	// 		t if t == concrete!(()) => (&self).execute((), node_to_evaluate).await.map_err(|e| e.to_string()),
	// 		t => Err(format!("Invalid input type {t:?}")),
	// 	};
	// 	let result = match result {
	// 		Ok(value) => value,
	// 		Err(e) => return Err(e),
	// 	};

	// 	Ok(result)
	// }
}

#[derive(Debug, Clone, Default)]
pub struct EditorContext {
	// pub footprint: Option<Footprint>,
	// pub downstream_transform: Option<DAffine2>,
	// pub real_time: Option<f64>,
	// pub animation_time: Option<f64>,
	// pub index: Option<usize>,
	// pub editor_var_args: Option<(Vec<String>, Vec<Arc<Box<[dyn std::any::Any + 'static + std::panic::UnwindSafe]>>>)>,

	// TODO: Temporarily used to execute with RenderConfig as call argument, will be removed once these fields can be passed
	// As a scope input to the reworked render node. This will allow the Editor Context to be used to evaluate any node
	pub render_config: RenderConfig,
}

unsafe impl StaticType for EditorContext {
	type Static = EditorContext;
}

// impl Default for EditorContext {
// 	fn default() -> Self {
// 		EditorContext {
// 			footprint: None,
// 			downstream_transform: None,
// 			real_time: None,
// 			animation_time: None,
// 			index: None,
// 			// editor_var_args: None,
// 		}
// 	}
// }

// impl EditorContext {
// 	pub fn to_context(&self) -> graphene_std::Context {
// 		let mut context = OwnedContextImpl::default();
// 		if let Some(footprint) = self.footprint {
// 			context.set_footprint(footprint);
// 		}
// 		if let Some(footprint) = self.footprint {
// 			context.set_footprint(footprint);
// 		}
// 		if let Some(downstream_transform) = self.downstream_transform {
// 			context.set_downstream_transform(downstream_transform);
// 		}
// 		if let Some(real_time) = self.real_time {
// 			context.set_real_time(real_time);
// 		}
// 		if let Some(animation_time) = self.animation_time {
// 			context.set_animation_time(animation_time);
// 		}
// 		if let Some(index) = self.index {
// 			context.set_index(index);
// 		}
// 		// if let Some(editor_var_args) = self.editor_var_args {
// 		// 	let (variable_names, values)
// 		// 	context.set_varargs((variable_names, values))
// 		// }
// 		context.into_context()
// 	}
// }

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IntrospectError {
	PathNotFound(Vec<NodeId>),
	ProtoNodeNotFound(SNI),
	NoData,
	RuntimeNotReady,
	IntrospectNotImplemented,
}

impl std::fmt::Display for IntrospectError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			IntrospectError::PathNotFound(path) => write!(f, "Path not found: {:?}", path),
			IntrospectError::ProtoNodeNotFound(id) => write!(f, "ProtoNode not found: {:?}", id),
			IntrospectError::NoData => write!(f, "No data found for this node"),
			IntrospectError::RuntimeNotReady => write!(f, "Node runtime is not ready"),
			IntrospectError::IntrospectNotImplemented => write!(f, "Intospect not implemented"),
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
	// A hashmap of node IDs and dynamically typed nodes, as well as the number of inserted monitor nodes
	nodes: HashMap<SNI, SharedNodeContainer>,
}

impl BorrowTree {
	pub async fn new(proto_network: Vec<ProtoNode>, typing_context: &TypingContext) -> Result<BorrowTree, GraphErrors> {
		let mut nodes = BorrowTree::default();
		for node in proto_network {
			nodes.push_node(node, typing_context).await?
		}
		Ok(nodes)
	}

	/// Pushes new nodes into the tree and returns a vec of document nodes that had their types changed, and a vec of all nodes that were removed (including auto inserted value nodes)
	pub async fn update(&mut self, proto_network: Vec<ProtoNode>, typing_context: &TypingContext) -> Result<(Vec<SNI>, HashSet<SNI>), GraphErrors> {
		let mut old_nodes = self.nodes.keys().copied().into_iter().collect::<HashSet<_>>();
		// List of all document node paths that need to be updated, which occurs if their path changes or type changes
		let mut nodes_with_new_type = Vec::new();
		for node in proto_network {
			let sni = node.stable_node_id;
			old_nodes.remove(&sni);
			let sni = node.stable_node_id;
			if !self.nodes.contains_key(&sni) {
				if node.original_location.send_types_to_editor {
					nodes_with_new_type.push(sni)
				}
				self.push_node(node, typing_context);
			}
		}

		Ok((nodes_with_new_type, old_nodes))
	}

	fn node_deps(&self, nodes: &[SNI]) -> Vec<SharedNodeContainer> {
		nodes.iter().map(|node| self.nodes.get(node).unwrap().clone()).collect()
	}

	/// Evaluate the output node of the [`BorrowTree`].
	pub async fn eval<'i, I, O>(&'i self, id: NodeId, input: I) -> Option<O>
	where
		I: StaticType + 'i + Send + Sync,
		O: StaticType + 'i,
	{
		let node = self.nodes.get(&id).cloned()?;
		let output = node.eval(Box::new(input));
		dyn_any::downcast::<O>(output.await).ok().map(|o| *o)
	}
	/// Evaluate the output node of the [`BorrowTree`] and cast it to a tagged value.
	/// This ensures that no borrowed data can escape the node graph.
	pub async fn eval_tagged_value<I>(&self, id: SNI, input: I) -> Result<TaggedValue, String>
	where
		I: StaticType + 'static + Send + Sync,
	{
		let inserted_node = self.nodes.get(&id).cloned().ok_or("Output node not found in executor")?;
		let output = inserted_node.eval(Box::new(input));
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
	pub fn free_node(&mut self, id: &SNI, inputs: usize) {
		self.nodes.remove(&id);
		// Also remove all corresponding monitor nodes
		for monitor_index in 1..=inputs {
			self.nodes.remove(&NodeId(id.0 + monitor_index as u64));
		}
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
	async fn push_node(&mut self, proto_node: ProtoNode, typing_context: &TypingContext) -> Result<(), GraphErrors> {
		let sni = proto_node.stable_node_id;
		// Move the value into the upcast node instead of cloning it
		match proto_node.construction_args {
			ConstructionArgs::Value(value) => {
				// The constructor for nodes with value construction args (value nodes) is not called.
				// It is not necessary to clone the Arc for the wasm editor api, since the value node is deduplicated and only called once.
				// It is cloned whenever it is evaluated
				let upcasted = UpcastNode::new(value);
				let node = Box::new(upcasted) as TypeErasedBox<'_>;
				self.nodes.insert(sni, NodeContainer::new(node));
			}
			ConstructionArgs::Inline(_) => unimplemented!("Inline nodes are not supported yet"),
			ConstructionArgs::Nodes(ref node_construction_args) => {
				let construction_nodes = self.node_deps(&node_construction_args.inputs);

				let types = typing_context.type_of(sni).ok_or_else(|| vec![GraphError::new(&proto_node, GraphErrorType::NoConstructor)])?;
				let monitor_nodes = construction_nodes
					.into_iter()
					.enumerate()
					.map(|(input_index, construction_node)| {
						let input_type = types.inputs.get(input_index).unwrap(); //.ok_or_else(|| vec![GraphError::new(&proto_node, GraphErrorType::NoConstructor)])?;
						let monitor_constructor = typing_context.monitor_constructor(input_type).unwrap(); // .ok_or_else(|| vec![GraphError::new(&proto_node, GraphErrorType::NoConstructor)])?;
						let monitor = monitor_constructor(construction_node);
						let monitor_node_container = NodeContainer::new(monitor);
						self.nodes.insert(NodeId(sni.0 + input_index as u64 + 1), monitor_node_container.clone());
						monitor_node_container
					})
					.collect();

				let constructor = typing_context.constructor(sni).ok_or_else(|| vec![GraphError::new(&proto_node, GraphErrorType::NoConstructor)])?;
				let node = constructor(monitor_nodes).await;
				let node = NodeContainer::new(node);
				self.nodes.insert(sni, node);
			}
		};
		Ok(())
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use graph_craft::document::value::TaggedValue;
	use graphene_std::uuid::NodeId;

	#[test]
	fn push_node_sync() {
		let mut tree = BorrowTree::default();
		let val_1_protonode = ProtoNode::value(ConstructionArgs::Value(TaggedValue::U32(2u32).into()), vec![], NodeId(0));
		let context = TypingContext::default();
		let future = tree.push_node(val_1_protonode, &context);
		futures::executor::block_on(future).unwrap();
		let _node = tree.get(NodeId(0)).unwrap();
		let result = futures::executor::block_on(tree.eval(NodeId(0), ()));
		assert_eq!(result, Some(2u32));
	}
}
