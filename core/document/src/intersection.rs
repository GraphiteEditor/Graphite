use glam::DVec2;
use kurbo::{BezPath, Line, PathSeg, Point, Shape, Vec2};

fn to_point(vec: DVec2) -> Point {
	Point::new(vec.x, vec.y)
}

pub fn intersect_quad_bez_path(quad: [DVec2; 4], shape: &BezPath, closed: bool) -> bool {
	let lines = vec![
		Line::new(to_point(quad[0]), to_point(quad[1])),
		Line::new(to_point(quad[1]), to_point(quad[2])),
		Line::new(to_point(quad[2]), to_point(quad[3])),
		Line::new(to_point(quad[3]), to_point(quad[0])),
	];
	// check if outlines intersect
	for path_segment in shape.segments() {
		for line in &lines {
			if !path_segment.intersect_line(*line).is_empty() {
				return true;
			}
		}
	}
	// check if selection is entirely within the shape
	if closed && quad.iter().any(|q| shape.contains(to_point(*q))) {
		return true;
	}
	// check if shape is entirely within the selection
	if let Some(shape_point) = get_arbitrary_point_on_path(shape) {
		let mut pos = 0;
		let mut neg = 0;
		for line in lines {
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
