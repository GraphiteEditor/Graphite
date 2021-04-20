use super::layer_props;
use super::LayerData;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Circle {
	shape: kurbo::Circle,
	stroke: Option<layer_props::Stroke>,
	fill: Option<layer_props::Fill>,
}

impl Circle {
	pub fn new(center: impl Into<kurbo::Point>, radius: f64, stroke: Option<layer_props::Stroke>, fill: Option<layer_props::Fill>) -> Circle {
		Circle {
			shape: kurbo::Circle::new(center, radius),
			stroke,
			fill,
		}
	}
}

impl LayerData for Circle {
	fn render(&self) -> String {
		format!(
			r#"<circle cx="{}" cy="{}" r="{}" style="{}{}" />"#,
			self.shape.center.x,
			self.shape.center.y,
			self.shape.radius,
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
