use super::style;
use super::LayerData;

use std::fmt::Write;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Line {
	shape: kurbo::Line,
	style: style::PathStyle,
}

impl Line {
	pub fn from_points<T: Into<kurbo::Point>>(p1: T, p2: T, style: style::PathStyle) -> Line {
		Line {
			shape: kurbo::Line::new(p1, p2),
			style,
		}
	}
	pub fn new(style: style::PathStyle) -> Line {
		Line::from_points((0., 0.), (1., 1.), style)
	}
}

impl LayerData for Line {
	fn render(&mut self, svg: &mut String) {
		let kurbo::Point { x: x1, y: y1 } = self.shape.p0;
		let kurbo::Point { x: x2, y: y2 } = self.shape.p1;

		let _ = write!(svg, r#"<line x1="{}" y1="{}" x2="{}" y2="{}"{} />"#, x1, y1, x2, y2, self.style.render(),);
	}
}
