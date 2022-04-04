// WIP

use kurbo::{BezPath, PathEl};

use crate::viewport_tools::vector_editor::vector_control_point::VectorControlPoint;

use super::vector_anchor::VectorAnchor;

struct ShapeRenderer {}

impl ShapeRenderer {
	pub fn new() -> Self {}

	/// Return a bezpath based on our internal representation points
	fn to_bezpath(&self, anchors: &[VectorAnchor]) -> BezPath {
		if anchors.is_empty() {
			return BezPath::new();
		}
		let point_to_kurbo = |x: &VectorControlPoint| kurbo::Point::new(x.position.x, x.position.y);
		let point = anchors[0].points[0].as_ref().unwrap().position;
		let mut bez_path = vec![PathEl::MoveTo((point.x, point.y).into())];

		for elements in anchors.windows(2) {
			let first = &elements[0];
			let second = &elements[1];
			let new_segment = match [&first.points[2], &second.points[1], &second.points[0]] {
				[None, None, Some(p)] => PathEl::LineTo(point_to_kurbo(p)),
				[None, Some(a), Some(p)] => PathEl::QuadTo(point_to_kurbo(a), point_to_kurbo(p)),
				[Some(a1), Some(a2), Some(p)] => PathEl::CurveTo(point_to_kurbo(a1), point_to_kurbo(a2), point_to_kurbo(p)),
				_ => panic!("unexpected path found"),
			};
			bez_path.push(new_segment);
		}
		if self.closed {
			bez_path.push(PathEl::ClosePath);
		}
		log::debug!("path: {:?}", bez_path);
		BezPath::from_vec(bez_path)
	}

	pub fn draw(&self, anchors: &[VectorAnchor]) {
		// If we are using kurbo, render bezpath
		let path = self.to_bezpath(anchors);
		// Submit the path to a renderer in this case kurbo?
	}
}
