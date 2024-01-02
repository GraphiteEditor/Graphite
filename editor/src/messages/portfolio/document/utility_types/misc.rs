pub use super::layer_panel::LayerPanelEntry;

use glam::DVec2;
use serde::{Deserialize, Serialize};
use std::fmt;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct DocumentId(pub u64);

#[derive(PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize, Hash)]
pub enum FlipAxis {
	X,
	Y,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize, Hash, specta::Type)]
pub enum AlignAxis {
	X,
	Y,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize, Hash, specta::Type)]
pub enum AlignAggregate {
	Min,
	Max,
	Center,
}

#[derive(Default, PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum DocumentMode {
	#[default]
	DesignMode,
	SelectMode,
	GuideMode,
}

impl fmt::Display for DocumentMode {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			DocumentMode::DesignMode => write!(f, "Design Mode"),
			DocumentMode::SelectMode => write!(f, "Select Mode"),
			DocumentMode::GuideMode => write!(f, "Guide Mode"),
		}
	}
}

impl DocumentMode {
	pub fn icon_name(&self) -> String {
		match self {
			DocumentMode::DesignMode => "ViewportDesignMode".to_string(),
			DocumentMode::SelectMode => "ViewportSelectMode".to_string(),
			DocumentMode::GuideMode => "ViewportGuideMode".to_string(),
		}
	}
}

#[derive(Clone, Debug)]
/// SnappingState determines the current individual snapping states
pub struct SnappingState {
	pub snapping_enabled: bool,
	pub bounding_box_snapping: bool,
	pub node_snapping: bool,
	pub grid_snapping: bool,
	pub bounds: BoundsSnapping,
	pub nodes: NodeSnapping,
	pub grid: GridSnapping,
	pub tolerance: f64,
	pub artboards: bool,
}
impl Default for SnappingState {
	fn default() -> Self {
		Self {
			snapping_enabled: true,
			bounding_box_snapping: true,
			node_snapping: true,
			grid_snapping: false,
			bounds: BoundsSnapping {
				edges: true,
				corners: true,
				edge_midpoints: false,
				centres: false,
			},
			nodes: NodeSnapping {
				paths: true,
				path_intersections: true,
				sharp_nodes: true,
				smooth_nodes: true,
				line_midpoints: true,
				perpendicular: true,
				tangents: true,
			},
			grid: GridSnapping {
				origin: DVec2::ZERO,
				size: DVec2::ONE,
				dots: false,
			},
			tolerance: 20.,
			artboards: true,
		}
	}
}
impl SnappingState {
	pub const fn target_enabled(&self, target: SnapTarget) -> bool {
		if !self.snapping_enabled {
			return false;
		}
		match target {
			SnapTarget::BoundingBox(bounding_box) if self.bounding_box_snapping => match bounding_box {
				BoundingBoxSnapTarget::Corner => self.bounds.corners,
				BoundingBoxSnapTarget::Edge => self.bounds.edges,
				BoundingBoxSnapTarget::EdgeMidpoint => self.bounds.edge_midpoints,
				BoundingBoxSnapTarget::Centre => self.bounds.centres,
			},
			SnapTarget::Node(nodes) if self.node_snapping => match nodes {
				NodeSnapTarget::Smooth => self.nodes.smooth_nodes,
				NodeSnapTarget::Sharp => self.nodes.sharp_nodes,
				NodeSnapTarget::LineMidpoint => self.nodes.line_midpoints,
				NodeSnapTarget::Path => self.nodes.paths,
				NodeSnapTarget::Parpendicular => self.nodes.perpendicular,
				NodeSnapTarget::Tangent => self.nodes.tangents,
				NodeSnapTarget::Intersection => self.nodes.path_intersections,
			},
			SnapTarget::Board(_) => self.artboards,
			_ => false,
		}
	}
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoundsSnapping {
	pub edges: bool,
	pub corners: bool,
	pub edge_midpoints: bool,
	pub centres: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeSnapping {
	pub paths: bool,
	pub path_intersections: bool,
	pub sharp_nodes: bool,
	pub smooth_nodes: bool,
	pub line_midpoints: bool,
	pub perpendicular: bool,
	pub tangents: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GridSnapping {
	pub origin: DVec2,
	pub size: DVec2,
	pub dots: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundingBoxSnapSource {
	Corner,
	Centre,
	EdgeMidpoint,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardSnapSource {
	Centre,
	Corner,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeSnapSource {
	Smooth,
	Sharp,
	LineMidpoint,
	PathIntersection,
}
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapSource {
	#[default]
	None,
	BoundingBox(BoundingBoxSnapSource),
	Board(BoardSnapSource),
	Node(NodeSnapSource),
}
impl SnapSource {
	pub fn is_some(&self) -> bool {
		self != &Self::None
	}
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundingBoxSnapTarget {
	Corner,
	Edge,
	EdgeMidpoint,
	Centre,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeSnapTarget {
	Smooth,
	Sharp,
	LineMidpoint,
	Path,
	Parpendicular,
	Tangent,
	Intersection,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardSnapTarget {
	Edge,
	Corner,
	Centre,
}
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapTarget {
	#[default]
	None,
	BoundingBox(BoundingBoxSnapTarget),
	Node(NodeSnapTarget),
	Board(BoardSnapTarget),
}
impl SnapTarget {
	pub fn is_some(&self) -> bool {
		self != &Self::None
	}
}
// TODO: implement icons for SnappingOptions eventually
pub enum SnappingOptions {
	BoundingBoxes,
	Points,
}

impl fmt::Display for SnappingOptions {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			SnappingOptions::BoundingBoxes => write!(f, "Bounding Boxes"),
			SnappingOptions::Points => write!(f, "Points"),
		}
	}
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PTZ {
	pub pan: DVec2,
	pub tilt: f64,
	pub zoom: f64,
}

impl Default for PTZ {
	fn default() -> Self {
		Self { pan: DVec2::ZERO, tilt: 0., zoom: 1. }
	}
}
