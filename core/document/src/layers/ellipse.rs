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
		let kurbo::Vec2 { x: rx, y: ry } = self.shape.radii();
		let kurbo::Point { x: cx, y: cy } = self.shape.center();

		let _ = write!(
			svg,
			r#"<ellipse cx="0" cy="0" rx="{}" ry="{}" transform="translate({} {}) rotate({})"{}/>"#,
			rx,
			ry,
			cx,
			cy,
			self.shape.rotation().to_degrees(),
			self.style.render(),
		);
	}
}
