use crate::color::Color;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Fill {
	None(),
	Some(Color),
}
impl Fill {
	pub fn new(col: Color) -> Self {
		Self::Some(col)
	}
	pub fn render(&self) -> String {
		match self {
			Fill::None() => String::new(),
			Fill::Some(col) => format!("fill: #{};", col.as_hex()),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Stroke {
	None(),
	Some(Color, f32),
}

impl Stroke {
	pub fn new(col: Color, width: f32) -> Self {
		Self::Some(col, width)
	}
	pub fn render(&self) -> String {
		match self {
			Stroke::None() => String::new(),
			Stroke::Some(col, width) => format!("stroke: #{};stroke-width:{};", col.as_hex(), width),
		}
	}
}
