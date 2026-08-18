use crate::vector::misc::dvec2_to_point;
use glam::{DAffine2, DVec2};
use kurbo::{CubicBez, Line, PathSeg, QuadBez};
use std::fmt::{Debug, Formatter, Result};
use std::hash::Hash;

/// An id type used for each [ManipulatorGroup].
pub trait Identifier: Sized + Clone + PartialEq + Hash + graphene_hash::CacheHash + 'static {
	fn new() -> Self;
}

/// Structure used to represent a single anchor with up to two optional associated handles along a `Subpath`
#[derive(Copy, Clone, PartialEq, graphene_hash::CacheHash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ManipulatorGroup<PointId: Identifier> {
	pub anchor: DVec2,
	pub in_handle: Option<DVec2>,
	pub out_handle: Option<DVec2>,
	pub id: PointId,
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
}

/// Representation of the handle point(s) in a bezier segment.
#[derive(Copy, Clone, PartialEq, Debug, graphene_hash::CacheHash)]
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

impl BezierHandles {
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
