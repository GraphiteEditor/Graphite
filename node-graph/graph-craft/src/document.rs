pub mod value;

use crate::document::value::TaggedValue;
use crate::proto::{ConstructionArgs, ProtoNetwork, ProtoNode};
use dyn_any::DynAny;
use glam::IVec2;
use graphene_core::memo::MemoHashGuard;
pub use graphene_core::uuid::NodeId;
pub use graphene_core::uuid::generate_uuid;
use graphene_core::{Context, ContextDependencies, Cow, MemoHash, ProtoNodeIdentifier, Type};
use log::Metadata;
use rustc_hash::{FxBuildHasher, FxHashMap};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Hash two IDs together, returning a new ID that is always consistent for two input IDs in a specific order.
/// This is used during [`NodeNetwork::flatten`] in order to ensure consistent yet non-conflicting IDs for inner networks.
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

/// An instance of a [`DocumentNodeDefinition`] that has been instantiated in a [`NodeNetwork`].
/// Currently, when an instance is made, it lives all on its own without any lasting connection to the definition.
/// But we will want to change it in the future so it merely references its definition.
#[derive(Clone, Debug, PartialEq, Hash, DynAny, serde::Serialize, serde::Deserialize)]
pub struct DocumentNode {
	/// The inputs to a node, which are either:
	/// - From other nodes within this graph [`NodeInput::Node`],
	/// - A constant value [`NodeInput::Value`],
	/// - A [`NodeInput::Import`] which specifies that this input comes from outside the graph, which is resolved in the graph flattening step in the case of nested networks.
	///
	/// In the root network, it is resolved when evaluating the borrow tree.
	/// Ensure the click target in the encapsulating network is updated when the inputs cause the node shape to change (currently only when exposing/hiding an input)
	/// by using network.update_click_target(node_id).
	#[cfg_attr(target_family = "wasm", serde(alias = "outputs"))]
	pub inputs: Vec<NodeInput>,
	/// Type of the argument which this node can be evaluated with.
	#[serde(default, alias = "manual_composition", deserialize_with = "migrate_call_argument")]
	pub call_argument: Type,
	// A nested document network or a proto-node identifier.
	pub implementation: DocumentNodeImplementation,
	/// Represents the eye icon for hiding/showing the node in the graph UI. When hidden, a node gets replaced with an identity node during the graph flattening step.
	#[serde(default = "return_true")]
	pub visible: bool,
	/// When two different proto nodes hash to the same value (e.g. two value nodes each containing `2_u32` or two multiply nodes that have the same node IDs as input), the duplicates are removed.
	/// See [`ProtoNetwork::generate_stable_node_ids`] for details.
	/// However sometimes this is not desirable, for example in the case of a [`graphene_core::memo::MonitorNode`] that needs to be accessed outside of the graph.
	#[serde(default)]
	pub skip_deduplication: bool,
	/// List of Extract and Inject annotations for the Context.
	#[serde(default)]
	pub context_features: ContextDependencies,
	/// The path to this node and its inputs and outputs as of when [`NodeNetwork::generate_node_paths`] was called.
	#[serde(skip)]
	pub original_location: OriginalLocation,
}

/// Represents the original location of a node input/output when [`NodeNetwork::generate_node_paths`] was called, allowing the types and errors to be derived.
#[derive(Clone, Debug, PartialEq, Eq, Hash, DynAny, serde::Serialize, serde::Deserialize)]
pub struct Source {
	pub node: Vec<NodeId>,
	pub index: usize,
}

/// The path to this node and its inputs and outputs as of when [`NodeNetwork::generate_node_paths`] was called.
#[derive(Clone, Debug, PartialEq, Eq, DynAny, Default, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub struct OriginalLocation {
	/// The original location to the document node - e.g. [grandparent_id, parent_id, node_id].
	pub path: Option<Vec<NodeId>>,
	/// Each document input source maps to one proto node input (however one proto node input may come from several sources)
	pub inputs_source: HashMap<Source, usize>,
	/// List of nodes which depend on this node
	pub dependants: Vec<Vec<NodeId>>,
	/// A list of flags indicating whether the input is exposed in the UI
	pub inputs_exposed: Vec<bool>,
}

impl Default for DocumentNode {
	fn default() -> Self {
		Self {
			inputs: Default::default(),
			call_argument: concrete!(Context),
			implementation: Default::default(),
			visible: true,
			skip_deduplication: Default::default(),
			original_location: OriginalLocation::default(),
			context_features: Default::default(),
		}
	}
}

