use glam::DVec2;
use kurbo::Point;

use crate::LayerId;

use super::style;
use super::LayerData;

use std::fmt::Write;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Line {}

impl Line {
	pub fn new() -> Line {
		Line {}
	}
}

impl LayerData for Line {
	fn to_kurbo_path(&self, transform: glam::DAffine2, _style: style::PathStyle) -> kurbo::BezPath {
		fn new_point(a: DVec2) -> Point {
			Point::new(a.x, a.y)
		}
		let mut path = kurbo::BezPath::new();
		path.move_to(new_point(transform.translation));
		path.line_to(new_point(transform.transform_point2(DVec2::ONE)));
		path
	}

	fn render(&mut self, svg: &mut String, transform: glam::DAffine2, style: style::PathStyle) {
		let [x1, y1] = transform.translation.to_array();
		let [x2, y2] = transform.transform_point2(DVec2::ONE).to_array();

		let _ = write!(svg, r#"<line x1="{}" y1="{}" x2="{}" y2="{}"{} />"#, x1, y1, x2, y2, style.render(),);
	}

	fn intersects_quad(&self, quad: [DVec2; 4], path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>, style: style::PathStyle) {
	}

	fn intersects_point(&self, point: DVec2, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>, style: style::PathStyle) {
		// let [x1, y1] = transform.translation.to_array();
		// let [x2, y2] = transform.transform_point2(DVec2::ONE).to_array();
		// if point_line_segment_dist(point, self.shape.p0, self.shape.p1) < POINT_SELECTION_TOLERANCE {
		// 	intersections.push(path.clone());
		// }
	}
}
