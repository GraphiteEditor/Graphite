use crate::vector::algorithms::intersection::filtered_segment_intersections;
use crate::vector::misc::{dvec2_to_point, handles_to_segment};
use glam::{DAffine2, DVec2};
use kurbo::{CubicBez, Line, PathSeg, QuadBez, Shape};
use std::fmt::{Debug, Formatter, Result};
use std::hash::Hash;

/// An id type used for each [ManipulatorGroup].
pub trait Identifier: Sized + Clone + PartialEq + Hash + 'static {
	fn new() -> Self;
}

/// Structure used to represent a single anchor with up to two optional associated handles along a `Subpath`
#[derive(Copy, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ManipulatorGroup<PointId: Identifier> {
	pub anchor: DVec2,
	pub in_handle: Option<DVec2>,
	pub out_handle: Option<DVec2>,
	pub id: PointId,
}

// TODO: Remove once we no longer need to hash floats in Graphite
impl<PointId: Identifier> Hash for ManipulatorGroup<PointId> {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.anchor.to_array().iter().for_each(|x| x.to_bits().hash(state));
		self.in_handle.is_some().hash(state);
		if let Some(in_handle) = self.in_handle {
			in_handle.to_array().iter().for_each(|x| x.to_bits().hash(state));
		}
		self.out_handle.is_some().hash(state);
		if let Some(out_handle) = self.out_handle {
			out_handle.to_array().iter().for_each(|x| x.to_bits().hash(state));
		}
		self.id.hash(state);
	}
}

impl<PointId: Identifier> Debug for ManipulatorGroup<PointId> {
	fn fmt(&self, f: &mut Formatter<'_>) -> Result {
		f.debug_struct("ManipulatorGroup")
			.field("anchor", &self.anchor)
			.field("in_handle", &self.in_handle)
			.field("out_handle", &self.out_handle)
			.finish()
	}
}

impl<PointId: Identifier> ManipulatorGroup<PointId> {
	/// Construct a new manipulator group from an anchor, in handle and out handle
	pub fn new(anchor: DVec2, in_handle: Option<DVec2>, out_handle: Option<DVec2>) -> Self {
		let id = PointId::new();
		Self { anchor, in_handle, out_handle, id }
	}

	/// Construct a new manipulator point with just an anchor position
	pub fn new_anchor(anchor: DVec2) -> Self {
		Self::new(anchor, Some(anchor), Some(anchor))
	}

	pub fn new_anchor_linear(anchor: DVec2) -> Self {
		Self::new(anchor, None, None)
	}

	/// Construct a new manipulator group from an anchor, in handle, out handle and an id
	pub fn new_with_id(anchor: DVec2, in_handle: Option<DVec2>, out_handle: Option<DVec2>, id: PointId) -> Self {
		Self { anchor, in_handle, out_handle, id }
	}

	/// Construct a new manipulator point with just an anchor position and an id
	pub fn new_anchor_with_id(anchor: DVec2, id: PointId) -> Self {
		Self::new_with_id(anchor, Some(anchor), Some(anchor), id)
	}

	/// Create a bezier curve that starts at the current manipulator group and finishes in the `end_group` manipulator group.
	pub fn to_bezier(&self, end_group: &ManipulatorGroup<PointId>) -> PathSeg {
		let start = self.anchor;
		let end = end_group.anchor;
		let out_handle = self.out_handle;
		let in_handle = end_group.in_handle;

		match (out_handle, in_handle) {
			(Some(handle1), Some(handle2)) => PathSeg::Cubic(CubicBez::new(dvec2_to_point(start), dvec2_to_point(handle1), dvec2_to_point(handle2), dvec2_to_point(end))),
			(Some(handle), None) | (None, Some(handle)) => PathSeg::Quad(QuadBez::new(dvec2_to_point(start), dvec2_to_point(handle), dvec2_to_point(end))),
			(None, None) => PathSeg::Line(Line::new(dvec2_to_point(start), dvec2_to_point(end))),
		}
	}

