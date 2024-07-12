use crate::document::value::TaggedValue;
use crate::proto::{ConstructionArgs, ProtoNetwork, ProtoNode, ProtoNodeInput};

use dyn_any::{DynAny, StaticType};
pub use graphene_core::uuid::generate_uuid;
use graphene_core::{ProtoNodeIdentifier, Type};

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

pub mod value;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct NodeId(pub u64);

// TODO: Find and replace all `NodeId(generate_uuid())` with `NodeId::new()`.
impl NodeId {
	pub fn new() -> Self {
		Self(generate_uuid())
	}
}

impl core::fmt::Display for NodeId {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		write!(f, "{}", self.0)
	}
}

/// Hash two IDs together, returning a new ID that is always consistant for two input IDs in a specific order.
/// This is used during [`NodeNetwork::flatten`] in order to ensure consistant yet non-conflicting IDs for inner networks.
fn merge_ids(a: NodeId, b: NodeId) -> NodeId {
	let mut hasher = DefaultHasher::new();
	a.hash(&mut hasher);
	b.hash(&mut hasher);
	NodeId(hasher.finish())
}

/// Utility function for providing a default boolean value to serde.
#[inline(always)]
fn return_true() -> bool {
	true
}

// TODO: Eventually remove this (probably starting late 2024)
fn migrate_layer_to_merge<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
	let mut s: String = serde::Deserialize::deserialize(deserializer)?;
	if s == "Layer" {
		s = "Merge".to_string();
	}
	Ok(s)
}

// TODO: Eventually remove this (probably starting late 2024)
#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
enum NodeInputVersions {
	OldNodeInput(OldNodeInput),
	NodeInput(NodeInput),
}

// TODO: Eventually remove this (probably starting late 2024)
#[derive(Debug, serde::Deserialize)]
pub enum OldNodeInput {
	/// A reference to another node in the same network from which this node can receive its input.
	Node { node_id: NodeId, output_index: usize, lambda: bool },

	/// A hardcoded value that can't change after the graph is compiled. Gets converted into a value node during graph compilation.
	Value { tagged_value: TaggedValue, exposed: bool },

	/// Input that is provided by the parent network to this document node, instead of from a hardcoded value or another node within the same network.
	Network(Type),

	/// A Rust source code string. Allows us to insert literal Rust code. Only used for GPU compilation.
	/// We can use this whenever we spin up Rustc. Sort of like inline assembly, but because our language is Rust, it acts as inline Rust.
	Inline(InlineRust),
}

// TODO: Eventually remove this (probably starting late 2024)
use serde::Deserialize;
fn deserialize_inputs<'de, D>(deserializer: D) -> Result<Vec<NodeInput>, D::Error>
where
	D: serde::Deserializer<'de>,
{
	let input_versions = Vec::<NodeInputVersions>::deserialize(deserializer)?;

	let inputs = input_versions
		.into_iter()
		.map(|old_input| {
			let old_input = match old_input {
				NodeInputVersions::OldNodeInput(old_input) => old_input,
				NodeInputVersions::NodeInput(node_input) => return node_input,
			};
			match old_input {
				OldNodeInput::Node { node_id, output_index, .. } => NodeInput::node(node_id, output_index),
				OldNodeInput::Value { tagged_value, exposed } => NodeInput::value(tagged_value, exposed),
				OldNodeInput::Network(network_type) => NodeInput::network(network_type, 0),
				OldNodeInput::Inline(inline) => NodeInput::Inline(inline),
			}
		})
		.collect();

	Ok(inputs)
}

/// An instance of a [`DocumentNodeDefinition`] that has been instantiated in a [`NodeNetwork`].
/// Currently, when an instance is made, it lives all on its own without any lasting connection to the definition.
/// But we will want to change it in the future so it merely references its definition.
#[derive(Clone, Debug, PartialEq, Hash, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DocumentNode {
	/// The inputs to a node, which are either:
	/// - From other nodes within this graph [`NodeInput::Node`],
	/// - A constant value [`NodeInput::Value`],
	/// - A [`NodeInput::Network`] which specifies that this input is from outside the graph, which is resolved in the graph flattening step in the case of nested networks.
	///
	/// In the root network, it is resolved when evaluating the borrow tree.
	/// Ensure the click target in the encapsulating network is updated when the inputs cause the node shape to change (currently only when exposing/hiding an input) by using network.update_click_target(node_id).
	#[serde(deserialize_with = "deserialize_inputs")]
	pub inputs: Vec<NodeInput>,
	/// Manual composition is a way to override the default composition flow of one node into another.
	///
	/// Through the usual node composition flow, the upstream node providing the primary input for a node is evaluated before the node itself is run.
	/// - Abstract example: upstream node `G` is evaluated and its data feeds into the primary input of downstream node `F`,
	///   just like function composition where function `G` is evaluated and its result is fed into function `F`.
	/// - Concrete example: a node that takes an image as primary input will get that image data from an upstream node that produces image output data and is evaluated first before being fed downstream.
	///
	/// This is achieved by automatically inserting `ComposeNode`s, which run the first node with the overall input and then feed the resulting output into the second node.
	/// The `ComposeNode` is basically a function composition operator: the parentheses in `F(G(x))` or circle math operator in `(F ∘ G)(x)`.
	/// For flexibility, instead of being a language construct, Graphene splits out composition itself as its own low-level node so that behavior can be overridden.
	/// The `ComposeNode`s are then inserted during the graph rewriting step for nodes that don't opt out with `manual_composition`.
	/// Instead of node `G` feeding into node `F` feeding as the result back to the caller,
	/// the graph is rewritten so nodes `G` and `F` both feed as lambdas into the parameters of a `ComposeNode` which calls `F(G(input))` and returns the result to the caller.
	///
	/// A node's manual composition input represents an input that is not resolved through graph rewriting with a `ComposeNode`,
	/// and is instead just passed in when evaluating this node within the borrow tree.
	/// This is similar to having the first input be a `NodeInput::Network` after the graph flattening.
	///
	/// ## Example Use Case: CacheNode
	///
	/// The `CacheNode` is a pass-through node on cache miss, but on cache hit it needs to avoid evaluating the upstream node and instead just return the cached value.
	///
	/// First, let's consider what that would look like using the default composition flow if the `CacheNode` instead just always acted as a pass-through (akin to a cache that always misses):
	///
	/// ```text
	/// ┌───────────────┐    ┌───────────────┐    ┌───────────────┐
	/// │               │◄───┤               │◄───┤               │◄─── EVAL (START)
	/// │       G       │    │PassThroughNode│    │       F       │
	/// │               ├───►│               ├───►│               │───► RESULT (END)
	/// └───────────────┘    └───────────────┘    └───────────────┘
	/// ```
	///
	/// This acts like the function call `F(PassThroughNode(G(input)))` when evaluating `F` with some `input`: `F.eval(input)`.
	/// - The diagram's upper track of arrows represents the flow of building up the call stack:
	///   since `F` is the output it is encountered first but deferred to its upstream caller `PassThroughNode` and that is once again deferred to its upstream caller `G`.
	/// - The diagram's lower track of arrows represents the flow of evaluating the call stack:
	///   `G` is evaluated first, then `PassThroughNode` is evaluated with the result of `G`, and finally `F` is evaluated with the result of `PassThroughNode`.
	///
	/// With the default composition flow (no manual composition), `ComposeNode`s would be automatically inserted during the graph rewriting step like this:
	///
	/// ```text
	///                                           ┌───────────────┐
	///                                           │               │◄─── EVAL (START)
	///                                           │  ComposeNode  │
	///                      ┌───────────────┐    │               ├───► RESULT (END)
	///                      │               │◄─┐ ├───────────────┤
	///                      │       G       │  └─┤               │
	///                      │               ├─┐  │     First     │
	///                      └───────────────┘ └─►│               │
	///                      ┌───────────────┐    ├───────────────┤
	///                      │               │◄───┤               │
	///                      │  ComposeNode  │    │     Second    │
	/// ┌───────────────┐    │               ├───►│               │
	/// │               │◄─┐ ├───────────────┤    └───────────────┘
	/// │PassThroughNode│  └─┤               │
	/// │               ├─┐  │     First     │
	/// └───────────────┘ └─►│               │
	/// ┌───────────────┐    ├───────────────┤
	/// |               │◄───┤               │
	/// │       F       │    │     Second    │
	/// │               ├───►│               │
	/// └───────────────┘    └───────────────┘
	/// ```
	///
	/// Now let's swap back from the `PassThroughNode` to the `CacheNode` to make caching actually work.
	/// It needs to override the default composition flow so that `G` is not automatically evaluated when the cache is hit.
	/// We need to give the `CacheNode` more manual control over the order of execution.
	/// So the `CacheNode` opts into manual composition and, instead of deferring to its upstream caller, it consumes the input directly:
	///
	/// ```text
	///                      ┌───────────────┐    ┌───────────────┐
	///                      │               │◄───┤               │◄─── EVAL (START)
	///                      │   CacheNode   │    │       F       │
	///                      │               ├───►│               │───► RESULT (END)
	/// ┌───────────────┐    ├───────────────┤    └───────────────┘
	/// │               │◄───┤               │
	/// │       G       │    │  Cached Data  │
	/// │               ├───►│               │
	/// └───────────────┘    └───────────────┘
	/// ```
	///
	/// Now, the call from `F` directly reaches the `CacheNode` and the `CacheNode` can decide whether to call `G.eval(input_from_f)`
	/// in the event of a cache miss or just return the cached data in the event of a cache hit.
	pub manual_composition: Option<Type>,
	// A nested document network or a proto-node identifier.
	pub implementation: DocumentNodeImplementation,
	/// Represents the eye icon for hiding/showing the node in the graph UI. When hidden, a node gets replaced with an identity node during the graph flattening step.
	#[serde(default = "return_true")]
	pub visible: bool,
	/// When two different proto nodes hash to the same value (e.g. two value nodes each containing `2_u32` or two multiply nodes that have the same node IDs as input), the duplicates are removed.
	/// See [`crate::proto::ProtoNetwork::generate_stable_node_ids`] for details.
	/// However sometimes this is not desirable, for example in the case of a [`graphene_core::memo::MonitorNode`] that needs to be accessed outside of the graph.
	#[serde(default)]
	pub skip_deduplication: bool,
	/// Used as a hash of the graph input where applicable. This ensures that proto nodes that depend on the graph's input are always regenerated.
	#[serde(default)]
	pub world_state_hash: u64,
	/// The path to this node and its inputs and outputs as of when [`NodeNetwork::generate_node_paths`] was called.
	#[serde(skip)]
	pub original_location: OriginalLocation,
}

