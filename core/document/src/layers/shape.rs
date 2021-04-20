use crate::shape_points;

use super::layer_props;
use super::LayerData;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shape {
	shape: shape_points::ShapePoints,
	stroke: Option<layer_props::Stroke>,
	fill: Option<layer_props::Fill>,
}

impl Shape {
	pub fn new(center: impl Into<kurbo::Point>, extent: impl Into<kurbo::Vec2>, sides: u8, stroke: Option<layer_props::Stroke>, fill: Option<layer_props::Fill>) -> Shape {
		Shape {
			shape: shape_points::ShapePoints::new(center, extent, sides),
			stroke,
			fill,
		}
	}
}

impl LayerData for Shape {
	fn render(&self) -> String {
		format!(
			r#"<polygon points="{}" style="{}{}" />"#,
			self.shape,
			match self.fill {
				Some(fill) => fill.render(),
				None => String::new(),
			},
			match self.stroke {
				Some(stroke) => stroke.render(),
				None => String::new(),
			},
		)
	}
}