	/// Apply a transformation to all of the [ManipulatorGroup] points
	pub fn apply_transform(&mut self, affine_transform: DAffine2) {
		self.anchor = affine_transform.transform_point2(self.anchor);
		self.in_handle = self.in_handle.map(|in_handle| affine_transform.transform_point2(in_handle));
		self.out_handle = self.out_handle.map(|out_handle| affine_transform.transform_point2(out_handle));
	}

	/// Are all handles at finite positions
	pub fn is_finite(&self) -> bool {
		self.anchor.is_finite() && self.in_handle.is_none_or(|handle| handle.is_finite()) && self.out_handle.is_none_or(|handle| handle.is_finite())
	}

	/// Reverse directions of handles
	pub fn flip(mut self) -> Self {
		std::mem::swap(&mut self.in_handle, &mut self.out_handle);
		self
	}

	pub fn has_in_handle(&self) -> bool {
		self.in_handle.map(|handle| Self::has_handle(self.anchor, handle)).unwrap_or(false)
	}

	pub fn has_out_handle(&self) -> bool {
		self.out_handle.map(|handle| Self::has_handle(self.anchor, handle)).unwrap_or(false)
	}

	fn has_handle(anchor: DVec2, handle: DVec2) -> bool {
		!((handle.x - anchor.x).abs() < f64::EPSILON && (handle.y - anchor.y).abs() < f64::EPSILON)
	}
}

#[derive(Copy, Clone)]
pub enum AppendType {
	IgnoreStart,
	SmoothJoin(f64),
}

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub enum ArcType {
	Open,
	Closed,
	PieSlice,
}

/// Representation of the handle point(s) in a bezier segment.
#[derive(Copy, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BezierHandles {
	Linear,
	/// Handles for a quadratic curve.
	Quadratic {
		/// Point representing the location of the single handle.
		handle: DVec2,
	},
	/// Handles for a cubic curve.
	Cubic {
		/// Point representing the location of the handle associated to the start point.
		handle_start: DVec2,
		/// Point representing the location of the handle associated to the end point.
		handle_end: DVec2,
	},
}

impl std::hash::Hash for BezierHandles {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		std::mem::discriminant(self).hash(state);
		match self {
			BezierHandles::Linear => {}
			BezierHandles::Quadratic { handle } => handle.to_array().map(|v| v.to_bits()).hash(state),
			BezierHandles::Cubic { handle_start, handle_end } => [handle_start, handle_end].map(|handle| handle.to_array().map(|v| v.to_bits())).hash(state),
		}
	}
}

impl BezierHandles {
	pub fn is_cubic(&self) -> bool {
		matches!(self, Self::Cubic { .. })
	}

	pub fn is_finite(&self) -> bool {
		match self {
			BezierHandles::Linear => true,
			BezierHandles::Quadratic { handle } => handle.is_finite(),
			BezierHandles::Cubic { handle_start, handle_end } => handle_start.is_finite() && handle_end.is_finite(),
		}
	}

	/// Get the coordinates of the bezier segment's first handle point. This represents the only handle in a quadratic segment.
	pub fn start(&self) -> Option<DVec2> {
		match *self {
			BezierHandles::Cubic { handle_start, .. } | BezierHandles::Quadratic { handle: handle_start } => Some(handle_start),
			_ => None,
		}
	}

	/// Get the coordinates of the second handle point. This will return `None` for a quadratic segment.
	pub fn end(&self) -> Option<DVec2> {
		match *self {
			BezierHandles::Cubic { handle_end, .. } => Some(handle_end),
			_ => None,
		}
	}

	pub fn move_start(&mut self, delta: DVec2) {
		if let BezierHandles::Cubic { handle_start, .. } | BezierHandles::Quadratic { handle: handle_start } = self {
			*handle_start += delta
		}
	}

	pub fn move_end(&mut self, delta: DVec2) {
		if let BezierHandles::Cubic { handle_end, .. } = self {
			*handle_end += delta
		}
	}

