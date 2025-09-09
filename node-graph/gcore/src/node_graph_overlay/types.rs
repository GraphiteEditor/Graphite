use glam::{DAffine2, DVec2};
use graphene_core_shaders::color::Color;
use kurbo::BezPath;

use crate::{Graphic, node_graph_overlay::consts::*, uuid::NodeId};
use std::{
	collections::HashMap,
	hash::{Hash, Hasher},
};

#[derive(Clone, Debug, Default, PartialEq, dyn_any::DynAny, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct NodeGraphTransform {
	pub scale: f64,
	pub x: f64,
	pub y: f64,
}

impl Hash for NodeGraphTransform {
	fn hash<H: Hasher>(&self, state: &mut H) {
		// Convert f64 to u64 bit pattern for hashing
		self.scale.to_bits().hash(state);
		self.x.to_bits().hash(state);
		self.y.to_bits().hash(state);
	}
}

impl NodeGraphTransform {
	pub fn to_daffine2(&self) -> DAffine2 {
		DAffine2::from_scale_angle_translation(DVec2::splat(self.scale), 0.0, DVec2::new(self.x, self.y))
	}
}

#[derive(Clone, Debug, Default, PartialEq, dyn_any::DynAny, serde::Serialize, serde::Deserialize)]
pub struct NodeGraphOverlayData {
	pub nodes_to_render: Vec<FrontendNodeToRender>,
	pub open: bool,
	pub in_selected_network: bool,
	// Displays a dashed border around the node
	pub previewed_node: Option<NodeId>,
	pub thumbnails: HashMap<NodeId, Graphic>,
}

impl Hash for NodeGraphOverlayData {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.nodes_to_render.hash(state);
		self.open.hash(state);
		self.in_selected_network.hash(state);
		self.previewed_node.hash(state);
		let mut entries: Vec<_> = self.thumbnails.iter().collect();
		entries.sort_by(|a, b| a.0.cmp(b.0));
		let mut hasher = std::collections::hash_map::DefaultHasher::new();
		entries.hash(&mut hasher);
	}
}

#[derive(Clone, Debug, Default, PartialEq, dyn_any::DynAny, serde::Serialize, serde::Deserialize)]
pub struct FrontendNodeToRender {
	pub metadata: FrontendNodeMetadata,
	#[serde(rename = "nodeOrLayer")]
	pub node_or_layer: FrontendNodeOrLayer,
	//TODO: Remove
	pub wires: Vec<(BezPath, bool, FrontendGraphDataType)>,
}

impl Hash for FrontendNodeToRender {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.metadata.hash(state);
		self.node_or_layer.hash(state);
	}
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
	pub primary_output: Option<FrontendGraphOutput>,
	pub primary_input: Option<FrontendGraphInput>,
	pub secondary_inputs: Vec<FrontendGraphInput>,
	pub secondary_outputs: Vec<FrontendGraphOutput>,
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

// Should be an enum but those are hard to serialize/deserialize to TS
#[derive(Clone, Debug, Default, PartialEq, Hash, dyn_any::DynAny, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendNodeOrLayer {
	pub node: Option<FrontendNode>,
	pub layer: Option<FrontendLayer>,
}

impl FrontendNodeOrLayer {
	pub fn to_enum(self) -> NodeOrLayer {
		let node_or_layer = if let Some(node) = self.node {
			Some(NodeOrLayer::Node(node))
		} else if let Some(layer) = self.layer {
			Some(NodeOrLayer::Layer(layer))
		} else {
			None
		};
		node_or_layer.unwrap()
	}
}

pub enum NodeOrLayer {
	Node(FrontendNode),
	Layer(FrontendLayer),
}

#[derive(Clone, Debug, Default, PartialEq, Hash, dyn_any::DynAny, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendGraphInput {
	#[serde(rename = "dataType")]
	pub data_type: FrontendGraphDataType,
	pub name: String,
	/// Used to render the upstream node once this node is rendered
	#[serde(rename = "connectedToNode")]
	pub connected_to_node: Option<NodeId>,
}

#[derive(Clone, Debug, Default, PartialEq, Hash, dyn_any::DynAny, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendGraphOutput {
	#[serde(rename = "dataType")]
	pub data_type: FrontendGraphDataType,
	pub name: String,
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

impl FrontendGraphDataType {
	pub fn data_color(&self) -> Color {
		let color_str = match self {
			FrontendGraphDataType::General => COLOR_DATA_GENERAL,
			FrontendGraphDataType::Number => COLOR_DATA_NUMBER,
			FrontendGraphDataType::Artboard => COLOR_DATA_ARTBOARD,
			FrontendGraphDataType::Graphic => COLOR_DATA_GRAPHIC,
			FrontendGraphDataType::Raster => COLOR_DATA_RASTER,
			FrontendGraphDataType::Vector => COLOR_DATA_VECTOR,
			FrontendGraphDataType::Color => COLOR_DATA_COLOR,
			FrontendGraphDataType::Gradient => COLOR_DATA_GRADIENT,
			FrontendGraphDataType::Typography => COLOR_DATA_TYPOGRAPHY,
		};
		Color::from_rgba8_no_srgb(color_str).unwrap()
	}
	pub fn data_color_dim(&self) -> Color {
		let color_str = match self {
			FrontendGraphDataType::General => COLOR_DATA_GENERAL_DIM,
			FrontendGraphDataType::Number => COLOR_DATA_NUMBER_DIM,
			FrontendGraphDataType::Artboard => COLOR_DATA_ARTBOARD_DIM,
			FrontendGraphDataType::Graphic => COLOR_DATA_GRAPHIC_DIM,
			FrontendGraphDataType::Raster => COLOR_DATA_RASTER_DIM,
			FrontendGraphDataType::Vector => COLOR_DATA_VECTOR_DIM,
			FrontendGraphDataType::Color => COLOR_DATA_COLOR_DIM,
			FrontendGraphDataType::Gradient => COLOR_DATA_GRADIENT_DIM,
			FrontendGraphDataType::Typography => COLOR_DATA_TYPOGRAPHY_DIM,
		};
		Color::from_rgba8_no_srgb(color_str).unwrap()
	}
}
