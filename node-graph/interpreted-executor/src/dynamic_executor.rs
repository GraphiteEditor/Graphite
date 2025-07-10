use crate::node_registry::{CACHE_NODES, NODE_REGISTRY};
use dyn_any::StaticType;
use graph_craft::document::value::{TaggedValue, UpcastNode};
use graph_craft::proto::{ConstructionArgs, GraphError, LocalFuture, NodeContainer, ProtoNetwork, ProtoNode, SharedNodeContainer, TypeErasedBox, TypingContext, UpstreamInputMetadata};
use graph_craft::proto::{GraphErrorType, GraphErrors};
use graph_craft::{Type, concrete};
use graphene_std::Context;
use graphene_std::any::{EditorContext, EditorContextToContext, NullificationNode};
use graphene_std::memo::IntrospectMode;
use graphene_std::uuid::{CompiledProtonodeInput, NodeId, SNI};
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
			typing_context: TypingContext::new(&NODE_REGISTRY, &CACHE_NODES),
		}
	}
}

impl DynamicExecutor {
	pub async fn new(proto_network: ProtoNetwork) -> Result<Self, GraphErrors> {
		let mut typing_context = TypingContext::default();
		typing_context.update(&proto_network)?;
		let output = Some(proto_network.output);
		let tree = BorrowTree::new(proto_network, &typing_context).await?;
		Ok(Self { tree, output, typing_context })
	}

	/// Updates the existing [`BorrowTree`] to reflect the new [`ProtoNetwork`], reusing nodes where possible.
	#[cfg_attr(debug_assertions, inline(never))]
	pub async fn update(&mut self, proto_network: ProtoNetwork) -> Result<(Vec<(SNI, Vec<Type>)>, Vec<(SNI, usize)>), GraphErrors> {
		self.output = Some(proto_network.output);
		self.typing_context.update(&proto_network)?;
		// A protonode id can change while having the same document path, and the path can change while having the same stable node id.
		// Either way, the mapping of paths to ids and ids to paths has to be kept in sync.
		// The mapping of monitor node paths has to kept in sync as well.
		let (add, orphaned_proto_nodes) = self.tree.update(proto_network, &self.typing_context).await?;
		let mut remove = Vec::new();
		for sni in orphaned_proto_nodes {
			if let Some(number_of_inputs) = self.tree.free_node(&sni) {
				remove.push((sni, number_of_inputs));
			}
			self.typing_context.remove_inference(&sni);
		}

		let add_with_types = add
			.into_iter()
			.filter_map(|sni| {
				let Some(types) = self.typing_context.type_of(sni) else {
					return None;
				};
				Some((sni, types.inputs.clone()))
			})
			.collect();

		Ok((add_with_types, remove))
	}

	/// Intospect the value for that specific protonode input, returning for example the cached value for a monitor node.
	pub fn introspect(&self, protonode_input: CompiledProtonodeInput, introspect_mode: IntrospectMode) -> Result<Option<Arc<dyn std::any::Any + Send + Sync>>, IntrospectError> {
		let node = self.get_introspect_node_container(protonode_input)?;
		Ok(node.introspect(introspect_mode))
	}

	pub fn set_introspect(&self, protonode_input: CompiledProtonodeInput, introspect_mode: IntrospectMode) {
		let Ok(node) = self.get_introspect_node_container(protonode_input) else {
			log::error!("Could not get monitor node for input: {:?}", protonode_input);
			return;
		};
		node.set_introspect(introspect_mode);
	}

