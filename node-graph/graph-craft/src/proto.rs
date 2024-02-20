use crate::document::{value, InlineRust};
use crate::document::{NodeId, OriginalLocation};

use dyn_any::DynAny;
use graphene_core::*;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;
use std::ops::Deref;
use std::pin::Pin;

pub type DynFuture<'n, T> = Pin<Box<dyn core::future::Future<Output = T> + 'n>>;
pub type LocalFuture<'n, T> = Pin<Box<dyn core::future::Future<Output = T> + 'n>>;
pub type Any<'n> = Box<dyn DynAny<'n> + 'n>;
pub type FutureAny<'n> = DynFuture<'n, Any<'n>>;
// TODO: is this safe? This is assumed to be send+sync.
pub type TypeErasedNode<'n> = dyn for<'i> NodeIO<'i, Any<'i>, Output = FutureAny<'i>> + 'n;
pub type TypeErasedPinnedRef<'n> = Pin<&'n TypeErasedNode<'n>>;
pub type TypeErasedRef<'n> = &'n TypeErasedNode<'n>;
pub type TypeErasedBox<'n> = Box<TypeErasedNode<'n>>;
pub type TypeErasedPinned<'n> = Pin<Box<TypeErasedNode<'n>>>;

pub type SharedNodeContainer = std::rc::Rc<NodeContainer>;

pub type NodeConstructor = fn(Vec<SharedNodeContainer>) -> DynFuture<'static, TypeErasedBox<'static>>;

#[derive(Clone)]
pub struct NodeContainer {
	#[cfg(feature = "dealloc_nodes")]
	pub node: *mut TypeErasedNode<'static>,
	#[cfg(not(feature = "dealloc_nodes"))]
	pub node: TypeErasedRef<'static>,
}

impl Deref for NodeContainer {
	type Target = TypeErasedNode<'static>;

	#[cfg(feature = "dealloc_nodes")]
	fn deref(&self) -> &Self::Target {
		unsafe { &*(self.node as *const TypeErasedNode) }
		#[cfg(not(feature = "dealloc_nodes"))]
		self.node
	}
	#[cfg(not(feature = "dealloc_nodes"))]
	fn deref(&self) -> &Self::Target {
		self.node
	}
}

#[cfg(feature = "dealloc_nodes")]
impl Drop for NodeContainer {
	fn drop(&mut self) {
		unsafe { self.dealloc_unchecked() }
	}
}

impl core::fmt::Debug for NodeContainer {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("NodeContainer").finish()
	}
}

impl NodeContainer {
	pub fn new(node: TypeErasedBox<'static>) -> SharedNodeContainer {
		let node = Box::leak(node);
		Self { node }.into()
	}

	#[cfg(feature = "dealloc_nodes")]
	unsafe fn dealloc_unchecked(&mut self) {
		std::mem::drop(Box::from_raw(self.node));
	}
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Default, PartialEq, Clone, Hash, Eq)]
/// A list of [`ProtoNode`]s, which is an intermediate step between the [`crate::document::NodeNetwork`] and the `BorrowTree` containing a single flattened network.
pub struct ProtoNetwork {
	// TODO: remove this since it seems to be unused?
	// Should a proto Network even allow inputs? Don't think so
	pub inputs: Vec<NodeId>,
	/// The node ID that provides the output. This node is then responsible for calling the rest of the graph.
	pub output: NodeId,
	/// A list of nodes stored in a Vec to allow for sorting.
	pub nodes: Vec<(NodeId, ProtoNode)>,
}

impl core::fmt::Display for ProtoNetwork {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.write_str("Proto Network with nodes: ")?;
		fn write_node(f: &mut core::fmt::Formatter<'_>, network: &ProtoNetwork, id: NodeId, indent: usize) -> core::fmt::Result {
			f.write_str(&"\t".repeat(indent))?;
			let Some((_, node)) = network.nodes.iter().find(|(node_id, _)| *node_id == id) else {
				return f.write_str("{{Unknown Node}}");
			};
			f.write_str("Node: ")?;
			f.write_str(&node.identifier.name)?;

			f.write_str("\n")?;
			f.write_str(&"\t".repeat(indent))?;
			f.write_str("{\n")?;

			f.write_str(&"\t".repeat(indent + 1))?;
			f.write_str("Primary input: ")?;
			match &node.input {
				ProtoNodeInput::None => f.write_str("None")?,
				ProtoNodeInput::ManualComposition(ty) => f.write_fmt(format_args!("Manual Composition (type = {ty:?})"))?,
				ProtoNodeInput::Node(_) => f.write_str("Node")?,
				ProtoNodeInput::NodeLambda(_) => f.write_str("Lambda Node")?,
			}
			f.write_str("\n")?;

			match &node.construction_args {
				ConstructionArgs::Value(value) => {
					f.write_str(&"\t".repeat(indent + 1))?;
					f.write_fmt(format_args!("Value construction argument: {value:?}"))?
				}
				ConstructionArgs::Nodes(nodes) => {
					for id in nodes {
						write_node(f, network, id.0, indent + 1)?;
					}
				}
				ConstructionArgs::Inline(inline) => {
					f.write_str(&"\t".repeat(indent + 1))?;
					f.write_fmt(format_args!("Inline construction argument: {inline:?}"))?
				}
			}
			f.write_str(&"\t".repeat(indent))?;
			f.write_str("}\n")?;
			Ok(())
		}