/// Represents the original location of a node input/output when [`NodeNetwork::generate_node_paths`] was called, allowing the types and errors to be derived.
#[derive(Clone, Debug, PartialEq, Eq, Hash, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Source {
	pub node: Vec<NodeId>,
	pub index: usize,
}

/// The path to this node and its inputs and outputs as of when [`NodeNetwork::generate_node_paths`] was called.
#[derive(Clone, Debug, PartialEq, Eq, DynAny, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OriginalLocation {
	/// The original location to the document node - e.g. [grandparent_id, parent_id, node_id].
	pub path: Option<Vec<NodeId>>,
	/// Each document input source maps to one proto node input (however one proto node input may come from several sources)
	pub inputs_source: HashMap<Source, usize>,
	/// A list of document sources for the node's output
	pub outputs_source: HashMap<Source, usize>,
	pub inputs_exposed: Vec<bool>,
	/// Skipping inputs is useful for the manual composition thing - whereby a hidden `Footprint` input is added as the first input.
	pub skip_inputs: usize,
}

impl Default for DocumentNode {
	fn default() -> Self {
		Self {
			inputs: Default::default(),
			manual_composition: Default::default(),
			implementation: Default::default(),
			visible: true,
			skip_deduplication: Default::default(),
			world_state_hash: Default::default(),
			original_location: OriginalLocation::default(),
		}
	}
}

impl Hash for OriginalLocation {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.path.hash(state);
		self.inputs_source.iter().for_each(|val| val.hash(state));
		self.outputs_source.iter().for_each(|val| val.hash(state));
		self.inputs_exposed.hash(state);
		self.skip_inputs.hash(state);
	}
}
impl OriginalLocation {
	pub fn inputs(&self, index: usize) -> impl Iterator<Item = Source> + '_ {
		[(index >= self.skip_inputs).then(|| Source {
			node: self.path.clone().unwrap_or_default(),
			index: self.inputs_exposed.iter().take(index - self.skip_inputs).filter(|&&exposed| exposed).count(),
		})]
		.into_iter()
		.flatten()
		.chain(self.inputs_source.iter().filter(move |x| *x.1 == index).map(|(source, _)| source.clone()))
	}
	pub fn outputs(&self, index: usize) -> impl Iterator<Item = Source> + '_ {
		[Source {
			node: self.path.clone().unwrap_or_default(),
			index,
		}]
		.into_iter()
		.chain(self.outputs_source.iter().filter(move |x| *x.1 == index).map(|(source, _)| source.clone()))
	}
}
impl DocumentNode {
	/// Locate the input that is a [`NodeInput::Network`] at index `offset` and replace it with a [`NodeInput::Node`].
	pub fn populate_first_network_input(&mut self, node_id: NodeId, output_index: usize, offset: usize, lambda: bool, source: impl Iterator<Item = Source>, skip: usize) {
		let (index, _) = self
			.inputs
			.iter()
			.enumerate()
			.nth(offset)
			.unwrap_or_else(|| panic!("no network input found for {self:#?} and offset: {offset}"));

		self.inputs[index] = NodeInput::Node { node_id, output_index, lambda };
		let input_source = &mut self.original_location.inputs_source;
		for source in source {
			input_source.insert(source, index + self.original_location.skip_inputs - skip);
		}
	}

	fn resolve_proto_node(mut self) -> ProtoNode {
		assert!(!self.inputs.is_empty() || self.manual_composition.is_some(), "Resolving document node {self:#?} with no inputs");
		let DocumentNodeImplementation::ProtoNode(fqn) = self.implementation else {
			unreachable!("tried to resolve not flattened node on resolved node {self:?}");
		};
		let (input, mut args) = if let Some(ty) = self.manual_composition {
			(ProtoNodeInput::ManualComposition(ty), ConstructionArgs::Nodes(vec![]))
		} else {
			let first = self.inputs.remove(0);
			match first {
				NodeInput::Value { tagged_value, .. } => {
					assert_eq!(self.inputs.len(), 0, "A value node cannot have any inputs. Current inputs: {:?}", self.inputs);
					(ProtoNodeInput::None, ConstructionArgs::Value(tagged_value))
				}
				NodeInput::Node { node_id, output_index, lambda } => {
					assert_eq!(output_index, 0, "Outputs should be flattened before converting to proto node");
					let node = if lambda { ProtoNodeInput::NodeLambda(node_id) } else { ProtoNodeInput::Node(node_id) };
					(node, ConstructionArgs::Nodes(vec![]))
				}
				NodeInput::Network { import_type, .. } => (ProtoNodeInput::ManualComposition(import_type), ConstructionArgs::Nodes(vec![])),
				NodeInput::Inline(inline) => (ProtoNodeInput::None, ConstructionArgs::Inline(inline)),
			}
		};
		assert!(!self.inputs.iter().any(|input| matches!(input, NodeInput::Network { .. })), "received non resolved parameter");
		assert!(
			!self.inputs.iter().any(|input| matches!(input, NodeInput::Value { .. })),
			"received value as parameter. inputs: {:#?}, construction_args: {:#?}",
			self.inputs,
			args
		);

		// If we have one parameter of the type inline, set it as the construction args
		if let &[NodeInput::Inline(ref inline)] = self.inputs.as_slice() {
			args = ConstructionArgs::Inline(inline.clone());
		}
		if let ConstructionArgs::Nodes(nodes) = &mut args {
			nodes.extend(self.inputs.iter().map(|input| match input {
				NodeInput::Node { node_id, lambda, .. } => (*node_id, *lambda),
				_ => unreachable!(),
			}));
		}
		ProtoNode {
			identifier: fqn,
			input,
			construction_args: args,
			original_location: self.original_location,
			skip_deduplication: self.skip_deduplication,
			world_state_hash: self.world_state_hash,
		}
	}
}