impl Hash for OriginalLocation {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.path.hash(state);
		self.inputs_source.iter().for_each(|val| val.hash(state));
		self.inputs_exposed.hash(state);
	}
}
impl OriginalLocation {
	pub fn inputs(&self, index: usize) -> impl Iterator<Item = Source> + '_ {
		[(index >= 1).then(|| Source {
			node: self.path.clone().unwrap_or_default(),
			index: self.inputs_exposed.iter().take(index - 1).filter(|&&exposed| exposed).count(),
		})]
		.into_iter()
		.flatten()
		.chain(self.inputs_source.iter().filter(move |x| *x.1 == index).map(|(source, _)| source.clone()))
	}
}
impl DocumentNode {
	/// Locate the input that is a [`NodeInput::Import`] at index `offset` and replace it with a [`NodeInput::Node`].
	pub fn populate_first_network_input(&mut self, node_id: NodeId, output_index: usize, offset: usize, source: impl Iterator<Item = Source>, skip: usize) {
		let (index, _) = self
			.inputs
			.iter()
			.enumerate()
			.nth(offset)
			.unwrap_or_else(|| panic!("no network input found for {self:#?} and offset: {offset}"));

		self.inputs[index] = NodeInput::Node { node_id, output_index };
		let input_source = &mut self.original_location.inputs_source;
		for source in source {
			input_source.insert(source, (index + 1).saturating_sub(skip));
		}
	}

	fn resolve_proto_node(self) -> ProtoNode {
		let DocumentNodeImplementation::ProtoNode(identifier) = self.implementation else {
			unreachable!("tried to resolve not flattened node on resolved node {self:?}");
		};
		assert!(!self.inputs.iter().any(|input| matches!(input, NodeInput::Import { .. })), "received non-resolved input");

		let mut construction_args = ConstructionArgs::Nodes(vec![]);

		// If we have one input of the type inline, set it as the construction args
		if let &[NodeInput::Inline(ref inline)] = self.inputs.as_slice() {
			construction_args = ConstructionArgs::Inline(inline.clone());
		}
		// If we have one input of the type inline, set it as the construction args
		if let &[NodeInput::Value { ref tagged_value, .. }] = self.inputs.as_slice() {
			construction_args = ConstructionArgs::Value(tagged_value.clone());
		}
		if let ConstructionArgs::Nodes(nodes) = &mut construction_args {
			nodes.extend(self.inputs.iter().map(|input| match input {
				NodeInput::Node { node_id, .. } => *node_id,
				_ => unreachable!(),
			}));
		}

		ProtoNode {
			identifier,
			call_argument: self.call_argument,
			construction_args,
			original_location: self.original_location,
			skip_deduplication: self.skip_deduplication,
			context_features: self.context_features,
		}
	}
}

/// Represents the possible inputs to a node.
#[derive(Debug, Clone, PartialEq, Hash, DynAny, serde::Serialize, serde::Deserialize)]
pub enum NodeInput {
	/// A reference to another node in the same network from which this node can receive its input.
	Node { node_id: NodeId, output_index: usize },

	/// A hardcoded value that can't change after the graph is compiled. Gets converted into a value node during graph compilation.
	Value { tagged_value: MemoHash<TaggedValue>, exposed: bool },

	// TODO: Remove import_type and get type from parent node input
	/// Input that is provided by the import from the parent network to this document node network.
	#[serde(alias = "Network")]
	Import { import_type: Type, import_index: usize },

	/// Input that is extracted from the parent scopes the node resides in. The string argument is the key.
	Scope(Cow<'static, str>),

	/// Input that is extracted from the parent scopes the node resides in. The string argument is the key.
	Reflection(DocumentNodeMetadata),

	/// A Rust source code string. Allows us to insert literal Rust code. Only used for GPU compilation.
	/// We can use this whenever we spin up Rustc. Sort of like inline assembly, but because our language is Rust, it acts as inline Rust.
	Inline(InlineRust),
}

#[derive(Debug, Clone, PartialEq, Hash, DynAny, serde::Serialize, serde::Deserialize)]
pub struct InlineRust {
	pub expr: String,
	pub ty: Type,
}

impl InlineRust {
	pub fn new(expr: String, ty: Type) -> Self {
		Self { expr, ty }
	}
}

#[derive(Debug, Clone, PartialEq, Hash, DynAny, serde::Serialize, serde::Deserialize)]
pub enum DocumentNodeMetadata {
	DocumentNodePath,
}

impl NodeInput {
	pub const fn node(node_id: NodeId, output_index: usize) -> Self {
		Self::Node { node_id, output_index }
	}

	pub fn value(tagged_value: TaggedValue, exposed: bool) -> Self {
		let tagged_value = tagged_value.into();
		Self::Value { tagged_value, exposed }
	}

	pub const fn import(import_type: Type, import_index: usize) -> Self {
		Self::Import { import_type, import_index }
	}

	pub fn scope(key: impl Into<Cow<'static, str>>) -> Self {
		Self::Scope(key.into())
	}

	fn map_ids(&mut self, f: impl Fn(NodeId) -> NodeId) {
		if let &mut NodeInput::Node { node_id, output_index } = self {
			*self = NodeInput::Node { node_id: f(node_id), output_index }
		}
	}