		let id = self.output;
		write_node(f, self, id, 0)
	}
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
/// Defines the arguments used to construct the boxed node struct. This is used to call the constructor function in the `node_registry.rs` file - which is hidden behind a wall of macros.
pub enum ConstructionArgs {
	/// A value of a type that is known, allowing serialization (serde::Deserialize is not object safe)
	Value(value::TaggedValue),
	// TODO: use a struct for clearer naming.
	/// A list of nodes used as inputs to the constructor function in `node_registry.rs`.
	/// The bool indicates whether to treat the node as lambda node.
	Nodes(Vec<(NodeId, bool)>),
	// TODO: What?
	Inline(InlineRust),
}

impl Eq for ConstructionArgs {}

impl PartialEq for ConstructionArgs {
	fn eq(&self, other: &Self) -> bool {
		match (&self, &other) {
			(Self::Nodes(n1), Self::Nodes(n2)) => n1 == n2,
			(Self::Value(v1), Self::Value(v2)) => v1 == v2,
			_ => {
				use std::hash::Hasher;
				let hash = |input: &Self| {
					let mut hasher = rustc_hash::FxHasher::default();
					input.hash(&mut hasher);
					hasher.finish()
				};
				hash(self) == hash(other)
			}
		}
	}
}

impl Hash for ConstructionArgs {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		core::mem::discriminant(self).hash(state);
		match self {
			Self::Nodes(nodes) => {
				for node in nodes {
					node.hash(state);
				}
			}
			Self::Value(value) => value.hash(state),
			Self::Inline(inline) => inline.hash(state),
		}
	}
}

impl ConstructionArgs {
	// TODO: what? Used in the gpu_compiler crate for something.
	pub fn new_function_args(&self) -> Vec<String> {
		match self {
			ConstructionArgs::Nodes(nodes) => nodes.iter().map(|(n, _)| format!("n{:0x}", n.0)).collect(),
			ConstructionArgs::Value(value) => vec![value.to_primitive_string()],
			ConstructionArgs::Inline(inline) => vec![inline.expr.clone()],
		}
	}
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Hash, Eq)]
/// A protonode is an intermediate step between the `DocumentNode` and the boxed struct that actually runs the node (found in the [`BorrowTree`]). It has one primary input and several secondary inputs in [`ConstructionArgs`].
pub struct ProtoNode {
	pub construction_args: ConstructionArgs,
	pub input: ProtoNodeInput,
	pub identifier: ProtoNodeIdentifier,
	pub original_location: OriginalLocation,
	pub skip_deduplication: bool,
	// TODO: This is a hack, figure out a proper solution
	/// Represents a global state on which the node depends.
	pub world_state_hash: u64,
}

impl Default for ProtoNode {
	fn default() -> Self {
		Self {
			identifier: ProtoNodeIdentifier::new("graphene_core::ops::IdentityNode"),
			construction_args: ConstructionArgs::Value(value::TaggedValue::U32(0)),
			input: ProtoNodeInput::None,
			original_location: OriginalLocation::default(),
			skip_deduplication: false,
			world_state_hash: 0,
		}
	}
}

/// A ProtoNodeInput represents the primary input of a node in a ProtoNetwork.
/// Similar to [`crate::document::NodeInput`].
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ProtoNodeInput {
	/// [`ProtoNode`]s do not require any input, e.g. the value node just takes in [`ConstructionArgs`].
	None,
	/// A ManualComposition input represents an input that opts out of being resolved through the default `ComposeNode`, which first runs the previous (upstream) node, then passes that evaluated
	/// result to this node. Instead, ManualComposition lets this node actually consume the provided input instead of passing it to its predecessor.
	///
	/// Say we have the network `a -> b -> c` where `c` is the output node and `a` is the input node.
	/// We would expect `a` to get input from the network, `b` to get input from `a`, and `c` to get input from `b`.
	/// This could be represented as `f(x) = c(b(a(x)))`. `a` is run with input `x` from the network. `b` is run with input from `a`. `c` is run with input from `b`.
	///
	/// However if `b`'s input is using manual composition, this means it would instead be `f(x) = c(b(x))`. This means that `b` actually gets input from the network, and `a` is not automatically
	/// executed as it would be using the default ComposeNode flow. Now `b` can use its own logic to decide when or if it wants to run `a` and how to use its output. For example, the CacheNode can
	/// look up `x` in its cache and return the result, or otherwise call `a`, cache the result, and return it.
	ManualComposition(Type),
	/// The previous node where automatic (not manual) composition occurs when compiled. The entire network, of which the node is the output, is fed as input.
	///
	/// Grayscale example:
	///
	/// We're interested in receiving an input of the desaturated image data which has been fed through a grayscale filter.
	/// (If we were interested in the grayscale filter itself, we would use the `NodeLambda` variant.)
	Node(NodeId),
	/// Unlike the `Node` variant, with `NodeLambda` we treat the connected node singularly as a lambda node while ignoring all nodes which feed into it from upstream.
	///
	/// Grayscale example:
	///
	/// We're interested in receiving an input of a particular image filter, such as a grayscale filter in the form of a grayscale node lambda.
	/// (If we were interested in some image data that had been fed through a grayscale filter, we would use the `Node` variant.)
	NodeLambda(NodeId),
}

