use super::PointId;
use super::algorithms::offset_subpath::MAX_ABSOLUTE_DIFFERENCE;
use crate::subpath::{BezierHandles, ManipulatorGroup};
use crate::vector::{SegmentId, Vector};
use dyn_any::DynAny;
use glam::DVec2;
use kurbo::{BezPath, CubicBez, Line, ParamCurve, PathSeg, Point, QuadBez};
use std::ops::Sub;

/// Represents different ways of calculating the centroid.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash, DynAny, specta::Type, node_macro::ChoiceType)]
#[widget(Radio)]
pub enum CentroidType {
	/// The center of mass for the area of a solid shape's interior, as if made out of an infinitely flat material.
	#[default]
	Area,
	/// The center of mass for the arc length of a curved shape's perimeter, as if made out of an infinitely thin wire.
	Length,
}

pub trait AsU64 {
	fn as_u64(&self) -> u64;
}
impl AsU64 for u32 {
	fn as_u64(&self) -> u64 {
		*self as u64
	}
}
impl AsU64 for u64 {
	fn as_u64(&self) -> u64 {
		*self
	}
}
impl AsU64 for f64 {
	fn as_u64(&self) -> u64 {
		*self as u64
	}
}

pub trait AsI64 {
	fn as_i64(&self) -> i64;
}
impl AsI64 for u32 {
	fn as_i64(&self) -> i64 {
		*self as i64
	}
}
impl AsI64 for u64 {
	fn as_i64(&self) -> i64 {
		*self as i64
	}
}
impl AsI64 for f64 {
	fn as_i64(&self) -> i64 {
		*self as i64
	}
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash, DynAny, specta::Type, node_macro::ChoiceType)]
#[widget(Radio)]
pub enum GridType {
	#[default]
	Rectangular = 0,
	Isometric,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash, DynAny, specta::Type, node_macro::ChoiceType)]
#[widget(Radio)]
pub enum ArcType {
	#[default]
	Open = 0,
	Closed,
	PieSlice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash, DynAny, specta::Type, node_macro::ChoiceType)]
#[widget(Radio)]
pub enum MergeByDistanceAlgorithm {
	#[default]
	Spatial,
	Topological,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash, DynAny, specta::Type, node_macro::ChoiceType)]
#[widget(Radio)]
pub enum PointSpacingType {
	#[default]
	/// The desired spacing distance between points.
	Separation,
	/// The exact number of points to span the path.
	Quantity,
}

pub fn point_to_dvec2(point: Point) -> DVec2 {
	DVec2 { x: point.x, y: point.y }
}

pub fn dvec2_to_point(value: DVec2) -> Point {
	Point { x: value.x, y: value.y }
}

pub fn get_line_endpoints(line: Line) -> (DVec2, DVec2) {
	(point_to_dvec2(line.p0), point_to_dvec2(line.p1))
}

pub fn segment_to_handles(segment: &PathSeg) -> BezierHandles {
	match *segment {
		PathSeg::Line(_) => BezierHandles::Linear,
		PathSeg::Quad(QuadBez { p0: _, p1, p2: _ }) => BezierHandles::Quadratic { handle: point_to_dvec2(p1) },
		PathSeg::Cubic(CubicBez { p0: _, p1, p2, p3: _ }) => BezierHandles::Cubic {
			handle_start: point_to_dvec2(p1),
			handle_end: point_to_dvec2(p2),
		},
	}
}

pub fn handles_to_segment(start: DVec2, handles: BezierHandles, end: DVec2) -> PathSeg {
	match handles {
		BezierHandles::Linear => {
			let p0 = dvec2_to_point(start);
			let p1 = dvec2_to_point(end);
			PathSeg::Line(Line::new(p0, p1))
		}
		BezierHandles::Quadratic { handle } => {
			let p0 = dvec2_to_point(start);
			let p1 = dvec2_to_point(handle);
			let p2 = dvec2_to_point(end);
			PathSeg::Quad(QuadBez::new(p0, p1, p2))
		}
		BezierHandles::Cubic { handle_start, handle_end } => {
			let p0 = dvec2_to_point(start);
			let p1 = dvec2_to_point(handle_start);
			let p2 = dvec2_to_point(handle_end);
			let p3 = dvec2_to_point(end);
			PathSeg::Cubic(CubicBez::new(p0, p1, p2, p3))
		}
	}
}

