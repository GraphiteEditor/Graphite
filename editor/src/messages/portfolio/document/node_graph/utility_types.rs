use graph_craft::document::NodeId;
use graphene_std::{Type, simplify_identifier_name};

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
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
	Invalid,
}

impl FrontendGraphDataType {
	pub fn from_type(input: &Type) -> Self {
		// Color a wire by its element type, peeling a rank-0 `Item` or rank-1 `List` wrapper (and a whole-list `Bundle` cell) so all ranks share the element's color
		let nested_type = input.nested_type();
		let element = match nested_type.bundle_element_name() {
			Some(bundle_element) => simplify_identifier_name(bundle_element),
			None => nested_type.list_element().unwrap_or(nested_type).identifier_name(),
		};

		match element.as_str() {
			"Vector" => Self::Vector,
			"Graphic" => Self::Graphic,
			"Artboard" => Self::Artboard,
			"Color" => Self::Color,
			"Gradient" => Self::Gradient,
			"String" => Self::Typography,
			"f64" | "i64" | "bool" | "DVec2" | "DAffine2" => Self::Number,
			raster if raster.starts_with("Raster") => Self::Raster,
			_ => Self::General,
		}
	}
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FrontendGraphInput {
	#[serde(rename = "dataType")]
	pub data_type: FrontendGraphDataType,
	pub name: String,
	pub description: String,
	#[serde(rename = "resolvedType")]
	pub resolved_type: String,
	#[serde(rename = "validTypes")]
	pub valid_types: Vec<String>,
	/// Whether a wire feeds this input.
	pub connected: bool,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FrontendGraphOutput {
	#[serde(rename = "dataType")]
	pub data_type: FrontendGraphDataType,
	pub name: String,
	pub description: String,
	#[serde(rename = "resolvedType")]
	pub resolved_type: String,
	/// Every type this output could produce given its node's current inputs, or empty where that isn't known.
	#[serde(rename = "possibleTypes")]
	pub possible_types: Vec<String>,
	/// Whether this output feeds at least one wire.
	pub connected: bool,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FrontendNode {
	pub id: graph_craft::document::NodeId,
	#[serde(rename = "isLayer")]
	pub is_layer: bool,
	#[serde(rename = "canBeLayer")]
	pub can_be_layer: bool,
	/// Whether the node is a redirect dot, which is a Passthrough node drawn as a small dot bending the wire running through it.
	#[serde(rename = "isRedirectDot")]
	pub is_redirect_dot: bool,
	pub reference: Option<String>,
	#[serde(rename = "displayName")]
	pub display_name: String,
	#[serde(rename = "implementationName")]
	pub implementation_name: String,
	#[serde(rename = "primaryInput")]
	pub primary_input: Option<FrontendGraphInput>,
	#[serde(rename = "exposedInputs")]
	pub exposed_inputs: Vec<FrontendGraphInput>,
	#[serde(rename = "primaryOutput")]
	pub primary_output: Option<FrontendGraphOutput>,
	#[serde(rename = "exposedOutputs")]
	pub exposed_outputs: Vec<FrontendGraphOutput>,
	#[serde(rename = "primaryInputConnectedToLayer")]
	pub primary_input_connected_to_layer: bool,
	#[serde(rename = "primaryOutputConnectedToLayer")]
	pub primary_output_connected_to_layer: bool,
	pub position: (i32, i32),
	pub previewed: bool,
	pub visible: bool,
	pub locked: bool,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum InputTypeConstraint {
	Limited(std::collections::BTreeSet<String>),
	All,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FrontendNodeType {
	pub identifier: String,
	pub name: String,
	pub category: String,
	#[serde(rename = "inputTypes")]
	pub input_types: Vec<InputTypeConstraint>,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DragStart {
	pub start_x: f64,
	pub start_y: f64,
	pub round_x: i32,
	pub round_y: i32,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ContextMenuData {
	ModifyNode {
		#[serde(rename = "nodeId")]
		node_id: NodeId,
		#[serde(rename = "canBeLayer")]
		can_be_layer: bool,
		#[serde(rename = "currentlyIsNode")]
		currently_is_node: bool,
		#[serde(rename = "hasSelectedLayers")]
		has_selected_layers: bool,
		#[serde(rename = "allSelectedLayersLocked")]
		all_selected_layers_locked: bool,
	},
	CreateNode {
		#[serde(rename = "compatibleType")]
		compatible_type: Option<String>,
	},
	/// An import or export of the viewed subgraph, by its index among the imports or the exports.
	ModifyImportExport {
		#[serde(rename = "isImport")]
		is_import: bool,
		index: u32,
		/// For an import, whether the subgraph node shows its input as a connector rather than as a parameter in the Inspector.
		exposed: bool,
		/// Whether any wire is attached to it within the subgraph.
		connected: bool,
	},
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ContextMenuInformation {
	// The menu's display position in graph coordinates, which may be shifted from the click point to keep the menu on-screen
	#[serde(rename = "contextMenuCoordinates")]
	pub context_menu_coordinates: (i32, i32),
	// The click point in graph coordinates, where a node created from the menu is placed (unshifted, unlike the display position)
	#[serde(rename = "nodeCreationCoordinates")]
	pub node_creation_coordinates: (i32, i32),
	#[serde(rename = "contextMenuData")]
	pub context_menu_data: ContextMenuData,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct NodeGraphErrorDiagnostic {
	/// The node the error is shown beside, which it follows as the node glides into place.
	#[serde(rename = "nodeId")]
	pub node_id: NodeId,
	pub position: (i32, i32),
	pub error: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NodeEvaluationError {
	pub node_path: Vec<NodeId>,
	pub message: String,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, PartialEq, Default, serde::Serialize, serde::Deserialize)]
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
	#[serde(rename = "importExportNames")]
	pub import_export_names: Vec<String>,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Direction {
	Up,
	Down,
	Left,
	Right,
}