impl ProtoNode {
	/// A stable node ID is a hash of a node that should stay constant. This is used in order to remove duplicates from the graph.
	/// In the case of `skip_deduplication`, the `document_node_path` is also hashed in order to avoid duplicate monitor nodes from being removed (which would make it impossible to load thumbnails).
	pub fn stable_node_id(&self) -> Option<NodeId> {
		use std::hash::Hasher;
		let mut hasher = rustc_hash::FxHasher::default();

		self.identifier.name.hash(&mut hasher);
		self.construction_args.hash(&mut hasher);
		if self.skip_deduplication {
			self.original_location.path.hash(&mut hasher);
		}
		self.world_state_hash.hash(&mut hasher);
		std::mem::discriminant(&self.input).hash(&mut hasher);
		match self.input {
			ProtoNodeInput::None => (),
			ProtoNodeInput::ManualComposition(ref ty) => {
				ty.hash(&mut hasher);
			}
			ProtoNodeInput::Node(id) => (id, false).hash(&mut hasher),
			ProtoNodeInput::NodeLambda(id) => (id, true).hash(&mut hasher),
		};
		Some(NodeId(hasher.finish()))
	}

	/// Construct a new [`ProtoNode`] with the specified construction args and a `ClonedNode` implementation.
	pub fn value(value: ConstructionArgs, path: Vec<NodeId>) -> Self {
		let inputs_exposed = match &value {
			ConstructionArgs::Nodes(nodes) => nodes.len() + 1,
			_ => 2,
		};
		Self {
			identifier: ProtoNodeIdentifier::new("graphene_core::value::ClonedNode"),
			construction_args: value,
			input: ProtoNodeInput::None,
			original_location: OriginalLocation {
				path: Some(path),
				inputs_exposed: vec![false; inputs_exposed],
				..Default::default()
			},
			skip_deduplication: false,
			world_state_hash: 0,
		}
	}

	/// Converts all references to other node IDs into new IDs by running the specified function on them.
	/// This can be used when changing the IDs of the nodes, for example in the case of generating stable IDs.
	pub fn map_ids(&mut self, f: impl Fn(NodeId) -> NodeId, skip_lambdas: bool) {
		match self.input {
			ProtoNodeInput::Node(id) => self.input = ProtoNodeInput::Node(f(id)),
			ProtoNodeInput::NodeLambda(id) => {
				if !skip_lambdas {
					self.input = ProtoNodeInput::NodeLambda(f(id))
				}
			}
			_ => (),
		}

		if let ConstructionArgs::Nodes(ids) = &mut self.construction_args {
			ids.iter_mut().filter(|(_, lambda)| !(skip_lambdas && *lambda)).for_each(|(id, _)| *id = f(*id));
		}
	}

	pub fn unwrap_construction_nodes(&self) -> Vec<(NodeId, bool)> {
		match &self.construction_args {
			ConstructionArgs::Nodes(nodes) => nodes.clone(),
			_ => panic!("tried to unwrap nodes from non node construction args \n node: {self:#?}"),
		}
	}
}

impl ProtoNetwork {
	fn check_ref(&self, ref_id: &NodeId, id: &NodeId) {
		assert!(
			self.nodes.iter().any(|(check_id, _)| check_id == ref_id),
			"Node id:{id} has a reference which uses node id:{ref_id} which doesn't exist in network {self:#?}"
		);
	}

	/// Construct a hashmap containing a list of the nodes that depend on this proto network.
	pub fn collect_outwards_edges(&self) -> HashMap<NodeId, Vec<NodeId>> {
		let mut edges: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
		for (id, node) in &self.nodes {
			match &node.input {
				ProtoNodeInput::Node(ref_id) | ProtoNodeInput::NodeLambda(ref_id) => {
					self.check_ref(ref_id, id);
					edges.entry(*ref_id).or_default().push(*id)
				}
				_ => (),
			}

			if let ConstructionArgs::Nodes(ref_nodes) = &node.construction_args {
				for (ref_id, _) in ref_nodes {
					self.check_ref(ref_id, id);
					edges.entry(*ref_id).or_default().push(*id)
				}
			}
		}
		edges
	}

	/// Convert all node IDs to be stable (based on the hash generated by [`ProtoNode::stable_node_id`]).
	/// This function requires that the graph be topologically sorted.
	pub fn generate_stable_node_ids(&mut self) {
		debug_assert!(self.is_topologically_sorted());
		let outwards_edges = self.collect_outwards_edges();

		for index in 0..self.nodes.len() {
			let Some(sni) = self.nodes[index].1.stable_node_id() else {
				panic!("failed to generate stable node id for node {:#?}", self.nodes[index].1);
			};
			self.replace_node_id(&outwards_edges, NodeId(index as u64), sni, false);
			self.nodes[index].0 = sni;
		}
	}

