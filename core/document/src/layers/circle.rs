use super::style;
use super::LayerData;

use std::fmt::Write;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Circle {
	shape: kurbo::Circle,
	style: style::PathStyle,
}

impl Circle {
	pub fn new(center: impl Into<kurbo::Point>, radius: f64, style: style::PathStyle) -> Circle {
		Circle {
			shape: kurbo::Circle::new(center, radius),
			style,
		}
	}
}

impl LayerData for Circle {
	fn render(&mut self, svg: &mut String) {
		let _ = write!(
			svg,
			r#"<circle cx="{}" cy="{}" r="{}"{} />"#,
			self.shape.center.x,
			self.shape.center.y,
			self.shape.radius,
			self.style.render(),
		);
	}
}
