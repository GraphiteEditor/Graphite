use super::*;

// TODO: Eventually remove this document upgrade code
#[derive(Clone, Debug, DynAny, serde::Serialize, serde::Deserialize)]
/// Represents the implementation of a node, which can be a nested [`NodeNetwork`], a proto [`ProtoNodeIdentifier`], or `Extract`.
pub enum OldDocumentNodeImplementation {
	Network(OldNodeNetwork),
	#[serde(alias = "Unresolved")]
	ProtoNode(ProtoNodeIdentifier),
	Extract,
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
pub(super) fn deserialize_exports<'de, D>(deserializer: D) -> Result<Vec<NodeInput>, D::Error>
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

pub(super) fn migrate_call_argument<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Type, D::Error> {
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
