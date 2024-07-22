use crate::document::value::TaggedValue;
use crate::proto::{ConstructionArgs, ProtoNetwork, ProtoNode, ProtoNodeInput};

use dyn_any::{DynAny, StaticType};
pub use graphene_core::uuid::generate_uuid;
use graphene_core::{Cow, ProtoNodeIdentifier, Type};

use glam::IVec2;
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
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

#[derive(Clone, Debug, PartialEq, Default, specta::Type, Hash, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Metadata about the node including its position in the graph UI
pub struct DocumentNodeMetadata {
	pub position: IVec2,
}

impl DocumentNodeMetadata {
	pub fn position(position: impl Into<IVec2>) -> Self {
		Self { position: position.into() }
	}
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
	/// A name chosen by the user for this instance of the node. Empty indicates no given name, in which case the node definition's name is displayed to the user in italics.
	///  Ensure the click target in the encapsulating network is updated when this is modified by using network.update_click_target(node_id).
	#[serde(default)]
	pub alias: String,
	// TODO: Replace this name with a reference to the [`DocumentNodeDefinition`] node definition to use the name from there instead.
	/// The name of the node definition, as originally set by [`DocumentNodeDefinition`], used to display in the UI and to display the appropriate properties.
	#[serde(deserialize_with = "migrate_layer_to_merge")]
	pub name: String,
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
	// TODO: Remove once this references its definition instead (see above TODO).
	/// Indicates to the UI if a primary output should be drawn for this node.
	/// True for most nodes, but the Split Channels node is an example of a node that has multiple secondary outputs but no primary output.
	#[serde(default = "return_true")]
	pub has_primary_output: bool,
	// A nested document network or a proto-node identifier.
	pub implementation: DocumentNodeImplementation,
	/// User chosen state for displaying this as a left-to-right node or bottom-to-top layer. Ensure the click target in the encapsulating network is updated when the node changes to a layer by using network.update_click_target(node_id).
	#[serde(default)]
	pub is_layer: bool,
	/// Represents the eye icon for hiding/showing the node in the graph UI. When hidden, a node gets replaced with an identity node during the graph flattening step.
	#[serde(default = "return_true")]
	pub visible: bool,
	/// Represents the lock icon for locking/unlocking the node in the graph UI. When locked, a node cannot be moved in the graph UI.
	#[serde(default)]
	pub locked: bool,
	/// Metadata about the node including its position in the graph UI. Ensure the click target in the encapsulating network is updated when the node moves by using network.update_click_target(node_id).
	pub metadata: DocumentNodeMetadata,
	/// When two different proto nodes hash to the same value (e.g. two value nodes each containing `2_u32` or two multiply nodes that have the same node IDs as input), the duplicates are removed.
	/// See [`crate::proto::ProtoNetwork::generate_stable_node_ids`] for details.
	/// However sometimes this is not desirable, for example in the case of a [`graphene_core::memo::MonitorNode`] that needs to be accessed outside of the graph.
	#[serde(default)]
	pub skip_deduplication: bool,
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
			alias: Default::default(),
			name: Default::default(),
			inputs: Default::default(),
			manual_composition: Default::default(),
			has_primary_output: true,
			implementation: Default::default(),
			is_layer: false,
			visible: true,
			locked: Default::default(),
			metadata: DocumentNodeMetadata::default(),
			skip_deduplication: Default::default(),
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
					assert_eq!(self.inputs.len(), 0, "{}, {:?}", self.name, self.inputs);
					(ProtoNodeInput::None, ConstructionArgs::Value(tagged_value))
				}
				NodeInput::Node { node_id, output_index, lambda } => {
					assert_eq!(output_index, 0, "Outputs should be flattened before converting to proto node. {:#?}", self.name);
					let node = if lambda { ProtoNodeInput::NodeLambda(node_id) } else { ProtoNodeInput::Node(node_id) };
					(node, ConstructionArgs::Nodes(vec![]))
				}
				NodeInput::Network { import_type, .. } => (ProtoNodeInput::ManualComposition(import_type), ConstructionArgs::Nodes(vec![])),
				NodeInput::Inline(inline) => (ProtoNodeInput::None, ConstructionArgs::Inline(inline)),
				NodeInput::Scope(_) => unreachable!("Scope input was not resolved"),
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
		}
	}

	/// Converts all node id inputs to a new id based on a HashMap.
	///
	/// If the node is not in the hashmap then a default input is found based on the compiled network
	pub fn map_ids(mut self, default_inputs: Vec<NodeInput>, new_ids: &HashMap<NodeId, NodeId>) -> Self {
		for (input_index, input) in self.inputs.iter_mut().enumerate() {
			if let &mut NodeInput::Node { node_id: id, output_index, lambda } = input {
				if let Some(&new_id) = new_ids.get(&id) {
					*input = NodeInput::Node {
						node_id: new_id,
						output_index,
						lambda,
					};
				} else {
					*input = default_inputs[input_index].clone();
				}
			} else if let &mut NodeInput::Network { .. } = input {
				*input = default_inputs[input_index].clone();
			}
		}
		self
	}

	pub fn is_artboard(&self) -> bool {
		// TODO: Use something more robust than checking against a string.
		// TODO: Or, more fundamentally separate the concept of a layer from a node.
		self.name == "Artboard"
	}

	// TODO: Is this redundant with `LayerNodeIdentifier::has_children()`? Consider removing this in favor of that.
	/// Determines if a document node acting as a layer has any nested children where its secondary input eventually leads to a layer along horizontal flow.
	pub fn layer_has_child_layers(&self, network: &NodeNetwork) -> bool {
		if !self.is_layer {
			return false;
		}

		self.inputs.iter().skip(1).any(|input| {
			input.as_node().map_or(false, |node_id| {
				network.upstream_flow_back_from_nodes(vec![node_id], FlowType::HorizontalFlow).any(|(node, _)| node.is_layer)
			})
		})
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

	/// Input that is extracted from the parent scopes the node resides in. The string argument is the key.
	Scope(Cow<'static, str>),

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

	pub fn scope(key: impl Into<Cow<'static, str>>) -> Self {
		Self::Scope(key.into())
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
			NodeInput::Scope(_) => false,
		}
	}

	pub fn ty(&self) -> Type {
		match self {
			NodeInput::Node { .. } => unreachable!("ty() called on NodeInput::Node"),
			NodeInput::Value { tagged_value, .. } => tagged_value.ty(),
			NodeInput::Network { import_type, .. } => import_type.clone(),
			NodeInput::Inline(_) => panic!("ty() called on NodeInput::Inline"),
			NodeInput::Scope(_) => unreachable!("ty() called on NodeInput::Scope"),
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

#[derive(Clone, Copy, Debug, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Root Node is the "default" export for a node network. Used by document metadata, displaying UI-only "Export" node, and for restoring the default preview node.
pub struct RootNode {
	pub id: NodeId,
	pub output_index: usize,
}
#[derive(PartialEq, Debug, Clone, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Previewing {
	/// If there is a node to restore the connection to the export for, then it is stored in the option.
	/// Otherwise, nothing gets restored and the primary export is disconnected.
	Yes { root_node_to_restore: Option<RootNode> },
	#[default]
	No,
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

// TODO: Eventually remove this (probably starting late 2024)
fn default_import_metadata() -> (NodeId, IVec2) {
	(NodeId(generate_uuid()), IVec2::new(-25, -4))
}
// TODO: Eventually remove this (probably starting late 2024)
fn default_export_metadata() -> (NodeId, IVec2) {
	(NodeId(generate_uuid()), IVec2::new(8, -4))
}

#[derive(Clone, Debug, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A network (subgraph) of nodes containing each [`DocumentNode`] and its ID, as well as list mapping each export to its connected node, or a value if disconnected
pub struct NodeNetwork {
	/// The list of data outputs that are exported from this network to the parent network.
	/// Each export is a reference to a node within this network, paired with its output index, that is the source of the network's exported data.
	#[serde(alias = "outputs", deserialize_with = "deserialize_exports")] // TODO: Eventually remove this alias (probably starting late 2024)
	pub exports: Vec<NodeInput>,
	/// The list of all nodes in this network.
	pub nodes: HashMap<NodeId, DocumentNode>,
	/// Indicates whether the network is currently rendered with a particular node that is previewed, and if so, which connection should be restored when the preview ends.
	#[serde(default)]
	pub previewing: Previewing,
	/// Temporary fields to store metadata for "Import"/"Export" UI-only nodes, eventually will be replaced with lines leading to edges
	#[serde(default = "default_import_metadata")]
	pub imports_metadata: (NodeId, IVec2),
	#[serde(default = "default_export_metadata")]
	pub exports_metadata: (NodeId, IVec2),

	/// A network may expose nodes as constants which can by used by other nodes using a `NodeInput::Scope(key)`.
	#[serde(default)]
	pub scope_injections: HashMap<String, (NodeId, Type)>,
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
		self.previewing.hash(state);
	}
}
impl Default for NodeNetwork {
	fn default() -> Self {
		NodeNetwork {
			exports: Default::default(),
			nodes: Default::default(),
			previewing: Default::default(),
			imports_metadata: default_import_metadata(),
			exports_metadata: default_export_metadata(),
			scope_injections: Default::default(),
		}
	}
}
impl PartialEq for NodeNetwork {
	fn eq(&self, other: &Self) -> bool {
		self.exports == other.exports && self.previewing == other.previewing && self.imports_metadata == other.imports_metadata && self.exports_metadata == other.exports_metadata
	}
}

/// Graph modification functions
impl NodeNetwork {
	pub fn current_hash(&self) -> u64 {
		let mut hasher = DefaultHasher::new();
		self.hash(&mut hasher);
		hasher.finish()
	}

	/// Returns the root node (the node that the solid line is connect to), or None if no nodes are connected to the output
	pub fn get_root_node(&self) -> Option<RootNode> {
		match self.previewing {
			Previewing::Yes { root_node_to_restore } => root_node_to_restore,
			Previewing::No => self.exports.first().and_then(|export| {
				if let NodeInput::Node { node_id, output_index, .. } = export {
					Some(RootNode {
						id: *node_id,
						output_index: *output_index,
					})
				} else {
					None
				}
			}),
		}
	}

	/// Sets the root node only if a node is being previewed
	pub fn update_root_node(&mut self, node_id: NodeId, output_index: usize) {
		if let Previewing::Yes { root_node_to_restore } = self.previewing {
			// Only continue previewing if the new root node is not the same as the primary export. If it is the same, end the preview
			if let Some(root_node_to_restore) = root_node_to_restore {
				if root_node_to_restore.id != node_id {
					self.start_previewing(node_id, output_index);
				} else {
					self.stop_preview();
				}
			} else {
				self.stop_preview();
			}
		}
	}

	/// Start previewing with a restore node
	pub fn start_previewing(&mut self, previous_node_id: NodeId, output_index: usize) {
		self.previewing = Previewing::Yes {
			root_node_to_restore: Some(RootNode { id: previous_node_id, output_index }),
		};
	}

	/// Start previewing without a restore node
	pub fn start_previewing_without_restore(&mut self) {
		self.previewing = Previewing::Yes { root_node_to_restore: None };
	}

	/// Stops preview, does not reset export
	pub fn stop_preview(&mut self) {
		self.previewing = Previewing::No;
	}

	pub fn value_network(node: DocumentNode) -> Self {
		Self {
			exports: vec![NodeInput::node(NodeId(0), 0)],
			nodes: [(NodeId(0), node)].into_iter().collect(),
			..Default::default()
		}
	}

	/// A graph with just an input node
	pub fn new_network() -> Self {
		Self {
			exports: vec![NodeInput::node(NodeId(0), 0)],
			nodes: [(
				NodeId(0),
				DocumentNode {
					name: "Input Frame".into(),
					manual_composition: Some(concrete!(u32)),
					implementation: DocumentNodeImplementation::ProtoNode("graphene_core::ops::IdentityNode".into()),
					metadata: DocumentNodeMetadata { position: (8, 4).into() },
					..Default::default()
				},
			)]
			.into_iter()
			.collect(),
			..Default::default()
		}
	}

	/// Appends a new node to the network after the output node and sets it as the new output
	// pub fn push_node_to_document_network(&mut self, mut node: DocumentNode) -> NodeId {
	// 	let id = NodeId(self.nodes.len().try_into().expect("Too many nodes in network"));
	// 	// Set the correct position for the new node
	// 	if node.metadata.position == IVec2::default() {
	// 		if let Some(pos) = self.get_root_node().and_then(|root_node| self.nodes.get(&root_node.id)).map(|n| n.metadata.position) {
	// 			node.metadata.position = pos + IVec2::new(8, 0);
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

	/// Get the network the selected nodes are part of, which is either self or the nested network from nested_path. Used to get nodes selected in the layer panel when viewing a nested network.
	pub fn nested_network_for_selected_nodes<'a>(&self, nested_path: &[NodeId], mut selected_nodes: impl Iterator<Item = &'a NodeId>) -> Option<&Self> {
		if selected_nodes.any(|node_id| self.nodes.contains_key(node_id) || self.exports_metadata.0 == *node_id || self.imports_metadata.0 == *node_id) {
			Some(self)
		} else {
			self.nested_network(nested_path)
		}
	}

	/// Get the mutable network the selected nodes are part of, which is either self or the nested network from nested_path. Used to modify nodes selected in the layer panel when viewing a nested network.
	pub fn nested_network_for_selected_nodes_mut<'a>(&mut self, nested_path: &[NodeId], mut selected_nodes: impl Iterator<Item = &'a NodeId>) -> Option<&mut Self> {
		if selected_nodes.any(|node_id| self.nodes.contains_key(node_id)) {
			Some(self)
		} else {
			self.nested_network_mut(nested_path)
		}
	}

	/// Check if the specified node id is connected to the output
	pub fn connected_to_output(&self, target_node_id: NodeId) -> bool {
		// If the node is the output then return true
		if self
			.exports
			.iter()
			.any(|export| if let NodeInput::Node { node_id, .. } = export { *node_id == target_node_id } else { false })
		{
			return true;
		}

		if self.exports_metadata.0 == target_node_id {
			return true;
		}
		// Get the outputs
		let mut stack = self
			.exports
			.iter()
			.filter_map(|output| if let NodeInput::Node { node_id, .. } = output { self.nodes.get(node_id) } else { None })
			.collect::<Vec<_>>();
		let mut already_visited = HashSet::new();
		already_visited.extend(self.exports.iter().filter_map(|output| if let NodeInput::Node { node_id, .. } = output { Some(node_id) } else { None }));

		while let Some(node) = stack.pop() {
			for input in &node.inputs {
				if let &NodeInput::Node { node_id: ref_id, .. } = input {
					// Skip if already viewed
					if already_visited.contains(&ref_id) {
						continue;
					}
					// If the target node is used as input then return true
					if ref_id == target_node_id {
						return true;
					}
					// Add the referenced node to the stack
					let Some(ref_node) = self.nodes.get(&ref_id) else {
						continue;
					};
					already_visited.insert(ref_id);
					stack.push(ref_node);
				}
			}
		}

		false
	}

	/// Is the node being used directly as an output?
	pub fn outputs_contain(&self, node_id_to_check: NodeId) -> bool {
		self.exports
			.iter()
			.any(|output| if let NodeInput::Node { node_id, .. } = output { *node_id == node_id_to_check } else { false })
	}

	/// Gives an iterator to all nodes connected to the given nodes (inclusive) by all inputs (primary or primary + secondary depending on `only_follow_primary` choice), traversing backwards upstream starting from the given node's inputs.
	pub fn upstream_flow_back_from_nodes(&self, node_ids: Vec<NodeId>, flow_type: FlowType) -> impl Iterator<Item = (&DocumentNode, NodeId)> {
		FlowIter {
			stack: node_ids,
			network: self,
			flow_type,
		}
	}

	/// In the network `X -> Y -> Z`, `is_node_upstream_of_another_by_primary_flow(Z, X)` returns true.
	pub fn is_node_upstream_of_another_by_horizontal_flow(&self, node: NodeId, potentially_upstream_node: NodeId) -> bool {
		self.upstream_flow_back_from_nodes(vec![node], FlowType::HorizontalFlow).any(|(_, id)| id == potentially_upstream_node)
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

#[derive(PartialEq)]
pub enum FlowType {
	/// Iterate over all upstream nodes from every input (the primary and all secondary).
	UpstreamFlow,
	/// Iterate over nodes connected to the primary input.
	PrimaryFlow,
	/// Iterate over the secondary input for layer nodes and primary input for non layer nodes.
	HorizontalFlow,
}
/// Iterate over upstream nodes. The behavior changes based on the `flow_type` that's set.
/// - [`FlowType::UpstreamFlow`]: iterates over all upstream nodes from every input (the primary and all secondary).
/// - [`FlowType::PrimaryFlow`]: iterates along the horizontal inputs of nodes, so in the case of a node chain `a -> b -> c`, this would yield `c, b, a` if we started from `c`.
/// - [`FlowType::HorizontalFlow`]: iterates over the secondary input for layer nodes and primary input for non layer nodes.
struct FlowIter<'a> {
	stack: Vec<NodeId>,
	network: &'a NodeNetwork,
	flow_type: FlowType,
}
impl<'a> Iterator for FlowIter<'a> {
	type Item = (&'a DocumentNode, NodeId);
	fn next(&mut self) -> Option<Self::Item> {
		loop {
			let mut node_id = self.stack.pop()?;

			// Special handling for iterating from ROOT_PARENT in load_structure`
			if node_id == NodeId(u64::MAX) {
				if let Some(root_node) = self.network.get_root_node() {
					node_id = root_node.id
				} else {
					return None;
				}
			}

			if let Some(document_node) = self.network.nodes.get(&node_id) {
				let skip = if self.flow_type == FlowType::HorizontalFlow && document_node.is_layer { 1 } else { 0 };
				let take = if self.flow_type == FlowType::UpstreamFlow { usize::MAX } else { 1 };
				let inputs = document_node.inputs.iter().skip(skip).take(take);

				let node_ids = inputs.filter_map(|input| if let NodeInput::Node { node_id, .. } = input { Some(node_id) } else { None });

				self.stack.extend(node_ids);

				return Some((document_node, node_id));
			}
		}
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
		if let Previewing::Yes { root_node_to_restore } = &mut self.previewing {
			if let Some(root_node_to_restore) = root_node_to_restore.as_mut() {
				root_node_to_restore.id = f(root_node_to_restore.id);
			}
		}
		self.scope_injections.values_mut().for_each(|(id, _ty)| *id = f(*id));
		let nodes = std::mem::take(&mut self.nodes);
		self.nodes = nodes
			.into_iter()
			.map(|(id, mut node)| {
				node.inputs.iter_mut().for_each(|input| input.map_ids(f));
				(f(id), node)
			})
			.collect();
	}

	/// Collect a hashmap of nodes with a list of the nodes that use it as input
	pub fn collect_outwards_wires(&self) -> HashMap<NodeId, Vec<NodeId>> {
		let mut outwards_wires: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
		for (current_node_id, node) in &self.nodes {
			for input in &node.inputs {
				if let NodeInput::Node { node_id, .. } = input {
					let outward_wires_entry = outwards_wires.entry(*node_id).or_default();
					outward_wires_entry.push(*current_node_id);
				}
			}
		}
		outwards_wires
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

	pub fn resolve_scope_inputs(&mut self) {
		for node in self.nodes.values_mut() {
			for input in node.inputs.iter_mut() {
				if let NodeInput::Scope(key) = input {
					let (import_id, _ty) = self.scope_injections.get(key.as_ref()).expect("Tried to import a non existent key from scope");
					// TODO use correct output index
					*input = NodeInput::node(*import_id, 0);
				}
			}
		}
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
								name: "Value".into(),
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
							name: "Value".into(),
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

			for (key, value) in inner_network.scope_injections.into_iter() {
				match self.scope_injections.entry(key) {
					std::collections::hash_map::Entry::Occupied(o) => {
						log::warn!("Found duplicate scope injection for key {}, ignoring", o.key());
					}
					std::collections::hash_map::Entry::Vacant(v) => {
						v.insert(value);
					}
				}
			}

			// Match the document node input and the inputs of the inner network
			for (nested_node_id, mut nested_node) in inner_network.nodes.into_iter() {
				if nested_node.name == "To Artboard" {
					let label_index = 1;
					let label = if !node.alias.is_empty() { node.alias.clone() } else { node.name.clone() };
					let label_input = NodeInput::value(TaggedValue::String(label), false);
					nested_node.inputs[label_index] = label_input;
				}

				for (nested_input_index, nested_input) in nested_node.clone().inputs.iter().enumerate() {
					if let NodeInput::Network { import_index, .. } = nested_input {
						let parent_input = node.inputs.get(*import_index).unwrap_or_else(|| panic!("Import index {} should always exist", import_index));
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
							NodeInput::Scope(ref key) => {
								let (import_id, _ty) = self.scope_injections.get(key.as_ref()).expect("Tried to import a non existent key from scope");
								// TODO use correct output index
								nested_node.inputs[nested_input_index] = NodeInput::node(*import_id, 0);
							}
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
						name: "Cons".into(),
						inputs: vec![NodeInput::network(concrete!(u32), 0), NodeInput::network(concrete!(u32), 1)],
						implementation: DocumentNodeImplementation::ProtoNode("graphene_core::structural::ConsNode".into()),
						..Default::default()
					},
				),
				(
					NodeId(1),
					DocumentNode {
						name: "Add".into(),
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
						name: "Cons".into(),
						inputs: vec![NodeInput::network(concrete!(u32), 0), NodeInput::network(concrete!(u32), 1)],
						implementation: DocumentNodeImplementation::ProtoNode("graphene_core::structural::ConsNode".into()),
						..Default::default()
					},
				),
				(
					NodeId(2),
					DocumentNode {
						name: "Add".into(),
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
		network.exports_metadata.0 = NodeId(0);
		network.imports_metadata.0 = NodeId(0);
		mapped_add.exports_metadata.0 = NodeId(0);
		mapped_add.imports_metadata.0 = NodeId(0);
		assert_eq!(network, mapped_add);
	}

	#[test]
	fn extract_node() {
		let id_node = DocumentNode {
			name: "Id".into(),
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
					name: "Extract".into(),
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
					name: "Inc".into(),
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
		network.exports_metadata.0 = NodeId(0);
		network.imports_metadata.0 = NodeId(0);
		let mut flat_network = flat_network();
		flat_network.imports_metadata.0 = NodeId(0);
		flat_network.exports_metadata.0 = NodeId(0);
		println!("{flat_network:#?}");
		println!("{network:#?}");

		assert_eq!(flat_network, network);
	}

	#[test]
	fn resolve_proto_node_add() {
		let document_node = DocumentNode {
			name: "Cons".into(),
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
						name: "Cons".into(),
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
						name: "Value".into(),
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
						name: "Add".into(),
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
						name: "Identity 1".into(),
						inputs: vec![NodeInput::network(concrete!(u32), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IdentityNode")),
						..Default::default()
					},
				),
				(
					NodeId(2),
					DocumentNode {
						name: "Identity 2".into(),
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
						name: "Nested network".into(),
						inputs: vec![NodeInput::value(TaggedValue::F64(1.), false), NodeInput::value(TaggedValue::F64(2.), false)],
						implementation: DocumentNodeImplementation::Network(two_node_identity()),
						..Default::default()
					},
				),
				(
					NodeId(2),
					DocumentNode {
						name: "Result".into(),
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
