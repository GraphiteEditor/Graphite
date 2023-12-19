use graphene_core::uuid::ManipulatorGroupId;

use glam::{DAffine2, DVec2};
use kurbo::{BezPath, Line, PathSeg, Point, Shape};
use std::ops::Mul;

#[derive(Debug, Clone, Default, Copy)]
/// A quad defined by four vertices.
pub struct Quad([DVec2; 4]);

impl Quad {
	/// Get all the edges in the quad.
	fn lines_glam(&self) -> impl Iterator<Item = bezier_rs::Bezier> + '_ {
		[[self.0[0], self.0[1]], [self.0[1], self.0[2]], [self.0[2], self.0[3]], [self.0[3], self.0[0]]]
			.into_iter()
			.map(|[start, end]| bezier_rs::Bezier::from_linear_dvec2(start, end))
	}

	/// Get all the edges in the quad.
	fn lines(&self) -> [Line; 4] {
		[
			Line::new(to_point(self.0[0]), to_point(self.0[1])),
			Line::new(to_point(self.0[1]), to_point(self.0[2])),
			Line::new(to_point(self.0[2]), to_point(self.0[3])),
			Line::new(to_point(self.0[3]), to_point(self.0[0])),
		]
	}

	/// Generate a [BezPath] of the quad
	fn path(&self) -> BezPath {
		let mut path = kurbo::BezPath::new();
		path.move_to(to_point(self.0[0]));
		path.line_to(to_point(self.0[1]));
		path.line_to(to_point(self.0[2]));
		path.line_to(to_point(self.0[3]));
		path.close_path();
		path
	}

	/// Gets the center of a quad
	fn center(&self) -> DVec2 {
		self.0.iter().sum::<DVec2>() / 4.
	}
}

impl Mul<Quad> for DAffine2 {
	type Output = Quad;

	fn mul(self, rhs: Quad) -> Self::Output {
		let mut output = Quad::default();
		for (i, point) in rhs.0.iter().enumerate() {
			output.0[i] = self.transform_point2(*point);
		}
		output
	}
}

fn to_point(vec: DVec2) -> Point {
	Point::new(vec.x, vec.y)
}

/// Return `true` if `quad` intersects `shape`.
/// This is the case if any of the following conditions are true:
/// - the edges of `quad` and `shape` intersect
/// - `shape` is entirely contained within `quad`
/// - `filled` is `true` and `quad` is entirely contained within `shape`.
pub fn intersect_quad_bez_path(quad: Quad, shape: &BezPath, filled: bool) -> bool {
	let mut shape = shape.clone();

	// For filled shapes act like shape was closed even if it isn't
	if filled && shape.elements().last() != Some(&kurbo::PathEl::ClosePath) {
		shape.close_path();
	}

	// Check if outlines intersect
	if shape.segments().any(|path_segment| quad.lines().iter().any(|line| !path_segment.intersect_line(*line).is_empty())) {
		return true;
	}
	// Check if selection is entirely within the shape
	if filled && shape.contains(to_point(quad.center())) {
		return true;
	}

	let shape_entirely_within_selection = shape
		.segments()
		.next()
		.map(|seg| match seg {
			PathSeg::Line(line) => line.p0,
			PathSeg::Quad(quad) => quad.p0,
			PathSeg::Cubic(cubic) => cubic.p0,
		})
		.map(|shape_point| quad.path().contains(shape_point))
		.unwrap_or_default();

	shape_entirely_within_selection
}

pub fn intersect_quad_subpath(quad: Quad, subpath: &bezier_rs::Subpath<ManipulatorGroupId>, close_subpath: bool) -> bool {
	let mut subpath = subpath.clone();

	// For close_subpath shapes act like shape was closed even if it isn't
	if close_subpath && !subpath.closed() {
		subpath.set_closed(true);
	}

	// Check if outlines intersect
	if subpath
		.iter()
		.any(|path_segment| quad.lines_glam().any(|line| !path_segment.intersections(&line, None, None).is_empty()))
	{
		return true;
	}
	// Check if selection is entirely within the shape
	if close_subpath && subpath.contains_point(quad.center()) {
		return true;
	}

	// Check if shape is entirely within selection
	subpath
		.manipulator_groups()
		.first()
		.map(|group| group.anchor)
		.map(|shape_point| quad.path().contains(to_point(shape_point)))
		.unwrap_or_default()
}