	pub fn is_exposed(&self) -> bool {
		match self {
			NodeInput::Node { .. } => true,
			NodeInput::Value { exposed, .. } => *exposed,
			NodeInput::Import { .. } => true,
			NodeInput::Inline(_) => false,
			NodeInput::Scope(_) => false,
			NodeInput::Reflection(_) => false,
		}
	}

	pub fn ty(&self) -> Type {
		match self {
			NodeInput::Node { .. } => unreachable!("ty() called on NodeInput::Node"),
			NodeInput::Value { tagged_value, .. } => tagged_value.ty(),
			NodeInput::Import { import_type, .. } => import_type.clone(),
			NodeInput::Inline(_) => panic!("ty() called on NodeInput::Inline"),
			NodeInput::Scope(_) => panic!("ty() called on NodeInput::Scope"),
			NodeInput::Reflection(_) => concrete!(Metadata),
		}
	}

	pub fn as_value(&self) -> Option<&TaggedValue> {
		if let NodeInput::Value { tagged_value, .. } = self { Some(tagged_value) } else { None }
	}
	pub fn as_value_mut(&mut self) -> Option<MemoHashGuard<'_, TaggedValue>> {
		if let NodeInput::Value { tagged_value, .. } = self { Some(tagged_value.inner_mut()) } else { None }
	}
	pub fn as_non_exposed_value(&self) -> Option<&TaggedValue> {
		if let NodeInput::Value { tagged_value, exposed: false } = self { Some(tagged_value) } else { None }
	}

	pub fn as_node(&self) -> Option<NodeId> {
		if let NodeInput::Node { node_id, .. } = self { Some(*node_id) } else { None }
	}
}

// TODO: Eventually remove this document upgrade code
#[derive(Clone, Debug, DynAny, serde::Serialize, serde::Deserialize)]
/// Represents the implementation of a node, which can be a nested [`NodeNetwork`], a proto [`ProtoNodeIdentifier`], or `Extract`.
pub enum OldDocumentNodeImplementation {
	Network(OldNodeNetwork),
	#[serde(alias = "Unresolved")]
	ProtoNode(ProtoNodeIdentifier),
	Extract,
}

#[derive(Clone, Debug, PartialEq, Hash, DynAny, serde::Serialize, serde::Deserialize)]
/// Represents the implementation of a node, which can be a nested [`NodeNetwork`], a proto [`ProtoNodeIdentifier`], or `Extract`.
pub enum DocumentNodeImplementation {
	/// This describes a (document) node built out of a subgraph of other (document) nodes.
	///
	/// A nested [`NodeNetwork`] that is flattened by the [`NodeNetwork::flatten`] function.
	Network(NodeNetwork),
	/// This describes a (document) node implemented as a proto node.
	///
	/// A proto node identifier which can be found in `node_registry.rs`.
	#[serde(alias = "Unresolved")] // TODO: Eventually remove this alias document upgrade code
	ProtoNode(ProtoNodeIdentifier),
	/// The Extract variant is a tag which tells the compilation process to do something special: it invokes language-level functionality built for use by the ExtractNode to enable metaprogramming.
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

	pub fn get_proto_node(&self) -> Option<&ProtoNodeIdentifier> {
		match self {
			DocumentNodeImplementation::ProtoNode(p) => Some(p),
			_ => None,
		}
	}

	pub fn output_count(&self) -> usize {
		match self {
			DocumentNodeImplementation::Network(network) => network.exports.len(),
			_ => 1,
		}
	}
}

// TODO: Eventually remove this document upgrade code
#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
pub enum NodeExportVersions {
	OldNodeInput(NodeOutput),
	NodeInput(NodeInput),
}

// TODO: Eventually remove this document upgrade code
#[derive(Debug, serde::Deserialize)]
pub struct NodeOutput {
	pub node_id: NodeId,
	pub node_output_index: usize,
}

