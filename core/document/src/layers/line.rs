use super::layer_props;
use super::LayerData;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Line {
	shape: kurbo::Line,
	stroke: Option<layer_props::Stroke>,
}

impl Line {
	pub fn new(p0: impl Into<kurbo::Point>, p1: impl Into<kurbo::Point>, stroke: Option<layer_props::Stroke>) -> Line {
		Line {
			shape: kurbo::Line::new(p0, p1),
			stroke,
		}
	}
}

impl LayerData for Line {
	fn render(&self) -> String {
		format!(
			r#"<line x1="{}" y1="{}" x2="{}" y2="{}" style="{}" />"#,
			self.shape.p0.x,
			self.shape.p0.y,
			self.shape.p1.x,
			self.shape.p1.y,
			match self.stroke {
				Some(stroke) => stroke.render(),
				None => String::new(),
			},
		)
	}
}
