use std::ops::Mul;

use glam::{DAffine2, DVec2};
use kurbo::{BezPath, Line, PathSeg, Point, Shape, Vec2};

#[derive(Debug, Clone, Default, Copy)]
pub struct Quad([DVec2; 4]);

impl Quad {
	pub fn from_box(bbox: [DVec2; 2]) -> Self {
		let size = bbox[1] - bbox[0];
		Self([bbox[0], bbox[0] + size * DVec2::X, bbox[0] + size * DVec2::Y, bbox[1]])
	}
	pub fn lines(&self) -> [Line; 4] {
		[
			Line::new(to_point(self.0[0]), to_point(self.0[1])),
			Line::new(to_point(self.0[1]), to_point(self.0[2])),
			Line::new(to_point(self.0[2]), to_point(self.0[3])),
			Line::new(to_point(self.0[3]), to_point(self.0[0])),
		]
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

pub fn intersect_quad_bez_path(quad: Quad, shape: &BezPath, closed: bool) -> bool {
	// check if outlines intersect
	for path_segment in shape.segments() {
		for line in quad.lines() {
			if !path_segment.intersect_line(line).is_empty() {
				return true;
			}
		}
	}
	// check if selection is entirely within the shape
	if closed && quad.0.iter().any(|q| shape.contains(to_point(*q))) {
		return true;
	}
	// check if shape is entirely within the selection
	if let Some(shape_point) = get_arbitrary_point_on_path(shape) {
		let mut pos = 0;
		let mut neg = 0;
		for line in quad.lines() {
			if line.p0 == shape_point {
				return true;
			};
			let line_vec = Vec2::new(line.p1.x - line.p0.x, line.p1.y - line.p0.y);
			let point_vec = Vec2::new(line.p1.x - shape_point.x, line.p1.y - shape_point.y);
			let cross = line_vec.cross(point_vec);
			if cross > 0.0 {
				pos += 1;
			} else if cross < 0.0 {
				neg += 1;
			}
			if pos > 0 && neg > 0 {
				return false;
			}
		}
	}
	true
}

pub fn get_arbitrary_point_on_path(path: &BezPath) -> Option<Point> {
	path.segments().next().map(|seg| match seg {
		PathSeg::Line(line) => line.p0,
		PathSeg::Quad(quad) => quad.p0,
		PathSeg::Cubic(cubic) => cubic.p0,
	})
}
