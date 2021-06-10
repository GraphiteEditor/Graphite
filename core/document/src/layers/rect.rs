use super::style;
use super::LayerData;

use std::fmt::Write;

#[derive(Debug, Clone, PartialEq)]
pub struct Rect {
	shape: kurbo::BezPath,
	style: style::PathStyle,
}

impl Rect {
	pub fn new(x0: f64, y0: f64, x1: f64, y1: f64, style: style::PathStyle) -> Rect {
		let mut path = kurbo::BezPath::new();
		path.move_to((x0, y0));
		path.line_to((x1, y0));
		path.line_to((x1, y1));
		path.line_to((x0, y1));
		path.close_path();
		Rect { shape: path, style }
	}
}

impl LayerData for Rect {
	fn render(&mut self, svg: &mut String) {
		let _ = write!(svg, r#"<path d="{}"{} />"#, self.shape.to_svg(), self.style.render());
	}
}
