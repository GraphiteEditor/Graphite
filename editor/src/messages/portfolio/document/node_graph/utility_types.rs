use graph_craft::document::NodeId;
use graph_craft::document::value::TaggedValue;
use graphene_std::Type;
use std::borrow::Cow;

use crate::messages::portfolio::document::utility_types::network_interface::resolved_types::TypeSource;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize, specta::Type)]
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

impl FrontendGraphDataType {
	pub fn from_type(input: &Type) -> Self {
		match TaggedValue::from_type_or_none(input) {
			TaggedValue::U32(_)
			| TaggedValue::U64(_)
			| TaggedValue::F32(_)
			| TaggedValue::F64(_)
			| TaggedValue::DVec2(_)
			| TaggedValue::F64Array4(_)
			| TaggedValue::VecF64(_)
			| TaggedValue::VecDVec2(_)
			| TaggedValue::DAffine2(_) => Self::Number,
			TaggedValue::Artboard(_) => Self::Artboard,
			TaggedValue::Graphic(_) => Self::Graphic,
			TaggedValue::Raster(_) => Self::Raster,
			TaggedValue::Vector(_) => Self::Vector,
			TaggedValue::Color(_) => Self::Color,
			TaggedValue::Gradient(_) | TaggedValue::GradientStops(_) | TaggedValue::GradientTable(_) => Self::Gradient,
			TaggedValue::String(_) => Self::Typography,
			_ => Self::General,
		}
	}

	pub fn displayed_type(type_source: &TypeSource) -> Self {
		match type_source.compiled_nested_type() {
			Some(nested_type) => Self::from_type(&nested_type),
			None => Self::General,
		}
	}
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendXY {
	pub x: i32,
	pub y: i32,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendGraphInput {
	#[serde(rename = "dataType")]
	pub data_type: FrontendGraphDataType,
	pub name: String,
	pub description: String,
	#[serde(rename = "resolvedType")]
	pub resolved_type: String,
	/// Either "nothing", "import index {index}", or "{node name} output {output_index}".
	#[serde(rename = "connectedToString")]
	pub connected_to: String,
	/// Used to render the upstream node once this node is rendered
	#[serde(rename = "connectedToNode")]
	pub connected_to_node: Option<NodeId>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
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

// Metadata that is common to nodes and layers
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendNodeMetadata {
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

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendNode {
	// pub position: FrontendNodePosition,
	pub position: FrontendXY,
	pub inputs: Vec<Option<FrontendGraphInput>>,
	pub outputs: Vec<Option<FrontendGraphOutput>>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
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
	layer_has_left_border_gap: bool,
	#[serde(rename = "primaryInputConnectedToLayer")]
	pub primary_input_connected_to_layer: bool,
	#[serde(rename = "primaryOutputConnectedToLayer")]
	pub primary_output_connected_to_layer: bool,
}

// // Should be an enum but those are hard to serialize/deserialize to TS
// #[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
// pub struct FrontendNodePosition {
// 	pub absolute: Option<FrontendXY>,
// 	pub chain: Option<bool>,
// }

// // Should be an enum but those are hard to serialize/deserialize to TS
// #[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
// pub struct FrontendLayerPosition {
// 	pub absolute: Option<FrontendXY>,
// 	pub stack: Option<u32>,
// }

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendNodeOrLayer {
	pub metadata: FrontendNodeMetadata,
	pub node: Option<FrontendNode>,
	pub layer: Option<FrontendLayer>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendNodeType {
	pub name: Cow<'static, str>,
	pub category: Cow<'static, str>,
	#[serde(rename = "inputTypes")]
	pub input_types: Option<Vec<Cow<'static, str>>>,
}

impl FrontendNodeType {
	pub fn new(name: impl Into<Cow<'static, str>>, category: impl Into<Cow<'static, str>>) -> Self {
		Self {
			name: name.into(),
			category: category.into(),
			input_types: None,
		}
	}

	pub fn with_input_types(name: impl Into<Cow<'static, str>>, category: impl Into<Cow<'static, str>>, input_types: Vec<Cow<'static, str>>) -> Self {
		Self {
			name: name.into(),
			category: category.into(),
			input_types: Some(input_types),
		}
	}
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct DragStart {
	pub start_x: f64,
	pub start_y: f64,
	pub round_x: i32,
	pub round_y: i32,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct Transform {
	pub scale: f64,
	pub x: f64,
	pub y: f64,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct BoxSelection {
	#[serde(rename = "startX")]
	pub start_x: u32,
	#[serde(rename = "startY")]
	pub start_y: u32,
	#[serde(rename = "endX")]
	pub end_x: u32,
	#[serde(rename = "endY")]
	pub end_y: u32,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum ContextMenuData {
	ToggleLayer {
		#[serde(rename = "nodeId")]
		node_id: NodeId,
		#[serde(rename = "currentlyIsNode")]
		currently_is_node: bool,
	},
	CreateNode {
		#[serde(rename = "compatibleType")]
		#[serde(default)]
		compatible_type: Option<String>,
	},
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct ContextMenuInformation {
	// Stores whether the context menu is open and its position in graph coordinates
	#[serde(rename = "contextMenuCoordinates")]
	pub context_menu_coordinates: (i32, i32),
	#[serde(rename = "contextMenuData")]
	pub context_menu_data: ContextMenuData,
}

#[derive(Clone, Debug, PartialEq, Default, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendClickTargets {
	#[serde(rename = "nodeClickTargets")]
	pub node_click_targets: Vec<String>,
	#[serde(rename = "layerClickTargets")]
	pub layer_click_targets: Vec<String>,
	#[serde(rename = "connectorClickTargets")]
	pub connector_click_targets: Vec<String>,
	#[serde(rename = "iconClickTargets")]
	pub icon_click_targets: Vec<String>,
	#[serde(rename = "allNodesBoundingBox")]
	pub all_nodes_bounding_box: String,
	#[serde(rename = "importExportsBoundingBox")]
	pub import_exports_bounding_box: String,
	#[serde(rename = "modifyImportExport")]
	pub modify_import_export: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum Direction {
	Up,
	Down,
	Left,
	Right,
}
