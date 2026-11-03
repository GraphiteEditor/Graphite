pub mod value;

use crate::document::value::TaggedValue;
use core_types::memo::MemoHashGuard;
pub use core_types::uuid::NodeId;
pub use core_types::uuid::generate_uuid;
use core_types::{Context, Cow, MemoHash, NodeCompileMetadata, NodeParameter, ProtoNodeIdentifier, Type};
use dyn_any::DynAny;
use glam::IVec2;
use log::Metadata;
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::HashMap;
use std::hash::Hash;

/// Utility function for providing a default boolean value to serde.
#[inline(always)]
fn return_true() -> bool {
	true
}

/// An instance of a [`DocumentNodeDefinition`] that has been instantiated in a [`NodeNetwork`].
/// Currently, when an instance is made, it lives all on its own without any lasting connection to the definition.
/// But we will want to change it in the future so it merely references its definition.
#[derive(Clone, Debug, PartialEq, DynAny, serde::Serialize, serde::Deserialize)]
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
	/// Represents the eye icon for hiding/showing the node in the graph UI. When hidden, a node gets replaced with a passthrough node during the graph flattening step.
	#[serde(default = "return_true")]
	pub visible: bool,
	/// When two different proto nodes hash to the same value (e.g. two value nodes each containing `2_u32` or two multiply nodes that have the same node IDs as input), the duplicates are removed.
	/// See [`ProtoNetwork::generate_stable_node_ids`] for details.
	/// However sometimes this is not desirable, for example in the case of a [`graphene_core::memo::MonitorNode`] that needs to be accessed outside of the graph.
	#[serde(default)]
	pub skip_deduplication: bool,
	#[serde(skip)]
	pub compile_metadata: Option<NodeCompileMetadata>,
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
#[derive(Clone, Debug, PartialEq, Eq, Hash, DynAny, Default, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub struct OriginalLocation {
	/// The original location to the document node - e.g. [grandparent_id, parent_id, node_id].
	pub path: Option<Vec<NodeId>>,
	/// List of nodes which depend on this node
	pub dependants: Vec<Vec<NodeId>>,
	/// For automatically inserted Convert and Into nodes, if there is an error, display it on the node it is connect to.
	pub auto_convert_index: Option<usize>,
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
			compile_metadata: Default::default(),
		}
	}
}

impl DocumentNode {
	/// The input slot named by the given parameter symbol, e.g. `node.input(stroke::WeightInput)`.
	pub fn input<P: NodeParameter>(&self, _parameter: P) -> Option<&NodeInput> {
		self.inputs.get(P::INDEX)
	}

	/// Mutable access to the input slot named by the given parameter symbol.
	pub fn input_mut<P: NodeParameter>(&mut self, _parameter: P) -> Option<&mut NodeInput> {
		self.inputs.get_mut(P::INDEX)
	}

	/// The stored value of the given parameter, if that input currently holds a value rather than a wire.
	pub fn input_value<P: NodeParameter>(&self, parameter: P) -> Option<&TaggedValue> {
		self.input(parameter)?.as_value()
	}

	/// Normalizes this node's stored types (call argument, `Import` input types, `TypeDefault` value payloads, and any nested network) to their structural form.
	/// Applied once at ingestion (document migration and clipboard paste) so no name-encoded ranked type enters a live document.
	pub fn normalize_stored_types(&mut self) {
		self.call_argument = self.call_argument.clone().normalize_rank();
		for input in &mut self.inputs {
			normalize_input_stored_type(input);
		}
		if let Some(network) = self.implementation.get_network_mut() {
			network.normalize_stored_types();
		}
	}
}

/// Represents the possible inputs to a node.
#[derive(Debug, Clone, PartialEq, core_types::CacheHash, DynAny, serde::Serialize, serde::Deserialize)]
pub enum NodeInput {
	/// A reference to another node in the same network from which this node can receive its input.
	Node { node_id: NodeId, output_index: usize },

	/// A hardcoded value that can't change after the graph is compiled. Gets converted into a value node during graph compilation.
	Value {
		#[cfg_attr(feature = "loading", serde(deserialize_with = "crate::document::value::deserialize_tagged_value_with_legacy_migration"))]
		tagged_value: MemoHash<TaggedValue>,
		exposed: bool,
	},

