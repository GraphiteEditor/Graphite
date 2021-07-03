use kurbo::Shape;

use super::style;
use super::LayerData;

use std::fmt::Write;

#[derive(Debug, Clone, Copy, Default)]
pub struct Ellipse {}

impl Ellipse {
	pub fn new() -> Ellipse {
		Ellipse {}
	}
}

impl LayerData for Ellipse {
	fn to_kurbo_path(&mut self, transform: glam::DAffine2, _style: style::PathStyle) -> kurbo::BezPath {
		kurbo::Ellipse::from_affine(kurbo::Affine::new(transform.to_cols_array())).to_path(0.1)
	}
	fn render(&mut self, svg: &mut String, transform: glam::DAffine2, style: style::PathStyle) {
		let _ = write!(svg, r#"<path d="{}" {} />"#, self.to_kurbo_path(transform, style).to_svg(), style.render());
	}
}