/// Represents the possible inputs to a node.
#[derive(Debug, Clone, PartialEq, Hash, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum NodeInput {
	/// A reference to another node in the same network from which this node can receive its input.
	Node { node_id: NodeId, output_index: usize, lambda: bool },

	/// A hardcoded value that can't change after the graph is compiled. Gets converted into a value node during graph compilation.
	Value { tagged_value: TaggedValue, exposed: bool },

	// TODO: Remove import_type and get type from parent node input
	/// Input that is provided by the parent network to this document node, instead of from a hardcoded value or another node within the same network.
	Network { import_type: Type, import_index: usize },

	/// A Rust source code string. Allows us to insert literal Rust code. Only used for GPU compilation.
	/// We can use this whenever we spin up Rustc. Sort of like inline assembly, but because our language is Rust, it acts as inline Rust.
	Inline(InlineRust),
}

#[derive(Debug, Clone, PartialEq, Hash, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InlineRust {
	pub expr: String,
	pub ty: Type,
}

impl InlineRust {
	pub fn new(expr: String, ty: Type) -> Self {
		Self { expr, ty }
	}
}

impl NodeInput {
	pub const fn node(node_id: NodeId, output_index: usize) -> Self {
		Self::Node { node_id, output_index, lambda: false }
	}
	pub const fn lambda(node_id: NodeId, output_index: usize) -> Self {
		Self::Node { node_id, output_index, lambda: true }
	}
	pub const fn value(tagged_value: TaggedValue, exposed: bool) -> Self {
		Self::Value { tagged_value, exposed }
	}
	pub const fn network(import_type: Type, import_index: usize) -> Self {
		Self::Network { import_type, import_index }
	}
	fn map_ids(&mut self, f: impl Fn(NodeId) -> NodeId) {
		if let &mut NodeInput::Node { node_id, output_index, lambda } = self {
			*self = NodeInput::Node {
				node_id: f(node_id),
				output_index,
				lambda,
			}
		}
	}
	pub fn is_exposed(&self) -> bool {
		match self {
			NodeInput::Node { .. } => true,
			NodeInput::Value { exposed, .. } => *exposed,
			NodeInput::Network { .. } => true,
			NodeInput::Inline(_) => false,
		}
	}
	/// Network node inputs in the document network are not displayed, but still exist in the compiled network
	pub fn is_exposed_to_frontend(&self, is_document_network: bool) -> bool {
		match self {
			NodeInput::Node { .. } => true,
			NodeInput::Value { exposed, .. } => *exposed,
			NodeInput::Network { .. } => !is_document_network,
			NodeInput::Inline(_) => false,
		}
	}
	pub fn ty(&self) -> Type {
		match self {
			NodeInput::Node { .. } => unreachable!("ty() called on NodeInput::Node"),
			NodeInput::Value { tagged_value, .. } => tagged_value.ty(),
			NodeInput::Network { import_type, .. } => import_type.clone(),
			NodeInput::Inline(_) => panic!("ty() called on NodeInput::Inline"),
		}
	}
	pub fn as_value(&self) -> Option<&TaggedValue> {
		if let NodeInput::Value { tagged_value, .. } = self {
			Some(tagged_value)
		} else {
			None
		}
	}
	pub fn as_node(&self) -> Option<NodeId> {
		if let NodeInput::Node { node_id, .. } = self {
			Some(*node_id)
		} else {
			None
		}
	}
}

#[derive(Clone, Debug, PartialEq, Hash, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Represents the implementation of a node, which can be a nested [`NodeNetwork`], a proto [`ProtoNodeIdentifier`], or `Extract`.
pub enum DocumentNodeImplementation {
	/// This describes a (document) node built out of a subgraph of other (document) nodes.
	///
	/// A nested [`NodeNetwork`] that is flattened by the [`NodeNetwork::flatten`] function.
	Network(NodeNetwork),
	/// This describes a (document) node implemented as a proto node.
	///
	/// A proto node identifier which can be found in `node_registry.rs`.
	#[serde(alias = "Unresolved")] // TODO: Eventually remove this alias (probably starting late 2024)
	ProtoNode(ProtoNodeIdentifier),
	/// The Extract variant is a tag which tells the compilation process to do something special. It invokes language-level functionality built for use by the ExtractNode to enable metaprogramming.
	/// When the ExtractNode is compiled, it gets replaced by a value node containing a representation of the source code for the function/lambda of the document node that's fed into the ExtractNode
	/// (but only that one document node, not upstream nodes).
	///
	/// This is explained in more detail here: <https://www.youtube.com/watch?v=72KJa3jQClo>
	///
	/// Currently we use it for GPU execution, where a node has to get "extracted" to its source code representation and stored as a value that can be given to the GpuCompiler node at runtime
	/// (to become a compute shader). Future use could involve the addition of an InjectNode to convert the source code form back into an executable node, enabling metaprogramming in the node graph.
	/// We would use an assortment of nodes that operate on Graphene source code (just data, no different from any other data flowing through the graph) to make graph transformations.
	///
	/// We use this for dealing with macros in a syntactic way of modifying the node graph from within the graph itself. Just like we often deal with lambdas to represent a whole group of
	/// operations/code/logic, this allows us to basically deal with a lambda at a meta/source-code level, because we need to pass the GPU SPIR-V compiler the source code for a lambda,
	/// not the executable logic of a lambda.
	///
	/// This is analogous to how Rust macros operate at the level of source code, not executable code. When we speak of source code, that represents Graphene's source code in the form of a
	/// DocumentNode network, not the text form of Rust's source code. (Analogous to the token stream/AST of a Rust macro.)
	///
	/// `DocumentNode`s with a `DocumentNodeImplementation::Extract` are converted into a `ClonedNode` that returns the `DocumentNode` specified by the single `NodeInput::Node`. The referenced node
	/// (specified by the single `NodeInput::Node`) is removed from the network, and any `NodeInput::Node`s used by the referenced node are replaced with a generically typed network input.
	Extract,
}

impl Default for DocumentNodeImplementation {
	fn default() -> Self {
		Self::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IdentityNode"))
	}
}

impl DocumentNodeImplementation {
	pub fn get_network(&self) -> Option<&NodeNetwork> {
		match self {
			DocumentNodeImplementation::Network(n) => Some(n),
			_ => None,
		}
	}

	pub fn get_network_mut(&mut self) -> Option<&mut NodeNetwork> {
		match self {
			DocumentNodeImplementation::Network(n) => Some(n),
			_ => None,
		}
	}

	pub const fn proto(name: &'static str) -> Self {
		Self::ProtoNode(ProtoNodeIdentifier::new(name))
	}
}

// TODO: Eventually remove this (probably starting late 2024)
#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
pub enum NodeExportVersions {
	OldNodeInput(NodeOutput),
	NodeInput(NodeInput),
}

// TODO: Eventually remove this (probably starting late 2024)
#[derive(Debug, serde::Deserialize)]
pub struct NodeOutput {
	pub node_id: NodeId,
	pub node_output_index: usize,
}