	/// Create a hashmap with the list of nodes this proto network depends on/uses as inputs.
	pub fn collect_inwards_edges(&self) -> HashMap<NodeId, Vec<NodeId>> {
		let mut edges: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
		for (id, node) in &self.nodes {
			match &node.input {
				ProtoNodeInput::Node(ref_id) | ProtoNodeInput::NodeLambda(ref_id) => {
					self.check_ref(ref_id, id);
					edges.entry(*id).or_default().push(*ref_id)
				}
				_ => (),
			}

			if let ConstructionArgs::Nodes(ref_nodes) = &node.construction_args {
				for (ref_id, _) in ref_nodes {
					self.check_ref(ref_id, id);
					edges.entry(*id).or_default().push(*ref_id)
				}
			}
		}
		edges
	}

	/// Inserts a [`graphene_core::structural::ComposeNode`] for each node that has a [`ProtoNodeInput::Node`]. The compose node evaluates the first node, and then sends the result into the second node.
	pub fn resolve_inputs(&mut self) -> Result<(), String> {
		// Perform topological sort once
		self.reorder_ids()?;

		let max_id = self.nodes.len() as u64 - 1;

		// Collect outward edges once
		let outwards_edges = self.collect_outwards_edges();

		// Iterate over nodes in topological order
		for node_id in 0..=max_id {
			let node_id = NodeId(node_id);

			let (_, node) = &mut self.nodes[node_id.0 as usize];

			if let ProtoNodeInput::Node(input_node_id) = node.input {
				// Create a new node that composes the current node and its input node
				let compose_node_id = NodeId(self.nodes.len() as u64);

				let (_, input_node_id_proto) = &self.nodes[input_node_id.0 as usize];

				let input = input_node_id_proto.input.clone();

				let mut path = input_node_id_proto.original_location.path.clone();
				if let Some(path) = &mut path {
					path.push(node_id);
				}

				self.nodes.push((
					compose_node_id,
					ProtoNode {
						identifier: ProtoNodeIdentifier::new("graphene_core::structural::ComposeNode<_, _, _>"),
						construction_args: ConstructionArgs::Nodes(vec![(input_node_id, false), (node_id, true)]),
						input,
						original_location: OriginalLocation { path, ..Default::default() },
						skip_deduplication: false,
						world_state_hash: 0,
					},
				));

				self.replace_node_id(&outwards_edges, node_id, compose_node_id, true);
			}
		}
		self.reorder_ids()?;
		Ok(())
	}

	/// Update all of the references to a node ID in the graph with a new ID named `compose_node_id`.
	fn replace_node_id(&mut self, outwards_edges: &HashMap<NodeId, Vec<NodeId>>, node_id: NodeId, compose_node_id: NodeId, skip_lambdas: bool) {
		// Update references in other nodes to use the new compose node
		if let Some(referring_nodes) = outwards_edges.get(&node_id) {
			for &referring_node_id in referring_nodes {
				let (_, referring_node) = &mut self.nodes[referring_node_id.0 as usize];
				referring_node.map_ids(|id| if id == node_id { compose_node_id } else { id }, skip_lambdas)
			}
		}

		if self.output == node_id {
			self.output = compose_node_id;
		}

		self.inputs.iter_mut().for_each(|id| {
			if *id == node_id {
				*id = compose_node_id;
			}
		});
	}
	// Based on https://en.wikipedia.org/wiki/Topological_sorting#Depth-first_search
	// This approach excludes nodes that are not connected
	pub fn topological_sort(&self) -> Result<Vec<NodeId>, String> {
		let mut sorted = Vec::new();
		let inwards_edges = self.collect_inwards_edges();
		fn visit(node_id: NodeId, temp_marks: &mut HashSet<NodeId>, sorted: &mut Vec<NodeId>, inwards_edges: &HashMap<NodeId, Vec<NodeId>>, network: &ProtoNetwork) -> Result<(), String> {
			if sorted.contains(&node_id) {
				return Ok(());
			};
			if temp_marks.contains(&node_id) {
				return Err(format!("Cycle detected {inwards_edges:#?}, {network:#?}"));
			}

			if let Some(dependencies) = inwards_edges.get(&node_id) {
				temp_marks.insert(node_id);
				for &dependant in dependencies {
					visit(dependant, temp_marks, sorted, inwards_edges, network)?;
				}
				temp_marks.remove(&node_id);
			}
			sorted.push(node_id);
			Ok(())
		}

		if !self.nodes.iter().any(|(id, _)| *id == self.output) {
			return Err(format!("Output id {} does not exist", self.output));
		}
		visit(self.output, &mut HashSet::new(), &mut sorted, &inwards_edges, self)?;
		Ok(sorted)
	}

	fn is_topologically_sorted(&self) -> bool {
		let mut visited = HashSet::new();

		let inwards_edges = self.collect_inwards_edges();
		for (id, _) in &self.nodes {
			for &dependency in inwards_edges.get(id).unwrap_or(&Vec::new()) {
				if !visited.contains(&dependency) {
					dbg!(id, dependency);
					dbg!(&visited);
					dbg!(&self.nodes);
					return false;
				}
			}
			visited.insert(*id);
		}
		true
	}

