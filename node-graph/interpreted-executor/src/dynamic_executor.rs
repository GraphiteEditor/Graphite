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

		let (add, orphaned) = self
			.tree
			.update(proto_network, &self.typing_context)
			.await
			.map_err(|e| (ResolvedDocumentNodeTypesDelta::default(), e))?;
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
	I: StaticType + 'static + Send + Sync,
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
				let mut construction_nodes = self.node_deps(&ids);

				// Wrap arguments the typing pass marked for rank promotion with their adapter node
				if let Some(promotions) = typing_context.promotions(id) {
					for (argument_index, promotion) in promotions {
						let adapter_constructor = typing_context
							.adapter_constructor(&promotion.adapter_identifier())
							.ok_or_else(|| vec![GraphError::new(&proto_node, GraphErrorType::NoConstructor)])?;
						let adapter = adapter_constructor(vec![construction_nodes[*argument_index].clone()]).await;
						construction_nodes[*argument_index] = NodeContainer::new(adapter);
					}
				}

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

// TODO(Keavon): Move these many new tests into a separate file
#[cfg(test)]
mod test {
	use super::*;
	use core_types::Context;
	use core_types::descriptor;
	use core_types::list::{Item, List};
	use graph_craft::ProtoNodeIdentifier;
	use graph_craft::document::value::TaggedValue;
	use graphene_std::vector::Vector;

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

