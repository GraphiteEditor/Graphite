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

/// SnappingState determines the current individual snapping states
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(default)]
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
			bounds: Default::default(),
			nodes: Default::default(),
			grid: Default::default(),
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
			SnapTarget::Artboard(_) => self.artboards,
			SnapTarget::Grid(_) => self.grid_snapping,
			SnapTarget::Alignment(AlignmentSnapTarget::Handle) => self.nodes.align,
			SnapTarget::Alignment(_) => self.bounds.align,
			SnapTarget::Distribution(_) => self.bounds.distribute,
			_ => false,
		}
	}
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct BoundsSnapping {
	pub edges: bool,
	pub corners: bool,
	pub edge_midpoints: bool,
	pub centers: bool,
	pub align: bool,
	pub distribute: bool,
}

impl Default for BoundsSnapping {
	fn default() -> Self {
		Self {
			edges: true,
			corners: true,
			edge_midpoints: false,
			centers: true,
			align: true,
			distribute: true,
		}
	}
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct PointSnapping {
	pub paths: bool,
	pub path_intersections: bool,
	pub anchors: bool,
	pub line_midpoints: bool,
	pub normals: bool,
	pub tangents: bool,
	pub align: bool,
}

impl Default for PointSnapping {
	fn default() -> Self {
		Self {
			paths: true,
			path_intersections: true,
			anchors: true,
			line_midpoints: true,
			normals: true,
			tangents: true,
			align: false,
		}
	}
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum GridType {
	Rectangle { spacing: DVec2 },
	Isometric { y_axis_spacing: f64, angle_a: f64, angle_b: f64 },
}

impl Default for GridType {
	fn default() -> Self {
		Self::RECTANGLE
	}
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
#[serde(default)]
pub struct GridSnapping {
	pub origin: DVec2,
	pub grid_type: GridType,
	pub grid_color: Color,
	pub dot_display: bool,
}

impl Default for GridSnapping {
	fn default() -> Self {
		Self {
			origin: DVec2::ZERO,
			grid_type: Default::default(),
			grid_color: COLOR_OVERLAY_GRAY
				.strip_prefix('#')
				.and_then(Color::from_rgb_str)
				.expect("Should create Color from prefixed hex string"),
			dot_display: false,
		}
	}
}

impl GridSnapping {
	// Double grid size until it takes up at least 10px.
	pub fn compute_rectangle_spacing(mut size: DVec2, navigation: &PTZ) -> Option<DVec2> {
		let mut iterations = 0;
		size = size.abs();
		while (size * navigation.zoom()).cmplt(DVec2::splat(10.)).any() {
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
		while (length / divisor.abs().max(1.)) * multiplier * navigation.zoom() < 10. {
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
pub enum ArtboardSnapSource {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignmentSnapSource {
	BoundsCorner,
	BoundsCenter,
	BoundsEdgeMidpoint,
	ArtboardCorner,
	ArtboardCenter,
	Handle,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapSource {
	#[default]
	None,
	BoundingBox(BoundingBoxSnapSource),
	Artboard(ArtboardSnapSource),
	Geometry(GeometrySnapSource),
	Alignment(AlignmentSnapSource),
}

impl SnapSource {
	pub fn is_some(&self) -> bool {
		self != &Self::None
	}
	pub fn bounding_box(&self) -> bool {
		matches!(self, Self::BoundingBox(_) | Self::Artboard(_))
	}
	pub fn align(&self) -> bool {
		matches!(self, Self::Alignment(_))
	}
	pub fn center(&self) -> bool {
		matches!(
			self,
			Self::Alignment(AlignmentSnapSource::ArtboardCenter | AlignmentSnapSource::BoundsCenter) | Self::Artboard(ArtboardSnapSource::Center) | Self::BoundingBox(BoundingBoxSnapSource::Center)
		)
	}
}

type GetSnapState = for<'a> fn(&'a mut SnappingState) -> &'a mut bool;
pub const GET_SNAP_BOX_FUNCTIONS: [(&str, GetSnapState); 6] = [
	("Box Center", (|snapping_state| &mut snapping_state.bounds.centers) as GetSnapState),
	("Box Corner", (|snapping_state| &mut snapping_state.bounds.corners) as GetSnapState),
	("Along Edge", (|snapping_state| &mut snapping_state.bounds.edges) as GetSnapState),
	("Midpoint of Edge", (|snapping_state| &mut snapping_state.bounds.edge_midpoints) as GetSnapState),
	("Align to Box", (|snapping_state| &mut snapping_state.bounds.align) as GetSnapState),
	("Evenly Distribute Boxes", (|snapping_state| &mut snapping_state.bounds.distribute) as GetSnapState),
];
pub const GET_SNAP_GEOMETRY_FUNCTIONS: [(&str, GetSnapState); 7] = [
	("Anchor", (|snapping_state: &mut SnappingState| &mut snapping_state.nodes.anchors) as GetSnapState),
	("Line Midpoint", (|snapping_state: &mut SnappingState| &mut snapping_state.nodes.line_midpoints) as GetSnapState),
	("Path", (|snapping_state: &mut SnappingState| &mut snapping_state.nodes.paths) as GetSnapState),
	("Normal to Path", (|snapping_state: &mut SnappingState| &mut snapping_state.nodes.normals) as GetSnapState),
	("Tangent to Path", (|snapping_state: &mut SnappingState| &mut snapping_state.nodes.tangents) as GetSnapState),
	("Intersection", (|snapping_state: &mut SnappingState| &mut snapping_state.nodes.path_intersections) as GetSnapState),
	("Align to Selected Path", (|snapping_state: &mut SnappingState| &mut snapping_state.nodes.align) as GetSnapState),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BoundingBoxSnapTarget {
	Center,
	Corner,
	Edge,
	EdgeMidpoint,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtboardSnapTarget {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignmentSnapTarget {
	BoundsCorner,
	BoundsCenter,
	ArtboardCorner,
	ArtboardCenter,
	Handle,
	Intersection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistributionSnapTarget {
	X,
	Y,
	Right,
	Left,
	Up,
	Down,
	Xy,
}

impl DistributionSnapTarget {
	pub const fn is_x(&self) -> bool {
		matches!(self, Self::Left | Self::Right | Self::X)
	}
	pub const fn is_y(&self) -> bool {
		matches!(self, Self::Up | Self::Down | Self::Y)
	}
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapTarget {
	#[default]
	None,
	BoundingBox(BoundingBoxSnapTarget),
	Geometry(GeometrySnapTarget),
	Artboard(ArtboardSnapTarget),
	Grid(GridSnapTarget),
	Alignment(AlignmentSnapTarget),
	Distribution(DistributionSnapTarget),
}

impl SnapTarget {
	pub fn is_some(&self) -> bool {
		self != &Self::None
	}
	pub fn bounding_box(&self) -> bool {
		matches!(self, Self::BoundingBox(_) | Self::Artboard(_))
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

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct PTZ {
	/// Offset distance.
	pub pan: DVec2,
	/// Angle in radians.
	tilt: f64,
	/// Scale factor.
	zoom: f64,
}

impl Default for PTZ {
	fn default() -> Self {
		Self { pan: DVec2::ZERO, tilt: 0., zoom: 1. }
	}
}

impl PTZ {
	/// Get the tilt angle between -180° and 180° in radians.
	pub fn tilt(&self) -> f64 {
		(((self.tilt + std::f64::consts::PI) % std::f64::consts::TAU) + std::f64::consts::TAU) % std::f64::consts::TAU - std::f64::consts::PI
	}

	/// Set a new tilt angle in radians.
	pub fn set_tilt(&mut self, tilt: f64) {
		self.tilt = tilt;
	}

	/// Get the scale factor.
	pub fn zoom(&self) -> f64 {
		self.zoom
	}

	/// Set a new scale factor.
	pub fn set_zoom(&mut self, zoom: f64) {
		self.zoom = zoom.clamp(crate::consts::VIEWPORT_ZOOM_SCALE_MIN, crate::consts::VIEWPORT_ZOOM_SCALE_MAX)
	}
}
