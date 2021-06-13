use super::style;
use super::LayerData;

use std::fmt::Write;

#[derive(Debug, Clone, PartialEq)]
pub struct Rect {
	shape: kurbo::BezPath,
	style: style::PathStyle,
}

impl Rect {
	pub fn new(cols: [f64; 6], style: style::PathStyle) -> Rect {
		let mut path = kurbo::BezPath::new();
		path.move_to((cols[4] + cols[0] * 0. + cols[1] * 0., cols[5] + cols[2] * 0. + cols[3] * 0.));
		path.line_to((cols[4] + cols[0] * 1. + cols[1] * 0., cols[5] + cols[2] * 1. + cols[3] * 0.));
		path.line_to((cols[4] + cols[0] * 1. + cols[1] * 1., cols[5] + cols[2] * 1. + cols[3] * 1.));
		path.line_to((cols[4] + cols[0] * 0. + cols[1] * 1., cols[5] + cols[2] * 0. + cols[3] * 1.));
		path.close_path();
		Rect { shape: path, style }
	}
}

impl LayerData for Rect {
	fn render(&mut self, svg: &mut String) {
		let _ = write!(svg, r#"<path d="{}"{} />"#, self.shape.to_svg(), self.style.render());
	}
}
