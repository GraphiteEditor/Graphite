use glam::{DAffine2, DVec2};
use kurbo::{BezPath, Line, PathSeg, Point, Shape};
use std::ops::Mul;

#[derive(Debug, Clone, Default, Copy)]
/// A quadriliteral defined by four vertices.
pub struct Quad([DVec2; 4]);

impl Quad {
    /// Convert a box defined by two corner points to a quad.
	pub fn from_box(bbox: [DVec2; 2]) -> Self {
		let size = bbox[1] - bbox[0];
		Self([bbox[0], bbox[0] + size * DVec2::X, bbox[1], bbox[0] + size * DVec2::Y])
	}

    /// Get all the edges in the quad.
	pub fn lines(&self) -> [Line; 4] {
		[
			Line::new(to_point(self.0[0]), to_point(self.0[1])),
			Line::new(to_point(self.0[1]), to_point(self.0[2])),
			Line::new(to_point(self.0[2]), to_point(self.0[3])),
			Line::new(to_point(self.0[3]), to_point(self.0[0])),
		]
	}

    /// Compute a Bezier Path along every point in the Quad
	pub fn path(&self) -> BezPath {
		let mut path = kurbo::BezPath::new();
		path.move_to(to_point(self.0[0]));
		path.line_to(to_point(self.0[1]));
		path.line_to(to_point(self.0[2]));
		path.line_to(to_point(self.0[3]));
		path.close_path();
		path
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
/// * the edges of `quad` and `shape` intersect
/// * `shape` is entirely contained within `quad`
/// * `filled` is `true` and `quad` is entirely contained within `shape`.
pub fn intersect_quad_bez_path(quad: Quad, shape: &BezPath, filled: bool) -> bool {
	let mut shape = shape.clone();
	// for filled shapes act like shape was closed even if it isn't
	if filled && shape.elements().last() != Some(&kurbo::PathEl::ClosePath) {
		shape.close_path();
	}

	// check if outlines intersect
	if shape.segments().any(|path_segment| quad.lines().iter().any(|line| !path_segment.intersect_line(*line).is_empty())) {
		return true;
	}
	// check if selection is entirely within the shape
	if filled && shape.contains(to_point(quad.0[0])) {
		return true;
	}

	// check if shape is entirely within selection
    get_arbitrary_point_on_path(&shape).map(|shape_point| quad.path().contains(shape_point)).unwrap_or_default()
}

/// Returns any point on `path`.
pub fn get_arbitrary_point_on_path(path: &BezPath) -> Option<Point> {
	path.segments().next().map(|seg| match seg {
		PathSeg::Line(line) => line.p0,
		PathSeg::Quad(quad) => quad.p0,
		PathSeg::Cubic(cubic) => cubic.p0,
	})
}