	/// Returns a Bezier curve that results from applying the transformation function to each handle point in the Bezier.
	#[must_use]
	pub fn apply_transformation(&self, transformation_function: impl Fn(DVec2) -> DVec2) -> Self {
		match *self {
			BezierHandles::Linear => Self::Linear,
			BezierHandles::Quadratic { handle } => {
				let handle = transformation_function(handle);
				Self::Quadratic { handle }
			}
			BezierHandles::Cubic { handle_start, handle_end } => {
				let handle_start = transformation_function(handle_start);
				let handle_end = transformation_function(handle_end);
				Self::Cubic { handle_start, handle_end }
			}
		}
	}

	#[must_use]
	pub fn reversed(self) -> Self {
		match self {
			BezierHandles::Cubic { handle_start, handle_end } => Self::Cubic {
				handle_start: handle_end,
				handle_end: handle_start,
			},
			_ => self,
		}
	}
}

/// Representation of a bezier curve with 2D points.
#[derive(Copy, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Bezier {
	/// Start point of the bezier curve.
	pub start: DVec2,
	/// End point of the bezier curve.
	pub end: DVec2,
	/// Handles of the bezier curve.
	pub handles: BezierHandles,
}

impl Debug for Bezier {
	fn fmt(&self, f: &mut Formatter<'_>) -> Result {
		let mut debug_struct = f.debug_struct("Bezier");
		let mut debug_struct_ref = debug_struct.field("start", &self.start);
		debug_struct_ref = match self.handles {
			BezierHandles::Linear => debug_struct_ref,
			BezierHandles::Quadratic { handle } => debug_struct_ref.field("handle", &handle),
			BezierHandles::Cubic { handle_start, handle_end } => debug_struct_ref.field("handle_start", &handle_start).field("handle_end", &handle_end),
		};
		debug_struct_ref.field("end", &self.end).finish()
	}
}

/// Functionality for the getters and setters of the various points in a Bezier
impl Bezier {
	/// Set the coordinates of the start point.
	pub fn set_start(&mut self, s: DVec2) {
		self.start = s;
	}

	/// Set the coordinates of the end point.
	pub fn set_end(&mut self, e: DVec2) {
		self.end = e;
	}

	/// Set the coordinates of the first handle point. This represents the only handle in a quadratic segment. If used on a linear segment, it will be changed to a quadratic.
	pub fn set_handle_start(&mut self, h1: DVec2) {
		match self.handles {
			BezierHandles::Linear => {
				self.handles = BezierHandles::Quadratic { handle: h1 };
			}
			BezierHandles::Quadratic { ref mut handle } => {
				*handle = h1;
			}
			BezierHandles::Cubic { ref mut handle_start, .. } => {
				*handle_start = h1;
			}
		};
	}

	/// Set the coordinates of the second handle point. This will convert both linear and quadratic segments into cubic ones. For a linear segment, the first handle will be set to the start point.
	pub fn set_handle_end(&mut self, h2: DVec2) {
		match self.handles {
			BezierHandles::Linear => {
				self.handles = BezierHandles::Cubic {
					handle_start: self.start,
					handle_end: h2,
				};
			}
			BezierHandles::Quadratic { handle } => {
				self.handles = BezierHandles::Cubic { handle_start: handle, handle_end: h2 };
			}
			BezierHandles::Cubic { ref mut handle_end, .. } => {
				*handle_end = h2;
			}
		};
	}

	/// Get the coordinates of the bezier segment's start point.
	pub fn start(&self) -> DVec2 {
		self.start
	}

	/// Get the coordinates of the bezier segment's end point.
	pub fn end(&self) -> DVec2 {
		self.end
	}

	/// Get the coordinates of the bezier segment's first handle point. This represents the only handle in a quadratic segment.
	pub fn handle_start(&self) -> Option<DVec2> {
		self.handles.start()
	}

	/// Get the coordinates of the second handle point. This will return `None` for a quadratic segment.
	pub fn handle_end(&self) -> Option<DVec2> {
		self.handles.end()
	}