// TODO: Eventually remove this document upgrade code
fn deserialize_exports<'de, D>(deserializer: D) -> Result<Vec<NodeInput>, D::Error>
where
	D: serde::Deserializer<'de>,
{
	use serde::Deserialize;
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

/// An instance of a [`DocumentNodeDefinition`] that has been instantiated in a [`NodeNetwork`].
/// Currently, when an instance is made, it lives all on its own without any lasting connection to the definition.
/// But we will want to change it in the future so it merely references its definition.
#[derive(Clone, Debug, DynAny, serde::Serialize, serde::Deserialize)]
pub struct OldDocumentNode {
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
	/// - A [`NodeInput::Import`] which specifies that this input is from outside the graph, which is resolved in the graph flattening step in the case of nested networks.
	///
	/// In the root network, it is resolved when evaluating the borrow tree.
	/// Ensure the click target in the encapsulating network is updated when the inputs cause the node shape to change (currently only when exposing/hiding an input) by using network.update_click_target(node_id).
	#[cfg_attr(target_family = "wasm", serde(alias = "outputs"))]
	pub inputs: Vec<NodeInput>,
	pub manual_composition: Option<Type>,
	// TODO: Remove once this references its definition instead (see above TODO).
	/// Indicates to the UI if a primary output should be drawn for this node.
	/// True for most nodes, but the Split Channels node is an example of a node that has multiple secondary outputs but no primary output.
	#[serde(default = "return_true")]
	pub has_primary_output: bool,
	// A nested document network or a proto-node identifier.
	pub implementation: OldDocumentNodeImplementation,
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
	pub metadata: OldDocumentNodeMetadata,
	/// When two different proto nodes hash to the same value (e.g. two value nodes each containing `2_u32` or two multiply nodes that have the same node IDs as input), the duplicates are removed.
	/// See [`ProtoNetwork::generate_stable_node_ids`] for details.
	/// However sometimes this is not desirable, for example in the case of a [`graphene_core::memo::MonitorNode`] that needs to be accessed outside of the graph.
	#[serde(default)]
	pub skip_deduplication: bool,
	/// The path to this node and its inputs and outputs as of when [`NodeNetwork::generate_node_paths`] was called.
	#[serde(skip)]
	pub original_location: OriginalLocation,
}

// TODO: Eventually remove this document upgrade code
#[derive(Clone, Debug, PartialEq, Default, specta::Type, Hash, DynAny, serde::Serialize, serde::Deserialize)]
/// Metadata about the node including its position in the graph UI
pub struct OldDocumentNodeMetadata {
	pub position: IVec2,
}

// TODO: Eventually remove this document upgrade code
#[derive(Clone, Copy, Debug, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
/// Root Node is the "default" export for a node network. Used by document metadata, displaying UI-only "Export" node, and for restoring the default preview node.
pub struct OldRootNode {
	pub id: NodeId,
	pub output_index: usize,
}

// TODO: Eventually remove this document upgrade code
#[derive(PartialEq, Debug, Clone, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum OldPreviewing {
	/// If there is a node to restore the connection to the export for, then it is stored in the option.
	/// Otherwise, nothing gets restored and the primary export is disconnected.
	Yes { root_node_to_restore: Option<OldRootNode> },
	#[default]
	No,
}

// TODO: Eventually remove this document upgrade code
#[derive(Clone, Debug, DynAny, serde::Serialize, serde::Deserialize)]
/// A network (subgraph) of nodes containing each [`DocumentNode`] and its ID, as well as list mapping each export to its connected node, or a value if disconnected
pub struct OldNodeNetwork {
	/// The list of data outputs that are exported from this network to the parent network.
	/// Each export is a reference to a node within this network, paired with its output index, that is the source of the network's exported data.
	#[serde(alias = "outputs", deserialize_with = "deserialize_exports")] // TODO: Eventually remove this alias document upgrade code
	pub exports: Vec<NodeInput>,
	/// The list of all nodes in this network.
	//cfg_attr(feature = "serde", #[serde(serialize_with = "graphene_core::vector::serialize_hashmap", deserialize_with = "graphene_core::vector::deserialize_hashmap"))]
	pub nodes: HashMap<NodeId, OldDocumentNode>,
	/// Indicates whether the network is currently rendered with a particular node that is previewed, and if so, which connection should be restored when the preview ends.
	#[serde(default)]
	pub previewing: OldPreviewing,
	/// Temporary fields to store metadata for "Import"/"Export" UI-only nodes, eventually will be replaced with lines leading to edges
	#[serde(default = "default_import_metadata")]
	pub imports_metadata: (NodeId, IVec2),
	#[serde(default = "default_export_metadata")]
	pub exports_metadata: (NodeId, IVec2),

	/// A network may expose nodes as constants which can by used by other nodes using a `NodeInput::Scope(key)`.
	#[serde(default)]
	//cfg_attr(feature = "serde", #[serde(serialize_with = "graphene_core::vector::serialize_hashmap", deserialize_with = "graphene_core::vector::deserialize_hashmap"))]
	pub scope_injections: HashMap<String, (NodeId, Type)>,
}

// TODO: Eventually remove this document upgrade code
fn migrate_layer_to_merge<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
	let mut s: String = serde::Deserialize::deserialize(deserializer)?;
	if s == "Layer" {
		s = "Merge".to_string();
	}
	Ok(s)
}
// TODO: Eventually remove this document upgrade code
fn default_import_metadata() -> (NodeId, IVec2) {
	(NodeId::new(), IVec2::new(-25, -4))
}
// TODO: Eventually remove this document upgrade code
fn default_export_metadata() -> (NodeId, IVec2) {
	(NodeId::new(), IVec2::new(8, -4))
}