	// TODO: Remove import_type and get type from parent node input
	/// Input that is provided by the import from the parent network to this document node network.
	#[serde(alias = "Network")]
	Import { import_type: Type, import_index: usize },

	/// Input that is extracted from the parent scopes the node resides in. The string argument is the key.
	Scope(Cow<'static, str>),

	/// Input that is replaced at graph compilation with introspective metadata about this node's location.
	Reflection(DocumentNodeMetadata),
}

#[derive(Debug, Clone, PartialEq, Hash, core_types::CacheHash, DynAny, serde::Serialize, serde::Deserialize)]
pub enum DocumentNodeMetadata {
	DocumentNodePath,
	SourceId,
}

impl DocumentNodeMetadata {
	pub fn ty(&self) -> Type {
		match self {
			DocumentNodeMetadata::DocumentNodePath => item!(core_types::list::NodeIdPath),
			DocumentNodeMetadata::SourceId => item!(core_types::SourceId),
		}
	}
}

impl NodeInput {
	pub const fn node(node_id: NodeId, output_index: usize) -> Self {
		Self::Node { node_id, output_index }
	}

	pub fn value(tagged_value: TaggedValue, exposed: bool) -> Self {
		let tagged_value = tagged_value.into();
		Self::Value { tagged_value, exposed }
	}

	/// Constructs a `NodeInput::Value` whose tagged value is `TaggedValue::TypeDefault(ty)`, recording only the
	/// type so the runtime materializes its default rather than baking a placeholder value into the saved document.
	pub fn type_default(ty: Type, exposed: bool) -> Self {
		Self::value(TaggedValue::TypeDefault(ty.normalize_rank()), exposed)
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
			NodeInput::Scope(_) => false,
			NodeInput::Reflection(_) => false,
		}
	}

	pub fn ty(&self) -> Type {
		match self {
			NodeInput::Node { .. } => unreachable!("ty() called on NodeInput::Node"),
			NodeInput::Value { tagged_value, .. } => tagged_value.ty(),
			// Stored import types are normalized to their structural form once at document migration
			NodeInput::Import { import_type, .. } => import_type.clone(),
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

#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, core_types::CacheHash, PartialOrd, Ord, DynAny, serde::Serialize, serde::Deserialize)]
pub struct NetworkId(pub u64);

impl NetworkId {
	pub fn new() -> Self {
		Self(generate_uuid())
	}
}

#[derive(Clone, Debug, PartialEq, core_types::CacheHash, DynAny, serde::Serialize, serde::Deserialize)]
/// Represents the implementation of a node, which can be a nested [`NodeNetwork`], a proto [`ProtoNodeIdentifier`], or `Extract`.
pub enum DocumentNodeImplementation {
	/// This describes a (document) node built out of a subgraph of other (document) nodes.
	///
	/// A nested [`NodeNetwork`] that is flattened by the [`NodeNetwork::flatten`] function.
	Network(NodeNetwork),
	Definition(NetworkId),
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
	/// Future use could involve the addition of an InjectNode to convert the source code form back into an executable node, enabling metaprogramming in the node graph.
	/// We would use an assortment of nodes that operate on Graphene source code (just data, no different from any other data flowing through the graph) to make graph transformations.
	///
	/// We use this for dealing with macros in a syntactic way of modifying the node graph from within the graph itself. Just like we often deal with lambdas to represent a whole group of
	/// operations/code/logic, this allows us to basically deal with a lambda at a meta/source-code level, rather than its executable logic.
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
		Self::ProtoNode(graphene_core::ops::passthrough::IDENTIFIER)
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
			DocumentNodeImplementation::Definition(id) => {
				log::error!("The output count of definition {id:?} was requested without access to the definitions table");
				1
			}
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
	/// The name of the node definition, as originally set by [`DocumentNodeDefinition`], used to display in the UI and to display the appropriate Inspector layout.
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
	/// Represents the eye icon for hiding/showing the node in the graph UI. When hidden, a node gets replaced with a passthrough node during the graph flattening step.
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
#[derive(Clone, Debug, PartialEq, Default, Hash, DynAny, serde::Serialize, serde::Deserialize)]
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
	//cfg_attr(feature = "serde", #[serde(serialize_with = "core_types::vector::serialize_hashmap", deserialize_with = "core_types::vector::deserialize_hashmap"))]
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
	//cfg_attr(feature = "serde", #[serde(serialize_with = "core_types::vector::serialize_hashmap", deserialize_with = "core_types::vector::deserialize_hashmap"))]
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
	#[serde(
		serialize_with = "graphic_types::vector_types::vector::serialize_hashmap",
		deserialize_with = "graphic_types::vector_types::vector::deserialize_hashmap"
	)]
	pub nodes: FxHashMap<NodeId, DocumentNode>,
	/// A network may expose nodes as constants which can by used by other nodes using a `NodeInput::Scope(key)`.
	#[serde(default)]
	#[serde(
		serialize_with = "graphic_types::vector_types::vector::serialize_hashmap",
		deserialize_with = "graphic_types::vector_types::vector::deserialize_hashmap"
	)]
	pub scope_injections: FxHashMap<String, (NodeId, Type)>,
	#[serde(default, skip_serializing_if = "FxHashMap::is_empty")]
	pub definitions: FxHashMap<NetworkId, NodeNetwork>,
	#[serde(skip)]
	pub generated: bool,
}

