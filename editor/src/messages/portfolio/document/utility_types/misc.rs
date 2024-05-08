use crate::consts::COLOR_OVERLAY_GRAY;

use graphene_core::raster::Color;

use glam::DVec2;
use std::fmt;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct DocumentId(pub u64);

#[derive(PartialEq, Eq, Clone, Copy, Debug, serde::Serialize, serde::Deserialize, Hash)]
pub enum FlipAxis {
	X,
	Y,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, serde::Serialize, serde::Deserialize, Hash, specta::Type)]
pub enum AlignAxis {
	X,
	Y,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, serde::Serialize, serde::Deserialize, Hash, specta::Type)]
pub enum AlignAggregate {
	Min,
	Max,
	Center,
}

#[derive(Default, PartialEq, Eq, Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
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

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
/// SnappingState determines the current individual snapping states
pub struct SnappingState {
	pub snapping_enabled: bool,
	pub grid_snapping: bool,
	pub bounds: BoundsSnapping,
	pub nodes: PointSnapping,
	pub grid: GridSnapping,
	pub tolerance: f64,
	pub artboards: bool,
}
impl Default for SnappingState {
	fn default() -> Self {
		Self {
			snapping_enabled: true,
			grid_snapping: false,
			bounds: BoundsSnapping {
				edges: true,
				corners: true,
				edge_midpoints: false,
				centers: true,
			},
			nodes: PointSnapping {
				paths: true,
				path_intersections: true,
				anchors: true,
				line_midpoints: true,
				normals: true,
				tangents: true,
			},
			grid: GridSnapping {
				origin: DVec2::ZERO,
				grid_type: GridType::RECTANGLE,
				grid_color: COLOR_OVERLAY_GRAY
					.strip_prefix("#")
					.and_then(|value| Color::from_rgb_str(value))
					.expect("Should create Color from prefixed hex string"),
				dot_display: false,
			},
			tolerance: 8.,
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
			SnapTarget::BoundingBox(bounding_box) => match bounding_box {
				BoundingBoxSnapTarget::Corner => self.bounds.corners,
				BoundingBoxSnapTarget::Edge => self.bounds.edges,
				BoundingBoxSnapTarget::EdgeMidpoint => self.bounds.edge_midpoints,
				BoundingBoxSnapTarget::Center => self.bounds.centers,
			},
			SnapTarget::Geometry(nodes) => match nodes {
				GeometrySnapTarget::AnchorWithColinearHandles => self.nodes.anchors,
				GeometrySnapTarget::AnchorWithFreeHandles => self.nodes.anchors,
				GeometrySnapTarget::LineMidpoint => self.nodes.line_midpoints,
				GeometrySnapTarget::Path => self.nodes.paths,
				GeometrySnapTarget::Normal => self.nodes.normals,
				GeometrySnapTarget::Tangent => self.nodes.tangents,
				GeometrySnapTarget::Intersection => self.nodes.path_intersections,
			},
			SnapTarget::Board(_) => self.artboards,
			SnapTarget::Grid(_) => self.grid_snapping,
			_ => false,
		}
	}
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BoundsSnapping {
	pub edges: bool,
	pub corners: bool,
	pub edge_midpoints: bool,
	pub centers: bool,
}
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct OptionBoundsSnapping {
	pub edges: Option<bool>,
	pub corners: Option<bool>,
	pub edge_midpoints: Option<bool>,
	pub centers: Option<bool>,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PointSnapping {
	pub paths: bool,
	pub path_intersections: bool,
	pub anchors: bool,
	pub line_midpoints: bool,
	pub normals: bool,
	pub tangents: bool,
}
#[derive(PartialEq, Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct OptionPointSnapping {
	pub paths: Option<bool>,
	pub path_intersections: Option<bool>,
	pub anchors: Option<bool>,
	pub line_midpoints: Option<bool>,
	pub normals: Option<bool>,
	pub tangents: Option<bool>,
}
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum GridType {
	Rectangle { spacing: DVec2 },
	Isometric { y_axis_spacing: f64, angle_a: f64, angle_b: f64 },
}
impl GridType {
	pub const RECTANGLE: Self = GridType::Rectangle { spacing: DVec2::ONE };
	pub const ISOMETRIC: Self = GridType::Isometric {
		y_axis_spacing: 1.,
		angle_a: 30.,
		angle_b: 30.,
	};
	pub fn rect_spacing(&mut self) -> Option<&mut DVec2> {
		match self {
			Self::Rectangle { spacing } => Some(spacing),
			_ => None,
		}
	}
	pub fn isometric_y_spacing(&mut self) -> Option<&mut f64> {
		match self {
			Self::Isometric { y_axis_spacing, .. } => Some(y_axis_spacing),
			_ => None,
		}
	}
	pub fn angle_a(&mut self) -> Option<&mut f64> {
		match self {
			Self::Isometric { angle_a, .. } => Some(angle_a),
			_ => None,
		}
	}
	pub fn angle_b(&mut self) -> Option<&mut f64> {
		match self {
			Self::Isometric { angle_b, .. } => Some(angle_b),
			_ => None,
		}
	}
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct GridSnapping {
	pub origin: DVec2,
	pub grid_type: GridType,
	pub grid_color: Color,
	pub dot_display: bool,
}
impl GridSnapping {
	// Double grid size until it takes up at least 10px.
	pub fn compute_rectangle_spacing(mut size: DVec2, navigation: &PTZ) -> Option<DVec2> {
		let mut iterations = 0;
		size = size.abs();
		while (size * navigation.zoom).cmplt(DVec2::splat(10.)).any() {
			if iterations > 100 {
				return None;
			}
			size *= 2.;
			iterations += 1;
		}
		Some(size)
	}

