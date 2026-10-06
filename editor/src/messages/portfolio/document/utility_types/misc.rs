use crate::consts::COLOR_OVERLAY_GRAY;
use crate::messages::portfolio::document::DocumentMessageHandler;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use glam::DVec2;
use graphene_std::vector::misc::BooleanOperation;
use std::fmt;

/// How far apart on screen, in pixels, major grid lines must be to be drawn. Minor lines may come half as close.
const GRID_MIN_SPACING_IN_VIEWPORT: f64 = 10.;

#[repr(transparent)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify), tsify(large_number_types_as_bigints))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct DocumentId(pub u64);

#[derive(PartialEq, Eq, Clone, Copy, Debug, serde::Serialize, serde::Deserialize, Hash)]
pub enum FlipAxis {
	X,
	Y,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(PartialEq, Eq, Clone, Copy, Debug, serde::Serialize, serde::Deserialize, Hash)]
pub enum AlignAxis {
	X,
	Y,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(PartialEq, Eq, Clone, Copy, Debug, serde::Serialize, serde::Deserialize, Hash)]
pub enum AlignAggregate {
	Min,
	Max,
	Center,
}

// #[derive(Default, PartialEq, Eq, Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
// pub enum DocumentMode {
// 	#[default]
// 	DesignMode,
// 	SelectMode,
// 	GuideMode,
// }

// impl fmt::Display for DocumentMode {
// 	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
// 		match self {
// 			DocumentMode::DesignMode => write!(f, "Design Mode"),
// 			DocumentMode::SelectMode => write!(f, "Select Mode"),
// 			DocumentMode::GuideMode => write!(f, "Guide Mode"),
// 		}
// 	}
// }

// impl DocumentMode {
// 	pub fn icon_name(&self) -> String {
// 		match self {
// 			DocumentMode::DesignMode => "ViewportDesignMode".to_string(),
// 			DocumentMode::SelectMode => "ViewportSelectMode".to_string(),
// 			DocumentMode::GuideMode => "ViewportGuideMode".to_string(),
// 		}
// 	}
// }

/// SnappingState determines the current individual snapping states
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SnappingState {
	pub snapping_enabled: bool,
	pub grid_snapping: bool,
	pub grid_visible: bool,
	pub artboards: bool,
	pub bounding_box: BoundingBoxSnapping,
	pub path: PathSnapping,
	pub grid: GridSnapping,
}

impl Default for SnappingState {
	fn default() -> Self {
		Self {
			snapping_enabled: true,
			grid_snapping: false,
			grid_visible: false,
			artboards: true,
			bounding_box: BoundingBoxSnapping::default(),
			path: PathSnapping::default(),
			grid: GridSnapping::default(),
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
	pub align_with_anchor_point: bool, // TODO: Rename
}

impl Default for PathSnapping {
	fn default() -> Self {
		Self {
			anchor_point: true,
			line_midpoint: true,
			along_path: true,
			normal_to_path: true,
			tangent_to_path: true,
			align_with_anchor_point: true,
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
	pub color: String,
	pub dot_display: bool,
	/// How many parts minor lines split each space between major lines of a rectangular grid into, where 1 draws none.
	pub subdivisions: u32,
	/// How far a rectangular grid is turned, in degrees.
	pub angle: f64,
	/// Whether `origin` is an offset from the top left corner of the active artboard, rather than a document position.
	pub origin_follows_artboard: bool,
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
			color: COLOR_OVERLAY_GRAY.to_string(),
			dot_display: false,
			subdivisions: 1,
			angle: 0.,
			origin_follows_artboard: false,
		}
	}
}

/// The spacings of a rectangular grid as drawn at a zoom level, which are also what it snaps to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridLevels {
	/// The unit direction the X spacing is measured along.
	pub x_axis: DVec2,
	/// The spacing of the major lines, coarsened as needed to keep them apart on screen.
	pub major: DVec2,
	/// The spacing of the minor lines, while they are far enough apart on screen to be drawn.
	pub minor: Option<DVec2>,
}

impl GridLevels {
	/// The spacing of the finest lines drawn.
	pub fn finest(&self) -> DVec2 {
		self.minor.unwrap_or(self.major)
	}

	/// The unit direction the Y spacing is measured along.
	pub fn y_axis(&self) -> DVec2 {
		self.x_axis.perp()
	}

	/// The two families of lines with this spacing, as the direction each runs along and the distance across between its lines.
	pub fn families(&self, spacing: DVec2) -> [(DVec2, f64); 2] {
		[(self.y_axis(), spacing.x), (self.x_axis, spacing.y)]
	}
}

impl GridSnapping {
	// Double grid size until it takes up at least 10px.
	pub fn compute_rectangle_spacing(mut size: DVec2, navigation: &PTZ) -> Option<DVec2> {
		if !size.is_finite() || size.cmple(DVec2::ZERO).any() {
			return None;
		}

		let mut iterations = 0;
		while (size * navigation.zoom()).cmplt(DVec2::splat(GRID_MIN_SPACING_IN_VIEWPORT)).any() {
			if iterations > 100 {
				return None;
			}
			size *= 2.;
			iterations += 1;
		}
		Some(size)
	}

