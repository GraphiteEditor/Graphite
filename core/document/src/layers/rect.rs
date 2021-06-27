use glam::DVec2;
use kurbo::Point;

use super::style;
use super::LayerData;

use serde::{Deserialize, Serialize};
use std::fmt::Write;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
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

		// TODO: Use into_iter when new impls get added in rust 2021
		[(1., 0.), (1., 1.), (0., 1.)].iter().for_each(|v| path.line_to(new_point(transform.transform_point2((*v).into()))));
		path.close_path();
		path
	}
	fn render(&mut self, svg: &mut String, transform: glam::DAffine2, style: style::PathStyle) {
		let _ = write!(svg, r#"<path d="{}" {} />"#, self.to_kurbo_path(transform, style).to_svg(), style.render());
	}
}
