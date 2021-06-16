use kurbo::Shape;

use super::style;
use super::LayerData;

use std::fmt::Write;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Ellipse {
	shape: kurbo::Ellipse,
	style: style::PathStyle,
}

impl Ellipse {
	pub fn new(style: style::PathStyle) -> Ellipse {
		Ellipse {
			shape: kurbo::Ellipse::new((0., 0.), (1., 1.), 0.),
			style,
		}
	}
}

impl LayerData for Ellipse {
	fn render(&mut self, svg: &mut String) {
		let _ = write!(svg, r#"<path d="{}" {} />"#, self.shape.to_path(0.1).to_svg(), self.style.render());
	}
}