#[derive(Clone, Default, Debug, DynAny, serde::Serialize, serde::Deserialize)]
/// A network (subgraph) of nodes containing each [`DocumentNode`] and its ID, as well as list mapping each export to its connected node, or a value if disconnected
pub struct NodeNetwork {
	/// The list of data outputs that are exported from this network to the parent network.
	/// Each export is a reference to a node within this network, paired with its output index, that is the source of the network's exported data.
	// TODO: Eventually remove this alias document upgrade code
	#[cfg_attr(target_family = "wasm", serde(alias = "outputs", deserialize_with = "deserialize_exports"))]
	pub exports: Vec<NodeInput>,
	// TODO: Instead of storing import types in each NodeInput::Import connection, the types are stored here. This is similar to how types need to be defined for parameters when creating a function in Rust.
	// pub import_types: Vec<Type>,
	/// The list of all nodes in this network.
	#[serde(serialize_with = "graphene_core::vector::serialize_hashmap", deserialize_with = "graphene_core::vector::deserialize_hashmap")]
	pub nodes: FxHashMap<NodeId, DocumentNode>,
	/// A network may expose nodes as constants which can by used by other nodes using a `NodeInput::Scope(key)`.
	#[serde(default)]
	#[serde(serialize_with = "graphene_core::vector::serialize_hashmap", deserialize_with = "graphene_core::vector::deserialize_hashmap")]
	pub scope_injections: FxHashMap<String, (NodeId, Type)>,
	#[serde(skip)]
	pub generated: bool,
}

impl Hash for NodeNetwork {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.exports.hash(state);
		let mut nodes: Vec<_> = self.nodes.iter().collect();
		nodes.sort_by_key(|(id, _)| *id);
		for (id, node) in nodes {
			id.hash(state);
			node.hash(state);
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
		use std::hash::BuildHasher;
		FxBuildHasher.hash_one(self)
	}

