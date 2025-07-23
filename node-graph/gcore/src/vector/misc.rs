use bezier_rs::BezierHandles;
use dyn_any::DynAny;
use glam::DVec2;
use kurbo::{CubicBez, Line, PathSeg, Point, QuadBez};

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
	Isometric = 1,
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

pub fn get_line_endpoints(line: Line) -> (DVec2, DVec2) {
	let po = line.p0;
	let p1 = line.p1;

	(point_to_dvec2(po), point_to_dvec2(p1))
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