pub fn bezpath_from_manipulator_groups(manipulator_groups: &[ManipulatorGroup<PointId>], closed: bool) -> BezPath {
	let mut bezpath = kurbo::BezPath::new();
	let mut out_handle;

	let Some(first) = manipulator_groups.first() else { return bezpath };
	bezpath.move_to(dvec2_to_point(first.anchor));
	out_handle = first.out_handle;

	for manipulator in manipulator_groups.iter().skip(1) {
		match (out_handle, manipulator.in_handle) {
			(Some(handle_start), Some(handle_end)) => bezpath.curve_to(dvec2_to_point(handle_start), dvec2_to_point(handle_end), dvec2_to_point(manipulator.anchor)),
			(None, None) => bezpath.line_to(dvec2_to_point(manipulator.anchor)),
			(None, Some(handle)) => bezpath.quad_to(dvec2_to_point(handle), dvec2_to_point(manipulator.anchor)),
			(Some(handle), None) => bezpath.quad_to(dvec2_to_point(handle), dvec2_to_point(manipulator.anchor)),
		}
		out_handle = manipulator.out_handle;
	}

	if closed {
		match (out_handle, first.in_handle) {
			(Some(handle_start), Some(handle_end)) => bezpath.curve_to(dvec2_to_point(handle_start), dvec2_to_point(handle_end), dvec2_to_point(first.anchor)),
			(None, None) => bezpath.line_to(dvec2_to_point(first.anchor)),
			(None, Some(handle)) => bezpath.quad_to(dvec2_to_point(handle), dvec2_to_point(first.anchor)),
			(Some(handle), None) => bezpath.quad_to(dvec2_to_point(handle), dvec2_to_point(first.anchor)),
		}
		bezpath.close_path();
	}
	bezpath
}

pub fn bezpath_to_manipulator_groups(bezpath: &BezPath) -> (Vec<ManipulatorGroup<PointId>>, bool) {
	let mut manipulator_groups = Vec::<ManipulatorGroup<PointId>>::new();
	let mut is_closed = false;

	for element in bezpath.elements() {
		let manipulator_group = match *element {
			kurbo::PathEl::MoveTo(point) => ManipulatorGroup::new(point_to_dvec2(point), None, None),
			kurbo::PathEl::LineTo(point) => ManipulatorGroup::new(point_to_dvec2(point), None, None),
			kurbo::PathEl::QuadTo(point, point1) => ManipulatorGroup::new(point_to_dvec2(point1), Some(point_to_dvec2(point)), None),
			kurbo::PathEl::CurveTo(point, point1, point2) => {
				if let Some(last_manipulator_group) = manipulator_groups.last_mut() {
					last_manipulator_group.out_handle = Some(point_to_dvec2(point));
				}
				ManipulatorGroup::new(point_to_dvec2(point2), Some(point_to_dvec2(point1)), None)
			}
			kurbo::PathEl::ClosePath => {
				if let Some(last_manipulators) = manipulator_groups.pop()
					&& let Some(first_manipulators) = manipulator_groups.first_mut()
				{
					first_manipulators.out_handle = last_manipulators.in_handle;
				}
				is_closed = true;
				break;
			}
		};

		manipulator_groups.push(manipulator_group);
	}

	(manipulator_groups, is_closed)
}

/// Returns true if the [`PathSeg`] is equivalent to a line.
///
/// This is different from simply checking if the segment is [`PathSeg::Line`] or [`PathSeg::Quad`] or [`PathSeg::Cubic`]. Bezier curve can also be a line if the control points are colinear to the start and end points. Therefore if the handles exceed the start and end point, it will still be considered as a line.
pub fn is_linear(segment: PathSeg) -> bool {
	let is_colinear = |a: Point, b: Point, c: Point| -> bool { ((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)).abs() < MAX_ABSOLUTE_DIFFERENCE };

	match segment {
		PathSeg::Line(_) => true,
		PathSeg::Quad(QuadBez { p0, p1, p2 }) => is_colinear(p0, p1, p2),
		PathSeg::Cubic(CubicBez { p0, p1, p2, p3 }) => is_colinear(p0, p1, p3) && is_colinear(p0, p2, p3),
	}
}

/// Get an vec of all the points in a path segment.
pub fn pathseg_points_vec(segment: PathSeg) -> Vec<Point> {
	match segment {
		PathSeg::Line(line) => [line.p0, line.p1].to_vec(),
		PathSeg::Quad(quad_bez) => [quad_bez.p0, quad_bez.p1, quad_bez.p2].to_vec(),
		PathSeg::Cubic(cubic_bez) => [cubic_bez.p0, cubic_bez.p1, cubic_bez.p2, cubic_bez.p3].to_vec(),
	}
}