/// Where a network of the document lives: a definitions table entry, or `None` for the document network itself.
pub type NetworkLocation = Option<NetworkId>;

/// The output count of `implementation`, or `None` for a table entry that `definitions` lacks.
pub(crate) fn output_count_in(implementation: &DocumentNodeImplementation, definitions: &FxHashMap<NetworkId, NodeNetwork>) -> Option<usize> {
	match implementation {
		DocumentNodeImplementation::Definition(id) => definitions.get(id).map(|definition| definition.exports.len()),
		_ => Some(implementation.output_count()),
	}
}

/// One inline network that [`NodeNetwork::lift_nested_networks`] moved into the definitions table.
/// The node `node_id` in table entry `containing` (`None` for the document network) now names it as `id`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiftedNetwork {
	pub containing: Option<NetworkId>,
	pub node_id: NodeId,
	pub id: NetworkId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DefinitionError {
	Missing(NetworkId),
	Cycle(Vec<NetworkId>),
	NestedTable,
}

impl std::fmt::Display for DefinitionError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Missing(id) => write!(f, "Definition {id:?} is referenced but not defined"),
			Self::Cycle(cycle) => write!(f, "Definitions reference each other in a cycle: {cycle:?}"),
			Self::NestedTable => write!(f, "Only the document network may hold definitions"),
		}
	}
}

impl std::error::Error for DefinitionError {}

fn validate_named_definitions(
	network: &NodeNetwork,
	definitions: &FxHashMap<NetworkId, NodeNetwork>,
	visiting: &mut Vec<NetworkId>,
	validated: &mut FxHashSet<NetworkId>,
) -> Result<(), DefinitionError> {
	for node in network.nodes.values() {
		match &node.implementation {
			DocumentNodeImplementation::Definition(id) => {
				if let Some(start) = visiting.iter().position(|visiting| visiting == id) {
					return Err(DefinitionError::Cycle(visiting[start..].to_vec()));
				}
				if validated.contains(id) {
					continue;
				}

				let definition = definitions.get(id).ok_or(DefinitionError::Missing(*id))?;
				if !definition.definitions.is_empty() {
					return Err(DefinitionError::NestedTable);
				}
				visiting.push(*id);
				validate_named_definitions(definition, definitions, visiting, validated)?;
				visiting.pop();
				validated.insert(*id);
			}
			DocumentNodeImplementation::Network(nested) => {
				if !nested.definitions.is_empty() {
					return Err(DefinitionError::NestedTable);
				}
				validate_named_definitions(nested, definitions, visiting, validated)?;
			}
			_ => {}
		}
	}
	Ok(())
}

struct DefinitionInliner {
	definitions: FxHashMap<NetworkId, NodeNetwork>,
	inlined: FxHashMap<NetworkId, NodeNetwork>,
	visiting: Vec<NetworkId>,
}

impl DefinitionInliner {
	fn inline_network(&mut self, network: &mut NodeNetwork) -> Result<(), DefinitionError> {
		if !network.definitions.is_empty() {
			return Err(DefinitionError::NestedTable);
		}

		for node in network.nodes.values_mut() {
			if let DocumentNodeImplementation::Definition(id) = node.implementation {
				node.implementation = DocumentNodeImplementation::Network(self.inlined_definition(id)?.clone());
			} else if let DocumentNodeImplementation::Network(nested) = &mut node.implementation {
				self.inline_network(nested)?;
			}
		}

		Ok(())
	}

