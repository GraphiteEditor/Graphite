use super::layer_props;
use super::LayerData;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
	shape: kurbo::Rect,
	stroke: Option<layer_props::Stroke>,
	fill: Option<layer_props::Fill>,
}

impl Rect {
	pub fn new(p0: impl Into<kurbo::Point>, p1: impl Into<kurbo::Point>, stroke: Option<layer_props::Stroke>, fill: Option<layer_props::Fill>) -> Rect {
		Rect {
			shape: kurbo::Rect::from_points(p0, p1),
			stroke,
			fill,
		}
	}
}

impl LayerData for Rect {
	fn render(&self) -> String {
		format!(
			r#"<rect x="{}" y="{}" width="{}" height="{}" style="{}{}" />"#,
			self.shape.min_x(),
			self.shape.min_y(),
			self.shape.width(),
			self.shape.height(),
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