	/// The spacings a rectangular grid is drawn at for a zoom level. Returns `None` for an isometric grid or options that make no usable grid.
	pub fn drawn_levels(&self, navigation: &PTZ) -> Option<GridLevels> {
		let GridType::Rectangular { spacing } = self.grid_type else { return None };
		if self.subdivisions == 0 || !self.angle.is_finite() {
			return None;
		}

		let major = Self::compute_rectangle_spacing(spacing, navigation)?;
		let minor = major / self.subdivisions as f64;

		// Minor lines may come twice as close on screen as the major lines do
		let minor_apart_on_screen = (minor * navigation.zoom()).cmpge(DVec2::splat(GRID_MIN_SPACING_IN_VIEWPORT / 2.)).all();

		Some(GridLevels {
			x_axis: DVec2::from_angle(self.angle.to_radians()),
			major,
			minor: (self.subdivisions > 1 && minor_apart_on_screen).then_some(minor),
		})
	}

	/// Where the grid's origin is in document space. While following, `origin` is an offset from the top left corner of the active artboard,
	/// which is the one containing the first selected layer, else the first artboard in the document.
	pub fn effective_origin(&self, document: &DocumentMessageHandler) -> DVec2 {
		if !self.origin_follows_artboard {
			return self.origin;
		}

		let (metadata, network_interface) = (document.metadata(), &document.network_interface);
		let is_artboard = |layer: &LayerNodeIdentifier| *layer != LayerNodeIdentifier::ROOT_PARENT && network_interface.is_artboard(&layer.to_node(), &[]);

		let first_selected = network_interface.selected_nodes().selected_layers(metadata).next();
		let artboard = first_selected
			.and_then(|layer| layer.ancestors(metadata).find(is_artboard))
			.or_else(|| LayerNodeIdentifier::ROOT_PARENT.children(metadata).find(is_artboard));

		let top_left = artboard.and_then(|artboard| metadata.bounding_box_document(artboard)).map(|[min, _]| min);
		top_left.filter(|corner| corner.is_finite()).map_or(self.origin, |corner| corner + self.origin)
	}

	// Double grid size until it takes up at least 10px.
	pub fn compute_isometric_multiplier(length: f64, divisor: f64, navigation: &PTZ) -> Option<f64> {
		let length = length.abs();
		let mut iterations = 0;
		let mut multiplier = 1.;
		while (length / divisor.abs().max(1.)) * multiplier * navigation.zoom() < GRID_MIN_SPACING_IN_VIEWPORT {
			if iterations > 100 {
				return None;
			}
			multiplier *= 2.;
			iterations += 1;
		}
		Some(multiplier)
	}
}

type GetSnapState = for<'a> fn(&'a mut SnappingState) -> &'a mut bool;
pub const SNAP_FUNCTIONS_FOR_BOUNDING_BOXES: [(&str, GetSnapState, &str); 5] = [
	(
		"Align with Edges",
		(|snapping_state| &mut snapping_state.bounding_box.align_with_edges) as GetSnapState,
		"Snaps to horizontal/vertical alignment with the edges of any layer's bounding box.",
	),
	(
		"Corner Points",
		(|snapping_state| &mut snapping_state.bounding_box.corner_point) as GetSnapState,
		"Snaps to the four corners of any layer's bounding box.",
	),
	(
		"Center Points",
		(|snapping_state| &mut snapping_state.bounding_box.center_point) as GetSnapState,
		"Snaps to the center point of any layer's bounding box.",
	),
	(
		"Edge Midpoints",
		(|snapping_state| &mut snapping_state.bounding_box.edge_midpoint) as GetSnapState,
		"Snaps to any of the four points at the middle of the edges of any layer's bounding box.",
	),
	(
		"Distribute Evenly",
		(|snapping_state| &mut snapping_state.bounding_box.distribute_evenly) as GetSnapState,
		"Snaps to a consistent distance offset established by the bounding boxes of nearby layers.",
	),
];
pub const SNAP_FUNCTIONS_FOR_PATHS: [(&str, GetSnapState, &str); 6] = [
	(
		"Align with Anchor Points",
		(|snapping_state: &mut SnappingState| &mut snapping_state.path.align_with_anchor_point) as GetSnapState,
		"Snaps to horizontal/vertical alignment with the anchor points of any vector path.",
	),
	(
		"Anchor Points",
		(|snapping_state: &mut SnappingState| &mut snapping_state.path.anchor_point) as GetSnapState,
		"Snaps to the anchor point of any vector path.",
	),
	(
		// TODO: Extend to the midpoints of curved segments and rename to "Segment Midpoint"
		"Line Midpoints",
		(|snapping_state: &mut SnappingState| &mut snapping_state.path.line_midpoint) as GetSnapState,
		"Snaps to the point at the middle of any straight line segment of a vector path.",
	),
	(
		"Along Paths",
		(|snapping_state: &mut SnappingState| &mut snapping_state.path.along_path) as GetSnapState,
		"Snaps along the length of any vector path.",
	),
	(
		"Normal to Paths",
		(|snapping_state: &mut SnappingState| &mut snapping_state.path.normal_to_path) as GetSnapState,
		"Snaps a line to a point perpendicular to a vector path.",
	),
	(
		"Tangent to Paths",
		(|snapping_state: &mut SnappingState| &mut snapping_state.path.tangent_to_path) as GetSnapState,
		"Snaps a line to a point tangent to a vector path.",
	),
];

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
	#[serde(deserialize_with = "deserialize_clamped_zoom")]
	zoom: f64,
	/// Flipped status.
	pub flip: bool,
}

/// A zoom outside the allowed range, like a non-finite one loaded as 0, would leave the view unusable.
fn deserialize_clamped_zoom<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
	let zoom = <f64 as serde::Deserialize>::deserialize(deserializer)?;
	Ok(zoom.clamp(crate::consts::VIEWPORT_ZOOM_SCALE_MIN, crate::consts::VIEWPORT_ZOOM_SCALE_MAX))
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
	BooleanOperation(BooleanOperation),
	Blend,
	Morph,
}