	fn inlined_definition(&mut self, id: NetworkId) -> Result<&NodeNetwork, DefinitionError> {
		if !self.inlined.contains_key(&id) {
			if let Some(start) = self.visiting.iter().position(|&visiting| visiting == id) {
				return Err(DefinitionError::Cycle(self.visiting[start..].to_vec()));
			}

			let mut network = self.definitions.get(&id).ok_or(DefinitionError::Missing(id))?.clone();
			self.visiting.push(id);
			self.inline_network(&mut network)?;
			self.visiting.pop();
			self.inlined.insert(id, network);
		}

		Ok(&self.inlined[&id])
	}
}

impl core_types::CacheHash for NodeNetwork {
	fn cache_hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
		self.exports.cache_hash(state);

		let mut nodes: Vec<_> = self.nodes.iter().collect();
		nodes.sort_by_key(|(id, _)| *id);
		for (id, node) in nodes {
			id.cache_hash(state);
			node.cache_hash(state);
		}

		let mut scope_injections: Vec<_> = self.scope_injections.iter().collect();
		scope_injections.sort_by_key(|(key, _)| key.as_str());
		for (key, (node_id, ty)) in scope_injections {
			key.cache_hash(state);
			node_id.cache_hash(state);
			ty.cache_hash(state);
		}

		let mut definitions: Vec<_> = self.definitions.iter().collect();
		definitions.sort_by_key(|(id, _)| *id);
		for (id, definition) in definitions {
			id.cache_hash(state);
			definition.cache_hash(state);
		}
	}
}

impl PartialEq for NodeNetwork {
	fn eq(&self, other: &Self) -> bool {
		self.exports == other.exports && self.nodes == other.nodes && self.scope_injections == other.scope_injections && self.definitions == other.definitions
	}
}

/// Graph modification functions
impl NodeNetwork {
	pub fn current_hash(&self) -> u64 {
		use core_types::graphene_hash::CacheHash;
		use rustc_hash::FxHasher;
		use std::hash::Hasher;
		let mut hasher = FxHasher::default();
		self.cache_hash(&mut hasher);
		hasher.finish()
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
		let mut network = self;

		for segment in nested_path {
			network = self.implementation_network(&network.nodes.get(segment)?.implementation)?;
		}
		Some(network)
	}

	/// Get the mutable nested network given by the path of node ids
	pub fn nested_network_mut(&mut self, nested_path: &[NodeId]) -> Option<&mut Self> {
		let location = self.locate(nested_path)?;
		self.located_network_mut(location)
	}

	/// The location of the network at `nested_path`, or `None` if the path doesn't end at a table entry.
	pub fn locate(&self, nested_path: &[NodeId]) -> Option<NetworkLocation> {
		let Some((node_id, parent_path)) = nested_path.split_last() else { return Some(None) };
		match self.nested_network(parent_path)?.nodes.get(node_id)?.implementation {
			DocumentNodeImplementation::Definition(id) if self.definitions.contains_key(&id) => Some(Some(id)),
			_ => None,
		}
	}

	pub fn located_network(&self, location: NetworkLocation) -> Option<&Self> {
		match location {
			Some(id) => self.definitions.get(&id),
			None => Some(self),
		}
	}

	pub fn located_network_mut(&mut self, location: NetworkLocation) -> Option<&mut Self> {
		match location {
			Some(id) => self.definitions.get_mut(&id),
			None => Some(self),
		}
	}

	pub fn implementation_network<'a>(&'a self, implementation: &'a DocumentNodeImplementation) -> Option<&'a Self> {
		match implementation {
			DocumentNodeImplementation::Network(network) => Some(network),
			DocumentNodeImplementation::Definition(id) => self.definitions.get(id),
			_ => None,
		}
	}

	pub fn implementation_output_count(&self, implementation: &DocumentNodeImplementation) -> usize {
		output_count_in(implementation, &self.definitions).unwrap_or(0)
	}

