use glam::DVec2;
use kurbo::Point;

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
	fn to_kurbo_path(&mut self, transform: glam::DAffine2, _style: style::PathStyle) -> kurbo::BezPath {
		fn new_point(a: DVec2) -> Point {
			Point::new(a.x, a.y)
		}
		let mut path = kurbo::BezPath::new();
		path.move_to(new_point(transform.transform_point2(glam::const_dvec2!([0., 0.]))));
		path.line_to(new_point(transform.transform_point2(glam::const_dvec2!([1., 1.]))));
		path
	}
	fn render(&mut self, svg: &mut String, transform: glam::DAffine2, style: style::PathStyle) {
		let [x1, y1] = transform.transform_point2(glam::const_dvec2!([0., 0.])).to_array();
		let [x2, y2] = transform.transform_point2(glam::const_dvec2!([1., 1.])).to_array();

		let _ = write!(svg, r#"<line x1="{}" y1="{}" x2="{}" y2="{}"{} />"#, x1, y1, x2, y2, style.render(),);
	}
}
