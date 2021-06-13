use super::style;
use super::LayerData;

use std::fmt::Write;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Line {
	shape: kurbo::Line,
	style: style::PathStyle,
}

impl Line {
	pub fn new(x0: f64, y0: f64, x1: f64, y1: f64, style: style::PathStyle) -> Line {
		Line {
			shape: kurbo::Line::new((x0, y0), (x1, y1)),
			style,
		}
	}
}

impl LayerData for Line {
	fn render(&mut self, svg: &mut String) {
		let kurbo::Point { x: x1, y: y1 } = self.shape.p0;
		let kurbo::Point { x: x2, y: y2 } = self.shape.p1;

		let _ = write!(svg, r#"<line x1="{}" y1="{}" x2="{}" y2="{}"{} />"#, x1, y1, x2, y2, self.style.render(),);
	}
}
