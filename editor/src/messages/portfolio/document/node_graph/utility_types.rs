use std::borrow::Cow;

use graphene_std::uuid::NodeId;

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
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DragStart {
	pub start_x: f64,
	pub start_y: f64,
	pub round_x: i32,
	pub round_y: i32,
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