	pub fn value_network(node: DocumentNode) -> Self {
		Self {
			exports: vec![NodeInput::node(NodeId(0), 0)],
			nodes: [(NodeId(0), node)].into_iter().collect(),
			..Default::default()
		}
	}

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
		self.scope_injections.values_mut().for_each(|(id, _ty)| *id = f(*id));
		let nodes = std::mem::take(&mut self.nodes);
		self.nodes = nodes
			.into_iter()
			.map(|(id, mut node)| {
				node.inputs.iter_mut().for_each(|input| input.map_ids(f));
				node.original_location.dependants.iter_mut().for_each(|deps| deps.iter_mut().for_each(|id| *id = f(*id)));
				(f(id), node)
			})
			.collect();
	}

	/// Populate the [`DocumentNode::path`], which stores the location of the document node to allow for matching the resulting proto nodes to the document node for the purposes of typing and finding monitor nodes.
	pub fn generate_node_paths(&mut self, prefix: &[NodeId]) {
		for (node_id, node) in &mut self.nodes {
			let mut new_path = prefix.to_vec();
			if !self.generated {
				new_path.push(*node_id);
			}
			if let DocumentNodeImplementation::Network(network) = &mut node.implementation {
				network.generate_node_paths(new_path.as_slice());
			}
			if node.original_location.path.is_some() {
				log::warn!("Attempting to overwrite node path");
			} else {
				node.original_location = OriginalLocation {
					path: Some(new_path),
					inputs_exposed: node.inputs.iter().map(|input| input.is_exposed()).collect(),
					dependants: (0..node.implementation.output_count()).map(|_| Vec::new()).collect(),
					..Default::default()
				};
			}
		}
	}

	pub fn populate_dependants(&mut self) {
		let mut dep_changes = Vec::new();
		for (node_id, node) in &mut self.nodes {
			let len = node.original_location.dependants.len();
			node.original_location.dependants.extend(vec![vec![]; (node.implementation.output_count()).max(len) - len]);
			for input in &node.inputs {
				if let NodeInput::Node { node_id: dep_id, output_index, .. } = input {
					dep_changes.push((*dep_id, *output_index, *node_id));
				}
			}
		}
		// println!("{:#?}", self.nodes.get(&NodeId(1)));
		for (dep_id, output_index, node_id) in dep_changes {
			let node = self.nodes.get_mut(&dep_id).expect("Encountered invalid node id");
			let len = node.original_location.dependants.len();
			// One must be added to the index to find the length because indexing in rust starts from 0.
			node.original_location.dependants.extend(vec![vec![]; (output_index + 1).max(len) - len]);
			// println!("{node_id} {output_index} {}", node.implementation.output_count());
			node.original_location.dependants[output_index].push(node_id);
		}
	}

	/// Replace all references in any node of `old_input` with `new_input`
	fn replace_node_inputs(&mut self, node_id: NodeId, old_input: (NodeId, usize), new_input: (NodeId, usize)) {
		let Some(node) = self.nodes.get_mut(&node_id) else { return };
		node.inputs.iter_mut().for_each(|input| {
			if let NodeInput::Node { node_id: input_id, output_index, .. } = input {
				if (*input_id, *output_index) == old_input {
					(*input_id, *output_index) = new_input;
				}
			}
		});
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
				if let NodeInput::Import { import_index, .. } = node_input {
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
		self.flatten_with_fns(node_id, merge_ids, NodeId::new)
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

			// Connect layer node to the group below
			node.inputs.drain(1..);
			node.call_argument = concrete!(());
			self.nodes.insert(id, node);
			return;
		}

		let path = node.original_location.path.clone().unwrap_or_default();

		// Replace value inputs with dedicated value nodes
		if node.implementation != DocumentNodeImplementation::ProtoNode("graphene_core::value::ClonedNode".into()) {
			Self::replace_value_inputs_with_nodes(&mut node.inputs, &mut self.nodes, &path, gen_id, map_ids, id);
		}

		let DocumentNodeImplementation::Network(mut inner_network) = node.implementation else {
			// If the node is not a network, it is a primitive node and can be inserted into the network as is.
			assert!(!self.nodes.contains_key(&id), "Trying to insert a node into the network caused an id conflict");

			self.nodes.insert(id, node);
			return;
		};

		// Replace value and reflection imports with value nodes, added inside nested network
		Self::replace_value_inputs_with_nodes(
			&mut inner_network.exports,
			&mut inner_network.nodes,
			node.original_location.path.as_ref().unwrap_or(&vec![]),
			gen_id,
			map_ids,
			id,
		);

		// Connect all network inputs to either the parent network nodes, or newly created value nodes for the parent node.
		inner_network.map_ids(|inner_id| map_ids(id, inner_id));
		inner_network.populate_dependants();
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
			for (nested_input_index, nested_input) in nested_node.clone().inputs.iter().enumerate() {
				if let NodeInput::Import { import_index, .. } = nested_input {
					let parent_input = node.inputs.get(*import_index).unwrap_or_else(|| panic!("Import index {import_index} should always exist"));
					match *parent_input {
						// If the input to self is a node, connect the corresponding output of the inner network to it
						NodeInput::Node { node_id, output_index } => {
							nested_node.populate_first_network_input(node_id, output_index, nested_input_index, node.original_location.inputs(*import_index), 1);
							let input_node = self.nodes.get_mut(&node_id).unwrap_or_else(|| panic!("unable find input node {node_id:?}"));
							input_node.original_location.dependants[output_index].push(nested_node_id);
						}
						NodeInput::Import { import_index, .. } => {
							let parent_input_index = import_index;
							let Some(NodeInput::Import { import_index, .. }) = nested_node.inputs.get_mut(nested_input_index) else {
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
						NodeInput::Reflection(_) => unreachable!("Reflection inputs should have been replaced with value nodes"),
					}
				}
			}
			self.nodes.insert(nested_node_id, nested_node);
		}
		// TODO: Add support for flattening exports that are NodeInput::Import (https://github.com/GraphiteEditor/Graphite/issues/1762)

		// Connect all nodes that were previously connected to this node to the nodes of the inner network
		for (i, export) in inner_network.exports.into_iter().enumerate() {
			if let NodeInput::Node { node_id, output_index, .. } = &export {
				for deps in &node.original_location.dependants {
					for dep in deps {
						self.replace_node_inputs(*dep, (id, i), (*node_id, *output_index));
					}
				}

				if let Some(new_output_node) = self.nodes.get_mut(node_id) {
					for dep in &node.original_location.dependants[i] {
						new_output_node.original_location.dependants[*output_index].push(*dep);
					}
				}
			}

			self.replace_network_outputs(NodeInput::node(id, i), export);
		}

		for node_id in new_nodes {
			self.flatten_with_fns(node_id, map_ids, gen_id);
		}
	}

	#[inline(never)]
	fn replace_value_inputs_with_nodes(
		inputs: &mut [NodeInput],
		collection: &mut FxHashMap<NodeId, DocumentNode>,
		path: &[NodeId],
		gen_id: impl Fn() -> NodeId + Copy,
		map_ids: impl Fn(NodeId, NodeId) -> NodeId + Copy,
		id: NodeId,
	) {
		// Replace value exports and imports with value nodes, added inside the nested network
		for export in inputs {
			let export: &mut NodeInput = export;
			let previous_export = std::mem::replace(export, NodeInput::import(concrete!(()), 0));

			let (tagged_value, exposed) = match previous_export {
				NodeInput::Value { tagged_value, exposed } => (tagged_value, exposed),
				NodeInput::Reflection(reflect) => match reflect {
					DocumentNodeMetadata::DocumentNodePath => (TaggedValue::NodePath(path.to_vec()).into(), false),
				},
				previous_export => {
					*export = previous_export;
					continue;
				}
			};
			let value_node_id = gen_id();
			let merged_node_id = map_ids(id, value_node_id);
			let mut original_location = OriginalLocation {
				path: Some(path.to_vec()),
				dependants: vec![vec![id]],
				..Default::default()
			};

			if let Some(path) = &mut original_location.path {
				path.push(value_node_id);
			}
			collection.insert(
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
			};
		}
	}

	fn remove_id_node(&mut self, id: NodeId) -> Result<(), String> {
		let node = self.nodes.get(&id).ok_or_else(|| format!("Node with id {id} does not exist"))?.clone();
		if let DocumentNodeImplementation::ProtoNode(ident) = &node.implementation {
			if ident.name == "graphene_core::ops::IdentityNode" {
				assert_eq!(node.inputs.len(), 1, "Id node has more than one input");
				if let NodeInput::Node { node_id, output_index, .. } = node.inputs[0] {
					let node_input_output_index = output_index;
					// TODO fix
					if let Some(input_node) = self.nodes.get_mut(&node_id) {
						for &dep in &node.original_location.dependants[0] {
							input_node.original_location.dependants[output_index].push(dep);
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
										input_source.insert(source, index);
									}
								}
							}
						}
						for node_input in self.exports.iter_mut() {
							if let NodeInput::Node { node_id, output_index, .. } = node_input {
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
					NodeInput::Node { .. } => NodeInput::import(generic!(T), 0),
					ni => NodeInput::import(ni.ty(), 0),
				};
			}

			for input in input_node.inputs.iter_mut() {
				if let NodeInput::Node { .. } = input {
					*input = NodeInput::import(generic!(T), 0)
				}
			}
			node.inputs = vec![NodeInput::value(TaggedValue::DocumentNode(input_node), false)];
		}
		self.nodes.extend(extraction_nodes);
	}

	/// Creates a proto network for evaluating each output of this network.
	pub fn into_proto_networks(self) -> impl Iterator<Item = ProtoNetwork> {
		let nodes: Vec<_> = self.nodes.into_iter().map(|(id, node)| (id, node.resolve_proto_node())).collect();

		// Create a network to evaluate each output
		if self.exports.len() == 1 {
			if let NodeInput::Node { node_id, .. } = self.exports[0] {
				return vec![ProtoNetwork {
					inputs: Vec::new(),
					output: node_id,
					nodes,
				}]
				.into_iter();
			}
		}

		// Create a network to evaluate each output
		let networks: Vec<_> = self
			.exports
			.into_iter()
			.filter_map(move |output| {
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
			.collect();
		networks.into_iter()
	}

	/// Create a [`RecursiveNodeIter`] that iterates over all [`DocumentNode`]s, including ones that are deeply nested.
	pub fn recursive_nodes(&self) -> RecursiveNodeIter<'_> {
		let nodes = self.nodes.iter().map(|(id, node)| (id, node, Vec::new())).collect();
		RecursiveNodeIter { nodes }
	}
}

/// An iterator over all [`DocumentNode`]s, including ones that are deeply nested.
pub struct RecursiveNodeIter<'a> {
	nodes: Vec<(&'a NodeId, &'a DocumentNode, Vec<NodeId>)>,
}

