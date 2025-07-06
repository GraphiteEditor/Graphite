pub mod value;

use crate::document::value::TaggedValue;
use crate::proto::{ConstructionArgs, NodeConstructionArgs, OriginalLocation, ProtoNode};
use dyn_any::DynAny;
use glam::IVec2;
use graphene_core::memo::MemoHashGuard;
pub use graphene_core::uuid::generate_uuid;
use graphene_core::uuid::{CompiledProtonodeInput, NodeId, ProtonodePath, SNI};
use graphene_core::{Context, Cow, MemoHash, ProtoNodeIdentifier, Type};
use rustc_hash::FxHashMap;
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

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
	/// - A [`NodeInput::Network`] which specifies that this input is from outside the graph, which is resolved in the graph flattening step in the case of nested networks.
	///
	/// In the root network, it is resolved when evaluating the borrow tree.
	/// Ensure the click target in the encapsulating network is updated when the inputs cause the node shape to change (currently only when exposing/hiding an input)
	/// by using network.update_click_target(node_id).
	#[cfg_attr(target_arch = "wasm32", serde(alias = "outputs"))]
	pub inputs: Vec<NodeInput>,
	// A nested document network or a proto-node identifier.
	pub implementation: DocumentNodeImplementation,
	/// Represents the eye icon for hiding/showing the node in the graph UI. When hidden, a node gets replaced with an identity node during the graph flattening step.
	#[serde(default = "return_true")]
	pub visible: bool,
	pub manual_composition: Option<Type>,
	#[serde(default)]
	pub skip_deduplication: bool,
}

impl Hash for DocumentNode {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.inputs.hash(state);
		self.implementation.hash(state);
		self.visible.hash(state);
	}
}

impl Default for DocumentNode {
	fn default() -> Self {
		Self {
			inputs: Default::default(),
			implementation: Default::default(),
			visible: true,
			manual_composition: Some(generic!(T)),
			skip_deduplication: false,
		}
	}
}

/// Represents the possible inputs to a node.
#[derive(Debug, Clone, PartialEq, Hash, DynAny, serde::Serialize, serde::Deserialize)]
pub enum NodeInput {
	/// A reference to another node in the same network from which this node can receive its input.
	Node { node_id: NodeId, output_index: usize, lambda: bool },

	/// A hardcoded value that can't change after the graph is compiled. Gets converted into a value node during graph compilation.
	Value { tagged_value: MemoHash<TaggedValue>, exposed: bool },