/// Returns true if the corresponding points of the two [`PathSeg`]s are within the provided absolute value difference from each other.
pub fn pathseg_abs_diff_eq(seg1: PathSeg, seg2: PathSeg, max_abs_diff: f64) -> bool {
	let seg1 = if is_linear(seg1) { PathSeg::Line(Line::new(seg1.start(), seg1.end())) } else { seg1 };
	let seg2 = if is_linear(seg2) { PathSeg::Line(Line::new(seg2.start(), seg2.end())) } else { seg2 };

	let seg1_points = pathseg_points_vec(seg1);
	let seg2_points = pathseg_points_vec(seg2);

	let cmp = |a: f64, b: f64| a.sub(b).abs() < max_abs_diff;

	seg1_points.len() == seg2_points.len() && seg1_points.into_iter().zip(seg2_points).all(|(a, b)| cmp(a.x, b.x) && cmp(a.y, b.y))
}

/// A selectable part of a curve, either an anchor (start or end of a bézier) or a handle (doesn't necessarily go through the bézier but influences curvature).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, DynAny, serde::Serialize, serde::Deserialize)]
pub enum ManipulatorPointId {
	/// A control anchor - the start or end point of a bézier.
	Anchor(PointId),
	/// The handle for a bézier - the first handle on a cubic and the only handle on a quadratic.
	PrimaryHandle(SegmentId),
	/// The end handle on a cubic bézier.
	EndHandle(SegmentId),
}

impl ManipulatorPointId {
	/// Attempt to retrieve the manipulator position in layer space (no transformation applied).
	#[must_use]
	#[track_caller]
	pub fn get_position(&self, vector: &Vector) -> Option<DVec2> {
		match self {
			ManipulatorPointId::Anchor(id) => vector.point_domain.position_from_id(*id),
			ManipulatorPointId::PrimaryHandle(id) => vector.segment_from_id(*id).and_then(|bezier| bezier.handle_start()),
			ManipulatorPointId::EndHandle(id) => vector.segment_from_id(*id).and_then(|bezier| bezier.handle_end()),
		}
	}

	pub fn get_anchor_position(&self, vector: &Vector) -> Option<DVec2> {
		match self {
			ManipulatorPointId::EndHandle(_) | ManipulatorPointId::PrimaryHandle(_) => self.get_anchor(vector).and_then(|id| vector.point_domain.position_from_id(id)),
			_ => self.get_position(vector),
		}
	}

	/// Attempt to get a pair of handles. For an anchor this is the first two handles connected. For a handle it is self and the first opposing handle.
	#[must_use]
	pub fn get_handle_pair(self, vector: &Vector) -> Option<[HandleId; 2]> {
		match self {
			ManipulatorPointId::Anchor(point) => vector.all_connected(point).take(2).collect::<Vec<_>>().try_into().ok(),
			ManipulatorPointId::PrimaryHandle(segment) => {
				let point = vector.segment_domain.segment_start_from_id(segment)?;
				let current = HandleId::primary(segment);
				let other = vector.segment_domain.all_connected(point).find(|&value| value != current);
				other.map(|other| [current, other])
			}
			ManipulatorPointId::EndHandle(segment) => {
				let point = vector.segment_domain.segment_end_from_id(segment)?;
				let current = HandleId::end(segment);
				let other = vector.segment_domain.all_connected(point).find(|&value| value != current);
				other.map(|other| [current, other])
			}
		}
	}

	/// Finds all the connected handles of a point.
	/// For an anchor it is all the connected handles.
	/// For a handle it is all the handles connected to its corresponding anchor other than the current handle.
	pub fn get_all_connected_handles(self, vector: &Vector) -> Option<Vec<HandleId>> {
		match self {
			ManipulatorPointId::Anchor(point) => {
				let connected = vector.all_connected(point).collect::<Vec<_>>();
				Some(connected)
			}
			ManipulatorPointId::PrimaryHandle(segment) => {
				let point = vector.segment_domain.segment_start_from_id(segment)?;
				let current = HandleId::primary(segment);
				let connected = vector.segment_domain.all_connected(point).filter(|&value| value != current).collect::<Vec<_>>();
				Some(connected)
			}
			ManipulatorPointId::EndHandle(segment) => {
				let point = vector.segment_domain.segment_end_from_id(segment)?;
				let current = HandleId::end(segment);
				let connected = vector.segment_domain.all_connected(point).filter(|&value| value != current).collect::<Vec<_>>();
				Some(connected)
			}
		}
	}

