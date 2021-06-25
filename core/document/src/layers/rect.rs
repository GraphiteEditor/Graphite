use glam::DVec2;
use kurbo::Point;

use super::style;
use super::LayerData;

use std::fmt::Write;

#[derive(Debug, Clone, PartialEq)]
pub struct Rect {}

impl Rect {
	pub fn new() -> Rect {
		Rect {}
	}
}

impl LayerData for Rect {
	fn to_kurbo_path(&mut self, transform: glam::DAffine2, _style: style::PathStyle) -> kurbo::BezPath {
		fn new_point(a: DVec2) -> Point {
			Point::new(a.x, a.y)
		}
		let mut path = kurbo::BezPath::new();
		path.move_to(new_point(transform.translation));
		[(1., 0.), (1., 1.), (0., 1.)]
			.iter()
			.for_each(|(x, y)| path.line_to(new_point(transform.transform_point2(DVec2::new(*x, *y)))));
		path.close_path();
		path
	}
	fn render(&mut self, svg: &mut String, transform: glam::DAffine2, style: style::PathStyle) {
		let _ = write!(svg, r#"<path d="{}" {} />"#, self.to_kurbo_path(transform, style).to_svg(), style.render());
	}
}
