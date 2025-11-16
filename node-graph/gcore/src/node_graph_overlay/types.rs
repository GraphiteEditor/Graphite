use glam::{DAffine2, DVec2, IVec2};
use kurbo::BezPath;

use crate::{Graphic, uuid::NodeId};
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

/// Stores node graph coordinates which are then transformed in svelte based on the node graph transform
#[derive(Clone, Debug, Default, PartialEq, Hash, dyn_any::DynAny, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendXY {
	pub x: i32,
	pub y: i32,
}

impl From<DVec2> for FrontendXY {
	fn from(v: DVec2) -> Self {
		FrontendXY { x: v.x as i32, y: v.y as i32 }
	}
}

impl From<IVec2> for FrontendXY {
	fn from(v: IVec2) -> Self {
		FrontendXY { x: v.x as i32, y: v.y as i32 }
	}
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

#[derive(Clone, Debug, Default, PartialEq, Hash, dyn_any::DynAny, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendGraphInputNew {
	#[serde(rename = "dataType")]
	pub data_type: FrontendGraphDataType,
	pub name: String,
	/// Used to render the upstream node once this node is rendered
	#[serde(rename = "connectedToNode")]
	pub connected_to_node: Option<NodeId>,
}

#[derive(Clone, Debug, Default, PartialEq, Hash, dyn_any::DynAny, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendGraphOutputNew {
	#[serde(rename = "dataType")]
	pub data_type: FrontendGraphDataType,
	pub name: String,
	pub connected: bool,
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
	Invalid,
}

#[derive(Clone, Debug, Default, PartialEq, dyn_any::DynAny)]
pub struct NodeGraphOverlayData {
	pub nodes_to_render: Vec<FrontendNodeToRenderNew>,
	pub in_selected_network: bool,
	// Displays a dashed border around the node
	pub previewed_node: Option<NodeId>,
	pub thumbnails: HashMap<NodeId, Graphic>,
}

impl Hash for NodeGraphOverlayData {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.nodes_to_render.hash(state);
		self.in_selected_network.hash(state);
		self.previewed_node.hash(state);
		let mut entries: Vec<_> = self.thumbnails.iter().collect();
		entries.sort_by(|a, b| a.0.cmp(b.0));
		let mut hasher = std::collections::hash_map::DefaultHasher::new();
		entries.hash(&mut hasher);
	}
}

#[derive(Clone, Debug, PartialEq, dyn_any::DynAny)]
pub struct FrontendNodeToRenderNew {
	pub metadata: FrontendNodeMetadataNew,
	pub node_or_layer: FrontendNodeOrLayer,
	//TODO: Remove and replace with method of generating wires when generating nodes
	pub wires: Vec<(BezPath, bool, FrontendGraphDataType)>,
}

impl Hash for FrontendNodeToRenderNew {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.metadata.hash(state);
		self.node_or_layer.hash(state);
	}
}

// Metadata that is common to nodes and layers
#[derive(Clone, Debug, Default, PartialEq, Hash, dyn_any::DynAny)]
pub struct FrontendNodeMetadataNew {
	pub node_id: NodeId,
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

#[derive(Clone, Debug, Default, PartialEq, Hash, dyn_any::DynAny)]
pub struct FrontendNodeNew {
	// pub position: FrontendNodePosition,
	pub position: FrontendXY,
	pub primary_output: Option<FrontendGraphOutputNew>,
	pub primary_input: Option<FrontendGraphInputNew>,
	pub secondary_inputs: Vec<FrontendGraphInputNew>,
	pub secondary_outputs: Vec<FrontendGraphOutputNew>,
}

#[derive(Clone, Debug, Default, PartialEq, Hash, dyn_any::DynAny)]
pub struct FrontendLayerNew {
	pub bottom_input: FrontendGraphInputNew,
	pub side_input: Option<FrontendGraphInputNew>,
	pub output: FrontendGraphOutputNew,
	// pub position: FrontendLayerPosition,
	pub position: FrontendXY,
	pub locked: bool,
	pub chain_width: u32,
	pub layer_has_left_border_gap: bool,
	pub primary_input_connected_to_layer: bool,
	pub primary_output_connected_to_layer: bool,
}

#[derive(Clone, Debug, PartialEq, Hash, dyn_any::DynAny)]
pub enum FrontendNodeOrLayer {
	Node(FrontendNodeNew),
	Layer(FrontendLayerNew),
}
