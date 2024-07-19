use graph_craft::document::value::TaggedValue;
use graph_craft::document::NodeId;
use graphene_core::Type;
use graphene_std::renderer::ClickTarget;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum FrontendGraphDataType {
	#[default]
	General,
	Raster,
	VectorData,
	Number,
	Graphic,
	Artboard,
}

impl FrontendGraphDataType {
	pub fn with_type(input: &Type) -> Self {
		match TaggedValue::from_type(input) {
			TaggedValue::Image(_) | TaggedValue::ImageFrame(_) => Self::Raster,
			TaggedValue::Subpaths(_) | TaggedValue::VectorData(_) => Self::VectorData,
			TaggedValue::U32(_)
			| TaggedValue::U64(_)
			| TaggedValue::F64(_)
			| TaggedValue::UVec2(_)
			| TaggedValue::IVec2(_)
			| TaggedValue::DVec2(_)
			| TaggedValue::OptionalDVec2(_)
			| TaggedValue::F64Array4(_)
			| TaggedValue::VecF64(_)
			| TaggedValue::VecDVec2(_) => Self::Number,
			TaggedValue::GraphicGroup(_) | TaggedValue::GraphicElement(_) => Self::Graphic,
			TaggedValue::ArtboardGroup(_) => Self::Artboard,
			_ => Self::General,
		}
	}
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendGraphInput {
	#[serde(rename = "dataType")]
	pub data_type: FrontendGraphDataType,
	pub name: String,
	#[serde(rename = "resolvedType")]
	pub resolved_type: Option<String>,
	pub connected: Option<NodeId>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendGraphOutput {
	#[serde(rename = "dataType")]
	pub data_type: FrontendGraphDataType,
	pub name: String,
	#[serde(rename = "resolvedType")]
	pub resolved_type: Option<String>,
	pub connected: Vec<NodeId>,
	#[serde(rename = "connectedIndex")]
	pub connected_index: Vec<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendNode {
	pub id: graph_craft::document::NodeId,
	#[serde(rename = "isLayer")]
	pub is_layer: bool,
	#[serde(rename = "canBeLayer")]
	pub can_be_layer: bool,
	pub alias: String,
	pub name: String,
	#[serde(rename = "primaryInput")]
	pub primary_input: Option<FrontendGraphInput>,
	#[serde(rename = "exposedInputs")]
	pub exposed_inputs: Vec<FrontendGraphInput>,
	#[serde(rename = "primaryOutput")]
	pub primary_output: Option<FrontendGraphOutput>,
	#[serde(rename = "exposedOutputs")]
	pub exposed_outputs: Vec<FrontendGraphOutput>,
	pub position: (i32, i32),
	pub visible: bool,
	pub locked: bool,
	pub previewed: bool,
	pub errors: Option<String>,
	#[serde(rename = "uiOnly")]
	pub ui_only: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendNodeWire {
	#[serde(rename = "wireStart")]
	pub wire_start: NodeId,
	#[serde(rename = "wireStartOutputIndex")]
	pub wire_start_output_index: usize,
	#[serde(rename = "wireEnd")]
	pub wire_end: NodeId,
	#[serde(rename = "wireEndInputIndex")]
	pub wire_end_input_index: usize,
	pub dashed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendNodeType {
	pub name: String,
	pub category: String,
}

impl FrontendNodeType {
	pub fn new(name: &'static str, category: &'static str) -> Self {
		Self {
			name: name.to_string(),
			category: category.to_string(),
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
pub struct WirePath {
	#[serde(rename = "pathString")]
	pub path_string: String,
	#[serde(rename = "dataType")]
	pub data_type: FrontendGraphDataType,
	pub thick: bool,
	pub dashed: bool,
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
	CreateNode,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct ContextMenuInformation {
	// Stores whether the context menu is open and its position in graph coordinates
	#[serde(rename = "contextMenuCoordinates")]
	pub context_menu_coordinates: (i32, i32),
	#[serde(rename = "contextMenuData")]
	pub context_menu_data: ContextMenuData,
}

#[derive(Debug, Clone)]
pub struct NodeMetadata {
	/// Cache for all node click targets in node graph space. Ensure `update_click_target` is called when modifying a node property that changes its size. Currently this is `alias`, `inputs`, `is_layer`, and `metadata`.
	pub node_click_target: ClickTarget,
	/// Cache for all node inputs. Should be automatically updated when `update_click_target` is called.
	pub input_click_targets: Vec<ClickTarget>,
	/// Cache for all node outputs. Should be automatically updated when `update_click_target` is called.
	pub output_click_targets: Vec<ClickTarget>,
	/// Cache for all visibility buttons. Should be automatically updated when `update_click_target` is called.
	pub visibility_click_target: Option<ClickTarget>,
	/// Stores the width in grid cell units for layer nodes from the left edge of the thumbnail (+12px padding since thumbnail ends between grid spaces) to the end of the node.
	pub layer_width: Option<u32>,
}
