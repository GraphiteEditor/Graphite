use super::layer_props;
use super::LayerData;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Circle {
	shape: kurbo::Circle,
	stroke: layer_props::Stroke,
	fill: layer_props::Fill,
}

impl Circle {
	pub fn new(center: impl Into<kurbo::Point>, radius: f64, stroke: layer_props::Stroke, fill: layer_props::Fill) -> Circle {
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
			self.fill.render(),
			self.stroke.render(),
		)
	}
}