	/// Input that is provided by the parent network to this document node, instead of from a hardcoded value or another node within the same network.
	Network { import_index: usize, import_type: Type },

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
		Self::Node { node_id, output_index, lambda: false }
	}

	pub const fn lambda(node_id: NodeId, output_index: usize) -> Self {
		Self::Node { node_id, output_index, lambda: true }
	}

	pub fn value(tagged_value: TaggedValue, exposed: bool) -> Self {
		let tagged_value = tagged_value.into();
		Self::Value { tagged_value, exposed }
	}

	pub const fn network(import_type: Type, import_index: usize) -> Self {
		Self::Network { import_type, import_index }
	}

	pub fn scope(key: impl Into<Cow<'static, str>>) -> Self {
		Self::Scope(key.into())
	}

	pub fn is_exposed(&self) -> bool {
		match self {
			NodeInput::Node { .. } => true,
			NodeInput::Value { exposed, .. } => *exposed,
			NodeInput::Network { .. } => true,
			NodeInput::Inline(_) => false,
			NodeInput::Scope(_) => false,
			NodeInput::Reflection(_) => false,
		}
	}

	pub fn ty(&self) -> Type {
		match self {
			NodeInput::Node { .. } => unreachable!("ty() called on NodeInput::Node"),
			NodeInput::Value { tagged_value, .. } => tagged_value.ty(),
			NodeInput::Network { .. } => unreachable!("ty() called on NodeInput::Network"),
			NodeInput::Inline(_) => panic!("ty() called on NodeInput::Inline"),
			NodeInput::Scope(_) => unreachable!("ty() called on NodeInput::Scope"),
			NodeInput::Reflection(_) => concrete!(DocumentNodeMetadata),
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

	pub fn is_lambda(&self) -> bool {
		match self {
			NodeInput::Node { lambda, .. } => *lambda,
			_ => false,
		}
	}
}

#[derive(Clone, Debug, DynAny, serde::Serialize, serde::Deserialize)]
/// Represents the implementation of a node, which can be a nested [`NodeNetwork`], a proto [`ProtoNodeIdentifier`], or `Extract`.
pub enum OldDocumentNodeImplementation {
	/// This describes a (document) node built out of a subgraph of other (document) nodes.
	///
	/// A nested [`NodeNetwork`] that is flattened by the [`NodeNetwork::flatten`] function.
	Network(OldNodeNetwork),
	/// This describes a (document) node implemented as a proto node.
	///
	/// A proto node identifier which can be found in `node_registry.rs`.
	#[serde(alias = "Unresolved")] // TODO: Eventually remove this alias document upgrade code
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
	#[serde(default)]
	pub alias: String,
	#[serde(deserialize_with = "migrate_layer_to_merge")]
	pub name: String,
	#[cfg_attr(target_arch = "wasm32", serde(alias = "outputs"))]
	pub inputs: Vec<NodeInput>,
	pub manual_composition: Option<Type>,
	#[serde(default = "return_true")]
	pub has_primary_output: bool,
	pub implementation: OldDocumentNodeImplementation,
	#[serde(default)]
	pub is_layer: bool,
	#[serde(default = "return_true")]
	pub visible: bool,
	#[serde(default)]
	pub locked: bool,
	pub metadata: OldDocumentNodeMetadata,
	#[serde(default)]
	pub skip_deduplication: bool,
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
	#[cfg_attr(target_arch = "wasm32", serde(alias = "outputs", deserialize_with = "deserialize_exports"))]
	pub exports: Vec<NodeInput>,
	// TODO: Instead of storing import types in each NodeInput::Network connection, the types are stored here. This is similar to how types need to be defined for parameters when creating a function in Rust.
	// pub import_types: Vec<Type>,
	/// The list of all nodes in this network.
	#[serde(serialize_with = "graphene_core::vector::serialize_hashmap", deserialize_with = "graphene_core::vector::deserialize_hashmap")]
	pub nodes: FxHashMap<NodeId, DocumentNode>,
	/// A network may expose nodes as constants which can by used by other nodes using a `NodeInput::Scope(key)`.
	#[serde(default)]
	#[serde(serialize_with = "graphene_core::vector::serialize_hashmap", deserialize_with = "graphene_core::vector::deserialize_hashmap")]
	pub scope_injections: FxHashMap<String, TaggedValue>,
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

/// Graph helper functions
impl NodeNetwork {
	pub fn current_hash(&self) -> u64 {
		let mut hasher = DefaultHasher::new();
		self.hash(&mut hasher);
		hasher.finish()
	}

	/// Get the nested network given by the path of node ids
	pub fn nested_network(&self, nested_path: &[NodeId]) -> Option<&Self> {
		let mut network = Some(self);

		for segment in nested_path {
			network = network.and_then(|network| network.nodes.get(segment)).and_then(|node| node.implementation.get_network());
		}
		network
	}

	pub fn value_network(node: DocumentNode) -> Self {
		Self {
			exports: vec![NodeInput::node(NodeId(0), 0)],
			nodes: [(NodeId(0), node)].into_iter().collect(),
			..Default::default()
		}
	}

	/// Get the mutable nested network given by the path of node ids
	pub fn nested_network_mut(&mut self, nested_path: &[NodeId]) -> Option<&mut Self> {
		let mut network = Some(self);

		for segment in nested_path {
			network = network.and_then(|network| network.nodes.get_mut(segment)).and_then(|node| node.implementation.get_network_mut());
		}
		network
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
	// Returns a topologically sorted vec of protonodes, as well as metadata extracted during compilation
	// Compiles a network with one export where any scope injections are added the top level network, and the network to run is implemented as a DocumentNodeImplementation::Network
	// The traversal input is the node which calls the network to be flattened. If it is None, then start from the export.
	// Every value protonode stores the connector which directly called it, which is used to map the value input to the protonode caller.
	// Every value input connector is mapped to its caller, and every protonode is mapped to its caller. If there are multiple, then they are compared to ensure it is the same between compilations
	pub fn flatten(&mut self) -> Result<(Vec<ProtoNode>, Vec<(AbsoluteInputConnector, CompiledProtonodeInput)>, Vec<(ProtonodePath, CompiledProtonodeInput)>), String> {
		// These three arrays are stored in parallel
		let mut protonetwork = Vec::new();
		let mut value_connectors = Vec::new();
		let mut protonode_paths = Vec::new();
		let mut calling_protonodes = HashMap::new();

		// This function creates a flattened network with populated original location fields but unmapped inputs
		// The input to flattened protonode hashmap is used to map the inputs
		self.traverse_input(
			&mut protonetwork,
			&mut value_connectors,
			&mut protonode_paths,
			&mut calling_protonodes,
			&mut HashMap::new(),
			AbsoluteInputConnector::traversal_start(),
			(0, 0),
		);

		let mut generated_snis = HashSet::new();
		// Generate SNI's. This gets called after all node inputs are replaced with their indices
		for protonode_index in (0..protonetwork.len()).rev() {
			let protonode = protonetwork.get_mut(protonode_index).unwrap();
			if let ConstructionArgs::Nodes(NodeConstructionArgs { inputs: input_snis, .. }) = &protonode.construction_args {
				for input_sni in input_snis {
					assert_ne!(
						*input_sni,
						NodeId(0),
						"All inputs should be mapped to a stable node index, and the calling nodes inputs should be updated"
					);
				}
			}

			use std::hash::Hasher;
			let mut hasher = rustc_hash::FxHasher::default();
			protonode.construction_args.hash(&mut hasher);
			let mut stable_node_id = NodeId(hasher.finish());
			// The stable node index must be unique for every protonode. If it has the same hash as another protonode, continue hashing itself
			// For example two cache nodes connected to a Context getter node have two cache different values, even though the stable node id is the same.
			while !generated_snis.insert(stable_node_id) {
				stable_node_id.hash(&mut hasher);
				stable_node_id = NodeId(hasher.finish());
			}

			protonode.stable_node_id = stable_node_id;
			for (calling_node_index, input_index) in calling_protonodes.get(&protonode_index).unwrap() {
				match &mut protonetwork.get_mut(*calling_node_index).unwrap().construction_args {
					ConstructionArgs::Nodes(nodes) => {
						*nodes.inputs.get_mut(*input_index).unwrap() = stable_node_id;
					}
					// TODO: Implement for extract
					_ => unreachable!(),
				}
			}
		}

		// Do another traversal now that the caller SNI have been generated to collect metadata for the editor
		let mut value_connector_callers = Vec::new();
		let mut protonode_callers = Vec::new();

		for (protonode_index, (value_connector, protonode_path)) in value_connectors.iter_mut().zip(protonode_paths.iter_mut()).enumerate().rev() {
			let callers = calling_protonodes.get(&protonode_index).unwrap();

			let &(min_protonode_index, input_index) = callers.iter().min().unwrap();

			let protonode_id = protonetwork[min_protonode_index].stable_node_id;

			if let Some(value_connector) = value_connector.take() {
				value_connector_callers.push((value_connector, (protonode_id, input_index)));
			}

			if let Some(protonode_path) = protonode_path.take() {
				protonode_callers.push((protonode_path, (protonode_id, input_index)));
			}
		}

		let mut existing_ids = HashSet::new();
		// Value nodes can be deduplicated if they share the same hash, since they do not depend on the input
		let protonetwork = protonetwork
			.into_iter()
			.filter(|protonode| !(matches!(protonode.construction_args, ConstructionArgs::Value(_)) && !existing_ids.insert(protonode.stable_node_id)))
			.collect();
		Ok((protonetwork, value_connector_callers, protonode_callers))
	}

	fn get_input_from_absolute_connector(&mut self, traversal_input: &AbsoluteInputConnector) -> Option<&mut NodeInput> {
		let network_path = &traversal_input.network_path;
		let Some(nested_network) = self.nested_network_mut(network_path) else {
			log::error!("traversal_input network does not exist, path {:?}", network_path);
			return None;
		};
		match &traversal_input.connector {
			// Input from an export
			InputConnector::Export(export_index) => {
				let Some(input) = nested_network.exports.get_mut(*export_index) else {
					log::error!(
						"The output which was supposed to be flattened does not exist in the network {:?}, index {:?}",
						&network_path,
						export_index
					);
					return None;
				};
				Some(input)
			}
			// Input from a protonode or network node
			InputConnector::Node { node_id, input_index } => {
				let Some(document_node) = nested_network.nodes.get_mut(node_id) else {
					log::error!("The node which was supposed to be flattened does not exist in the network, id {node_id}");
					return None;
				};
				let Some(input) = document_node.inputs.get_mut(*input_index) else {
					log::error!("The output which was supposed to be flattened does not exist in the network, index {input_index}");
					return None;
				};
				Some(input)
			}
		}
	}
	// Performs a recursive graph traversal starting from all protonode inputs and the root export until reaching the next protonode or value input.
	// Automatically inserts value nodes by moving the value from the current network
	//
	// protonetwork - The topologically sorted flattened protonetwork. The caller of each protonode is at a lower index. The output of the network is the first protonode
	//
	// calling protonodes - anytime a protonode is reached, the caller is added as a value with (caller protonetwork index, caller input index).
	// This is necessary so the calling protonodes input can be looked up and mapped when generating SNI's
	//
	// Protonode indices - mapping of protonode path to its index in the protonetwork, updated when inserting a protonode
	//
	// Traversal input - current connector to traverse over. added to downstream_calling_inputs every time the function is called.
	//
	// downstream_calling_inputs - tracks all inputs reached during traversal
	//
	// any_input_to_downstream_protonode_input - used by the runtime/javascript to get the calling protonode input from any input connector.
	// When a protonode is reached, each input connector in downstream_calling_inputs, is looked up in `any_input_to_downstream_protonode_input`. If there is an entry,
	// Then the paths are compared, and the greater one is chosen using stable ordering.
	// This is to ensure a constant mapping, since an export for instance can have multiple calling nodes in the parent network
	//
	// any_input_to_upstream_protonode - used by the runtime to get the node to evaluate for any given input connector.
	// Each input connector is inserted into any_input_to_upstream_protonode with the value being the path to the reached protonode.
	// It doesnt matter if its overwritten since it must have previously pointed to the same protonode anyways
	//
	pub fn traverse_input(
		&mut self,
		protonetwork: &mut Vec<ProtoNode>, // Flattened node id to protonode, stable node ids can only be generated once the network is fully flattened, since it runs in reverse
		value_connector: &mut Vec<Option<AbsoluteInputConnector>>,
		protonode_path: &mut Vec<Option<ProtonodePath>>,
		calling_protonodes: &mut HashMap<usize, Vec<(usize, usize)>>, // A mapping of protonode path to all (flattened network indices, their input index) that called the protonode, used during SNI generation to remap inputs
		protonode_indices: &mut HashMap<Vec<SNI>, usize>,             // Mapping of protonode path to its index in the flattened protonetwork
		traversal_input: AbsoluteInputConnector,
		// Protonode index, input index
		traversal_start: (usize, usize),
	) {
		let network_path = &traversal_input.network_path;

		let Some(input) = self.get_input_from_absolute_connector(&traversal_input) else {
			return;
		};

		// Populate reflection inputs with the tagged value of the node path
		if let NodeInput::Reflection(metadata) = input {
			match metadata {
				DocumentNodeMetadata::DocumentNodePath => {
					let mut node_path = network_path.clone();
					if let Some(traversal_node_id) = traversal_input.connector.node_id() {
						node_path.push(traversal_node_id);
					}
					*input = NodeInput::Value {
						tagged_value: TaggedValue::NodePath(node_path).into(),
						exposed: true,
					}
				}
			}
		}

		if let NodeInput::Scope(cow) = input {
			let string = cow.to_string();
			let scope_node_value = match self.scope_injections.get(&string) {
				Some(value) => value.clone(), // Scope injections need to be small values so they can be cloned to every caller input
				// If the scope node value node has already been inserted, the other nodes will map to it
				None => TaggedValue::None,
			};
			let Some(input) = self.get_input_from_absolute_connector(&traversal_input) else {
				return;
			};
			*input = NodeInput::Value {
				tagged_value: scope_node_value.into(),
				exposed: false,
			}
		}

		let Some(input) = self.get_input_from_absolute_connector(&traversal_input) else {
			return;
		};

		// This input can be called by an export, protonode input, or document node input
		match input {
			NodeInput::Node { node_id, output_index, .. } => {
				let upstream_node_id = *node_id;
				let output_index = *output_index;
				let mut upstream_node_path = network_path.clone();
				upstream_node_path.push(upstream_node_id);
				let Some(nested_network) = self.nested_network(network_path) else {
					log::error!("traversal_input network does not exist, path {:?}", network_path);
					return;
				};
				let Some(upstream_document_node) = nested_network.nodes.get(&upstream_node_id) else {
					log::error!("The node which was supposed to be flattened does not exist in the network, id {upstream_node_id}");
					return;
				};

				match &upstream_document_node.implementation {
					DocumentNodeImplementation::Network(_node_network) => {
						let traversal_input = AbsoluteInputConnector {
							network_path: upstream_node_path.clone(),
							connector: InputConnector::Export(output_index),
						};
						self.traverse_input(protonetwork, value_connector, protonode_path, calling_protonodes, protonode_indices, traversal_input, traversal_start);
					}
					DocumentNodeImplementation::ProtoNode(protonode_id) => {
						// Only insert the protonode if it has not previously been inserted
						// Do not insert the protonode into the proto network or traverse over inputs if its already visited
						let reached_protonode_index = match protonode_indices.get(&upstream_node_path) {
							// The protonode has already been inserted, return its index
							Some(reached_protonode_index) => *reached_protonode_index,
							// Insert the protonode and traverse over inputs
							None => {
								let construction_args = ConstructionArgs::Nodes(NodeConstructionArgs {
									identifier: protonode_id.clone(),
									inputs: vec![NodeId(0); upstream_document_node.inputs.len()],
								});
								let protonode = ProtoNode {
									construction_args,
									// All protonodes take Context by default
									input: concrete!(Context),
									original_location: OriginalLocation {
										protonode_path: upstream_node_path.clone().into(),
										send_types_to_editor: true,
									},
									stable_node_id: NodeId(0),
								};
								let new_protonode_index = protonetwork.len();
								protonode_indices.insert(upstream_node_path.clone(), new_protonode_index);
								protonetwork.push(protonode);
								value_connector.push(None);
								protonode_path.push(Some(upstream_node_path.into_boxed_slice()));
								// Iterate over all upstream inputs, which will map the inputs to the index of the connected protonode
								for input_index in 0..upstream_document_node.inputs.len() {
									self.traverse_input(
										protonetwork,
										value_connector,
										protonode_path,
										calling_protonodes,
										protonode_indices,
										AbsoluteInputConnector {
											network_path: network_path.clone(),
											connector: InputConnector::node(upstream_node_id, input_index),
										},
										(new_protonode_index, input_index),
									);
								}
								new_protonode_index
							}
						};
						calling_protonodes.entry(reached_protonode_index).or_insert_with(Vec::new).push(traversal_start);
					}
					DocumentNodeImplementation::Extract => todo!(),
				}
			}
			NodeInput::Value { tagged_value, .. } => {
				// Deduplication of value nodes based on their tagged value, since they do not depend on the Context
				//
				use std::hash::Hasher;
				let mut hasher = rustc_hash::FxHasher::default();
				tagged_value.hash(&mut hasher);
				let value_node_path = vec![NodeId(hasher.finish())];

				// Only insert the value protonode if it has not previously been inserted
				let value_protonode_index = match protonode_indices.get(&value_node_path) {
					// The value input has already been inserted, return it the existing value nodes index
					Some(value_protonode_index) => *value_protonode_index,
					// Insert the protonode and traverse over inputs
					None => {
						let protonode = ProtoNode {
							construction_args: ConstructionArgs::Value(std::mem::replace(tagged_value, TaggedValue::None.into())),
							input: concrete!(Context), // Could be ()
							original_location: OriginalLocation {
								protonode_path: Vec::new().into(),
								send_types_to_editor: false,
							},
							stable_node_id: NodeId(0),
						};
						let new_protonode_index = protonetwork.len();
						protonode_indices.insert(value_node_path.clone(), new_protonode_index);
						protonetwork.push(protonode);
						value_connector.push(Some(traversal_input));
						protonode_path.push(None);
						new_protonode_index
					}
				};
				calling_protonodes.entry(value_protonode_index).or_insert_with(Vec::new).push(traversal_start);
			}
			// Continue traversal
			NodeInput::Network { import_index, .. } => {
				let mut encapsulating_network_path = network_path.clone();
				let node_id = encapsulating_network_path.pop().unwrap();
				let traversal_input = AbsoluteInputConnector {
					network_path: encapsulating_network_path,
					connector: InputConnector::node(node_id, *import_index),
				};
				self.traverse_input(protonetwork, value_connector, protonode_path, calling_protonodes, protonode_indices, traversal_input, traversal_start);
			}
			NodeInput::Scope(_cow) => unreachable!(),
			NodeInput::Reflection(_document_node_metadata) => unreachable!(),
			NodeInput::Inline(_inline_rust) => todo!(),
		}
	}

	// pub fn collect_downstream_metadata(
	// 	reached_protonode_index: usize,
	// 	calling_protonodes: &mut HashMap<usize, Vec<(usize, usize)>>,
	// 	protonode_indices: &mut HashMap<Vec<SNI>, usize>,
	// 	downstream_calling_inputs: Vec<AbsoluteInputConnector>,
	// ) {
	// 	// Map the first downstream calling node input (which is traversed for every node input) to the reached protonode
	// 	let downstream_protonode_caller = downstream_calling_inputs[0].clone();

	// 	match &downstream_protonode_caller.connector {
	// 		InputConnector::Node { node_id, input_index } => {
	// 			// The calling protonode has already been added to the flattened network, so it can be looked up by index and the reached node can be mapped to it
	// 			let mut calling_protonode_path = downstream_protonode_caller.network_path.clone();
	// 			calling_protonode_path.push(*node_id);
	// 			let calling_protonode_index = protonode_indices[&calling_protonode_path];

	// 		}
	// 		InputConnector::Export(_) => {}
	// 	}
	// }

	/// Converts the `DocumentNode`s with a `DocumentNodeImplementation::Extract` into a `ClonedNode` that returns
	/// the `DocumentNode` specified by the single `NodeInput::Node`.
	/// The referenced node is removed from the network, and any `NodeInput::Node`s used by the referenced node are replaced with a generically typed network input.
	pub fn resolve_extract_nodes(&mut self) {
		// let mut extraction_nodes = self
		// 	.nodes
		// 	.iter()
		// 	.filter(|(_, node)| matches!(node.implementation, DocumentNodeImplementation::Extract))
		// 	.map(|(id, node)| (*id, node.clone()))
		// 	.collect::<Vec<_>>();
		// self.nodes.retain(|_, node| !matches!(node.implementation, DocumentNodeImplementation::Extract));

		// for (_, node) in &mut extraction_nodes {
		// 	assert_eq!(node.inputs.len(), 1);
		// 	let NodeInput::Node { node_id, output_index, .. } = node.inputs.pop().unwrap() else {
		// 		panic!("Extract node has no input, inputs: {:?}", node.inputs);
		// 	};
		// 	assert_eq!(output_index, 0);
		// 	// TODO: check if we can read lambda checking?
		// 	let mut input_node = self.nodes.remove(&node_id).unwrap();
		// 	node.implementation = DocumentNodeImplementation::ProtoNode("graphene_core::value::ClonedNode".into());
		// 	if let Some(input) = input_node.inputs.get_mut(0) {
		// 		*input = match &input {
		// 			NodeInput::Node { .. } => NodeInput::network(generic!(T), 0),
		// 			ni => NodeInput::network(ni.ty(), 0),
		// 		};
		// 	}

		// 	for input in input_node.inputs.iter_mut() {
		// 		if let NodeInput::Node { .. } = input {
		// 			*input = NodeInput::network(generic!(T), 0)
		// 		}
		// 	}
		// 	node.inputs = vec![NodeInput::value(TaggedValue::DocumentNode(input_node), false)];
		// }
		// self.nodes.extend(extraction_nodes);
	}

	/// Create a [`RecursiveNodeIter`] that iterates over all [`DocumentNode`]s, including ones that are deeply nested.
	pub fn recursive_nodes(&self) -> RecursiveNodeIter<'_> {
		let nodes = self.nodes.iter().map(|(path, node)| (vec![*path], node)).collect();
		RecursiveNodeIter { nodes }
	}
}

#[derive(Debug)]
pub struct CompilationMetadata {
	// Stored for every value input in the compiled network
	pub protonode_callers_for_value: Vec<(AbsoluteInputConnector, CompiledProtonodeInput)>,
	// Stored for every protonode in the compiled network
	pub protonode_callers_for_node: Vec<(ProtonodePath, CompiledProtonodeInput)>,
	pub types_to_add: Vec<(SNI, Vec<Type>)>,
	pub types_to_remove: Vec<(SNI, usize)>,
}

//An Input connector with a node path for unique identification
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct AbsoluteInputConnector {
	pub network_path: Vec<NodeId>,
	pub connector: InputConnector,
}

impl AbsoluteInputConnector {
	pub fn traversal_start() -> Self {
		AbsoluteInputConnector {
			network_path: Vec::new(),
			connector: InputConnector::Export(0),
		}
	}
}
/// Represents an input connector with index based on the [`DocumentNode::inputs`] index, not the visible input index
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum InputConnector {
	#[serde(rename = "node")]
	Node {
		#[serde(rename = "nodeId")]
		node_id: NodeId,
		#[serde(rename = "inputIndex")]
		input_index: usize,
	},
	#[serde(rename = "export")]
	Export(usize),
}

impl Default for InputConnector {
	fn default() -> Self {
		InputConnector::Export(0)
	}
}

impl InputConnector {
	pub fn node(node_id: NodeId, input_index: usize) -> Self {
		InputConnector::Node { node_id, input_index }
	}

	pub fn input_index(&self) -> usize {
		match self {
			InputConnector::Node { input_index, .. } => *input_index,
			InputConnector::Export(input_index) => *input_index,
		}
	}

	pub fn node_id(&self) -> Option<NodeId> {
		match self {
			InputConnector::Node { node_id, .. } => Some(*node_id),
			_ => None,
		}
	}
}

//An Output connector with a node path for unique identification
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct AbsoluteOutputConnector {
	pub path: Vec<NodeId>,
	pub connector: OutputConnector,
}

/// Represents an output connector
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OutputConnector {
	#[serde(rename = "node")]
	Node {
		#[serde(rename = "nodeId")]
		node_id: NodeId,
		#[serde(rename = "outputIndex")]
		output_index: usize,
	},
	#[serde(rename = "import")]
	Import(usize),
}

impl Default for OutputConnector {
	fn default() -> Self {
		OutputConnector::Import(0)
	}
}

impl OutputConnector {
	pub fn node(node_id: NodeId, output_index: usize) -> Self {
		OutputConnector::Node { node_id, output_index }
	}

	pub fn index(&self) -> usize {
		match self {
			OutputConnector::Node { output_index, .. } => *output_index,
			OutputConnector::Import(output_index) => *output_index,
		}
	}

	pub fn node_id(&self) -> Option<NodeId> {
		match self {
			OutputConnector::Node { node_id, .. } => Some(*node_id),
			_ => None,
		}
	}

	pub fn from_input(input: &NodeInput) -> Option<Self> {
		match input {
			NodeInput::Network { import_index, .. } => Some(Self::Import(*import_index)),
			NodeInput::Node { node_id, output_index, .. } => Some(Self::node(*node_id, *output_index)),
			_ => None,
		}
	}
}

/// An iterator over all [`DocumentNode`]s, including ones that are deeply nested.
pub struct RecursiveNodeIter<'a> {
	nodes: Vec<(Vec<NodeId>, &'a DocumentNode)>,
}

impl<'a> Iterator for RecursiveNodeIter<'a> {
	type Item = (Vec<NodeId>, &'a DocumentNode);
	fn next(&mut self) -> Option<Self::Item> {
		let (path, node) = self.nodes.pop()?;
		if let DocumentNodeImplementation::Network(network) = &node.implementation {
			for (node_id, node) in &network.nodes {
				let mut new_path = path.to_vec();
				new_path.push(*node_id);
				self.nodes.push((new_path, node));
			}
		}
		Some((path, node))
	}
}

#[cfg(test)]
// mod test {
// 	use super::*;
// 	use crate::proto::{ConstructionArgs, ProtoNetwork, ProtoNode, ProtoNodeInput};
// 	use std::sync::atomic::AtomicU64;

// 	fn gen_node_id() -> NodeId {
// 		static NODE_ID: AtomicU64 = AtomicU64::new(4);
// 		NodeId(NODE_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst))
// 	}

// 	fn add_network() -> NodeNetwork {
// 		NodeNetwork {
// 			exports: vec![NodeInput::node(NodeId(1), 0)],
// 			nodes: [
// 				(
// 					NodeId(0),
// 					DocumentNode {
// 						inputs: vec![NodeInput::network(concrete!(u32), 0), NodeInput::network(concrete!(u32), 1)],
// 						implementation: DocumentNodeImplementation::ProtoNode("graphene_core::structural::ConsNode".into()),
// 						..Default::default()
// 					},
// 				),
// 				(
// 					NodeId(1),
// 					DocumentNode {
// 						inputs: vec![NodeInput::node(NodeId(0), 0)],
// 						implementation: DocumentNodeImplementation::ProtoNode("graphene_core::ops::AddPairNode".into()),
// 						..Default::default()
// 					},
// 				),
// 			]
// 			.into_iter()
// 			.collect(),
// 			..Default::default()
// 		}
// 	}

// 	#[test]
// 	fn map_ids() {
// 		let mut network = add_network();
// 		network.map_ids(|id| NodeId(id.0 + 1));
// 		let mapped_add = NodeNetwork {
// 			exports: vec![NodeInput::node(NodeId(2), 0)],
// 			nodes: [
// 				(
// 					NodeId(1),
// 					DocumentNode {
// 						inputs: vec![NodeInput::network(concrete!(u32), 0), NodeInput::network(concrete!(u32), 1)],
// 						implementation: DocumentNodeImplementation::ProtoNode("graphene_core::structural::ConsNode".into()),
// 						..Default::default()
// 					},
// 				),
// 				(
// 					NodeId(2),
// 					DocumentNode {
// 						inputs: vec![NodeInput::node(NodeId(1), 0)],
// 						implementation: DocumentNodeImplementation::ProtoNode("graphene_core::ops::AddPairNode".into()),
// 						..Default::default()
// 					},
// 				),
// 			]
// 			.into_iter()
// 			.collect(),
// 			..Default::default()
// 		};
// 		assert_eq!(network, mapped_add);
// 	}

// 	#[test]
// 	fn extract_node() {
// 		let id_node = DocumentNode {
// 			inputs: vec![],
// 			implementation: DocumentNodeImplementation::ProtoNode("graphene_core::ops::IdentityNode".into()),
// 			..Default::default()
// 		};
// 		// TODO: Extend test cases to test nested network
// 		let mut extraction_network = NodeNetwork {
// 			exports: vec![NodeInput::node(NodeId(1), 0)],
// 			nodes: [
// 				id_node.clone(),
// 				DocumentNode {
// 					inputs: vec![NodeInput::lambda(NodeId(0), 0)],
// 					implementation: DocumentNodeImplementation::Extract,
// 					..Default::default()
// 				},
// 			]
// 			.into_iter()
// 			.enumerate()
// 			.map(|(id, node)| (NodeId(id as u64), node))
// 			.collect(),
// 			..Default::default()
// 		};
// 		extraction_network.resolve_extract_nodes();
// 		assert_eq!(extraction_network.nodes.len(), 1);
// 		let inputs = extraction_network.nodes.get(&NodeId(1)).unwrap().inputs.clone();
// 		assert_eq!(inputs.len(), 1);
// 		assert!(matches!(&inputs[0].as_value(), &Some(TaggedValue::DocumentNode(network), ..) if network == &id_node));
// 	}

// 	#[test]
// 	fn flatten_add() {
// 		let mut network = NodeNetwork {
// 			exports: vec![NodeInput::node(NodeId(1), 0)],
// 			nodes: [(
// 				NodeId(1),
// 				DocumentNode {
// 					inputs: vec![NodeInput::network(concrete!(u32), 0), NodeInput::value(TaggedValue::U32(2), false)],
// 					implementation: DocumentNodeImplementation::Network(add_network()),
// 					..Default::default()
// 				},
// 			)]
// 			.into_iter()
// 			.collect(),
// 			..Default::default()
// 		};
// 		network.populate_dependants();
// 		network.flatten_with_fns(NodeId(1), |self_id, inner_id| NodeId(self_id.0 * 10 + inner_id.0), gen_node_id);
// 		let flat_network = flat_network();
// 		println!("{flat_network:#?}");
// 		println!("{network:#?}");

// 		assert_eq!(flat_network, network);
// 	}

// 	#[test]
// 	fn resolve_proto_node_add() {
// 		let document_node = DocumentNode {
// 			inputs: vec![NodeInput::network(concrete!(u32), 0), NodeInput::node(NodeId(0), 0)],
// 			implementation: DocumentNodeImplementation::ProtoNode("graphene_core::structural::ConsNode".into()),
// 			..Default::default()
// 		};

// 		let proto_node = document_node.resolve_proto_node();
// 		let reference = ProtoNode {
// 			construction_args: ConstructionArgs::Nodes(NodeConstructionArgs { identifier: "graphene_core::structural::ConsNode".into(), inputs: vec![(NodeId(0), false)]}),
// 			..Default::default()
// 		};
// 		assert_eq!(proto_node, reference);
// 	}

// 	#[test]
// 	fn resolve_flatten_add_as_proto_network() {
// 		let construction_network = ProtoNetwork {
// 			output: NodeId(11),
// 			nodes: [
// 				(
// 					NodeId(10),
// 					ProtoNode {
// 						identifier: "graphene_core::structural::ConsNode".into(),
// 						construction_args: ConstructionArgs::Nodes(vec![(NodeId(14), false)]),
// 						// document_node_path: OriginalLocation {
// 						// 	path: Some(vec![NodeId(1), NodeId(0)]),
// 						// 	inputs_source: [(Source { node: vec![NodeId(1)], index: 1 }, 1)].into(),
// 						// 	inputs_exposed: vec![true, true],
// 						// 	skip_inputs: 0,
// 						// 	..Default::default()
// 						// },
// 						..Default::default()
// 					},
// 				),
// 				(
// 					NodeId(11),
// 					ProtoNode {
// 						identifier: "graphene_core::ops::AddPairNode".into(),
// 						construction_args: ConstructionArgs::Nodes(vec![]),
// 						// document_node_path: OriginalLocation {
// 						// 	path: Some(vec![NodeId(1), NodeId(1)]),
// 						// 	inputs_source: HashMap::new(),
// 						// 	inputs_exposed: vec![true],
// 						// 	skip_inputs: 0,
// 						// 	..Default::default()
// 						// },
// 						..Default::default()
// 					},
// 				),
// 				(
// 					NodeId(14),
// 					ProtoNode {
// 						identifier: "graphene_core::value::ClonedNode".into(),
// 						construction_args: ConstructionArgs::Value(TaggedValue::U32(2).into()),
// 						// document_node_path: OriginalLocation {
// 						// 	path: Some(vec![NodeId(1), NodeId(4)]),
// 						// 	inputs_source: HashMap::new(),
// 						// 	inputs_exposed: vec![true, false],
// 						// 	skip_inputs: 0,
// 						// 	..Default::default()
// 						// },
// 						..Default::default()
// 					},
// 				),
// 			]
// 			.into_iter()
// 			.collect(),
// 		};
// 		let network = flat_network();
// 		let mut resolved_network = network.into_proto_network().collect::<Vec<_>>();
// 		resolved_network[0].nodes.sort_unstable_by_key(|(id, _)| *id);

// 		println!("{:#?}", resolved_network[0]);
// 		println!("{construction_network:#?}");
// 		pretty_assertions::assert_eq!(resolved_network[0], construction_network);
// 	}

// 	fn flat_network() -> NodeNetwork {
// 		NodeNetwork {
// 			exports: vec![NodeInput::node(NodeId(11), 0)],
// 			nodes: [
// 				(
// 					NodeId(10),
// 					DocumentNode {
// 						inputs: vec![NodeInput::network(concrete!(u32), 0), NodeInput::node(NodeId(14), 0)],
// 						implementation: DocumentNodeImplementation::ProtoNode("graphene_core::structural::ConsNode".into()),
// 						// document_node_path: OriginalLocation {
// 						// 	path: Some(vec![NodeId(1), NodeId(0)]),
// 						// 	inputs_source: [(Source { node: vec![NodeId(1)], index: 1 }, 1)].into(),
// 						// 	inputs_exposed: vec![true, true],
// 						// 	skip_inputs: 0,
// 						// 	..Default::default()
// 						// },
// 						..Default::default()
// 					},
// 				),
// 				(
// 					NodeId(14),
// 					DocumentNode {
// 						inputs: vec![NodeInput::value(TaggedValue::U32(2), false)],
// 						implementation: DocumentNodeImplementation::ProtoNode("graphene_core::value::ClonedNode".into()),
// 						// document_node_path: OriginalLocation {
// 						// 	path: Some(vec![NodeId(1), NodeId(4)]),
// 						// 	inputs_source: HashMap::new(),
// 						// 	inputs_exposed: vec![true, false],
// 						// 	skip_inputs: 0,
// 						// 	..Default::default()
// 						// },
// 						..Default::default()
// 					},
// 				),
// 				(
// 					NodeId(11),
// 					DocumentNode {
// 						inputs: vec![NodeInput::node(NodeId(10), 0)],
// 						implementation: DocumentNodeImplementation::ProtoNode("graphene_core::ops::AddPairNode".into()),
// 						// document_node_path: OriginalLocation {
// 						// 	path: Some(vec![NodeId(1), NodeId(1)]),
// 						// 	inputs_source: HashMap::new(),
// 						// 	inputs_exposed: vec![true],
// 						// 	skip_inputs: 0,
// 						// 	..Default::default()
// 						// },
// 						..Default::default()
// 					},
// 				),
// 			]
// 			.into_iter()
// 			.collect(),
// 			..Default::default()
// 		}
// 	}

	// fn two_node_identity() -> NodeNetwork {
	// 	NodeNetwork {
	// 		exports: vec![NodeInput::node(NodeId(1), 0), NodeInput::node(NodeId(2), 0)],
	// 		nodes: [
	// 			(
	// 				NodeId(1),
	// 				DocumentNode {
	// 					inputs: vec![NodeInput::network(concrete!(u32), 0)],
	// 					implementation: DocumentNodeImplementation::ProtoNode(graphene_core::ops::identity::IDENTIFIER),
	// 					..Default::default()
	// 				},
	// 			),
	// 			(
	// 				NodeId(2),
	// 				DocumentNode {
	// 					inputs: vec![NodeInput::network(concrete!(u32), 1)],
	// 					implementation: DocumentNodeImplementation::ProtoNode(graphene_core::ops::identity::IDENTIFIER),
	// 					..Default::default()
	// 				},
	// 			),
	// 		]
	// 		.into_iter()
	// 		.collect(),
	// 		..Default::default()
	// 	}
	// }

	// fn output_duplicate(network_outputs: Vec<NodeInput>, result_node_input: NodeInput) -> NodeNetwork {
	// 	let mut network = NodeNetwork {
	// 		exports: network_outputs,
	// 		nodes: [
	// 			(
	// 				NodeId(1),
	// 				DocumentNode {
	// 					inputs: vec![NodeInput::value(TaggedValue::F64(1.), false), NodeInput::value(TaggedValue::F64(2.), false)],
	// 					implementation: DocumentNodeImplementation::Network(two_node_identity()),
	// 					..Default::default()
	// 				},
	// 			),
	// 			(
	// 				NodeId(2),
	// 				DocumentNode {
	// 					inputs: vec![result_node_input],
	// 					implementation: DocumentNodeImplementation::ProtoNode(graphene_core::ops::identity::IDENTIFIER),
	// 					..Default::default()
	// 				},
	// 			),
	// 		]
	// 		.into_iter()
	// 		.collect(),
	// 		..Default::default()
	// 	};
	// 	let _new_ids = 101..;
	// 	network.populate_dependants();
	// 	network.flatten_with_fns(NodeId(1), |self_id, inner_id| NodeId(self_id.0 * 10 + inner_id.0), || NodeId(10000));
	// 	network.flatten_with_fns(NodeId(2), |self_id, inner_id| NodeId(self_id.0 * 10 + inner_id.0), || NodeId(10001));
	// 	network.remove_dead_nodes(0);
	// 	network
	// }

// 	#[test]
// 	fn simple_duplicate() {
// 		let result = output_duplicate(vec![NodeInput::node(NodeId(1), 0)], NodeInput::node(NodeId(1), 0));
// 		println!("{result:#?}");
// 		assert_eq!(result.exports.len(), 1, "The number of outputs should remain as 1");
// 		assert_eq!(result.exports[0], NodeInput::node(NodeId(11), 0), "The outer network output should be from a duplicated inner network");
// 		let mut ids = result.nodes.keys().copied().collect::<Vec<_>>();
// 		ids.sort();
// 		assert_eq!(ids, vec![NodeId(11), NodeId(10010)], "Should only contain identity and values");
// 	}

// 	// TODO: Write more tests
// 	// #[test]
// 	// fn out_of_order_duplicate() {
// 	// 	let result = output_duplicate(vec![NodeInput::node(NodeId(10), 1), NodeInput::node(NodeId(10), 0)], NodeInput::node(NodeId(10), 0);
// 	// 	assert_eq!(
// 	// 		result.outputs[0],
// 	// 		NodeInput::node(NodeId(101), 0),
// 	// 		"The first network output should be from a duplicated nested network"
// 	// 	);
// 	// 	assert_eq!(
// 	// 		result.outputs[1],
// 	// 		NodeInput::node(NodeId(10), 0),
// 	// 		"The second network output should be from the original nested network"
// 	// 	);
// 	// 	assert!(
// 	// 		result.nodes.contains_key(&NodeId(10)) && result.nodes.contains_key(&NodeId(101)) && result.nodes.len() == 2,
// 	// 		"Network should contain two duplicated nodes"
// 	// 	);
// 	// 	for (node_id, input_value, inner_id) in [(10, 1., 1), (101, 2., 2)] {
// 	// 		let nested_network_node = result.nodes.get(&NodeId(node_id)).unwrap();
// 	// 		assert_eq!(nested_network_node.name, "Nested network".to_string(), "Name should not change");
// 	// 		assert_eq!(nested_network_node.inputs, vec![NodeInput::value(TaggedValue::F32(input_value), false)], "Input should be stable");
// 	// 		let inner_network = nested_network_node.implementation.get_network().expect("Implementation should be network");
// 	// 		assert_eq!(inner_network.inputs, vec![inner_id], "The input should be sent to the second node");
// 	// 		assert_eq!(inner_network.outputs, vec![NodeInput::node(NodeId(inner_id), 0)], "The output should be node id");
// 	// 		assert_eq!(inner_network.nodes.get(&NodeId(inner_id)).unwrap().name, format!("Identity {inner_id}"), "The node should be identity");
// 	// 	}
// 	// }
// 	// #[test]
// 	// fn using_other_node_duplicate() {
// 	// 	let result = output_duplicate(vec![NodeInput::node(NodeId(11), 0)], NodeInput::node(NodeId(10), 1);
// 	// 	assert_eq!(result.outputs, vec![NodeInput::node(NodeId(11), 0)], "The network output should be the result node");
// 	// 	assert!(
// 	// 		result.nodes.contains_key(&NodeId(11)) && result.nodes.contains_key(&NodeId(101)) && result.nodes.len() == 2,
// 	// 		"Network should contain a duplicated node and a result node"
// 	// 	);
// 	// 	let result_node = result.nodes.get(&NodeId(11)).unwrap();
// 	// 	assert_eq!(result_node.inputs, vec![NodeInput::node(NodeId(101), 0)], "Result node should refer to duplicate node as input");
// 	// 	let nested_network_node = result.nodes.get(&NodeId(101)).unwrap();
// 	// 	assert_eq!(nested_network_node.name, "Nested network".to_string(), "Name should not change");
// 	// 	assert_eq!(nested_network_node.inputs, vec![NodeInput::value(TaggedValue::F32(2.), false)], "Input should be 2");
// 	// 	let inner_network = nested_network_node.implementation.get_network().expect("Implementation should be network");
// 	// 	assert_eq!(inner_network.inputs, vec![2], "The input should be sent to the second node");
// 	// 	assert_eq!(inner_network.outputs, vec![NodeInput::node(NodeId(2), 0)], "The output should be node id 2");
// 	// 	assert_eq!(inner_network.nodes.get(&NodeId(2)).unwrap().name, "Identity 2", "The node should be identity 2");
// 	// }
// }