	// Double grid size until it takes up at least 10px.
	pub fn compute_isometric_multiplier(length: f64, divisor: f64, navigation: &PTZ) -> Option<f64> {
		let length = length.abs();
		let mut iterations = 0;
		let mut multiplier = 1.;
		while (length / divisor.abs().max(1.)) * multiplier * navigation.zoom < 10. {
			if iterations > 100 {
				return None;
			}
			multiplier *= 2.;
			iterations += 1;
		}
		Some(multiplier)
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundingBoxSnapSource {
	Center,
	Corner,
	EdgeMidpoint,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardSnapSource {
	Center,
	Corner,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometrySnapSource {
	AnchorWithColinearHandles,
	AnchorWithFreeHandles,
	Handle,
	LineMidpoint,
	Intersection,
}
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapSource {
	#[default]
	None,
	BoundingBox(BoundingBoxSnapSource),
	Board(BoardSnapSource),
	Geometry(GeometrySnapSource),
}
impl SnapSource {
	pub fn is_some(&self) -> bool {
		self != &Self::None
	}
	pub fn bounding_box(&self) -> bool {
		matches!(self, Self::BoundingBox(_) | Self::Board(_))
	}
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BoundingBoxSnapTarget {
	Center,
	Corner,
	Edge,
	EdgeMidpoint,
}

impl fmt::Display for BoundingBoxSnapTarget {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Center => write!(f, "Box Center"),
			Self::Corner => write!(f, "Box Corner"),
			Self::Edge => write!(f, "Along Edge"),
			Self::EdgeMidpoint => write!(f, "Midpoint of Edge"),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GeometrySnapTarget {
	AnchorWithColinearHandles,
	AnchorWithFreeHandles,
	LineMidpoint,
	Path,
	Normal,
	Tangent,
	Intersection,
}

impl fmt::Display for GeometrySnapTarget {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::AnchorWithColinearHandles => write!(f, "Anchor (Colinear Handles)"),
			Self::AnchorWithFreeHandles => write!(f, "Anchor (Free Handles)"),
			Self::LineMidpoint => write!(f, "Line Midpoint"),
			Self::Path => write!(f, "Path"),
			Self::Normal => write!(f, "Normal to Path"),
			Self::Tangent => write!(f, "Tangent to Path"),
			Self::Intersection => write!(f, "Intersection"),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardSnapTarget {
	Edge,
	Corner,
	Center,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridSnapTarget {
	Line,
	LineNormal,
	Intersection,
}
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapTarget {
	#[default]
	None,
	BoundingBox(BoundingBoxSnapTarget),
	Geometry(GeometrySnapTarget),
	Board(BoardSnapTarget),
	Grid(GridSnapTarget),
}
impl SnapTarget {
	pub fn is_some(&self) -> bool {
		self != &Self::None
	}
	pub fn bounding_box(&self) -> bool {
		matches!(self, Self::BoundingBox(_) | Self::Board(_))
	}
}
// TODO: implement icons for SnappingOptions eventually
pub enum SnappingOptions {
	BoundingBoxes,
	Geometry,
}

impl fmt::Display for SnappingOptions {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			SnappingOptions::BoundingBoxes => write!(f, "Bounding Boxes"),
			SnappingOptions::Geometry => write!(f, "Geometry"),
		}
	}
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
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