	/// Attempt to find the closest anchor. If self is already an anchor then it is just self. If it is a start or end handle, then the start or end point is chosen.
	#[must_use]
	pub fn get_anchor(self, vector: &Vector) -> Option<PointId> {
		match self {
			ManipulatorPointId::Anchor(point) => Some(point),
			ManipulatorPointId::PrimaryHandle(segment) => vector.segment_start_from_id(segment),
			ManipulatorPointId::EndHandle(segment) => vector.segment_end_from_id(segment),
		}
	}

	/// Attempt to convert self to a [`HandleId`], returning none for an anchor.
	#[must_use]
	pub fn as_handle(self) -> Option<HandleId> {
		match self {
			ManipulatorPointId::PrimaryHandle(segment) => Some(HandleId::primary(segment)),
			ManipulatorPointId::EndHandle(segment) => Some(HandleId::end(segment)),
			ManipulatorPointId::Anchor(_) => None,
		}
	}

	/// Attempt to convert self to an anchor, returning None for a handle.
	#[must_use]
	pub fn as_anchor(self) -> Option<PointId> {
		match self {
			ManipulatorPointId::Anchor(point) => Some(point),
			_ => None,
		}
	}

	pub fn get_segment(self) -> Option<SegmentId> {
		match self {
			ManipulatorPointId::PrimaryHandle(segment) | ManipulatorPointId::EndHandle(segment) => Some(segment),
			_ => None,
		}
	}
}

/// The type of handle found on a bézier curve.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, DynAny, serde::Serialize, serde::Deserialize)]
pub enum HandleType {
	/// The first handle on a cubic bézier or the only handle on a quadratic bézier.
	Primary,
	/// The second handle on a cubic bézier.
	End,
}

/// Represents a primary or end handle found in a particular segment.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, DynAny, serde::Serialize, serde::Deserialize)]
pub struct HandleId {
	pub ty: HandleType,
	pub segment: SegmentId,
}

impl std::fmt::Display for HandleId {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self.ty {
			// I haven't checked if "out" and "in" are reversed, or are accurate translations of the "primary" and "end" terms used in the `HandleType` enum, so this naming is an assumption.
			HandleType::Primary => write!(f, "{} out", self.segment.inner()),
			HandleType::End => write!(f, "{} in", self.segment.inner()),
		}
	}
}

impl HandleId {
	/// Construct a handle for the first handle on a cubic bézier or the only handle on a quadratic bézier.
	#[must_use]
	pub const fn primary(segment: SegmentId) -> Self {
		Self { ty: HandleType::Primary, segment }
	}

	/// Construct a handle for the end handle on a cubic bézier.
	#[must_use]
	pub const fn end(segment: SegmentId) -> Self {
		Self { ty: HandleType::End, segment }
	}

	/// Convert to [`ManipulatorPointId`].
	#[must_use]
	pub fn to_manipulator_point(self) -> ManipulatorPointId {
		match self.ty {
			HandleType::Primary => ManipulatorPointId::PrimaryHandle(self.segment),
			HandleType::End => ManipulatorPointId::EndHandle(self.segment),
		}
	}

	/// Calculate the magnitude of the handle from the anchor.
	pub fn length(self, vector: &Vector) -> f64 {
		let Some(anchor_position) = self.to_manipulator_point().get_anchor_position(vector) else {
			// TODO: This was previously an unwrap which was encountered, so this is a temporary way to avoid a crash
			return 0.;
		};
		let handle_position = self.to_manipulator_point().get_position(vector);
		handle_position.map(|pos| (pos - anchor_position).length()).unwrap_or(f64::MAX)
	}

	/// Convert an end handle to the primary handle and a primary handle to an end handle. Note that the new handle may not exist (e.g. for a quadratic bézier).
	#[must_use]
	pub fn opposite(self) -> Self {
		match self.ty {
			HandleType::Primary => Self::end(self.segment),
			HandleType::End => Self::primary(self.segment),
		}
	}
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash, DynAny, specta::Type, node_macro::ChoiceType)]
#[widget(Dropdown)]
pub enum SpiralType {
	#[default]
	Archimedean,
	Logarithmic,
}
