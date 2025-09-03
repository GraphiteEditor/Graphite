use crate::uuid::NodeId;

#[derive(Clone, Debug, Default, PartialEq, Hash, dyn_any::DynAny, serde::Serialize, serde::Deserialize, specta::Type)]

pub struct NodeGraphOverlayData {
	pub nodes_to_render: Vec<FrontendNodeToRender>,
	pub open: bool,
	pub in_selected_network: bool,
	// Displays a dashed border around the node
	pub previewed_node: Option<NodeId>,
}

#[derive(Clone, Debug, Default, PartialEq, Hash, dyn_any::DynAny, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendNodeToRender {
	pub metadata: FrontendNodeMetadata,
	#[serde(rename = "nodeOrLayer")]
	pub node_or_layer: FrontendNodeOrLayer,
	//TODO: Remove
	pub wires: Vec<(String, bool, FrontendGraphDataType)>,
}

// Metadata that is common to nodes and layers
#[derive(Clone, Debug, Default, PartialEq, Hash, dyn_any::DynAny, serde::Serialize, serde::Deserialize, specta::Type)]

pub struct FrontendNodeMetadata {
	#[serde(rename = "nodeId")]
	pub node_id: NodeId,
	// TODO: Remove and replace with popup manager system
	#[serde(rename = "canBeLayer")]
	pub can_be_layer: bool,
	#[serde(rename = "displayName")]
	pub display_name: String,
	pub selected: bool,
	// Used to get the description, which is stored in a global hashmap
	pub reference: Option<String>,
	// Reduces opacity of node/hidden eye icon
	pub visible: bool,
	// The svg string for each input
	// pub wires: Vec<Option<String>>,
	pub errors: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Hash, dyn_any::DynAny, serde::Serialize, serde::Deserialize, specta::Type)]

pub struct FrontendNode {
	// pub position: FrontendNodePosition,
	pub position: FrontendXY,
	pub inputs: Vec<Option<FrontendGraphInput>>,
	pub outputs: Vec<Option<FrontendGraphOutput>>,
}

#[derive(Clone, Debug, Default, PartialEq, Hash, dyn_any::DynAny, serde::Serialize, serde::Deserialize, specta::Type)]

pub struct FrontendLayer {
	#[serde(rename = "bottomInput")]
	pub bottom_input: FrontendGraphInput,
	#[serde(rename = "sideInput")]
	pub side_input: Option<FrontendGraphInput>,
	pub output: FrontendGraphOutput,
	// pub position: FrontendLayerPosition,
	pub position: FrontendXY,
	pub locked: bool,
	#[serde(rename = "chainWidth")]
	pub chain_width: u32,
	#[serde(rename = "layerHasLeftBorderGap")]
	pub layer_has_left_border_gap: bool,
	#[serde(rename = "primaryInputConnectedToLayer")]
	pub primary_input_connected_to_layer: bool,
	#[serde(rename = "primaryOutputConnectedToLayer")]
	pub primary_output_connected_to_layer: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Hash, dyn_any::DynAny, serde::Serialize, serde::Deserialize, specta::Type)]

pub struct FrontendXY {
	pub x: i32,
	pub y: i32,
}

// // Should be an enum but those are hard to serialize/deserialize to TS
// #[derive(Clone, Debug, Default, PartialEq, Hash, dyn_any::DynAny, serde::Serialize, serde::Deserialize, specta::Type)]

// pub struct FrontendNodePosition {
// 	pub absolute: Option<FrontendXY>,
// 	pub chain: Option<bool>,
// }

// // Should be an enum but those are hard to serialize/deserialize to TS
// #[derive(Clone, Debug, Default, PartialEq, Hash, dyn_any::DynAny, serde::Serialize, serde::Deserialize, specta::Type)]

// pub struct FrontendLayerPosition {
// 	pub absolute: Option<FrontendXY>,
// 	pub stack: Option<u32>,
// }

#[derive(Clone, Debug, Default, PartialEq, Hash, dyn_any::DynAny, serde::Serialize, serde::Deserialize, specta::Type)]

pub struct FrontendNodeOrLayer {
	pub node: Option<FrontendNode>,
	pub layer: Option<FrontendLayer>,
}

#[derive(Clone, Debug, Default, PartialEq, Hash, dyn_any::DynAny, serde::Serialize, serde::Deserialize, specta::Type)]

pub struct FrontendGraphInput {
	#[serde(rename = "dataType")]
	pub data_type: FrontendGraphDataType,
	#[serde(rename = "resolvedType")]
	pub resolved_type: String,
	pub name: String,
	pub description: String,
	/// Either "nothing", "import index {index}", or "{node name} output {output_index}".
	#[serde(rename = "connectedToString")]
	pub connected_to: String,
	/// Used to render the upstream node once this node is rendered
	#[serde(rename = "connectedToNode")]
	pub connected_to_node: Option<NodeId>,
}

#[derive(Clone, Debug, Default, PartialEq, Hash, dyn_any::DynAny, serde::Serialize, serde::Deserialize, specta::Type)]

pub struct FrontendGraphOutput {
	#[serde(rename = "dataType")]
	pub data_type: FrontendGraphDataType,
	pub name: String,
	#[serde(rename = "resolvedType")]
	pub resolved_type: String,
	pub description: String,
	/// If connected to an export, it is "export index {index}".
	/// If connected to a node, it is "{node name} input {input_index}".
	#[serde(rename = "connectedTo")]
	pub connected_to: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Hash, dyn_any::DynAny, serde::Serialize, serde::Deserialize, specta::Type)]

pub struct FrontendExport {
	pub port: FrontendGraphInput,
	pub wire: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Hash, dyn_any::DynAny, serde::Serialize, serde::Deserialize, specta::Type)]

pub struct FrontendExports {
	/// If the primary export is not visible, then it is None.
	pub exports: Vec<Option<FrontendExport>>,
	#[serde(rename = "previewWire")]
	pub preview_wire: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Hash, dyn_any::DynAny, serde::Serialize, serde::Deserialize, specta::Type)]

pub struct FrontendImport {
	pub port: FrontendGraphOutput,
	pub wires: Vec<String>,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Hash, dyn_any::DynAny, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum FrontendGraphDataType {
	#[default]
	General,
	Number,
	Artboard,
	Graphic,
	Raster,
	Vector,
	Color,
	Gradient,
	Typography,
}
