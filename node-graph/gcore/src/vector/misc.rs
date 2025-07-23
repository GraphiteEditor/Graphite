use bezier_rs::{BezierHandles, ManipulatorGroup, Subpath};
use dyn_any::DynAny;
use glam::DVec2;
use kurbo::{BezPath, CubicBez, Line, PathSeg, Point, QuadBez};

use super::PointId;

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
	Rectangular,
	Isometric,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash, DynAny, specta::Type, node_macro::ChoiceType)]
#[widget(Radio)]
pub enum ArcType {
	#[default]
	Open,
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
		bezier_rs::BezierHandles::Linear => {
			let p0 = dvec2_to_point(start);
			let p1 = dvec2_to_point(end);
			PathSeg::Line(Line::new(p0, p1))
		}
		bezier_rs::BezierHandles::Quadratic { handle } => {
			let p0 = dvec2_to_point(start);
			let p1 = dvec2_to_point(handle);
			let p2 = dvec2_to_point(end);
			PathSeg::Quad(QuadBez::new(p0, p1, p2))
		}
		bezier_rs::BezierHandles::Cubic { handle_start, handle_end } => {
			let p0 = dvec2_to_point(start);
			let p1 = dvec2_to_point(handle_start);
			let p2 = dvec2_to_point(handle_end);
			let p3 = dvec2_to_point(end);
			PathSeg::Cubic(CubicBez::new(p0, p1, p2, p3))
		}
	}
}

pub fn subpath_to_kurbo_bezpath(subpath: Subpath<PointId>) -> BezPath {
	let maniputor_groups = subpath.manipulator_groups();
	let closed = subpath.closed();
	bezpath_from_manipulator_groups(maniputor_groups, closed)
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
