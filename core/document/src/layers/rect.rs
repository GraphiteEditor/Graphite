use super::style;
use super::LayerData;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
	shape: kurbo::Rect,
	style: style::PathStyle,
}

impl Rect {
	pub fn new(p0: impl Into<kurbo::Point>, p1: impl Into<kurbo::Point>, style: style::PathStyle) -> Rect {
		Rect {
			shape: kurbo::Rect::from_points(p0, p1),
			style,
		}
	}
}

impl LayerData for Rect {
	fn render(&self) -> String {
		format!(
			r#"<rect x="{}" y="{}" width="{}" height="{}" {} />"#,
			self.shape.min_x(),
			self.shape.min_y(),
			self.shape.width(),
			self.shape.height(),
			self.style.render(),
		)
	}
}
