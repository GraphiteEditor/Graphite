use super::style;
use super::LayerData;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Line {
	shape: kurbo::Line,
	style: style::PathStyle,
}

impl Line {
	pub fn new(p0: impl Into<kurbo::Point>, p1: impl Into<kurbo::Point>, style: style::PathStyle) -> Line {
		Line {
			shape: kurbo::Line::new(p0, p1),
			style,
		}
	}
}

impl LayerData for Line {
	fn render(&self) -> String {
		format!(
			r#"<line x1="{}" y1="{}" x2="{}" y2="{}" {} />"#,
			self.shape.p0.x,
			self.shape.p0.y,
			self.shape.p1.x,
			self.shape.p1.y,
			self.style.render(),
		)
	}
}