	pub fn get_introspect_node_container(&self, protonode_input: CompiledProtonodeInput) -> Result<SharedNodeContainer, IntrospectError> {
		// The SNI of the monitor nodes are the ids of the protonode + input index
		let inserted_node = self.tree.nodes.get(&protonode_input.0).ok_or(IntrospectError::ProtoNodeNotFound(protonode_input))?;
		let node = inserted_node
			.input_introspection_entrypoints
			.get(protonode_input.1)
			.ok_or(IntrospectError::InputIndexOutOfBounds(protonode_input))?;
		Ok(node.clone())
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

	// If node to evaluate is None then the most downstream node is used
	pub async fn evaluate_from_node(&self, editor_context: EditorContext, node_to_evaluate: Option<SNI>) -> Result<TaggedValue, String> {
		let node_to_evaluate: NodeId = node_to_evaluate
			.or_else(|| self.output)
			.ok_or("Could not find output node when evaluating network. Has the network been compiled?")?;
		let input_type = self
			.typing_context
			.type_of(node_to_evaluate)
			.map(|node_io| node_io.call_argument.clone())
			.ok_or("Could not get input type of network to execute".to_string())?;
		// A node to convert the EditorContext to the Context is automatically inserted for each node at id-1
		let result = match input_type {
			t if t == concrete!(Context) => self.execute(editor_context, node_to_evaluate).await.map_err(|e| e.to_string()),
			t if t == concrete!(()) => (&self).execute((), node_to_evaluate).await.map_err(|e| e.to_string()),
			t => Err(format!("Invalid input type {t:?}")),
		};
		let result = match result {
			Ok(value) => value,
			Err(e) => return Err(e),
		};

		Ok(result)
	}

	pub fn execute<I>(&self, input: I, protonode_id: SNI) -> LocalFuture<'_, Result<TaggedValue, Box<dyn Error>>>
	where
		I: dyn_any::StaticType + 'static + Send + Sync + std::panic::UnwindSafe,
	{
		Box::pin(async move {
			use futures::FutureExt;

			let result = self.tree.eval_tagged_value(protonode_id, input);
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IntrospectError {
	PathNotFound(Vec<NodeId>),
	ProtoNodeNotFound(CompiledProtonodeInput),
	InputIndexOutOfBounds(CompiledProtonodeInput),
	InvalidInputType(CompiledProtonodeInput),
	NoData,
	RuntimeNotReady,
	IntrospectNotImplemented,
}

impl std::fmt::Display for IntrospectError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			IntrospectError::PathNotFound(path) => write!(f, "Path not found: {:?}", path),
			IntrospectError::ProtoNodeNotFound(input) => write!(f, "ProtoNode not found: {:?}", input),
			IntrospectError::NoData => write!(f, "No data found for this node"),
			IntrospectError::RuntimeNotReady => write!(f, "Node runtime is not ready"),
			IntrospectError::IntrospectNotImplemented => write!(f, "Intospect not implemented"),
			IntrospectError::InputIndexOutOfBounds(input) => write!(f, "Invalid input index: {:?}", input),
			IntrospectError::InvalidInputType(input) => write!(f, "Invalid input type: {:?}", input),
		}
	}
}

#[derive(Clone)]
struct InsertedProtonode {
	// If the inserted protonode is a value node, then do not clear types when removing
	is_value: bool,
	// Either the value node, cache node, or protonode if output is not clone
	cached_protonode: SharedNodeContainer,
	// Value nodes are the entry points, since they can be directly evaluated
	// Nodes with cloneable outputs have a cache, then editor entry point
	// Nodes without cloneable outputs just have an editor entry point connected to their output
	output_editor_entrypoint: SharedNodeContainer,
	// Nodes with inputs store references to the entry points of the upstream node
	// This is used to generate thumbnails
	input_thumbnail_entrypoints: Vec<SharedNodeContainer>,
	// They also store references to the upstream cache/value node, used for introspection
	input_introspection_entrypoints: Vec<SharedNodeContainer>,
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
	// A hashmap of node IDs to dynamically typed proto nodes, as well as the auto inserted MonitorCache nodes, and editor entry point
	nodes: HashMap<SNI, InsertedProtonode>,
}

impl BorrowTree {
	pub async fn new(proto_network: ProtoNetwork, typing_context: &TypingContext) -> Result<BorrowTree, GraphErrors> {
		let mut nodes = BorrowTree::default();
		for node in proto_network.into_nodes() {
			nodes.push_node(node, typing_context).await?
		}
		Ok(nodes)
	}

	/// Pushes new nodes into the tree and returns a vec of document nodes that had their types changed, and a vec of all nodes that were removed (including auto inserted value nodes)
	pub async fn update(&mut self, proto_network: ProtoNetwork, typing_context: &TypingContext) -> Result<(Vec<SNI>, HashSet<SNI>), GraphErrors> {
		let mut old_nodes = self.nodes.keys().copied().into_iter().collect::<HashSet<_>>();
		// List of all document node paths that need to be updated, which occurs if their path changes or type changes
		let mut nodes_with_new_type = Vec::new();
		for node in proto_network.into_nodes() {
			let sni = node.stable_node_id;
			old_nodes.remove(&sni);
			if !self.nodes.contains_key(&sni) {
				// Do not send types for auto inserted value nodes
				if matches!(node.construction_args, ConstructionArgs::Nodes(_)) {
					nodes_with_new_type.push(sni)
				}
				self.push_node(node, typing_context).await?;
			}
		}

		Ok((nodes_with_new_type, old_nodes))
	}

	fn node_deps(&self, input_metadata: &Vec<Option<UpstreamInputMetadata>>) -> Vec<&InsertedProtonode> {
		input_metadata
			.iter()
			.map(|input_metadata| self.nodes.get(&input_metadata.as_ref().expect("input should be mapped during SNI generation").input_sni).unwrap())
			.collect()
	}

	/// Evaluate any node in the borrow tree
	pub async fn eval<'i, I, O>(&'i self, id: NodeId, input: I) -> Option<O>
	where
		I: StaticType + 'i + Send + Sync,
		O: StaticType + 'i,
	{
		let node = self.nodes.get(&id)?;
		let output = node.output_editor_entrypoint.eval(Box::new(input));
		dyn_any::downcast::<O>(output.await).ok().map(|o| *o)
	}
	/// Evaluate the output node of the [`BorrowTree`] and cast it to a tagged value.
	/// This ensures that no borrowed data can escape the node graph.
	pub async fn eval_tagged_value<I>(&self, id: SNI, input: I) -> Result<TaggedValue, String>
	where
		I: StaticType + 'static + Send + Sync,
	{
		let inserted_node = self.nodes.get(&id).ok_or("Output node not found in executor")?;
		let output = inserted_node.output_editor_entrypoint.eval(Box::new(input));
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
	pub fn free_node(&mut self, id: &SNI) -> Option<usize> {
		let removed_node = self.nodes.remove(&id).expect(&format!("Could not remove node: {:?}", id));
		removed_node.is_value.then_some(removed_node.input_thumbnail_entrypoints.len())
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
	/// Thumbnails is a mapping of the protonode input to the rendered thumbnail through the monitor cache node
	async fn push_node(&mut self, proto_node: ProtoNode, typing_context: &TypingContext) -> Result<(), GraphErrors> {
		let sni = proto_node.stable_node_id;
		// Move the value into the upcast node instead of cloning it
		match proto_node.construction_args {
			ConstructionArgs::Value(value_args) => {
				let upcasted = UpcastNode::new(value_args.value);
				let node = Box::new(upcasted) as TypeErasedBox<'_>;
				let value_node = NodeContainer::new(node);

				let inserted_protonode = InsertedProtonode {
					is_value: true,
					cached_protonode: value_node.clone(),
					output_editor_entrypoint: value_node,
					input_thumbnail_entrypoints: Vec::new(),
					input_introspection_entrypoints: Vec::new(),
				};
				self.nodes.insert(sni, inserted_protonode);
			}
			ConstructionArgs::Inline(_) => unimplemented!("Inline nodes are not supported yet"),
			ConstructionArgs::Nodes(node_construction_args) => {
				let construction_nodes = self.node_deps(&node_construction_args.inputs);

				let input_thumbnail_entrypoints = construction_nodes
					.iter()
					.map(|inserted_protonode| inserted_protonode.output_editor_entrypoint.clone())
					.collect::<Vec<_>>();
				let input_introspection_entrypoints = construction_nodes.iter().map(|inserted_protonode| inserted_protonode.cached_protonode.clone()).collect::<Vec<_>>();

				// Insert nullification if necessary
				let protonode_inputs = construction_nodes
					.iter()
					.zip(node_construction_args.inputs.into_iter())
					.map(|(inserted_protonode, input_metadata)| {
						let previous_input = inserted_protonode.cached_protonode.clone();
						let input_context_dependencies = input_metadata.unwrap().context_dependencies.unwrap();
						let protonode_input = if !input_context_dependencies.is_empty() {
							let nullification_node = NullificationNode::new(previous_input, input_context_dependencies);
							let node = Box::new(nullification_node) as TypeErasedBox<'_>;
							NodeContainer::new(node)
						} else {
							previous_input
						};
						protonode_input
					})
					.collect::<Vec<_>>();

				let constructor = typing_context.constructor(sni).ok_or_else(|| {
					vec![GraphError {
						stable_node_id: sni,
						identifier: node_construction_args.identifier.name.clone(),
						error: GraphErrorType::NoConstructor,
					}]
				})?;
				let node = constructor(protonode_inputs).await;
				let protonode = NodeContainer::new(node);

				let types = typing_context.type_of(sni).ok_or_else(|| {
					vec![GraphError {
						stable_node_id: sni,
						identifier: node_construction_args.identifier.name,
						error: GraphErrorType::NoConstructor,
					}]
				})?;

				// Insert cache nodes on the output if possible
				let cached_protonode = if let Some(cache_constructor) = typing_context.cache_constructor(&types.return_value.nested_type()) {
					let cache = cache_constructor(protonode);
					let cache_node_container = NodeContainer::new(cache);
					cache_node_container
				} else {
					protonode
				};

				// If the call argument is Context, insert a conversion node between EditorContext to Context so that it can be evaluated
				// Also insert the nullification node to whatever the protonode is not dependent on
				let mut editor_entrypoint_input = cached_protonode.clone();
				if types.call_argument == concrete!(Context) {
					let nullify = graphene_std::all_context_dependencies()
						.into_iter()
						.filter(|dependency| !node_construction_args.context_dependencies.contains(dependency))
						.collect::<Vec<_>>();
					if !nullify.is_empty() {
						let nullification_node = NullificationNode::new(cached_protonode.clone(), nullify);
						let node = Box::new(nullification_node) as TypeErasedBox<'_>;
						editor_entrypoint_input = NodeContainer::new(node)
					}
				}

				let editor_entry_point = EditorContextToContext::new(editor_entrypoint_input);
				let node = Box::new(editor_entry_point) as TypeErasedBox;
				let output_editor_entrypoint = NodeContainer::new(node);

				let inserted_protonode = InsertedProtonode {
					is_value: false,
					cached_protonode,
					output_editor_entrypoint,
					input_thumbnail_entrypoints,
					input_introspection_entrypoints,
				};

				self.nodes.insert(sni, inserted_protonode);
			}
		};
		Ok(())
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use graph_craft::{document::value::TaggedValue, proto::NodeValueArgs};
	use graphene_std::uuid::NodeId;

	#[test]
	fn push_node_sync() {
		let mut tree = BorrowTree::default();
		let val_1_protonode = ProtoNode::value(
			ConstructionArgs::Value(NodeValueArgs {
				value: TaggedValue::U32(2u32).into(),
				connector_paths: Vec::new(),
			}),
			NodeId(0),
		);
		let context = TypingContext::default();
		let future = tree.push_node(val_1_protonode, &context);
		futures::executor::block_on(future).unwrap();
		let _node = tree.nodes.get(&NodeId(0)).expect("Node should be added to tree");
		let result = futures::executor::block_on(tree.eval(NodeId(0), ()));
		assert_eq!(result, Some(2u32));
	}
}
