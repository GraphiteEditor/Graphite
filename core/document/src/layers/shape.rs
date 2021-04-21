use crate::shape_points;

use super::style;
use super::LayerData;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shape {
	shape: shape_points::ShapePoints,
	style: style::PathStyle,
}

impl Shape {
	pub fn new(center: impl Into<kurbo::Point>, extent: impl Into<kurbo::Vec2>, sides: u8, style: style::PathStyle) -> Shape {
		Shape {
			shape: shape_points::ShapePoints::new(center, extent, sides),
			style,
		}
	}
}

impl LayerData for Shape {
	fn render(&self) -> String {
		format!(r#"<polygon points="{}" {} />"#, self.shape, self.style.render(),)
	}
}