	/*// Based on https://en.wikipedia.org/wiki/Topological_sorting#Kahn's_algorithm
	pub fn topological_sort(&self) -> Vec<NodeId> {
		let mut sorted = Vec::new();
		let outwards_edges = self.collect_outwards_edges();
		let mut inwards_edges = self.collect_inwards_edges();
		let mut no_incoming_edges: Vec<_> = self.nodes.iter().map(|entry| entry.0).filter(|id| !inwards_edges.contains_key(id)).collect();

		assert_ne!(no_incoming_edges.len(), 0, "Acyclic graphs must have at least one node with no incoming edge");

		while let Some(node_id) = no_incoming_edges.pop() {
			sorted.push(node_id);

			if let Some(outwards_edges) = outwards_edges.get(&node_id) {
				for &ref_id in outwards_edges {
					let dependencies = inwards_edges.get_mut(&ref_id).unwrap();
					dependencies.retain(|&id| id != node_id);
					if dependencies.is_empty() {
						no_incoming_edges.push(ref_id)
					}
				}
			}
		}
		debug!("Sorted order {sorted:?}");
		sorted
	}*/

	/// Sort the nodes vec so it is in a topological order. This ensures that no node takes an input from a node that is found later in the list.
	fn reorder_ids(&mut self) -> Result<(), String> {
		let order = self.topological_sort()?;

		// Map of node ids to their current index in the nodes vector
		let current_positions: HashMap<_, _> = self.nodes.iter().enumerate().map(|(pos, (id, _))| (*id, pos)).collect();

		// Map of node ids to their new index based on topological order
		let new_positions: HashMap<_, _> = order.iter().enumerate().map(|(pos, id)| (*id, NodeId(pos as u64))).collect();

		// Create a new nodes vector based on the topological order
		let mut new_nodes = Vec::with_capacity(order.len());
		for (index, &id) in order.iter().enumerate() {
			let current_pos = *current_positions.get(&id).unwrap();
			new_nodes.push((NodeId(index as u64), self.nodes[current_pos].1.clone()));
		}

		// Update node references to reflect the new order
		new_nodes.iter_mut().for_each(|(_, node)| {
			node.map_ids(|id| *new_positions.get(&id).expect("node not found in lookup table"), false);
		});

		// Update the nodes vector and other references
		self.nodes = new_nodes;
		self.inputs = self.inputs.iter().filter_map(|id| new_positions.get(id).copied()).collect();
		self.output = *new_positions.get(&self.output).unwrap();

		assert_eq!(order.len(), self.nodes.len());
		Ok(())
	}
}
#[derive(Clone, PartialEq)]
pub enum GraphErrorType {
	NodeNotFound(NodeId),
	InputNodeNotFound(NodeId),
	UnexpectedGenerics { index: usize, parameters: Vec<Type> },
	NoImplementations,
	NoConstructor,
	InvalidImplementations { parameters: String, error_inputs: Vec<Vec<(usize, (Type, Type))>> },
	MultipleImplementations { parameters: String, valid: Vec<NodeIOTypes> },
}
impl core::fmt::Debug for GraphErrorType {
	// TODO: format with the document graph context so the input index is the same as in the graph UI.
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			GraphErrorType::NodeNotFound(id) => write!(f, "Input node {id} is not present in the typing context"),
			GraphErrorType::InputNodeNotFound(id) => write!(f, "Input node {id} is not present in the typing context"),
			GraphErrorType::UnexpectedGenerics { index, parameters } => write!(f, "Generic parameters should not exist but found at {index}: {parameters:?}"),
			GraphErrorType::NoImplementations => write!(f, "No implementations found"),
			GraphErrorType::NoConstructor => write!(f, "No construct found for node"),
			GraphErrorType::InvalidImplementations { parameters, error_inputs } => {
				let ordinal = |x: usize| match x.to_string().as_str() {
					x if x.ends_with('1') && !x.ends_with("11") => format!("{x}st"),
					x if x.ends_with('2') && !x.ends_with("12") => format!("{x}nd"),
					x if x.ends_with('3') && !x.ends_with("13") => format!("{x}rd"),
					x => format!("{x}th"),
				};
				let format_index = |index: usize| if index == 0 { "primary".to_string() } else { format!("{} parameter", ordinal(index)) };
				let format_error = |(index, (real, expected)): &(usize, (Type, Type))| format!("• The {} input expected {} but found {}", format_index(*index), expected, real);
				let format_error_list = |errors: &Vec<(usize, (Type, Type))>| errors.iter().map(format_error).collect::<Vec<_>>().join("\n");
				let errors = error_inputs.iter().map(format_error_list).collect::<Vec<_>>();
				write!(
					f,
					"Node graph type error! If this just appeared while editing the graph,\n\
					consider using undo to go back and try another way to connect the nodes.\n\
					\n\
					No node implementation exists for type ({parameters}).\n\
					\n\
					Caused by{}:\n\
					{}",
					if errors.len() > 1 { " one of" } else { "" },
					errors.join("\n")
				)
			}
			GraphErrorType::MultipleImplementations { parameters, valid } => write!(f, "Multiple implementations found ({parameters}):\n{valid:#?}"),
		}
	}
}
#[derive(Clone, PartialEq)]
pub struct GraphError {
	pub node_path: Vec<NodeId>,
	pub identifier: Cow<'static, str>,
	pub error: GraphErrorType,
}
impl GraphError {
	pub fn new(node: &ProtoNode, text: impl Into<GraphErrorType>) -> Self {
		Self {
			node_path: node.original_location.path.clone().unwrap_or_default(),
			identifier: node.identifier.name.clone(),
			error: text.into(),
		}
	}
}
impl core::fmt::Debug for GraphError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("NodeGraphError")
			.field("path", &self.node_path.iter().map(|id| id.0).collect::<Vec<_>>())
			.field("identifier", &self.identifier.to_string())
			.field("error", &self.error)
			.finish()
	}
}
pub type GraphErrors = Vec<GraphError>;

