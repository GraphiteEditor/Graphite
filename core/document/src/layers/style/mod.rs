use crate::color::Color;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Fill {
	color: Color,
}
impl Fill {
	pub fn new(color: Color) -> Self {
		Self { color }
	}
	pub fn render(&self) -> String {
		format!("fill: #{};", self.color.as_hex())
	}
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Stroke {
	color: Color,
	width: f32,
}

impl Stroke {
	pub fn new(color: Color, width: f32) -> Self {
		Self { color, width }
	}
	pub fn render(&self) -> String {
		format!("stroke: #{};stroke-width:{};", self.color.as_hex(), self.width)
	}
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PathStyle {
	stroke: Option<Stroke>,
	fill: Option<Fill>,
}
impl PathStyle {
	pub fn new(stroke: Option<Stroke>, fill: Option<Fill>) -> Self {
		Self { stroke, fill }
	}
	pub fn render(&self) -> String {
		format!(
			"style=\"{}{}\"",
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
