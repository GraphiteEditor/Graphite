use super::style;
use super::LayerData;

use std::fmt::Write;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Circle {
	shape: kurbo::Circle,
	rotation: f64,
	style: style::PathStyle,
}

impl Circle {
	pub fn new(center: impl Into<kurbo::Point>, radius: f64, rotation: f64, style: style::PathStyle) -> Circle {
		Circle {
			shape: kurbo::Circle::new(center, radius),
			rotation,
			style,
		}
	}
}

impl LayerData for Circle {
	fn render(&mut self, svg: &mut String) {
		let _ = write!(
			svg,
			r#"<circle cx="{}" cy="{}" r="{}" transform="rotate({})"{} />"#,
			self.shape.center.x,
			self.shape.center.y,
			self.shape.radius,
			self.rotation,
			self.style.render(),
		);
	}
}