	/// The path of the node producing this output once compiled, following network exports and the primary inputs of hidden and Passthrough nodes.
	pub fn output_source(&self, node_path: &[NodeId], output_index: usize) -> Option<Vec<NodeId>> {
		let (node_id, network_path) = node_path.split_last()?;
		let node = self.nested_network(network_path)?.nodes.get(node_id)?;

		let passthrough = DocumentNodeImplementation::ProtoNode(graphene_core::ops::passthrough::IDENTIFIER);
		if !node.visible || node.implementation == passthrough {
			return self.input_source(network_path, node.inputs.first()?);
		}

		match self.implementation_network(&node.implementation) {
			Some(network) => self.input_source(node_path, network.exports.get(output_index)?),
			None => Some(node_path.to_vec()),
		}
	}

	/// The path of the node producing what this input in the network at the path receives, following imports out to the network node's inputs.
	pub fn input_source(&self, network_path: &[NodeId], input: &NodeInput) -> Option<Vec<NodeId>> {
		match input {
			NodeInput::Node { node_id, output_index } => self.output_source(&[network_path, &[*node_id]].concat(), *output_index),
			NodeInput::Import { import_index, .. } => {
				let (network_node_id, parent_network_path) = network_path.split_last()?;
				let network_node = self.nested_network(parent_network_path)?.nodes.get(network_node_id)?;
				self.input_source(parent_network_path, network_node.inputs.get(*import_index)?)
			}
			_ => None,
		}
	}

	/// Moves every inline network at every nesting level into the definitions table under a fresh ID.
	/// Returns the moves so that each network is listed before the networks lifted out of it.
	pub fn lift_nested_networks(&mut self) -> Vec<LiftedNetwork> {
		let mut lifted = Vec::new();
		let mut pending: Vec<Option<NetworkId>> = std::iter::once(None).chain(self.definitions.keys().copied().map(Some)).collect();

		while let Some(containing) = pending.pop() {
			let network = match containing {
				None => &mut *self,
				Some(id) => match self.definitions.get_mut(&id) {
					Some(definition) => definition,
					None => continue,
				},
			};

			let mut node_ids: Vec<NodeId> = network
				.nodes
				.iter()
				.filter(|(_, node)| matches!(node.implementation, DocumentNodeImplementation::Network(_)))
				.map(|(node_id, _)| *node_id)
				.collect();
			node_ids.sort();

			let mut bodies = Vec::with_capacity(node_ids.len());
			for node_id in node_ids {
				let Some(node) = network.nodes.get_mut(&node_id) else { continue };
				let id = NetworkId::new();
				if let DocumentNodeImplementation::Network(body) = std::mem::replace(&mut node.implementation, DocumentNodeImplementation::Definition(id)) {
					bodies.push((id, body));
					lifted.push(LiftedNetwork { containing, node_id, id });
				}
			}

			for (id, body) in bodies {
				self.definitions.insert(id, body);
				pending.push(Some(id));
			}
		}

		lifted
	}

	/// The table entries the document network reaches through its nodes, directly or through other entries.
	pub fn reachable_definitions(&self) -> FxHashSet<NetworkId> {
		fn visit(network: &NodeNetwork, definitions: &FxHashMap<NetworkId, NodeNetwork>, reached: &mut FxHashSet<NetworkId>) {
			for node in network.nodes.values() {
				match &node.implementation {
					DocumentNodeImplementation::Definition(id) => {
						if reached.insert(*id)
							&& let Some(definition) = definitions.get(id)
						{
							visit(definition, definitions, reached);
						}
					}
					DocumentNodeImplementation::Network(nested) => visit(nested, definitions, reached),
					_ => {}
				}
			}
		}

		let mut reached = FxHashSet::default();
		visit(self, &self.definitions, &mut reached);
		reached
	}

	/// Drops every table entry the document network does not reach, and returns their IDs in sorted order.
	pub fn retain_reachable_definitions(&mut self) -> Vec<NetworkId> {
		let reached = self.reachable_definitions();
		let mut dropped: Vec<NetworkId> = self.definitions.keys().filter(|id| !reached.contains(id)).copied().collect();
		dropped.sort();
		for id in &dropped {
			self.definitions.remove(id);
		}
		dropped
	}

	/// Checks that every table entry a node names exists, that no entries name each other in a cycle, and that only this network holds a table.
	pub fn validate_definitions(&self) -> Result<(), DefinitionError> {
		validate_named_definitions(self, &self.definitions, &mut Vec::new(), &mut FxHashSet::default())
	}

	pub fn inline_definitions(&mut self) -> Result<(), DefinitionError> {
		let mut inliner = DefinitionInliner {
			definitions: std::mem::take(&mut self.definitions),
			inlined: FxHashMap::default(),
			visiting: Vec::new(),
		};
		inliner.inline_network(self)
	}