/// The `TypingContext` is used to store the types of the nodes indexed by their stable node id.
#[derive(Default, Clone)]
pub struct TypingContext {
	lookup: Cow<'static, HashMap<ProtoNodeIdentifier, HashMap<NodeIOTypes, NodeConstructor>>>,
	inferred: HashMap<NodeId, NodeIOTypes>,
	constructor: HashMap<NodeId, NodeConstructor>,
}

impl TypingContext {
	/// Creates a new `TypingContext` with the given lookup table.
	pub fn new(lookup: &'static HashMap<ProtoNodeIdentifier, HashMap<NodeIOTypes, NodeConstructor>>) -> Self {
		Self {
			lookup: Cow::Borrowed(lookup),
			..Default::default()
		}
	}

	/// Updates the `TypingContext` with a given proto network. This will infer the types of the nodes
	/// and store them in the `inferred` field. The proto network has to be topologically sorted
	/// and contain fully resolved stable node ids.
	pub fn update(&mut self, network: &ProtoNetwork) -> Result<(), GraphErrors> {
		let mut deleted_nodes = self.inferred.keys().copied().collect::<HashSet<_>>();

		for (id, node) in network.nodes.iter() {
			self.infer(*id, node)?;
			deleted_nodes.remove(id);
		}

		for node in deleted_nodes {
			self.inferred.remove(&node);
		}

		Ok(())
	}

	/// Returns the node constructor for a given node id.
	pub fn constructor(&self, node_id: NodeId) -> Option<NodeConstructor> {
		self.constructor.get(&node_id).copied()
	}

	/// Returns the type of a given node id if it exists
	pub fn type_of(&self, node_id: NodeId) -> Option<&NodeIOTypes> {
		self.inferred.get(&node_id)
	}

