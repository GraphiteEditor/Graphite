use glam::IVec2;
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendGraphInput {
	pub data_type: FrontendGraphDataType,
	pub name: String,
	pub description: String,
	pub resolved_type: String,
	pub valid_types: Vec<String>,
	/// Whether a wire feeds this input.
	pub connected: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendGraphOutput {
	pub data_type: FrontendGraphDataType,
	pub name: String,
	pub description: String,
	pub resolved_type: String,
	/// Every type this output could produce given its node's current inputs, or empty where that isn't known.
	pub possible_types: Vec<String>,
	/// Whether this output feeds at least one wire.
	pub connected: bool,
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
	/// How many grid cells the drag had come when the nodes were last stored where it put them.
	#[cfg_attr(feature = "wasm", tsify(type = "[number, number]"))]
	pub stored: IVec2,
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
		/// Whether the node names a table entry other nodes name too, which "Make Unique" gives it its own copy of.
		#[serde(rename = "isShared")]
		is_shared: bool,
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
		description: String,
		/// Where the description popover points, in node graph coordinates.
		#[serde(rename = "descriptionAnchor")]
		description_anchor: (f64, f64),
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

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NodeEvaluationError {
	pub node_path: Vec<NodeId>,
	pub message: String,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Direction {
	Up,
	Down,
	Left,
	Right,
}

impl Direction {
	/// The one grid cell step in this direction.
	pub fn offset(self) -> IVec2 {
		match self {
			Direction::Up => IVec2::NEG_Y,
			Direction::Down => IVec2::Y,
			Direction::Left => IVec2::NEG_X,
			Direction::Right => IVec2::X,
		}
	}
}

/// Another peer's pointer over the node graph, in viewport pixels.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FrontendRemoteCursor {
	pub x: f64,
	pub y: f64,
	pub name: String,
	/// A stand-in name, shown in italics.
	pub anonymous: bool,
	pub color: String,
	pub tool: Option<String>,
}