	/// Builds a two-node network feeding the given value into Bounding Box, whose primary input registers both `Item<Vector>` and `List<Vector>` wire variants.
	fn bounding_box_network(content: TaggedValue) -> ProtoNetwork {
		let value_node = ProtoNode::value(ConstructionArgs::Value(content.into()), vec![NodeId(0)]);

		let mut bounding_box_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(0)]), vec![NodeId(1)]);
		bounding_box_node.identifier = ProtoNodeIdentifier::new("core_types::vector::BoundingBoxNode");

		ProtoNetwork {
			inputs: vec![],
			output: NodeId(1),
			nodes: vec![(NodeId(0), value_node), (NodeId(1), bounding_box_node)],
		}
	}

	fn compile_bounding_box_network(content: TaggedValue) -> BorrowTree {
		let network = bounding_box_network(content);
		let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
		typing_context.update(&network).expect("The network should resolve against exactly one registered wire variant");
		futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("The resolved variant's constructor should instantiate")
	}

	#[test]
	fn item_wire_variant_resolves_and_executes() {
		let tree = compile_bounding_box_network(TaggedValue::TypeDefault(descriptor!(Item<Vector>)));

		let context: Context = None;
		let result: Option<Item<Vector>> = futures::executor::block_on(tree.eval(NodeId(1), context.clone()));
		assert!(result.is_some(), "The Item wire variant should downcast and execute end-to-end");

		let wrong_type: Option<List<Vector>> = futures::executor::block_on(tree.eval(NodeId(1), context));
		assert!(wrong_type.is_none(), "An Item wire should not downcast as a List");
	}

	#[test]
	fn item_wire_promotes_to_list_connector() {
		let value_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::TypeDefault(descriptor!(Item<f64>)).into()), vec![NodeId(0)]);

		let mut sum_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(0)]), vec![NodeId(1)]);
		sum_node.identifier = ProtoNodeIdentifier::new("math_nodes::SumNode");

		let network = ProtoNetwork {
			inputs: vec![],
			output: NodeId(1),
			nodes: vec![(NodeId(0), value_node), (NodeId(1), sum_node)],
		};
		let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
		typing_context.update(&network).expect("An Item wire should resolve a List connector via promotion");
		assert!(typing_context.promotions(NodeId(1)).is_some(), "The typing pass should record the promotion");
		let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("The promotion adapter should instantiate");

		let context: Context = None;
		let result: Option<Item<f64>> = futures::executor::block_on(tree.eval(NodeId(1), context));
		assert!(result.is_some(), "The promoted wire should execute end-to-end");
	}

	// The layer content path: a rank-0 content wire enters Wrap Graphic's `List` connector by singleton raise, and the
	// wrapped `Item<Graphic>` raises again at Extend's `List` connector, so layers accept rank-0 chains without new machinery
	#[test]
	fn rank_0_content_promotes_through_the_layer_coercion_path() {
		let content_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::TypeDefault(descriptor!(Item<Vector>)).into()), vec![NodeId(0)]);

		let mut wrap_graphic_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(0)]), vec![NodeId(1)]);
		wrap_graphic_node.identifier = ProtoNodeIdentifier::new("graphic_nodes::graphic::WrapGraphicNode");

		let base_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::TypeDefault(descriptor!(List<graphene_std::Graphic>)).into()), vec![NodeId(2)]);

		let mut extend_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(2), NodeId(1)]), vec![NodeId(3)]);
		extend_node.identifier = ProtoNodeIdentifier::new("graphic_nodes::graphic::ExtendNode");

		let network = ProtoNetwork {
			inputs: vec![],
			output: NodeId(3),
			nodes: vec![(NodeId(0), content_node), (NodeId(1), wrap_graphic_node), (NodeId(2), base_node), (NodeId(3), extend_node)],
		};
		let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
		typing_context.update(&network).expect("A rank-0 content wire should resolve the layer coercion path via promotion");
		assert!(typing_context.promotions(NodeId(1)).is_some(), "The rank-0 content should be raised at Wrap Graphic's List connector");
		assert!(typing_context.promotions(NodeId(3)).is_some(), "The wrapped Item<Graphic> should be raised at Extend's List connector");
		let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("The promotion adapters should instantiate");

		let context: Context = None;
		let result: Option<List<graphene_std::Graphic>> = futures::executor::block_on(tree.eval(NodeId(3), context));
		let stack = result.expect("The layer coercion path should execute end-to-end");
		assert_eq!(stack.len(), 1, "The rank-0 content should contribute exactly one graphic to the stack");
	}

	/// Builds a network feeding the given content plus a promoted bare distance into Offset Points, whose distance connector is ranked `Item<f64>`.
	fn offset_points_network(content: TaggedValue) -> ProtoNetwork {
		let content_node = ProtoNode::value(ConstructionArgs::Value(content.into()), vec![NodeId(0)]);
		let distance_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::F64(10.).into()), vec![NodeId(1)]);

		let mut field_adapter_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(1)]), vec![NodeId(2)]);
		field_adapter_node.identifier = ProtoNodeIdentifier::new("graphene_core::ops::FieldAdapterNode<f64>");

		let mut offset_points_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(0), NodeId(2)]), vec![NodeId(3)]);
		offset_points_node.identifier = ProtoNodeIdentifier::new("core_types::vector::OffsetPointsNode");

		ProtoNetwork {
			inputs: vec![],
			output: NodeId(3),
			nodes: vec![(NodeId(0), content_node), (NodeId(1), distance_node), (NodeId(2), field_adapter_node), (NodeId(3), offset_points_node)],
		}
	}

	#[test]
	fn mixed_rank_connectors_resolve_via_promotion() {
		let network = offset_points_network(TaggedValue::TypeDefault(descriptor!(List<Vector>)));
		let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
		typing_context
			.update(&network)
			.expect("A List primary with an Item parameter should resolve the mapped variant via promotion");
		assert!(typing_context.promotions(NodeId(3)).is_some(), "The Item distance should be marked for promotion");
		let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("Construction should wrap the promoted argument");

		let context: Context = None;
		let result: Option<List<Vector>> = futures::executor::block_on(tree.eval(NodeId(3), context));
		assert!(result.is_some(), "The zipped mapped variant should execute end-to-end");
	}

	#[test]
	fn all_item_connectors_resolve_without_promotion() {
		let network = offset_points_network(TaggedValue::TypeDefault(descriptor!(Item<Vector>)));
		let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
		typing_context.update(&network).expect("All-Item connectors should resolve the rank-0 variant exactly");
		assert!(typing_context.promotions(NodeId(3)).is_none(), "No promotion should be needed at rank 0");
		let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("The rank-0 variant should instantiate");

		let context: Context = None;
		let result: Option<Item<Vector>> = futures::executor::block_on(tree.eval(NodeId(3), context));
		assert!(result.is_some(), "The rank-0 variant should execute and stay rank 0");
	}

	/// Builds a Transform network: content (node 0) plus four parameter values, each promoted onto Item wires as the preprocessor would.
	fn transform_network(content: TaggedValue, rotation: TaggedValue) -> ProtoNetwork {
		let mut nodes = vec![(NodeId(0), ProtoNode::value(ConstructionArgs::Value(content.into()), vec![NodeId(0)]))];

		let parameters = [
			(TaggedValue::DVec2(glam::DVec2::new(5., 0.)), "DVec2"),
			(rotation, "f64"),
			(TaggedValue::DVec2(glam::DVec2::ONE), "DVec2"),
			(TaggedValue::DVec2(glam::DVec2::ZERO), "DVec2"),
		];
		let mut transform_inputs = vec![NodeId(0)];
		let mut next_id = 1;
		for (value, element) in parameters {
			let value_id = NodeId(next_id);
			let field_adapter_id = NodeId(next_id + 1);
			next_id += 2;

			nodes.push((value_id, ProtoNode::value(ConstructionArgs::Value(value.into()), vec![value_id])));
			let mut field_adapter_node = ProtoNode::value(ConstructionArgs::Nodes(vec![value_id]), vec![field_adapter_id]);
			field_adapter_node.identifier = ProtoNodeIdentifier::with_owned_string(format!("graphene_core::ops::FieldAdapterNode<{element}>"));
			nodes.push((field_adapter_id, field_adapter_node));
			transform_inputs.push(field_adapter_id);
		}

		let output = NodeId(next_id);
		let mut transform_node = ProtoNode::value(ConstructionArgs::Nodes(transform_inputs), vec![output]);
		transform_node.identifier = graphene_std::transform_nodes::transform::IDENTIFIER;
		nodes.push((output, transform_node));

		ProtoNetwork { inputs: vec![], output, nodes }
	}

	#[test]
	fn transform_composes_onto_item_wire() {
		use glam::{DAffine2, DVec2};

		let network = transform_network(TaggedValue::TypeDefault(descriptor!(Item<Vector>)), TaggedValue::F64(0.));
		let output = network.output;
		let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
		typing_context.update(&network).expect("Transform should resolve its rank-0 variant");
		assert!(typing_context.promotions(output).is_none(), "All-Item connectors should need no promotion");
		let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("Transform's rank-0 variant should instantiate");

		let context: Context = None;
		let result: Option<Item<Vector>> = futures::executor::block_on(tree.eval(output, context));
		let item = result.expect("A rank-0 chain through Transform should stay rank 0");
		let transform = item.attribute_cloned_or_default::<DAffine2>(core_types::ATTR_TRANSFORM);
		assert_eq!(transform.translation, DVec2::new(5., 0.), "The translation should compose onto the item's transform attribute");
	}

	#[test]
	fn transform_broadcasts_item_content_across_a_framed_parameter() {
		use glam::DAffine2;

		let network = transform_network(TaggedValue::TypeDefault(descriptor!(Item<Vector>)), TaggedValue::F64Array(vec![0., 90.]));
		let output = network.output;
		let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
		typing_context
			.update(&network)
			.expect("A framed rotation should resolve the mapped variant via promotion of the other connectors");
		assert!(typing_context.promotions(output).is_some(), "The Item-typed connectors should be raised into the frame");
		let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("The mapped variant should instantiate");

		let context: Context = None;
		let result: Option<List<Vector>> = futures::executor::block_on(tree.eval(output, context));
		let list = result.expect("The broadcast should produce a List");
		assert_eq!(list.len(), 2, "One output item per frame slot");

		let first: DAffine2 = list.attribute_cloned_or_default(core_types::ATTR_TRANSFORM, 0);
		let second: DAffine2 = list.attribute_cloned_or_default(core_types::ATTR_TRANSFORM, 1);
		assert!((first.matrix2.col(0).y - 0.).abs() < 1e-10, "Slot 0 should be unrotated");
		assert!((second.matrix2.col(0).y - 1.).abs() < 1e-10, "Slot 1 should be rotated 90 degrees");
	}

	#[test]
	fn generator_frames_over_a_list_parameter() {
		// A `()` generator (Circle) fed a `List<f64>` radius should frame into one circle per slot
		let primary = ProtoNode::value(ConstructionArgs::Value(TaggedValue::None.into()), vec![NodeId(0)]);

		let radii = ProtoNode::value(ConstructionArgs::Value(TaggedValue::F64Array(vec![10., 20., 30.]).into()), vec![NodeId(1)]);
		let mut radius_adapter = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(1)]), vec![NodeId(2)]);
		radius_adapter.identifier = ProtoNodeIdentifier::new("graphene_core::ops::FieldAdapterNode<f64>");

		let mut circle_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(0), NodeId(2)]), vec![NodeId(3)]);
		circle_node.identifier = graphene_std::vector_nodes::circle::IDENTIFIER;

		let network = ProtoNetwork {
			inputs: vec![],
			output: NodeId(3),
			nodes: vec![(NodeId(0), primary), (NodeId(1), radii), (NodeId(2), radius_adapter), (NodeId(3), circle_node)],
		};

		let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
		typing_context.update(&network).expect("A List<f64> radius should resolve Circle's mapped generator variant");
		let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("The mapped generator variant should instantiate");

		let context: Context = None;
		let result: Option<List<Vector>> = futures::executor::block_on(tree.eval(NodeId(3), context));
		let list = result.expect("The generator frame should produce a List<Vector>");
		assert_eq!(list.len(), 3, "One circle per radius slot");
	}

	/// Builds the compiler's cache chain (child, then Memoize, then Context Modification) around a value, as `insert_context_nullification_node` does.
	fn nullification_chain_network(value: TaggedValue) -> ProtoNetwork {
		let value_node = ProtoNode::value(ConstructionArgs::Value(value.into()), vec![NodeId(0)]);

		let mut memoize_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(0)]), vec![NodeId(1)]);
		memoize_node.identifier = graphene_core::memo::memoize::IDENTIFIER;

		let features_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::ContextFeatures(Default::default()).into()), vec![NodeId(2)]);

		let mut nullification_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(1), NodeId(2)]), vec![NodeId(3)]);
		nullification_node.identifier = graphene_core::context_modification::context_modification::IDENTIFIER;

		ProtoNetwork {
			inputs: vec![],
			output: NodeId(3),
			nodes: vec![(NodeId(0), value_node), (NodeId(1), memoize_node), (NodeId(2), features_node), (NodeId(3), nullification_node)],
		}
	}

	#[test]
	fn the_nullification_chain_resolves_for_ranked_enum_wires() {
		use graphene_std::vector::style::StrokeAlign;

		// The bare form, as a constant enum wire presents to the inserted cache chain
		let network = nullification_chain_network(TaggedValue::StrokeAlign(StrokeAlign::default()));
		let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
		typing_context.update(&network).expect("A bare StrokeAlign wire should resolve through the compiler's cache chain");

		// The Item form, as a wrapped field adapter's output presents to the chain
		let network = nullification_chain_network(TaggedValue::TypeDefault(descriptor!(Item<StrokeAlign>)));
		let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
		typing_context.update(&network).expect("An Item<StrokeAlign> wire should resolve through the compiler's cache chain");
	}

	#[test]
	fn bare_wires_promote_to_item_connectors_at_resolution() {
		use glam::{DAffine2, DVec2};

		let values = [
			TaggedValue::DAffine2(DAffine2::IDENTITY),
			TaggedValue::DVec2(DVec2::new(7., 0.)),
			TaggedValue::F64(0.),
			TaggedValue::DVec2(DVec2::ONE),
			TaggedValue::DVec2(DVec2::ZERO),
		];
		let mut nodes: Vec<_> = values
			.into_iter()
			.enumerate()
			.map(|(index, value)| (NodeId(index as u64), ProtoNode::value(ConstructionArgs::Value(value.into()), vec![NodeId(index as u64)])))
			.collect();
		let mut transform_node = ProtoNode::value(ConstructionArgs::Nodes((0..5).map(NodeId).collect()), vec![NodeId(5)]);
		transform_node.identifier = graphene_std::transform_nodes::transform::IDENTIFIER;
		nodes.push((NodeId(5), transform_node));

		let network = ProtoNetwork {
			inputs: vec![],
			output: NodeId(5),
			nodes,
		};
		let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
		typing_context.update(&network).expect("Bare wires should resolve Item connectors via wrap promotion");
		assert_eq!(typing_context.promotions(NodeId(5)).map(Vec::len), Some(5), "All five bare inputs should be wrapped");
		let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("The wrap adapters should instantiate");

		let context: Context = None;
		let result: Option<Item<DAffine2>> = futures::executor::block_on(tree.eval(NodeId(5), context));
		let item = result.expect("A bare matrix should flow through Transform as an Item");
		let transform = item.attribute_cloned_or_default::<DAffine2>(core_types::ATTR_TRANSFORM);
		assert_eq!(transform.translation, DVec2::new(7., 0.), "The translation should compose onto the gained transform attribute");
	}

	#[test]
	fn bare_value_promotes_to_item_wire() {
		let value_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::F64(3.).into()), vec![NodeId(0)]);

		let mut field_adapter_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(0)]), vec![NodeId(1)]);
		field_adapter_node.identifier = ProtoNodeIdentifier::new("graphene_core::ops::FieldAdapterNode<f64>");

		let network = ProtoNetwork {
			inputs: vec![],
			output: NodeId(1),
			nodes: vec![(NodeId(0), value_node), (NodeId(1), field_adapter_node)],
		};
		let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
		typing_context.update(&network).expect("A bare f64 should resolve the promotion variant");
		let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("The promotion constructor should instantiate");

		let context: Context = None;
		let result: Option<Item<f64>> = futures::executor::block_on(tree.eval(NodeId(1), context));
		assert_eq!(result.map(|item| *item.element()), Some(3.), "The bare value should arrive wrapped as an Item");
	}

	// Path Modify's ranked modification parameter: a bare `Box<VectorModification>` wraps onto the `Item` wire through its field adapter,
	// exercising the nested-generic identifier round-trip between the registered `stringify!` name and the preprocessor's simplified name
	#[test]
	fn bare_modification_promotes_to_item_wire() {
		use graphene_std::vector::VectorModification;

		let modification = TaggedValue::VectorModification(Box::new(VectorModification::default()));
		let value_node = ProtoNode::value(ConstructionArgs::Value(modification.into()), vec![NodeId(0)]);

		let mut field_adapter_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(0)]), vec![NodeId(1)]);
		field_adapter_node.identifier = ProtoNodeIdentifier::new("graphene_core::ops::FieldAdapterNode<Box<VectorModification>>");

		let network = ProtoNetwork {
			inputs: vec![],
			output: NodeId(1),
			nodes: vec![(NodeId(0), value_node), (NodeId(1), field_adapter_node)],
		};
		let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
		typing_context.update(&network).expect("A bare Box<VectorModification> should resolve the promotion variant");
		let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("The promotion constructor should instantiate");

		let context: Context = None;
		let result: Option<Item<Box<VectorModification>>> = futures::executor::block_on(tree.eval(NodeId(1), context));
		assert!(result.is_some(), "The bare modification should arrive wrapped as an Item");
	}

	// The Write Attribute value slot: an Item wire's element boxes into a type-erased attribute value, and a stored bare value reaches the same row via a wrap promotion
	#[test]
	fn item_wire_boxes_into_the_attribute_value_connector() {
		use graphene_std::list::AttributeValueDyn;

		let value_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::F64(3.).into()), vec![NodeId(0)]);
		let mut wrap_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(0)]), vec![NodeId(1)]);
		wrap_node.identifier = ProtoNodeIdentifier::new("graphene_core::ops::WrapItemNode<f64>");
		let mut attribute_adapter_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(1)]), vec![NodeId(2)]);
		attribute_adapter_node.identifier = ProtoNodeIdentifier::new("graphene_core::ops::FieldAdapterNode<AttributeValueDyn>");

		let network = ProtoNetwork {
			inputs: vec![],
			output: NodeId(2),
			nodes: vec![(NodeId(0), value_node), (NodeId(1), wrap_node), (NodeId(2), attribute_adapter_node)],
		};
		let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
		typing_context.update(&network).expect("An Item<f64> wire should resolve the attribute value boxing row");
		let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("The boxing constructor should instantiate");

		let context: Context = None;
		let result: Option<Item<AttributeValueDyn>> = futures::executor::block_on(tree.eval(NodeId(2), context));
		let boxed = result.expect("The boxed attribute value should arrive as an Item");
		assert_eq!(
			boxed.element().0.as_any().downcast_ref::<f64>(),
			Some(&3.),
			"The stored value should be the bare element, not the whole Item"
		);

		let value_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::F64(5.).into()), vec![NodeId(0)]);
		let mut attribute_adapter_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(0)]), vec![NodeId(1)]);
		attribute_adapter_node.identifier = ProtoNodeIdentifier::new("graphene_core::ops::FieldAdapterNode<AttributeValueDyn>");

		let network = ProtoNetwork {
			inputs: vec![],
			output: NodeId(1),
			nodes: vec![(NodeId(0), value_node), (NodeId(1), attribute_adapter_node)],
		};
		let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
		typing_context.update(&network).expect("A bare value should wrap-promote into the attribute value boxing row");
		assert!(typing_context.promotions(NodeId(1)).is_some(), "The bare value should be raised by a wrap promotion");
		let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("The wrap and boxing constructors should instantiate");

		let context: Context = None;
		let result: Option<Item<AttributeValueDyn>> = futures::executor::block_on(tree.eval(NodeId(1), context));
		let boxed = result.expect("The promoted bare value should arrive boxed as an Item");
		assert_eq!(boxed.element().0.as_any().downcast_ref::<f64>(), Some(&5.), "The stored value should be the bare element");
	}

	#[test]
	fn list_wire_variant_resolves_and_executes() {
		let tree = compile_bounding_box_network(TaggedValue::TypeDefault(descriptor!(List<Vector>)));

		let context: Context = None;
		let result: Option<List<Vector>> = futures::executor::block_on(tree.eval(NodeId(1), context.clone()));
		assert!(result.is_some(), "The mapped List wire variant should downcast and execute end-to-end");

		let wrong_type: Option<Item<Vector>> = futures::executor::block_on(tree.eval(NodeId(1), context));
		assert!(wrong_type.is_none(), "A List wire should not downcast as an Item");
	}

	#[test]
	fn expander_flattens_under_the_frame() {
		// A bare string wrapped onto an Item wire feeds String Split's expander primary; its parameters ride Item wires via promotion
		let string_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::String("a,b".into()).into()), vec![NodeId(0)]);
		let mut wrap_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(0)]), vec![NodeId(1)]);
		wrap_node.identifier = ProtoNodeIdentifier::new("graphene_core::ops::WrapItemNode<String>");

		let delimiter_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::String(",".into()).into()), vec![NodeId(2)]);
		let mut delimiter_field_adapter_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(2)]), vec![NodeId(3)]);
		delimiter_field_adapter_node.identifier = ProtoNodeIdentifier::new("graphene_core::ops::FieldAdapterNode<String>");

		let escaping_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::Bool(false).into()), vec![NodeId(4)]);
		let mut escaping_field_adapter_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(4)]), vec![NodeId(5)]);
		escaping_field_adapter_node.identifier = ProtoNodeIdentifier::new("graphene_core::ops::FieldAdapterNode<bool>");

		let output = NodeId(6);
		let mut string_split_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(1), NodeId(3), NodeId(5)]), vec![output]);
		string_split_node.identifier = graphene_std::text_nodes::string_split::IDENTIFIER;

		let network = ProtoNetwork {
			inputs: vec![],
			output,
			nodes: vec![
				(NodeId(0), string_node),
				(NodeId(1), wrap_node),
				(NodeId(2), delimiter_node),
				(NodeId(3), delimiter_field_adapter_node),
				(NodeId(4), escaping_node),
				(NodeId(5), escaping_field_adapter_node),
				(output, string_split_node),
			],
		};
		let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
		typing_context
			.update(&network)
			.expect("All-Item connectors should resolve the expander's direct `Item -> List` variant");
		assert!(typing_context.promotions(output).is_none(), "No promotion should be needed when every connector is already an Item");
		let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("The expander variant should instantiate");

		let context: Context = None;
		let result: Option<List<String>> = futures::executor::block_on(tree.eval(output, context));
		let list = result.expect("An Item-wired expander should produce a List");
		assert_eq!(list.len(), 2, "Splitting \"a,b\" on the comma should expand into two rows");
		let substrings: Vec<_> = list.iter_element_values().map(|s| s.as_str()).collect();
		assert_eq!(substrings, ["a", "b"], "The rows should hold the split substrings");
	}

	#[test]
	fn whole_list_switches_as_one_bundle() {
		// One bool selecting between two whole `List<Graphic>` stacks: each branch bundles into a rank-0 cell, and the result unbundles back to the flat stack
		let condition_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::Bool(true).into()), vec![NodeId(0)]);
		let if_true_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::TypeDefault(descriptor!(List<graphene_std::Graphic>)).into()), vec![NodeId(1)]);
		let if_false_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::TypeDefault(descriptor!(List<graphene_std::Graphic>)).into()), vec![NodeId(2)]);

		let mut switch_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(0), NodeId(1), NodeId(2)]), vec![NodeId(3)]);
		switch_node.identifier = ProtoNodeIdentifier::new("math_nodes::SwitchNode");

		let mut unbundle_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(3)]), vec![NodeId(4)]);
		unbundle_node.identifier = ProtoNodeIdentifier::new("graphene_core::ops::UnbundleNode<Graphic>");

		let network = ProtoNetwork {
			inputs: vec![],
			output: NodeId(4),
			nodes: vec![
				(NodeId(0), condition_node),
				(NodeId(1), if_true_node),
				(NodeId(2), if_false_node),
				(NodeId(3), switch_node),
				(NodeId(4), unbundle_node),
			],
		};
		let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
		typing_context
			.update(&network)
			.expect("A List<Graphic> branch should resolve the Item<Bundle<Graphic>> row via the bundle wrap");

		let promotions = typing_context.promotions(NodeId(3)).expect("The condition wrap and both branch bundles should be recorded");
		let branch_bundles = promotions
			.iter()
			.filter(|(index, adapter)| *index != 0 && matches!(adapter, graph_craft::proto::Promotion::Bundle(_)))
			.count();
		assert_eq!(branch_bundles, 2, "Both branches should bundle their whole list into one opaque cell");

		let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("The bundle, wrap, and unbundle adapters should instantiate");
		let context: Context = None;
		let result: Option<List<graphene_std::Graphic>> = futures::executor::block_on(tree.eval(NodeId(4), context));
		assert!(result.is_some(), "The whole stack should round-trip through the bundle switch back to a flat List<Graphic>");
	}

	#[test]
	fn a_bundle_unbundles_into_a_list_connector() {
		// A bundled wire (sourced here from a BundleNode, as a Switch branch produces one) feeding Extend's whole-`List` base connector
		let stack_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::TypeDefault(descriptor!(List<graphene_std::Graphic>)).into()), vec![NodeId(0)]);

		let mut bundle_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(0)]), vec![NodeId(1)]);
		bundle_node.identifier = ProtoNodeIdentifier::new("graphene_core::ops::BundleNode<Graphic>");

		let new_layers_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::TypeDefault(descriptor!(List<graphene_std::Graphic>)).into()), vec![NodeId(2)]);

		let mut extend_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(1), NodeId(2)]), vec![NodeId(3)]);
		extend_node.identifier = ProtoNodeIdentifier::new("graphic_nodes::graphic::ExtendNode");

		let network = ProtoNetwork {
			inputs: vec![],
			output: NodeId(3),
			nodes: vec![(NodeId(0), stack_node), (NodeId(1), bundle_node), (NodeId(2), new_layers_node), (NodeId(3), extend_node)],
		};
		let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
		typing_context.update(&network).expect("A bundled wire should feed Extend's List<Graphic> connector via the unbundle");

		let promotions = typing_context.promotions(NodeId(3)).expect("Extend's bundled base should be marked for unbundling");
		assert!(
			promotions.iter().any(|(index, adapter)| *index == 0 && matches!(adapter, graph_craft::proto::Promotion::Unbundle(_))),
			"The base connector should unbundle the whole list"
		);

		let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("The unbundle adapter should instantiate");
		let context: Context = None;
		let result: Option<List<graphene_std::Graphic>> = futures::executor::block_on(tree.eval(NodeId(3), context));
		assert!(result.is_some(), "The unbundled stack should flow into Extend as a List<Graphic>");
	}

	#[test]
	fn a_whole_list_of_scalars_switches_as_one_bundle() {
		// A single bool selecting between two whole `List<f64>` values, covering a primitive element type and confirming the selected list survives intact
		let condition_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::Bool(true).into()), vec![NodeId(0)]);
		let if_true_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::F64Array(vec![1., 2.]).into()), vec![NodeId(1)]);
		let if_false_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::F64Array(vec![3., 4., 5.]).into()), vec![NodeId(2)]);

		let mut switch_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(0), NodeId(1), NodeId(2)]), vec![NodeId(3)]);
		switch_node.identifier = ProtoNodeIdentifier::new("math_nodes::SwitchNode");

		let mut unbundle_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(3)]), vec![NodeId(4)]);
		unbundle_node.identifier = ProtoNodeIdentifier::new("graphene_core::ops::UnbundleNode<f64>");

		let network = ProtoNetwork {
			inputs: vec![],
			output: NodeId(4),
			nodes: vec![
				(NodeId(0), condition_node),
				(NodeId(1), if_true_node),
				(NodeId(2), if_false_node),
				(NodeId(3), switch_node),
				(NodeId(4), unbundle_node),
			],
		};
		let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
		typing_context
			.update(&network)
			.expect("A List<f64> branch should resolve the Item<Bundle<f64>> row via the bundle wrap");

		let promotions = typing_context.promotions(NodeId(3)).expect("The condition wrap and both branch bundles should be recorded");
		let branch_bundles = promotions
			.iter()
			.filter(|(index, adapter)| *index != 0 && matches!(adapter, graph_craft::proto::Promotion::Bundle(_)))
			.count();
		assert_eq!(branch_bundles, 2, "Both scalar-list branches should bundle into one opaque cell");

		let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("The bundle, wrap, and unbundle adapters should instantiate");
		let context: Context = None;
		let result: Option<List<f64>> = futures::executor::block_on(tree.eval(NodeId(4), context));
		let list = result.expect("The whole scalar list should round-trip through the bundle switch");
		assert_eq!(list.len(), 2, "The true branch's whole list should be selected and preserved intact");
	}
}