	/// Is the node being used directly as an output?
	pub fn outputs_contain(&self, node_id_to_check: NodeId) -> bool {
		self.exports
			.iter()
			.any(|output| if let NodeInput::Node { node_id, .. } = output { *node_id == node_id_to_check } else { false })
	}

	/// Check there are no cycles in the graph (this should never happen).
	pub fn is_acyclic(&self) -> bool {
		self.is_acyclic_with(None)
	}

	/// Whether the network is acyclic, optionally answering for one input replaced by a proposed value
	/// rather than for the inputs as they stand, so a write can be tested without being performed.
	///
	/// `replaced` names the node, the input index, and what would be written there.
	pub fn is_acyclic_with(&self, replaced: Option<(NodeId, usize, &NodeInput)>) -> bool {
		let dependency = |input: &NodeInput| if let NodeInput::Node { node_id, .. } = input { Some(*node_id) } else { None };

		let mut dependencies: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
		for (node_id, node) in &self.nodes {
			dependencies.insert(
				*node_id,
				node.inputs
					.iter()
					.enumerate()
					.filter_map(|(index, input)| match replaced {
						Some((replaced_node, replaced_index, proposed)) if replaced_node == *node_id && replaced_index == index => dependency(proposed),
						_ => dependency(input),
					})
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
/// Normalizes the ranked types an input can store: an `Import`'s type or a value's `TypeDefault` payload.
fn normalize_input_stored_type(input: &mut NodeInput) {
	match input {
		NodeInput::Import { import_type, .. } => *import_type = import_type.clone().normalize_rank(),
		NodeInput::Value { tagged_value, .. } => {
			if let TaggedValue::TypeDefault(ty) = &**tagged_value {
				let normalized = ty.clone().normalize_rank();
				if normalized != *ty {
					*tagged_value = TaggedValue::TypeDefault(normalized).into();
				}
			}
		}
		_ => {}
	}
}

impl NodeNetwork {
	/// Normalizes every stored type in the network (exports and each node's types) to the structural form, recursively.
	pub fn normalize_stored_types(&mut self) {
		for export in &mut self.exports {
			normalize_input_stored_type(export);
		}
		for node in self.nodes.values_mut() {
			node.normalize_stored_types();
		}
		for definition in self.definitions.values_mut() {
			definition.normalize_stored_types();
		}
	}

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

	/// Create a [`RecursiveNodeIter`] that iterates over all [`DocumentNode`]s, including ones that are deeply nested.
	pub fn recursive_nodes(&self) -> RecursiveNodeIter<'_> {
		let nodes = self.nodes.iter().map(|(id, node)| (id, node, Vec::new())).collect();
		RecursiveNodeIter {
			definitions: &self.definitions,
			visited_definitions: FxHashSet::default(),
			nodes,
		}
	}
}

/// An iterator over all [`DocumentNode`]s, including ones that are deeply nested.
/// Each definition body is visited once, under the path of the first instance reached.
pub struct RecursiveNodeIter<'a> {
	definitions: &'a FxHashMap<NetworkId, NodeNetwork>,
	visited_definitions: FxHashSet<NetworkId>,
	nodes: Vec<(&'a NodeId, &'a DocumentNode, Vec<NodeId>)>,
}

impl<'a> Iterator for RecursiveNodeIter<'a> {
	type Item = (&'a NodeId, &'a DocumentNode, Vec<NodeId>);
	fn next(&mut self) -> Option<Self::Item> {
		let (current_id, node, path) = self.nodes.pop()?;
		let nested_network = match &node.implementation {
			DocumentNodeImplementation::Network(network) => Some(network),
			DocumentNodeImplementation::Definition(id) if self.visited_definitions.insert(*id) => self.definitions.get(id),
			_ => None,
		};
		if let Some(network) = nested_network {
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

impl core_types::graphene_hash::CacheHash for DocumentNode {
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.inputs.cache_hash(state);
		self.call_argument.cache_hash(state);
		self.implementation.cache_hash(state);
		self.visible.cache_hash(state);
		self.skip_deduplication.cache_hash(state);
		self.compile_metadata.cache_hash(state);
	}
}

#[cfg(test)]
pub(crate) mod document_tests;
