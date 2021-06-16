use super::style;
use super::LayerData;

use std::fmt::Write;

#[derive(Debug, Clone, PartialEq)]
pub struct Rect {
	shape: kurbo::BezPath,
	style: style::PathStyle,
}

impl Rect {
	pub fn new(style: style::PathStyle) -> Rect {
		let mut path = kurbo::BezPath::new();
		path.move_to((0., 0.));
		path.move_to((0., 1.));
		path.move_to((1., 1.));
		path.move_to((1., 0.));
		path.close_path();
		Rect { shape: path, style }
	}
}

impl LayerData for Rect {
	fn render(&mut self, svg: &mut String) {
		let _ = write!(svg, r#"<path d="{}"{} />"#, self.shape.to_svg(), self.style.render());
	}
}