// TODO: Eventually remove this (probably starting late 2024)
fn deserialize_exports<'de, D>(deserializer: D) -> Result<Vec<NodeInput>, D::Error>
where
	D: serde::Deserializer<'de>,
{
	let node_input_versions = Vec::<NodeExportVersions>::deserialize(deserializer)?;

	// Convert Vec<NodeOutput> to Vec<NodeInput>
	let inputs = node_input_versions
		.into_iter()
		.map(|node_input_version| {
			let node_output = match node_input_version {
				NodeExportVersions::OldNodeInput(node_output) => node_output,
				NodeExportVersions::NodeInput(node_input) => return node_input,
			};
			NodeInput::node(node_output.node_id, node_output.node_output_index)
		})
		.collect();

	Ok(inputs)
}

#[derive(Clone, Debug, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A network (subgraph) of nodes containing each [`DocumentNode`] and its ID, as well as list mapping each export to its connected node, or a value if disconnected
pub struct NodeNetwork {
	/// The list of data outputs that are exported from this network to the parent network.
	/// Each export is a reference to a node within this network, paired with its output index, that is the source of the network's exported data.
	#[serde(alias = "outputs", deserialize_with = "deserialize_exports")] // TODO: Eventually remove this alias (probably starting late 2024)
	pub exports: Vec<NodeInput>,
	/// TODO: Instead of storing import types in each NodeInput::Network connection, the types are stored here. This is similar to how types need to be defined for parameters when creating a function in Rust.
	// pub import_types: Vec<Type>,
	/// The list of all nodes in this network.
	pub nodes: HashMap<NodeId, DocumentNode>,
}

impl std::hash::Hash for NodeNetwork {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.exports.hash(state);
		let mut nodes: Vec<_> = self.nodes.iter().collect();
		nodes.sort_by_key(|(id, _)| *id);
		for (id, node) in nodes {
			id.hash(state);
			node.hash(state);
		}
	}
}
impl Default for NodeNetwork {
	fn default() -> Self {
		NodeNetwork {
			exports: Default::default(),
			//import_types: Default::default(),
			nodes: Default::default(),
		}
	}
}
impl PartialEq for NodeNetwork {
	fn eq(&self, other: &Self) -> bool {
		self.exports == other.exports
	}
}

/// Graph modification functions
impl NodeNetwork {
	pub fn current_hash(&self) -> u64 {
		let mut hasher = DefaultHasher::new();
		self.hash(&mut hasher);
		hasher.finish()
	}

	pub fn value_network(node: DocumentNode) -> Self {
		Self {
			exports: vec![NodeInput::node(NodeId(0), 0)],
			nodes: [(NodeId(0), node)].into_iter().collect(),
			..Default::default()
		}
	}

	/// A graph with just an input node
	// pub fn new_network() -> Self {
	// 	Self {
	// 		exports: vec![NodeInput::node(NodeId(0), 0)],
	// 		nodes: [(
	// 			NodeId(0),
	// 			DocumentNode {
	// 				name: "Input Frame".into(),
	// 				manual_composition: Some(concrete!(u32)),
	// 				implementation: DocumentNodeImplementation::ProtoNode("graphene_core::ops::IdentityNode".into()),
	// 				metadata: DocumentNodeMetadata { position: (8, 4).into() },
	// 				..Default::default()
	// 			},
	// 		)]
	// 		.into_iter()
	// 		.collect(),
	// 		..Default::default()
	// 	}
	// }

	/// Appends a new node to the network after the output node and sets it as the new output
	// pub fn push_node_to_document_network(&mut self, mut node: DocumentNode) -> NodeId {
	// 	let id = NodeId(self.nodes.len().try_into().expect("Too many nodes in network"));
	// 	// Set the correct position for the new node
	// 	if node.metadata().position == IVec2::default() {
	// 		if let Some(pos) = self.get_root_node().and_then(|root_node| self.nodes.get(&root_node.id)).map(|n| n.metadata().position) {
	// 			node.metadata().position = pos + IVec2::new(8, 0);
	// 		}
	// 	}
	// 	if !self.exports.is_empty() {
	// 		let input = self.exports[0].clone();
	// 		if node.inputs.is_empty() {
	// 			node.inputs.push(input);
	// 		} else {
	// 			node.inputs[0] = input;
	// 		}
	// 	}
	//  // Use node_graph.insert_node
	// 	self.insert_node(id, node);
	// 	self.exports = vec![NodeInput::node(id, 0)];
	// 	id
	// }

	/// Get the nested network given by the path of node ids
	pub fn nested_network(&self, nested_path: &[NodeId]) -> Option<&Self> {
		let mut network = Some(self);

		for segment in nested_path {
			network = network.and_then(|network| network.nodes.get(segment)).and_then(|node| node.implementation.get_network());
		}
		network
	}

	/// Get the mutable nested network given by the path of node ids
	pub fn nested_network_mut(&mut self, nested_path: &[NodeId]) -> Option<&mut Self> {
		let mut network = Some(self);

		for segment in nested_path {
			network = network.and_then(|network| network.nodes.get_mut(segment)).and_then(|node| node.implementation.get_network_mut());
		}
		network
	}

	/// Is the node being used directly as an output?
	pub fn outputs_contain(&self, node_id_to_check: NodeId) -> bool {
		self.exports
			.iter()
			.any(|output| if let NodeInput::Node { node_id, .. } = output { *node_id == node_id_to_check } else { false })
	}

	/// Check there are no cycles in the graph (this should never happen).
	pub fn is_acyclic(&self) -> bool {
		let mut dependencies: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
		for (node_id, node) in &self.nodes {
			dependencies.insert(
				*node_id,
				node.inputs
					.iter()
					.filter_map(|input| if let NodeInput::Node { node_id, .. } = input { Some(*node_id) } else { None })
					.collect(),
			);
		}
		while !dependencies.is_empty() {
			let Some((&disconnected, _)) = dependencies.iter().find(|(_, l)| l.is_empty()) else {
				error!("Dependencies {dependencies:?}");
				return false;
			};
			dependencies.remove(&disconnected);
			for connections in dependencies.values_mut() {
				connections.retain(|&id| id != disconnected);
			}
		}
		true
	}
}

/// Functions for compiling the network
impl NodeNetwork {
	/// Replace all references in the graph of a node ID with a new node ID defined by the function `f`.
	pub fn map_ids(&mut self, f: impl Fn(NodeId) -> NodeId + Copy) {
		self.exports.iter_mut().for_each(|output| {
			if let NodeInput::Node { node_id, .. } = output {
				*node_id = f(*node_id)
			}
		});
		let nodes = std::mem::take(&mut self.nodes);
		self.nodes = nodes
			.into_iter()
			.map(|(id, mut node)| {
				node.inputs.iter_mut().for_each(|input| input.map_ids(f));
				(f(id), node)
			})
			.collect();
	}

	/// Populate the [`DocumentNode::path`], which stores the location of the document node to allow for matching the resulting proto nodes to the document node for the purposes of typing and finding monitor nodes.
	pub fn generate_node_paths(&mut self, prefix: &[NodeId]) {
		for (node_id, node) in &mut self.nodes {
			let mut new_path = prefix.to_vec();
			new_path.push(*node_id);
			if let DocumentNodeImplementation::Network(network) = &mut node.implementation {
				network.generate_node_paths(new_path.as_slice());
			}
			if node.original_location.path.is_some() {
				log::warn!("Attempting to overwrite node path");
			} else {
				node.original_location = OriginalLocation {
					path: Some(new_path),
					inputs_exposed: node.inputs.iter().map(|input| input.is_exposed()).collect(),
					skip_inputs: if node.manual_composition.is_some() { 1 } else { 0 },
					..Default::default()
				}
			}
		}
	}

