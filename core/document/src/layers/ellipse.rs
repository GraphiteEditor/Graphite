use super::style;
use super::LayerData;

use std::fmt::Write;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Ellipse {
	shape: kurbo::Ellipse,
	style: style::PathStyle,
}

impl Ellipse {
	pub fn new(center: impl Into<kurbo::Point>, radii: impl Into<kurbo::Vec2>, rotation: f64, style: style::PathStyle) -> Ellipse {
		Ellipse {
			shape: kurbo::Ellipse::new(center, radii, rotation),
			style,
		}
	}
}

impl LayerData for Ellipse {
	fn render(&mut self, svg: &mut String) {
		let _ = write!(
			svg,
			r#"<ellipse cx="0" cy="0" rx="{}" ry="{}" transform="translate({} {}) rotate({})" {} />"#,
			self.shape.radii().x,
			self.shape.radii().y,
			self.shape.center().x,
			self.shape.center().y,
			self.shape.rotation().to_degrees(),
			self.style.render(),
		);
	}
}