	/// Returns the inferred types for a given node id.
	pub fn infer(&mut self, node_id: NodeId, node: &ProtoNode) -> Result<NodeIOTypes, GraphErrors> {
		// Return the inferred type if it is already known
		if let Some(inferred) = self.inferred.get(&node_id) {
			return Ok(inferred.clone());
		}

		let parameters = match node.construction_args {
			// If the node has a value parameter we can infer the return type from it
			ConstructionArgs::Value(ref v) => {
				assert!(matches!(node.input, ProtoNodeInput::None));
				// TODO: This should return a reference to the value
				let types = NodeIOTypes::new(concrete!(()), v.ty(), vec![v.ty()]);
				self.inferred.insert(node_id, types.clone());
				return Ok(types);
			}
			// If the node has nodes as parameters we can infer the types from the node outputs
			ConstructionArgs::Nodes(ref nodes) => nodes
				.iter()
				.map(|(id, _)| {
					self.inferred
						.get(id)
						.ok_or_else(|| vec![GraphError::new(node, GraphErrorType::NodeNotFound(*id))])
						.map(|node| node.ty())
				})
				.collect::<Result<Vec<Type>, GraphErrors>>()?,
			ConstructionArgs::Inline(ref inline) => vec![inline.ty.clone()],
		};

		// Get the node input type from the protonode declaration
		let input = match node.input {
			ProtoNodeInput::None => concrete!(()),
			ProtoNodeInput::ManualComposition(ref ty) => ty.clone(),
			ProtoNodeInput::Node(id) | ProtoNodeInput::NodeLambda(id) => {
				let input = self.inferred.get(&id).ok_or_else(|| vec![GraphError::new(node, GraphErrorType::InputNodeNotFound(id))])?;
				input.output.clone()
			}
		};
		let impls = self.lookup.get(&node.identifier).ok_or_else(|| vec![GraphError::new(node, GraphErrorType::NoImplementations)])?;

		if let Some(index) = parameters.iter().position(|p| {
			matches!(p,
			Type::Fn(_, b) if matches!(b.as_ref(), Type::Generic(_)))
		}) {
			return Err(vec![GraphError::new(node, GraphErrorType::UnexpectedGenerics { index, parameters })]);
		}
		fn covariant(from: &Type, to: &Type) -> bool {
			match (from, to) {
				(Type::Concrete(t1), Type::Concrete(t2)) => t1 == t2,
				(Type::Fn(a1, b1), Type::Fn(a2, b2)) => covariant(a1, a2) && covariant(b1, b2),
				// TODO: Add proper generic counting which is not based on the name
				(Type::Generic(_), Type::Generic(_)) => true,
				(Type::Generic(_), _) => true,
				(_, Type::Generic(_)) => true,
				_ => false,
			}
		}

		// List of all implementations that match the input and parameter types
		let valid_output_types = impls
			.keys()
			.filter(|node_io| covariant(&input, &node_io.input) && parameters.iter().zip(node_io.parameters.iter()).all(|(p1, p2)| covariant(p1, p2)))
			.collect::<Vec<_>>();

		// Attempt to substitute generic types with concrete types and save the list of results
		let substitution_results = valid_output_types
			.iter()
			.map(|node_io| {
				collect_generics(node_io)
					.iter()
					.try_for_each(|generic| check_generic(node_io, &input, &parameters, generic).map(|_| ()))
					.map(|_| {
						if let Type::Generic(out) = &node_io.output {
							((*node_io).clone(), check_generic(node_io, &input, &parameters, out).unwrap())
						} else {
							((*node_io).clone(), node_io.output.clone())
						}
					})
			})
			.collect::<Vec<_>>();

		// Collect all substitutions that are valid
		let valid_impls = substitution_results.iter().filter_map(|result| result.as_ref().ok()).collect::<Vec<_>>();

		match valid_impls.as_slice() {
			[] => {
				dbg!(&self.inferred);
				let mut best_errors = usize::MAX;
				let mut error_inputs = Vec::new();
				for node_io in impls.keys() {
					let current_errors = [&input]
						.into_iter()
						.chain(&parameters)
						.cloned()
						.zip([&node_io.input].into_iter().chain(&node_io.parameters).cloned())
						.enumerate()
						.filter(|(_, (p1, p2))| !covariant(p1, p2))
						.map(|(index, ty)| (node.original_location.inputs(index).min_by_key(|s| s.node.len()).map(|s| s.index).unwrap_or(index), ty))
						.collect::<Vec<_>>();
					if current_errors.len() < best_errors {
						best_errors = current_errors.len();
						error_inputs.clear();
					}
					if current_errors.len() <= best_errors {
						error_inputs.push(current_errors);
					}
				}
				let parameters = [&input].into_iter().chain(&parameters).map(|t| t.to_string()).collect::<Vec<_>>().join(", ");
				Err(vec![GraphError::new(node, GraphErrorType::InvalidImplementations { parameters, error_inputs })])
			}
			[(org_nio, output)] => {
				let node_io = NodeIOTypes::new(input, (*output).clone(), parameters);

				// Save the inferred type
				self.inferred.insert(node_id, node_io.clone());
				self.constructor.insert(node_id, impls[org_nio]);
				Ok(node_io)
			}

			_ => {
				let parameters = [&input].into_iter().chain(&parameters).map(|t| t.to_string()).collect::<Vec<_>>().join(", ");
				let valid = valid_output_types.into_iter().cloned().collect();
				Err(vec![GraphError::new(node, GraphErrorType::MultipleImplementations { parameters, valid })])
			}
		}
	}
}

/// Returns a list of all generic types used in the node
fn collect_generics(types: &NodeIOTypes) -> Vec<Cow<'static, str>> {
	let inputs = [&types.input].into_iter().chain(types.parameters.iter().flat_map(|x| x.fn_output()));
	let mut generics = inputs
		.filter_map(|t| match t {
			Type::Generic(out) => Some(out.clone()),
			_ => None,
		})
		.collect::<Vec<_>>();
	if let Type::Generic(out) = &types.output {
		generics.push(out.clone());
	}
	generics.dedup();
	generics
}