	/// Replace all references in any node of `old_input` with `new_input`
	fn replace_node_inputs(&mut self, old_input: NodeInput, new_input: NodeInput) {
		for node in self.nodes.values_mut() {
			node.inputs.iter_mut().for_each(|input| {
				if *input == old_input {
					*input = new_input.clone();
				}
			});
		}
	}

	/// Replace all references in any node of `old_output` with `new_output`
	fn replace_network_outputs(&mut self, old_output: NodeInput, new_output: NodeInput) {
		for output in self.exports.iter_mut() {
			if *output == old_output {
				*output = new_output.clone();
			}
		}
	}

	/// Removes unused nodes from the graph. Returns a list of booleans which represent if each of the inputs have been retained.
	pub fn remove_dead_nodes(&mut self, number_of_inputs: usize) -> Vec<bool> {
		// Take all the nodes out of the nodes list
		let mut old_nodes = std::mem::take(&mut self.nodes);

		let mut stack = self
			.exports
			.iter()
			.filter_map(|output| if let NodeInput::Node { node_id, .. } = output { Some(*node_id) } else { None })
			.collect::<Vec<_>>();
		while let Some(node_id) = stack.pop() {
			let Some((node_id, mut document_node)) = old_nodes.remove_entry(&node_id) else {
				continue;
			};
			// Remove dead nodes from child networks
			if let DocumentNodeImplementation::Network(network) = &mut document_node.implementation {
				// Remove inputs to the parent node if they have been removed from the child
				let mut retain_inputs = network.remove_dead_nodes(document_node.inputs.len()).into_iter();
				document_node.inputs.retain(|_| retain_inputs.next().unwrap_or(true))
			}
			// Visit all nodes that this node references
			stack.extend(
				document_node
					.inputs
					.iter()
					.filter_map(|input| if let NodeInput::Node { node_id, .. } = input { Some(node_id) } else { None }),
			);
			// Add the node back to the list of nodes
			self.nodes.insert(node_id, document_node);
		}

		// Check if inputs are used and store for return value
		let mut are_inputs_used = vec![false; number_of_inputs];
		for node in &self.nodes {
			for node_input in &node.1.inputs {
				if let NodeInput::Network { import_index, .. } = node_input {
					if let Some(is_used) = are_inputs_used.get_mut(*import_index) {
						*is_used = true;
					}
				}
			}
		}
		are_inputs_used
	}

	/// Remove all nodes that contain [`DocumentNodeImplementation::Network`] by moving the nested nodes into the parent network.
	pub fn flatten(&mut self, node_id: NodeId) {
		self.flatten_with_fns(node_id, merge_ids, || NodeId(generate_uuid()))
	}

	/// Remove all nodes that contain [`DocumentNodeImplementation::Network`] by moving the nested nodes into the parent network.
	pub fn flatten_with_fns(&mut self, node_id: NodeId, map_ids: impl Fn(NodeId, NodeId) -> NodeId + Copy, gen_id: impl Fn() -> NodeId + Copy) {
		let Some((id, mut node)) = self.nodes.remove_entry(&node_id) else {
			warn!("The node which was supposed to be flattened does not exist in the network, id {node_id} network {self:#?}");
			return;
		};

		// If the node is hidden, replace it with an identity node
		let identity_node = DocumentNodeImplementation::ProtoNode("graphene_core::ops::IdentityNode".into());
		if !node.visible && node.implementation != identity_node {
			node.implementation = identity_node;

			// Connect layer node to the graphic group below
			node.inputs.drain(1..);
			self.nodes.insert(id, node);

			return;
		}

		// Skip nodes that are already value nodes
		if node.implementation != DocumentNodeImplementation::ProtoNode("graphene_core::value::ClonedNode".into()) {
			// Replace value exports with value nodes, added inside nested network
			if let DocumentNodeImplementation::Network(nested_network) = &mut node.implementation {
				for export in nested_network.exports.iter_mut() {
					let previous_export = std::mem::replace(export, NodeInput::network(concrete!(()), 0));
					if let NodeInput::Value { tagged_value, exposed } = previous_export {
						let value_node_id = gen_id();
						let merged_node_id = map_ids(id, value_node_id);
						let mut original_location = node.original_location.clone();
						if let Some(path) = &mut original_location.path {
							path.push(value_node_id);
						}
						nested_network.nodes.insert(
							merged_node_id,
							DocumentNode {
								inputs: vec![NodeInput::Value { tagged_value, exposed }],
								implementation: DocumentNodeImplementation::ProtoNode("graphene_core::value::ClonedNode".into()),
								original_location,
								..Default::default()
							},
						);
						*export = NodeInput::Node {
							node_id: merged_node_id,
							output_index: 0,
							lambda: false,
						};
					} else {
						*export = previous_export;
					}
				}
			}

			// Replace value inputs with value nodes, added to flattened network
			for input in node.inputs.iter_mut() {
				let previous_input = std::mem::replace(input, NodeInput::network(concrete!(()), 0));
				if let NodeInput::Value { tagged_value, exposed } = previous_input {
					let value_node_id = gen_id();
					let merged_node_id = map_ids(id, value_node_id);
					let mut original_location = node.original_location.clone();
					if let Some(path) = &mut original_location.path {
						path.push(value_node_id);
					}
					self.nodes.insert(
						merged_node_id,
						DocumentNode {
							inputs: vec![NodeInput::Value { tagged_value, exposed }],
							implementation: DocumentNodeImplementation::ProtoNode("graphene_core::value::ClonedNode".into()),
							original_location,
							..Default::default()
						},
					);
					*input = NodeInput::Node {
						node_id: merged_node_id,
						output_index: 0,
						lambda: false,
					};
				} else {
					*input = previous_input;
				}
			}
		}

		if let DocumentNodeImplementation::Network(mut inner_network) = node.implementation {
			// Connect all network inputs to either the parent network nodes, or newly created value nodes for the parent node.
			inner_network.map_ids(|inner_id| map_ids(id, inner_id));
			let new_nodes = inner_network.nodes.keys().cloned().collect::<Vec<_>>();

			// Match the document node input and the inputs of the inner network
			for (nested_node_id, mut nested_node) in inner_network.nodes.into_iter() {
				for (nested_input_index, nested_input) in nested_node.clone().inputs.iter().enumerate() {
					if let NodeInput::Network { import_index, .. } = nested_input {
						let parent_input = node.inputs.get(*import_index).expect(&format!("Import index {} should always exist", import_index));
						match *parent_input {
							// If the input to self is a node, connect the corresponding output of the inner network to it
							NodeInput::Node { node_id, output_index, lambda } => {
								let skip = node.original_location.skip_inputs;
								nested_node.populate_first_network_input(node_id, output_index, nested_input_index, lambda, node.original_location.inputs(*import_index), skip);
							}
							NodeInput::Network { import_index, .. } => {
								let parent_input_index = import_index;
								let Some(NodeInput::Network { import_index, .. }) = nested_node.inputs.get_mut(nested_input_index) else {
									log::error!("Nested node should have a network input");
									continue;
								};
								*import_index = parent_input_index;
							}
							NodeInput::Value { .. } => unreachable!("Value inputs should have been replaced with value nodes"),
							NodeInput::Inline(_) => (),
						}
					}
				}
				self.nodes.insert(nested_node_id, nested_node);
			}
			// TODO: Add support for flattening exports that are NodeInput::Network (https://github.com/GraphiteEditor/Graphite/issues/1762)
			// Match the document node input and the exports of the inner network if the export is a NodeInput::Network
			// for (i, export) in inner_network.exports.iter().enumerate() {
			// 	if let NodeInput::Network { import_index, .. } = export {
			// 		let parent_input = node.inputs.get(*import_index).expect(&format!("Import index {} should always exist", import_index));
			// 		match *parent_input {
			// 			// If the input to self is a node, connect the corresponding output of the inner network to it
			// 			NodeInput::Node { node_id, output_index, lambda } => {
			// 				inner_network.populate_first_network_export(&mut node, node_id, output_index, lambda, i, node.original_location.outputs(i), 0);
			// 			}
			// 			NodeInput::Network { import_index, .. } => {
			// 				let parent_input_index = import_index;
			// 				let Some(NodeInput::Network { import_index, .. }) = inner_network.exports.get_mut(i) else {
			// 					log::error!("Nested node should have a network input");
			// 					continue;
			// 				};
			// 				*import_index = parent_input_index;
			// 			}
			// 			NodeInput::Value { .. } => unreachable!("Value inputs should have been replaced with value nodes"),
			// 			NodeInput::Inline(_) => (),
			// 		}
			// 	}
			// }

			// Connect all nodes that were previously connected to this node to the nodes of the inner network
			for (i, export) in inner_network.exports.into_iter().enumerate() {
				let node_input = |node_id, output_index, lambda| NodeInput::Node { node_id, output_index, lambda };

				if let NodeInput::Node { node_id, output_index, .. } = &export {
					self.replace_node_inputs(node_input(id, i, false), node_input(*node_id, *output_index, false));
					self.replace_node_inputs(node_input(id, i, true), node_input(*node_id, *output_index, true));
					if let Some(new_output_node) = self.nodes.get_mut(node_id) {
						for source in node.original_location.outputs(i) {
							new_output_node.original_location.outputs_source.insert(source, *output_index);
						}
					}
				}

				self.replace_network_outputs(NodeInput::node(id, i), export);
			}

			for node_id in new_nodes {
				self.flatten_with_fns(node_id, map_ids, gen_id);
			}
		} else {
			// If the node is not a network, it is a primitive node and can be inserted into the network as is.
			assert!(!self.nodes.contains_key(&id), "Trying to insert a node into the network caused an id conflict");
			self.nodes.insert(id, node);
		}
	}

