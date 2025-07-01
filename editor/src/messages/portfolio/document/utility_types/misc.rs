use crate::consts::COLOR_OVERLAY_GRAY;
use glam::DVec2;
use graphene_std::raster::Color;
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
	pub artboards: bool,
	pub tolerance: f64,
	pub bounding_box: BoundingBoxSnapping,
	pub path: PathSnapping,
	pub grid: GridSnapping,
}

impl Default for SnappingState {
	fn default() -> Self {
		Self {
			snapping_enabled: true,
			grid_snapping: false,
			artboards: true,
			tolerance: 8.,
			bounding_box: BoundingBoxSnapping::default(),
			path: PathSnapping::default(),
			grid: GridSnapping::default(),
		}
	}
}

impl SnappingState {
	pub const fn target_enabled(&self, target: SnapTarget) -> bool {
		if !self.snapping_enabled {
			return false;
		}
		match target {
			SnapTarget::BoundingBox(target) => match target {
				BoundingBoxSnapTarget::CornerPoint => self.bounding_box.corner_point,
				BoundingBoxSnapTarget::EdgeMidpoint => self.bounding_box.edge_midpoint,
				BoundingBoxSnapTarget::CenterPoint => self.bounding_box.center_point,
			},
			SnapTarget::Path(target) => match target {
				PathSnapTarget::AnchorPointWithColinearHandles | PathSnapTarget::AnchorPointWithFreeHandles => self.path.anchor_point,
				PathSnapTarget::LineMidpoint => self.path.line_midpoint,
				PathSnapTarget::AlongPath => self.path.along_path,
				PathSnapTarget::NormalToPath => self.path.normal_to_path,
				PathSnapTarget::TangentToPath => self.path.tangent_to_path,
				PathSnapTarget::IntersectionPoint => self.path.path_intersection_point,
				PathSnapTarget::PerpendicularToEndpoint => self.path.perpendicular_from_endpoint,
			},
			SnapTarget::Artboard(_) => self.artboards,
			SnapTarget::Grid(_) => self.grid_snapping,
			SnapTarget::Alignment(AlignmentSnapTarget::AlignWithAnchorPoint) => self.path.align_with_anchor_point,
			SnapTarget::Alignment(_) => self.bounding_box.align_with_edges,
			SnapTarget::DistributeEvenly(_) => self.bounding_box.distribute_evenly,
			_ => false,
		}
	}
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct BoundingBoxSnapping {
	pub center_point: bool,
	pub corner_point: bool,
	pub edge_midpoint: bool,
	pub align_with_edges: bool,
	pub distribute_evenly: bool,
}

impl Default for BoundingBoxSnapping {
	fn default() -> Self {
		Self {
			center_point: true,
			corner_point: true,
			edge_midpoint: true,
			align_with_edges: true,
			distribute_evenly: true,
		}
	}
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct PathSnapping {
	pub anchor_point: bool,
	pub line_midpoint: bool,
	pub along_path: bool,
	pub normal_to_path: bool,
	pub tangent_to_path: bool,
	pub path_intersection_point: bool,
	pub align_with_anchor_point: bool, // TODO: Rename
	pub perpendicular_from_endpoint: bool,
}

impl Default for PathSnapping {
	fn default() -> Self {
		Self {
			anchor_point: true,
			line_midpoint: true,
			along_path: true,
			normal_to_path: true,
			tangent_to_path: true,
			path_intersection_point: true,
			align_with_anchor_point: true,
			perpendicular_from_endpoint: true,
		}
	}
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum GridType {
	#[serde(alias = "Rectangle")]
	Rectangular {
		spacing: DVec2,
	},
	Isometric {
		y_axis_spacing: f64,
		angle_a: f64,
		angle_b: f64,
	},
}

impl Default for GridType {
	fn default() -> Self {
		Self::Rectangular { spacing: DVec2::ONE }
	}
}

impl GridType {
	pub fn rectangular_spacing(&mut self) -> Option<&mut DVec2> {
		match self {
			Self::Rectangular { spacing } => Some(spacing),
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
	pub rectangular_spacing: DVec2,
	pub isometric_y_spacing: f64,
	pub isometric_angle_a: f64,
	pub isometric_angle_b: f64,
	pub grid_color: Color,
	pub dot_display: bool,
}

impl Default for GridSnapping {
	fn default() -> Self {
		Self {
			origin: DVec2::ZERO,
			grid_type: Default::default(),
			rectangular_spacing: DVec2::ONE,
			isometric_y_spacing: 1.,
			isometric_angle_a: 30.,
			isometric_angle_b: 30.,
			grid_color: Color::from_rgb_str(COLOR_OVERLAY_GRAY.strip_prefix('#').unwrap()).unwrap(),
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
	CornerPoint,
	CenterPoint,
	EdgeMidpoint,
}

impl fmt::Display for BoundingBoxSnapSource {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			BoundingBoxSnapSource::CornerPoint => write!(f, "Bounding Box: Corner Point"),
			BoundingBoxSnapSource::CenterPoint => write!(f, "Bounding Box: Center Point"),
			BoundingBoxSnapSource::EdgeMidpoint => write!(f, "Bounding Box: Edge Midpoint"),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtboardSnapSource {
	CornerPoint,
	CenterPoint,
}

impl fmt::Display for ArtboardSnapSource {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			ArtboardSnapSource::CornerPoint => write!(f, "Artboard: Corner Point"),
			ArtboardSnapSource::CenterPoint => write!(f, "Artboard: Center Point"),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathSnapSource {
	AnchorPointWithColinearHandles,
	AnchorPointWithFreeHandles,
	HandlePoint,
	LineMidpoint,
	IntersectionPoint,
}

impl fmt::Display for PathSnapSource {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			PathSnapSource::AnchorPointWithColinearHandles | PathSnapSource::AnchorPointWithFreeHandles => write!(f, "Path: Anchor Point"),
			PathSnapSource::HandlePoint => write!(f, "Path: Handle Point"),
			PathSnapSource::LineMidpoint => write!(f, "Path: Line Midpoint"),
			PathSnapSource::IntersectionPoint => write!(f, "Path: Intersection Point"),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignmentSnapSource {
	BoundingBoxCornerPoint,
	BoundingBoxCenterPoint,
	BoundingBoxEdgeMidpoint,
	ArtboardCornerPoint,
	ArtboardCenterPoint,
}

impl fmt::Display for AlignmentSnapSource {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			AlignmentSnapSource::BoundingBoxCornerPoint => write!(f, "{}", BoundingBoxSnapSource::CornerPoint),
			AlignmentSnapSource::BoundingBoxCenterPoint => write!(f, "{}", BoundingBoxSnapSource::CenterPoint),
			AlignmentSnapSource::BoundingBoxEdgeMidpoint => write!(f, "{}", BoundingBoxSnapSource::EdgeMidpoint),
			AlignmentSnapSource::ArtboardCornerPoint => write!(f, "{}", ArtboardSnapSource::CornerPoint),
			AlignmentSnapSource::ArtboardCenterPoint => write!(f, "{}", ArtboardSnapSource::CenterPoint),
		}
	}
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapSource {
	#[default]
	None,
	BoundingBox(BoundingBoxSnapSource),
	Artboard(ArtboardSnapSource),
	Path(PathSnapSource),
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
			Self::Alignment(AlignmentSnapSource::ArtboardCenterPoint | AlignmentSnapSource::BoundingBoxCenterPoint)
				| Self::Artboard(ArtboardSnapSource::CenterPoint)
				| Self::BoundingBox(BoundingBoxSnapSource::CenterPoint)
		)
	}
}

impl fmt::Display for SnapSource {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			SnapSource::None => write!(f, "None"),
			SnapSource::BoundingBox(bounding_box_snap_source) => write!(f, "{bounding_box_snap_source}"),
			SnapSource::Artboard(artboard_snap_source) => write!(f, "{artboard_snap_source}"),
			SnapSource::Path(path_snap_source) => write!(f, "{path_snap_source}"),
			SnapSource::Alignment(alignment_snap_source) => write!(f, "{alignment_snap_source}"),
		}
	}
}

type GetSnapState = for<'a> fn(&'a mut SnappingState) -> &'a mut bool;
pub const SNAP_FUNCTIONS_FOR_BOUNDING_BOXES: [(&str, GetSnapState, &str); 5] = [
	(
		"Align with Edges",
		(|snapping_state| &mut snapping_state.bounding_box.align_with_edges) as GetSnapState,
		"Snaps to horizontal/vertical alignment with the edges of any layer's bounding box",
	),
	(
		"Corner Points",
		(|snapping_state| &mut snapping_state.bounding_box.corner_point) as GetSnapState,
		"Snaps to the four corners of any layer's bounding box",
	),
	(
		"Center Points",
		(|snapping_state| &mut snapping_state.bounding_box.center_point) as GetSnapState,
		"Snaps to the center point of any layer's bounding box",
	),
	(
		"Edge Midpoints",
		(|snapping_state| &mut snapping_state.bounding_box.edge_midpoint) as GetSnapState,
		"Snaps to any of the four points at the middle of the edges of any layer's bounding box",
	),
	(
		"Distribute Evenly",
		(|snapping_state| &mut snapping_state.bounding_box.distribute_evenly) as GetSnapState,
		"Snaps to a consistent distance offset established by the bounding boxes of nearby layers",
	),
];
pub const SNAP_FUNCTIONS_FOR_PATHS: [(&str, GetSnapState, &str); 7] = [
	(
		"Align with Anchor Points",
		(|snapping_state: &mut SnappingState| &mut snapping_state.path.align_with_anchor_point) as GetSnapState,
		"Snaps to horizontal/vertical alignment with the anchor points of any vector path",
	),
	(
		"Anchor Points",
		(|snapping_state: &mut SnappingState| &mut snapping_state.path.anchor_point) as GetSnapState,
		"Snaps to the anchor point of any vector path",
	),
	(
		// TODO: Extend to the midpoints of curved segments and rename to "Segment Midpoint"
		"Line Midpoints",
		(|snapping_state: &mut SnappingState| &mut snapping_state.path.line_midpoint) as GetSnapState,
		"Snaps to the point at the middle of any straight line segment of a vector path",
	),
	(
		"Path Intersection Points",
		(|snapping_state: &mut SnappingState| &mut snapping_state.path.path_intersection_point) as GetSnapState,
		"Snaps to any points where vector paths intersect",
	),
	(
		"Along Paths",
		(|snapping_state: &mut SnappingState| &mut snapping_state.path.along_path) as GetSnapState,
		"Snaps along the length of any vector path",
	),
	(
		// TODO: This works correctly for line segments, but not curved segments.
		// TODO: Therefore, we should make this use the normal in relation to the incoming curve, not the straight line between the incoming curve's start point and the path.
		"Normal to Paths",
		(|snapping_state: &mut SnappingState| &mut snapping_state.path.normal_to_path) as GetSnapState,
		// TODO: Fix the bug/limitation that requires 'Intersections of Paths' to be enabled
		"Snaps a line to a point perpendicular to a vector path\n(due to a bug, 'Intersections of Paths' must be enabled)",
	),
	(
		// TODO: This works correctly for line segments, but not curved segments.
		// TODO: Therefore, we should make this use the tangent in relation to the incoming curve, not the straight line between the incoming curve's start point and the path.
		"Tangent to Paths",
		(|snapping_state: &mut SnappingState| &mut snapping_state.path.tangent_to_path) as GetSnapState,
		// TODO: Fix the bug/limitation that requires 'Intersections of Paths' to be enabled
		"Snaps a line to a point tangent to a vector path\n(due to a bug, 'Intersections of Paths' must be enabled)",
	),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BoundingBoxSnapTarget {
	CornerPoint,
	CenterPoint,
	EdgeMidpoint,
}

impl fmt::Display for BoundingBoxSnapTarget {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			BoundingBoxSnapTarget::CornerPoint => write!(f, "Bounding Box: Corner Point"),
			BoundingBoxSnapTarget::CenterPoint => write!(f, "Bounding Box: Center Point"),
			BoundingBoxSnapTarget::EdgeMidpoint => write!(f, "Bounding Box: Edge Midpoint"),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PathSnapTarget {
	AnchorPointWithColinearHandles,
	AnchorPointWithFreeHandles,
	LineMidpoint,
	AlongPath,
	NormalToPath,
	TangentToPath,
	IntersectionPoint,
	PerpendicularToEndpoint,
}

impl fmt::Display for PathSnapTarget {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			PathSnapTarget::AnchorPointWithColinearHandles | PathSnapTarget::AnchorPointWithFreeHandles => write!(f, "Path: Anchor Point"),
			PathSnapTarget::LineMidpoint => write!(f, "Path: Line Midpoint"),
			PathSnapTarget::AlongPath => write!(f, "Path: Along Path"),
			PathSnapTarget::NormalToPath => write!(f, "Path: Normal to Path"),
			PathSnapTarget::TangentToPath => write!(f, "Path: Tangent to Path"),
			PathSnapTarget::IntersectionPoint => write!(f, "Path: Intersection Point"),
			PathSnapTarget::PerpendicularToEndpoint => write!(f, "Path: Perp. to Endpoint"),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtboardSnapTarget {
	CornerPoint,
	CenterPoint,
	AlongEdge,
}

impl fmt::Display for ArtboardSnapTarget {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			ArtboardSnapTarget::CornerPoint => write!(f, "Artboard: Corner Point"),
			ArtboardSnapTarget::CenterPoint => write!(f, "Artboard: Center Point"),
			ArtboardSnapTarget::AlongEdge => write!(f, "Artboard: Along Edge"),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridSnapTarget {
	Line,
	LineNormal,
	Intersection,
}

impl fmt::Display for GridSnapTarget {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			GridSnapTarget::Line => write!(f, "Grid: Along Line"),
			GridSnapTarget::LineNormal => write!(f, "Grid: Normal to Line"),
			GridSnapTarget::Intersection => write!(f, "Grid: Intersection Point"),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignmentSnapTarget {
	BoundingBoxCornerPoint,
	BoundingBoxCenterPoint,
	ArtboardCornerPoint,
	ArtboardCenterPoint,
	AlignWithAnchorPoint,
	IntersectionPoint,
	PerpendicularToEndpoint,
}

impl fmt::Display for AlignmentSnapTarget {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			AlignmentSnapTarget::BoundingBoxCornerPoint => write!(f, "{}", BoundingBoxSnapTarget::CornerPoint),
			AlignmentSnapTarget::BoundingBoxCenterPoint => write!(f, "{}", BoundingBoxSnapTarget::CenterPoint),
			AlignmentSnapTarget::ArtboardCornerPoint => write!(f, "{}", ArtboardSnapTarget::CornerPoint),
			AlignmentSnapTarget::ArtboardCenterPoint => write!(f, "{}", ArtboardSnapTarget::CenterPoint),
			AlignmentSnapTarget::AlignWithAnchorPoint => write!(f, "{}", PathSnapTarget::AnchorPointWithColinearHandles),
			AlignmentSnapTarget::IntersectionPoint => write!(f, "{}", PathSnapTarget::IntersectionPoint),
			AlignmentSnapTarget::PerpendicularToEndpoint => write!(f, "{}", PathSnapTarget::PerpendicularToEndpoint),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistributionSnapTarget {
	X,
	Y,
	Right,
	Left,
	Up,
	Down,
	XY,
}

impl fmt::Display for DistributionSnapTarget {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			DistributionSnapTarget::X => write!(f, "Distribute: X"),
			DistributionSnapTarget::Y => write!(f, "Distribute: Y"),
			DistributionSnapTarget::Right => write!(f, "Distribute: Right"),
			DistributionSnapTarget::Left => write!(f, "Distribute: Left"),
			DistributionSnapTarget::Up => write!(f, "Distribute: Up"),
			DistributionSnapTarget::Down => write!(f, "Distribute: Down"),
			DistributionSnapTarget::XY => write!(f, "Distribute: XY"),
		}
	}
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
	Path(PathSnapTarget),
	Artboard(ArtboardSnapTarget),
	Grid(GridSnapTarget),
	Alignment(AlignmentSnapTarget),
	DistributeEvenly(DistributionSnapTarget),
}

impl SnapTarget {
	pub fn is_some(&self) -> bool {
		self != &Self::None
	}
	pub fn bounding_box(&self) -> bool {
		matches!(self, Self::BoundingBox(_) | Self::Artboard(_))
	}
}

impl fmt::Display for SnapTarget {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			SnapTarget::None => write!(f, "None"),
			SnapTarget::BoundingBox(bounding_box_snap_target) => write!(f, "{bounding_box_snap_target}"),
			SnapTarget::Path(path_snap_target) => write!(f, "{path_snap_target}"),
			SnapTarget::Artboard(artboard_snap_target) => write!(f, "{artboard_snap_target}"),
			SnapTarget::Grid(grid_snap_target) => write!(f, "{grid_snap_target}"),
			SnapTarget::Alignment(alignment_snap_target) => write!(f, "{alignment_snap_target}"),
			SnapTarget::DistributeEvenly(distribution_snap_target) => write!(f, "{distribution_snap_target}"),
		}
	}
}

// TODO: implement icons for SnappingOptions eventually
pub enum SnappingOptions {
	BoundingBoxes,
	Paths,
}

impl fmt::Display for SnappingOptions {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			SnappingOptions::BoundingBoxes => write!(f, "Bounding Boxes"),
			SnappingOptions::Paths => write!(f, "Paths"),
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
	/// Flipped status.
	pub flip: bool,
}

impl Default for PTZ {
	fn default() -> Self {
		Self {
			pan: DVec2::ZERO,
			tilt: 0.,
			zoom: 1.,
			flip: false,
		}
	}
}

impl PTZ {
	/// Get the tilt angle between -180° and 180° in radians.
	pub fn tilt(&self) -> f64 {
		(((self.tilt + std::f64::consts::PI) % std::f64::consts::TAU) + std::f64::consts::TAU) % std::f64::consts::TAU - std::f64::consts::PI
	}

	pub fn unmodified_tilt(&self) -> f64 {
		self.tilt
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

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum GroupFolderType {
	Layer,
	BooleanOperation(graphene_std::path_bool::BooleanOperation),
}