impl<'a> Iterator for RecursiveNodeIter<'a> {
	type Item = (&'a NodeId, &'a DocumentNode, Vec<NodeId>);
	fn next(&mut self) -> Option<Self::Item> {
		let (current_id, node, path) = self.nodes.pop()?;
		if let DocumentNodeImplementation::Network(network) = &node.implementation {
			self.nodes.extend(network.nodes.iter().map(|(id, node)| {
				let mut nested_path = path.clone();
				nested_path.push(*current_id);
				(id, node, nested_path)
			}));
		}
		Some((current_id, node, path))
	}
}

fn migrate_call_argument<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Type, D::Error> {
	use serde::Deserialize;

	#[derive(serde::Serialize, serde::Deserialize)]
	#[serde(untagged)]
	enum CallArg {
		New(Type),
		Old(Option<Type>),
	}

	Ok(match CallArg::deserialize(deserializer)? {
		CallArg::New(ty) => ty,
		CallArg::Old(ty) => ty.unwrap_or_default(),
	})
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::proto::{ConstructionArgs, ProtoNetwork, ProtoNode};
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
						inputs: vec![NodeInput::import(concrete!(u32), 0), NodeInput::import(concrete!(u32), 1)],
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
		let mapped_add = NodeNetwork {
			exports: vec![NodeInput::node(NodeId(2), 0)],
			nodes: [
				(
					NodeId(1),
					DocumentNode {
						inputs: vec![NodeInput::import(concrete!(u32), 0), NodeInput::import(concrete!(u32), 1)],
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
					inputs: vec![NodeInput::node(NodeId(0), 0)],
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
		assert!(matches!(&inputs[0].as_value(), &Some(TaggedValue::DocumentNode(network), ..) if network == &id_node));
	}

	#[test]
	fn flatten_add() {
		let mut network = NodeNetwork {
			exports: vec![NodeInput::node(NodeId(1), 0)],
			nodes: [(
				NodeId(1),
				DocumentNode {
					inputs: vec![NodeInput::import(concrete!(u32), 0), NodeInput::value(TaggedValue::U32(2), false)],
					implementation: DocumentNodeImplementation::Network(add_network()),
					..Default::default()
				},
			)]
			.into_iter()
			.collect(),
			..Default::default()
		};
		network.populate_dependants();
		network.flatten_with_fns(NodeId(1), |self_id, inner_id| NodeId(self_id.0 * 10 + inner_id.0), gen_node_id);
		let flat_network = flat_network();
		println!("{flat_network:#?}");
		println!("{network:#?}");

		assert_eq!(flat_network, network);
	}

	#[test]
	fn resolve_proto_node_add() {
		let document_node = DocumentNode {
			inputs: vec![NodeInput::node(NodeId(0), 0)],
			call_argument: concrete!(u32),
			implementation: DocumentNodeImplementation::ProtoNode("graphene_core::structural::ConsNode".into()),
			..Default::default()
		};

		let proto_node = document_node.resolve_proto_node();
		let reference = ProtoNode {
			identifier: "graphene_core::structural::ConsNode".into(),
			call_argument: concrete!(u32),
			construction_args: ConstructionArgs::Nodes(vec![NodeId(0)]),
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
						call_argument: concrete!(u32),
						construction_args: ConstructionArgs::Nodes(vec![NodeId(14)]),
						original_location: OriginalLocation {
							path: Some(vec![NodeId(1), NodeId(0)]),
							inputs_source: [(Source { node: vec![NodeId(1)], index: 1 }, 1)].into(),
							inputs_exposed: vec![true, true],
							..Default::default()
						},

						..Default::default()
					},
				),
				(
					NodeId(11),
					ProtoNode {
						identifier: "graphene_core::ops::AddPairNode".into(),
						call_argument: concrete!(Context),
						construction_args: ConstructionArgs::Nodes(vec![NodeId(10)]),
						original_location: OriginalLocation {
							path: Some(vec![NodeId(1), NodeId(1)]),
							inputs_source: HashMap::new(),
							inputs_exposed: vec![true],
							..Default::default()
						},
						..Default::default()
					},
				),
				(
					NodeId(14),
					ProtoNode {
						identifier: "graphene_core::value::ClonedNode".into(),
						call_argument: concrete!(graphene_core::Context),
						construction_args: ConstructionArgs::Value(TaggedValue::U32(2).into()),
						original_location: OriginalLocation {
							path: Some(vec![NodeId(1), NodeId(4)]),
							inputs_source: HashMap::new(),
							inputs_exposed: vec![true, false],
							..Default::default()
						},
						..Default::default()
					},
				),
			]
			.into_iter()
			.collect(),
		};
		let network = flat_network();
		let mut resolved_network = network.into_proto_networks().collect::<Vec<_>>();
		resolved_network[0].nodes.sort_unstable_by_key(|(id, _)| *id);

		println!("{:#?}", resolved_network[0]);
		println!("{construction_network:#?}");
		pretty_assertions::assert_eq!(resolved_network[0], construction_network);
	}

	fn flat_network() -> NodeNetwork {
		NodeNetwork {
			exports: vec![NodeInput::node(NodeId(11), 0)],
			nodes: [
				(
					NodeId(10),
					DocumentNode {
						inputs: vec![NodeInput::node(NodeId(14), 0)],
						call_argument: concrete!(u32),
						implementation: DocumentNodeImplementation::ProtoNode("graphene_core::structural::ConsNode".into()),
						original_location: OriginalLocation {
							path: Some(vec![NodeId(1), NodeId(0)]),
							inputs_source: [(Source { node: vec![NodeId(1)], index: 1 }, 1)].into(),
							inputs_exposed: vec![true, true],
							..Default::default()
						},
						..Default::default()
					},
				),
				(
					NodeId(14),
					DocumentNode {
						inputs: vec![NodeInput::value(TaggedValue::U32(2), false)],
						implementation: DocumentNodeImplementation::ProtoNode("graphene_core::value::ClonedNode".into()),
						original_location: OriginalLocation {
							path: Some(vec![NodeId(1), NodeId(4)]),
							inputs_source: HashMap::new(),
							inputs_exposed: vec![true, false],
							..Default::default()
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
							inputs_exposed: vec![true],
							..Default::default()
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
						inputs: vec![NodeInput::import(concrete!(u32), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(graphene_core::ops::identity::IDENTIFIER),
						..Default::default()
					},
				),
				(
					NodeId(2),
					DocumentNode {
						inputs: vec![NodeInput::import(concrete!(u32), 1)],
						implementation: DocumentNodeImplementation::ProtoNode(graphene_core::ops::identity::IDENTIFIER),
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
						implementation: DocumentNodeImplementation::ProtoNode(graphene_core::ops::identity::IDENTIFIER),
						..Default::default()
					},
				),
			]
			.into_iter()
			.collect(),
			..Default::default()
		};
		let _new_ids = 101..;
		network.populate_dependants();
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
}