	/// Locate the export that is a [`NodeInput::Network`] at index `offset` and replace it with a [`NodeInput::Node`].
	// fn populate_first_network_export(&mut self, node: &mut DocumentNode, node_id: NodeId, output_index: usize, lambda: bool, export_index: usize, source: impl Iterator<Item = Source>, skip: usize) {
	// 	self.exports[export_index] = NodeInput::Node { node_id, output_index, lambda };
	// 	let input_source = &mut node.original_location.inputs_source;
	// 	for source in source {
	// 		input_source.insert(source, output_index + node.original_location.skip_inputs - skip);
	// 	}
	// }

	fn remove_id_node(&mut self, id: NodeId) -> Result<(), String> {
		let node = self.nodes.get(&id).ok_or_else(|| format!("Node with id {id} does not exist"))?.clone();
		if let DocumentNodeImplementation::ProtoNode(ident) = &node.implementation {
			if ident.name == "graphene_core::ops::IdentityNode" {
				assert_eq!(node.inputs.len(), 1, "Id node has more than one input");
				if let NodeInput::Node { node_id, output_index, .. } = node.inputs[0] {
					let node_input_output_index = output_index;
					if let Some(input_node) = self.nodes.get_mut(&node_id) {
						for source in node.original_location.outputs(0) {
							input_node.original_location.outputs_source.insert(source, node_input_output_index);
						}
					}

					let input_node_id = node_id;
					for output in self.nodes.values_mut() {
						for (index, input) in output.inputs.iter_mut().enumerate() {
							if let NodeInput::Node {
								node_id: output_node_id,
								output_index: output_output_index,
								..
							} = input
							{
								if *output_node_id == id {
									*output_node_id = input_node_id;
									*output_output_index = node_input_output_index;

									let input_source = &mut output.original_location.inputs_source;
									for source in node.original_location.inputs(index) {
										input_source.insert(source, index + output.original_location.skip_inputs - node.original_location.skip_inputs);
									}
								}
							}
						}
						for node_input in self.exports.iter_mut() {
							if let NodeInput::Node {
								ref mut node_id,
								ref mut output_index,
								..
							} = node_input
							{
								if *node_id == id {
									*node_id = input_node_id;
									*output_index = node_input_output_index;
								}
							}
						}
					}
				}
				self.nodes.remove(&id);
			}
		}
		Ok(())
	}

	/// Strips out any [`graphene_core::ops::IdentityNode`]s that are unnecessary.
	pub fn remove_redundant_id_nodes(&mut self) {
		let id_nodes = self
			.nodes
			.iter()
			.filter(|(_, node)| {
				matches!(&node.implementation, DocumentNodeImplementation::ProtoNode(ident) if ident == &ProtoNodeIdentifier::new("graphene_core::ops::IdentityNode"))
					&& node.inputs.len() == 1
					&& matches!(node.inputs[0], NodeInput::Node { .. })
			})
			.map(|(id, _)| *id)
			.collect::<Vec<_>>();
		for id in id_nodes {
			if let Err(e) = self.remove_id_node(id) {
				log::warn!("{e}")
			}
		}
	}

	/// Converts the `DocumentNode`s with a `DocumentNodeImplementation::Extract` into a `ClonedNode` that returns
	/// the `DocumentNode` specified by the single `NodeInput::Node`.
	/// The referenced node is removed from the network, and any `NodeInput::Node`s used by the referenced node are replaced with a generically typed network input.
	pub fn resolve_extract_nodes(&mut self) {
		let mut extraction_nodes = self
			.nodes
			.iter()
			.filter(|(_, node)| matches!(node.implementation, DocumentNodeImplementation::Extract))
			.map(|(id, node)| (*id, node.clone()))
			.collect::<Vec<_>>();
		self.nodes.retain(|_, node| !matches!(node.implementation, DocumentNodeImplementation::Extract));

		for (_, node) in &mut extraction_nodes {
			assert_eq!(node.inputs.len(), 1);
			let NodeInput::Node { node_id, output_index, .. } = node.inputs.pop().unwrap() else {
				panic!("Extract node has no input, inputs: {:?}", node.inputs);
			};
			assert_eq!(output_index, 0);
			// TODO: check if we can read lambda checking?
			let mut input_node = self.nodes.remove(&node_id).unwrap();
			node.implementation = DocumentNodeImplementation::ProtoNode("graphene_core::value::ClonedNode".into());
			if let Some(input) = input_node.inputs.get_mut(0) {
				*input = match &input {
					NodeInput::Node { .. } => NodeInput::network(generic!(T), 0),
					ni => NodeInput::network(ni.ty(), 0),
				};
			}

			for input in input_node.inputs.iter_mut() {
				if let NodeInput::Node { .. } = input {
					*input = NodeInput::network(generic!(T), 0)
				}
			}
			node.inputs = vec![NodeInput::value(TaggedValue::DocumentNode(input_node), false)];
		}
		self.nodes.extend(extraction_nodes);
	}

	/// Creates a proto network for evaluating each output of this network.
	pub fn into_proto_networks(self) -> impl Iterator<Item = ProtoNetwork> {
		// let input_node = self.nodes.iter().find_map(|(node_id, node)| if node.name == "SetNode" { Some(node_id.clone()) } else { None });
		let mut nodes: Vec<_> = self.nodes.into_iter().map(|(id, node)| (id, node.resolve_proto_node())).collect();
		nodes.sort_unstable_by_key(|(i, _)| *i);

		// Create a network to evaluate each output
		self.exports.into_iter().filter_map(move |output| {
			if let NodeInput::Node { node_id, .. } = output {
				Some(ProtoNetwork {
					inputs: Vec::new(), // Inputs field is not used. Should be deleted
					// inputs: vec![input_node.expect("Set node should always exist")],
					// inputs: self.imports.clone(),
					output: node_id,
					nodes: nodes.clone(),
				})
			} else {
				None
			}
		})
	}

	/// Create a [`RecursiveNodeIter`] that iterates over all [`DocumentNode`]s, including ones that are deeply nested.
	pub fn recursive_nodes(&self) -> RecursiveNodeIter {
		let nodes = self.nodes.iter().collect();
		RecursiveNodeIter { nodes }
	}
}