/// Checks if a generic type can be substituted with a concrete type and returns the concrete type
fn check_generic(types: &NodeIOTypes, input: &Type, parameters: &[Type], generic: &str) -> Result<Type, String> {
	let inputs = [(Some(&types.input), Some(input))]
		.into_iter()
		.chain(types.parameters.iter().map(|x| x.fn_output()).zip(parameters.iter().map(|x| x.fn_output())));
	let concrete_inputs = inputs.filter(|(ni, _)| matches!(ni, Some(Type::Generic(input)) if generic == input));
	let mut outputs = concrete_inputs.flat_map(|(_, out)| out);
	let out_ty = outputs
		.next()
		.ok_or_else(|| format!("Generic output type {generic} is not dependent on input {input:?} or parameters {parameters:?}",))?;
	if outputs.any(|ty| ty != out_ty) {
		return Err(format!("Generic output type {generic} is dependent on multiple inputs or parameters",));
	}
	Ok(out_ty.clone())
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::proto::{ConstructionArgs, ProtoNetwork, ProtoNode, ProtoNodeInput};

	#[test]
	fn topological_sort() {
		let construction_network = test_network();
		let sorted = construction_network.topological_sort().expect("Error when calling 'topological_sort' on 'construction_network.");
		println!("{sorted:#?}");
		assert_eq!(sorted, vec![NodeId(14), NodeId(10), NodeId(11), NodeId(1)]);
	}

	#[test]
	fn topological_sort_with_cycles() {
		let construction_network = test_network_with_cycles();
		let sorted = construction_network.topological_sort();

		assert!(sorted.is_err())
	}

	#[test]
	fn id_reordering() {
		let mut construction_network = test_network();
		construction_network.reorder_ids().expect("Error when calling 'reorder_ids' on 'construction_network.");
		let sorted = construction_network.topological_sort().expect("Error when calling 'topological_sort' on 'construction_network.");
		println!("nodes: {:#?}", construction_network.nodes);
		assert_eq!(sorted, vec![NodeId(0), NodeId(1), NodeId(2), NodeId(3)]);
		let ids: Vec<_> = construction_network.nodes.iter().map(|(id, _)| *id).collect();
		println!("{ids:#?}");
		println!("nodes: {:#?}", construction_network.nodes);
		assert_eq!(construction_network.nodes[0].1.identifier.name.as_ref(), "value");
		assert_eq!(ids, vec![NodeId(0), NodeId(1), NodeId(2), NodeId(3)]);
	}

	#[test]
	fn id_reordering_idempotent() {
		let mut construction_network = test_network();
		construction_network.reorder_ids().expect("Error when calling 'reorder_ids' on 'construction_network.");
		construction_network.reorder_ids().expect("Error when calling 'reorder_ids' on 'construction_network.");
		let sorted = construction_network.topological_sort().expect("Error when calling 'topological_sort' on 'construction_network.");
		assert_eq!(sorted, vec![NodeId(0), NodeId(1), NodeId(2), NodeId(3)]);
		let ids: Vec<_> = construction_network.nodes.iter().map(|(id, _)| *id).collect();
		println!("{ids:#?}");
		assert_eq!(construction_network.nodes[0].1.identifier.name.as_ref(), "value");
		assert_eq!(ids, vec![NodeId(0), NodeId(1), NodeId(2), NodeId(3)]);
	}

	#[test]
	fn input_resolution() {
		let mut construction_network = test_network();
		construction_network.resolve_inputs().expect("Error when calling 'resolve_inputs' on 'construction_network.");
		println!("{construction_network:#?}");
		assert_eq!(construction_network.nodes[0].1.identifier.name.as_ref(), "value");
		assert_eq!(construction_network.nodes.len(), 6);
		assert_eq!(construction_network.nodes[5].1.construction_args, ConstructionArgs::Nodes(vec![(NodeId(3), false), (NodeId(4), true)]));
	}

	#[test]
	fn stable_node_id_generation() {
		let mut construction_network = test_network();
		construction_network.resolve_inputs().expect("Error when calling 'resolve_inputs' on 'construction_network.");
		construction_network.generate_stable_node_ids();
		assert_eq!(construction_network.nodes[0].1.identifier.name.as_ref(), "value");
		let ids: Vec<_> = construction_network.nodes.iter().map(|(id, _)| *id).collect();
		assert_eq!(
			ids,
			vec![
				NodeId(17957338642374791135),
				NodeId(1303972037140595827),
				NodeId(15485192931817078264),
				NodeId(9739645351331265115),
				NodeId(4165308598738454684),
				NodeId(5557529806312473178)
			]
		);
	}

	fn test_network() -> ProtoNetwork {
		ProtoNetwork {
			inputs: vec![NodeId(10)],
			output: NodeId(1),
			nodes: [
				(
					NodeId(7),
					ProtoNode {
						identifier: "id".into(),
						input: ProtoNodeInput::Node(NodeId(11)),
						construction_args: ConstructionArgs::Nodes(vec![]),
						..Default::default()
					},
				),
				(
					NodeId(1),
					ProtoNode {
						identifier: "id".into(),
						input: ProtoNodeInput::Node(NodeId(11)),
						construction_args: ConstructionArgs::Nodes(vec![]),
						..Default::default()
					},
				),
				(
					NodeId(10),
					ProtoNode {
						identifier: "cons".into(),
						input: ProtoNodeInput::ManualComposition(concrete!(u32)),
						construction_args: ConstructionArgs::Nodes(vec![(NodeId(14), false)]),
						..Default::default()
					},
				),
				(
					NodeId(11),
					ProtoNode {
						identifier: "add".into(),
						input: ProtoNodeInput::Node(NodeId(10)),
						construction_args: ConstructionArgs::Nodes(vec![]),
						..Default::default()
					},
				),
				(
					NodeId(14),
					ProtoNode {
						identifier: "value".into(),
						input: ProtoNodeInput::None,
						construction_args: ConstructionArgs::Value(value::TaggedValue::U32(2)),
						..Default::default()
					},
				),
			]
			.into_iter()
			.collect(),
		}
	}

	fn test_network_with_cycles() -> ProtoNetwork {
		ProtoNetwork {
			inputs: vec![NodeId(1)],
			output: NodeId(1),
			nodes: [
				(
					NodeId(1),
					ProtoNode {
						identifier: "id".into(),
						input: ProtoNodeInput::Node(NodeId(2)),
						construction_args: ConstructionArgs::Nodes(vec![]),
						..Default::default()
					},
				),
				(
					NodeId(2),
					ProtoNode {
						identifier: "id".into(),
						input: ProtoNodeInput::Node(NodeId(1)),
						construction_args: ConstructionArgs::Nodes(vec![]),
						..Default::default()
					},
				),
			]
			.into_iter()
			.collect(),
		}
	}
}