	/// Get an iterator over the coordinates of all points in a vector.
	/// - For a linear segment, the order of the points will be: `start`, `end`.
	/// - For a quadratic segment, the order of the points will be: `start`, `handle`, `end`.
	/// - For a cubic segment, the order of the points will be: `start`, `handle_start`, `handle_end`, `end`.
	pub fn get_points(&self) -> impl Iterator<Item = DVec2> + use<> {
		match self.handles {
			BezierHandles::Linear => [self.start, self.end, DVec2::ZERO, DVec2::ZERO].into_iter().take(2),
			BezierHandles::Quadratic { handle } => [self.start, handle, self.end, DVec2::ZERO].into_iter().take(3),
			BezierHandles::Cubic { handle_start, handle_end } => [self.start, handle_start, handle_end, self.end].into_iter().take(4),
		}
	}

	// TODO: Consider removing this function
	/// Create a linear bezier using the provided coordinates as the start and end points.
	pub fn from_linear_coordinates(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
		Bezier {
			start: DVec2::new(x1, y1),
			handles: BezierHandles::Linear,
			end: DVec2::new(x2, y2),
		}
	}

	/// Create a linear bezier using the provided DVec2s as the start and end points.
	pub fn from_linear_dvec2(p1: DVec2, p2: DVec2) -> Self {
		Bezier {
			start: p1,
			handles: BezierHandles::Linear,
			end: p2,
		}
	}

	// TODO: Consider removing this function
	/// Create a quadratic bezier using the provided coordinates as the start, handle, and end points.
	pub fn from_quadratic_coordinates(x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64) -> Self {
		Bezier {
			start: DVec2::new(x1, y1),
			handles: BezierHandles::Quadratic { handle: DVec2::new(x2, y2) },
			end: DVec2::new(x3, y3),
		}
	}

	/// Create a quadratic bezier using the provided DVec2s as the start, handle, and end points.
	pub fn from_quadratic_dvec2(p1: DVec2, p2: DVec2, p3: DVec2) -> Self {
		Bezier {
			start: p1,
			handles: BezierHandles::Quadratic { handle: p2 },
			end: p3,
		}
	}

	// TODO: Consider removing this function
	/// Create a cubic bezier using the provided coordinates as the start, handles, and end points.
	#[allow(clippy::too_many_arguments)]
	pub fn from_cubic_coordinates(x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64, x4: f64, y4: f64) -> Self {
		Bezier {
			start: DVec2::new(x1, y1),
			handles: BezierHandles::Cubic {
				handle_start: DVec2::new(x2, y2),
				handle_end: DVec2::new(x3, y3),
			},
			end: DVec2::new(x4, y4),
		}
	}

	/// Create a cubic bezier using the provided DVec2s as the start, handles, and end points.
	pub fn from_cubic_dvec2(p1: DVec2, p2: DVec2, p3: DVec2, p4: DVec2) -> Self {
		Bezier {
			start: p1,
			handles: BezierHandles::Cubic { handle_start: p2, handle_end: p3 },
			end: p4,
		}
	}

	/// Returns a Bezier curve that results from applying the transformation function to each point in the Bezier.
	pub fn apply_transformation(&self, transformation_function: impl Fn(DVec2) -> DVec2) -> Bezier {
		Self {
			start: transformation_function(self.start),
			end: transformation_function(self.end),
			handles: self.handles.apply_transformation(transformation_function),
		}
	}

	pub fn intersections(&self, other: &Bezier, accuracy: Option<f64>, minimum_separation: Option<f64>) -> Vec<f64> {
		let this = handles_to_segment(self.start, self.handles, self.end);
		let other = handles_to_segment(other.start, other.handles, other.end);
		filtered_segment_intersections(this, other, accuracy, minimum_separation)
	}

	pub fn winding(&self, point: DVec2) -> i32 {
		let this = handles_to_segment(self.start, self.handles, self.end);
		this.winding(dvec2_to_point(point))
	}
}