/// An iterator over all [`DocumentNode`]s, including ones that are deeply nested.
pub struct RecursiveNodeIter<'a> {
	nodes: Vec<(&'a NodeId, &'a DocumentNode)>,
}

impl<'a> Iterator for RecursiveNodeIter<'a> {
	type Item = (&'a NodeId, &'a DocumentNode);
	fn next(&mut self) -> Option<Self::Item> {
		let node = self.nodes.pop()?;
		if let DocumentNodeImplementation::Network(network) = &node.1.implementation {
			self.nodes.extend(network.nodes.iter());
		}
		Some(node)
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::proto::{ConstructionArgs, ProtoNetwork, ProtoNode, ProtoNodeInput};

	use graphene_core::ProtoNodeIdentifier;

	use std::sync::atomic::AtomicU64;

	fn gen_node_id() -> NodeId {
		static NODE_ID: AtomicU64 = AtomicU64::new(4);
		NodeId(NODE_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst))
	}

	fn add_network() -> NodeNetwork {
		NodeNetwork {
			exports: vec![NodeInput::node(NodeId(1), 0)],
			nodes: [
				(
					NodeId(0),
					DocumentNode {
						inputs: vec![NodeInput::network(concrete!(u32), 0), NodeInput::network(concrete!(u32), 1)],
						implementation: DocumentNodeImplementation::ProtoNode("graphene_core::structural::ConsNode".into()),
						..Default::default()
					},
				),
				(
					NodeId(1),
					DocumentNode {
						inputs: vec![NodeInput::node(NodeId(0), 0)],
						implementation: DocumentNodeImplementation::ProtoNode("graphene_core::ops::AddPairNode".into()),
						..Default::default()
					},
				),
			]
			.into_iter()
			.collect(),
			..Default::default()
		}
	}

	#[test]
	fn map_ids() {
		let mut network = add_network();
		network.map_ids(|id| NodeId(id.0 + 1));
		let mut mapped_add = NodeNetwork {
			exports: vec![NodeInput::node(NodeId(2), 0)],
			nodes: [
				(
					NodeId(1),
					DocumentNode {
						inputs: vec![NodeInput::network(concrete!(u32), 0), NodeInput::network(concrete!(u32), 1)],
						implementation: DocumentNodeImplementation::ProtoNode("graphene_core::structural::ConsNode".into()),
						..Default::default()
					},
				),
				(
					NodeId(2),
					DocumentNode {
						inputs: vec![NodeInput::node(NodeId(1), 0)],
						implementation: DocumentNodeImplementation::ProtoNode("graphene_core::ops::AddPairNode".into()),
						..Default::default()
					},
				),
			]
			.into_iter()
			.collect(),
			..Default::default()
		};
		assert_eq!(network, mapped_add);
	}

	#[test]
	fn extract_node() {
		let id_node = DocumentNode {
			inputs: vec![],
			implementation: DocumentNodeImplementation::ProtoNode("graphene_core::ops::IdentityNode".into()),
			..Default::default()
		};
		// TODO: Extend test cases to test nested network
		let mut extraction_network = NodeNetwork {
			exports: vec![NodeInput::node(NodeId(1), 0)],
			nodes: [
				id_node.clone(),
				DocumentNode {
					inputs: vec![NodeInput::lambda(NodeId(0), 0)],
					implementation: DocumentNodeImplementation::Extract,
					..Default::default()
				},
			]
			.into_iter()
			.enumerate()
			.map(|(id, node)| (NodeId(id as u64), node))
			.collect(),
			..Default::default()
		};
		extraction_network.resolve_extract_nodes();
		assert_eq!(extraction_network.nodes.len(), 1);
		let inputs = extraction_network.nodes.get(&NodeId(1)).unwrap().inputs.clone();
		assert_eq!(inputs.len(), 1);
		assert!(matches!(&inputs[0], &NodeInput::Value{ tagged_value: TaggedValue::DocumentNode(ref network), ..} if network == &id_node));
	}

	#[test]
	fn flatten_add() {
		let mut network = NodeNetwork {
			exports: vec![NodeInput::node(NodeId(1), 0)],
			nodes: [(
				NodeId(1),
				DocumentNode {
					inputs: vec![
						NodeInput::network(concrete!(u32), 0),
						NodeInput::Value {
							tagged_value: TaggedValue::U32(2),
							exposed: false,
						},
					],
					implementation: DocumentNodeImplementation::Network(add_network()),
					..Default::default()
				},
			)]
			.into_iter()
			.collect(),
			..Default::default()
		};
		network.generate_node_paths(&[]);
		network.flatten_with_fns(NodeId(1), |self_id, inner_id| NodeId(self_id.0 * 10 + inner_id.0), gen_node_id);
		let mut flat_network = flat_network();
		println!("{flat_network:#?}");
		println!("{network:#?}");

		assert_eq!(flat_network, network);
	}

	#[test]
	fn resolve_proto_node_add() {
		let document_node = DocumentNode {
			inputs: vec![NodeInput::network(concrete!(u32), 0), NodeInput::node(NodeId(0), 0)],
			implementation: DocumentNodeImplementation::ProtoNode("graphene_core::structural::ConsNode".into()),
			..Default::default()
		};

		let proto_node = document_node.resolve_proto_node();
		let reference = ProtoNode {
			identifier: "graphene_core::structural::ConsNode".into(),
			input: ProtoNodeInput::ManualComposition(concrete!(u32)),
			construction_args: ConstructionArgs::Nodes(vec![(NodeId(0), false)]),
			..Default::default()
		};
		assert_eq!(proto_node, reference);
	}

	#[test]
	fn resolve_flatten_add_as_proto_network() {
		let construction_network = ProtoNetwork {
			inputs: Vec::new(),
			output: NodeId(11),
			nodes: [
				(
					NodeId(10),
					ProtoNode {
						identifier: "graphene_core::structural::ConsNode".into(),
						input: ProtoNodeInput::ManualComposition(concrete!(u32)),
						construction_args: ConstructionArgs::Nodes(vec![(NodeId(14), false)]),
						original_location: OriginalLocation {
							path: Some(vec![NodeId(1), NodeId(0)]),
							inputs_source: [(Source { node: vec![NodeId(1)], index: 1 }, 1)].into(),
							outputs_source: HashMap::new(),
							inputs_exposed: vec![true, true],
							skip_inputs: 0,
						},

						..Default::default()
					},
				),
				(
					NodeId(11),
					ProtoNode {
						identifier: "graphene_core::ops::AddPairNode".into(),
						input: ProtoNodeInput::Node(NodeId(10)),
						construction_args: ConstructionArgs::Nodes(vec![]),
						original_location: OriginalLocation {
							path: Some(vec![NodeId(1), NodeId(1)]),
							inputs_source: HashMap::new(),
							outputs_source: [(Source { node: vec![NodeId(1)], index: 0 }, 0)].into(),
							inputs_exposed: vec![true],
							skip_inputs: 0,
						},
						..Default::default()
					},
				),
				(
					NodeId(14),
					ProtoNode {
						identifier: "graphene_core::value::ClonedNode".into(),
						input: ProtoNodeInput::None,
						construction_args: ConstructionArgs::Value(TaggedValue::U32(2)),
						original_location: OriginalLocation {
							path: Some(vec![NodeId(1), NodeId(4)]),
							inputs_source: HashMap::new(),
							outputs_source: HashMap::new(),
							inputs_exposed: vec![true, false],
							skip_inputs: 0,
						},
						..Default::default()
					},
				),
			]
			.into_iter()
			.collect(),
		};
		let network = flat_network();
		let resolved_network = network.into_proto_networks().collect::<Vec<_>>();

		println!("{:#?}", resolved_network[0]);
		println!("{construction_network:#?}");
		assert_eq!(resolved_network[0], construction_network);
	}

	fn flat_network() -> NodeNetwork {
		NodeNetwork {
			exports: vec![NodeInput::node(NodeId(11), 0)],
			nodes: [
				(
					NodeId(10),
					DocumentNode {
						inputs: vec![NodeInput::network(concrete!(u32), 0), NodeInput::node(NodeId(14), 0)],
						implementation: DocumentNodeImplementation::ProtoNode("graphene_core::structural::ConsNode".into()),
						original_location: OriginalLocation {
							path: Some(vec![NodeId(1), NodeId(0)]),
							inputs_source: [(Source { node: vec![NodeId(1)], index: 1 }, 1)].into(),
							outputs_source: HashMap::new(),
							inputs_exposed: vec![true, true],
							skip_inputs: 0,
						},
						..Default::default()
					},
				),
				(
					NodeId(14),
					DocumentNode {
						inputs: vec![NodeInput::Value {
							tagged_value: TaggedValue::U32(2),
							exposed: false,
						}],
						implementation: DocumentNodeImplementation::ProtoNode("graphene_core::value::ClonedNode".into()),
						original_location: OriginalLocation {
							path: Some(vec![NodeId(1), NodeId(4)]),
							inputs_source: HashMap::new(),
							outputs_source: HashMap::new(),
							inputs_exposed: vec![true, false],
							skip_inputs: 0,
						},
						..Default::default()
					},
				),
				(
					NodeId(11),
					DocumentNode {
						inputs: vec![NodeInput::node(NodeId(10), 0)],
						implementation: DocumentNodeImplementation::ProtoNode("graphene_core::ops::AddPairNode".into()),
						original_location: OriginalLocation {
							path: Some(vec![NodeId(1), NodeId(1)]),
							inputs_source: HashMap::new(),
							outputs_source: [(Source { node: vec![NodeId(1)], index: 0 }, 0)].into(),
							inputs_exposed: vec![true],
							skip_inputs: 0,
						},
						..Default::default()
					},
				),
			]
			.into_iter()
			.collect(),
			..Default::default()
		}
	}

	fn two_node_identity() -> NodeNetwork {
		NodeNetwork {
			exports: vec![NodeInput::node(NodeId(1), 0), NodeInput::node(NodeId(2), 0)],
			nodes: [
				(
					NodeId(1),
					DocumentNode {
						inputs: vec![NodeInput::network(concrete!(u32), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IdentityNode")),
						..Default::default()
					},
				),
				(
					NodeId(2),
					DocumentNode {
						inputs: vec![NodeInput::network(concrete!(u32), 1)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IdentityNode")),
						..Default::default()
					},
				),
			]
			.into_iter()
			.collect(),
			..Default::default()
		}
	}

	fn output_duplicate(network_outputs: Vec<NodeInput>, result_node_input: NodeInput) -> NodeNetwork {
		let mut network = NodeNetwork {
			exports: network_outputs,
			nodes: [
				(
					NodeId(1),
					DocumentNode {
						inputs: vec![NodeInput::value(TaggedValue::F64(1.), false), NodeInput::value(TaggedValue::F64(2.), false)],
						implementation: DocumentNodeImplementation::Network(two_node_identity()),
						..Default::default()
					},
				),
				(
					NodeId(2),
					DocumentNode {
						inputs: vec![result_node_input],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IdentityNode")),
						..Default::default()
					},
				),
			]
			.into_iter()
			.collect(),
			..Default::default()
		};
		let _new_ids = 101..;
		network.flatten_with_fns(NodeId(1), |self_id, inner_id| NodeId(self_id.0 * 10 + inner_id.0), || NodeId(10000));
		network.flatten_with_fns(NodeId(2), |self_id, inner_id| NodeId(self_id.0 * 10 + inner_id.0), || NodeId(10001));
		network.remove_dead_nodes(0);
		network
	}

	#[test]
	fn simple_duplicate() {
		let result = output_duplicate(vec![NodeInput::node(NodeId(1), 0)], NodeInput::node(NodeId(1), 0));
		println!("{result:#?}");
		assert_eq!(result.exports.len(), 1, "The number of outputs should remain as 1");
		assert_eq!(result.exports[0], NodeInput::node(NodeId(11), 0), "The outer network output should be from a duplicated inner network");
		let mut ids = result.nodes.keys().copied().collect::<Vec<_>>();
		ids.sort();
		assert_eq!(ids, vec![NodeId(11), NodeId(10010)], "Should only contain identity and values");
	}

	// TODO: Write more tests
	// #[test]
	// fn out_of_order_duplicate() {
	// 	let result = output_duplicate(vec![NodeInput::node(NodeId(10), 1), NodeInput::node(NodeId(10), 0)], NodeInput::node(NodeId(10), 0);
	// 	assert_eq!(
	// 		result.outputs[0],
	// 		NodeInput::node(NodeId(101), 0),
	// 		"The first network output should be from a duplicated nested network"
	// 	);
	// 	assert_eq!(
	// 		result.outputs[1],
	// 		NodeInput::node(NodeId(10), 0),
	// 		"The second network output should be from the original nested network"
	// 	);
	// 	assert!(
	// 		result.nodes.contains_key(&NodeId(10)) && result.nodes.contains_key(&NodeId(101)) && result.nodes.len() == 2,
	// 		"Network should contain two duplicated nodes"
	// 	);
	// 	for (node_id, input_value, inner_id) in [(10, 1., 1), (101, 2., 2)] {
	// 		let nested_network_node = result.nodes.get(&NodeId(node_id)).unwrap();
	// 		assert_eq!(nested_network_node.name, "Nested network".to_string(), "Name should not change");
	// 		assert_eq!(nested_network_node.inputs, vec![NodeInput::value(TaggedValue::F32(input_value), false)], "Input should be stable");
	// 		let inner_network = nested_network_node.implementation.get_network().expect("Implementation should be network");
	// 		assert_eq!(inner_network.inputs, vec![inner_id], "The input should be sent to the second node");
	// 		assert_eq!(inner_network.outputs, vec![NodeInput::node(NodeId(inner_id), 0)], "The output should be node id");
	// 		assert_eq!(inner_network.nodes.get(&NodeId(inner_id)).unwrap().name, format!("Identity {inner_id}"), "The node should be identity");
	// 	}
	// }
	// #[test]
	// fn using_other_node_duplicate() {
	// 	let result = output_duplicate(vec![NodeInput::node(NodeId(11), 0)], NodeInput::node(NodeId(10), 1);
	// 	assert_eq!(result.outputs, vec![NodeInput::node(NodeId(11), 0)], "The network output should be the result node");
	// 	assert!(
	// 		result.nodes.contains_key(&NodeId(11)) && result.nodes.contains_key(&NodeId(101)) && result.nodes.len() == 2,
	// 		"Network should contain a duplicated node and a result node"
	// 	);
	// 	let result_node = result.nodes.get(&NodeId(11)).unwrap();
	// 	assert_eq!(result_node.inputs, vec![NodeInput::node(NodeId(101), 0)], "Result node should refer to duplicate node as input");
	// 	let nested_network_node = result.nodes.get(&NodeId(101)).unwrap();
	// 	assert_eq!(nested_network_node.name, "Nested network".to_string(), "Name should not change");
	// 	assert_eq!(nested_network_node.inputs, vec![NodeInput::value(TaggedValue::F32(2.), false)], "Input should be 2");
	// 	let inner_network = nested_network_node.implementation.get_network().expect("Implementation should be network");
	// 	assert_eq!(inner_network.inputs, vec![2], "The input should be sent to the second node");
	// 	assert_eq!(inner_network.outputs, vec![NodeInput::node(NodeId(2), 0)], "The output should be node id 2");
	// 	assert_eq!(inner_network.nodes.get(&NodeId(2)).unwrap().name, "Identity 2", "The node should be identity 2");
	// }
}
