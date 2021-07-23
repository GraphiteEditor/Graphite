use crate::color::Color;
use serde::{Deserialize, Serialize};
const OPACITY_PRECISION: usize = 3;

fn format_opacity(name: &str, opacity: f32) -> String {
	if (opacity - 1.).abs() > 10f32.powi(-(OPACITY_PRECISION as i32)) {
		format!(r#" {}-opacity="{:.precision$}""#, name, opacity, precision = OPACITY_PRECISION)
	} else {
		String::new()
	}
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Fill {
	color: Option<Color>,
}
impl Fill {
	pub fn new(color: Color) -> Self {
		Self { color: Some(color) }
	}
	pub fn none() -> Self {
		Self { color: None }
	}
	pub fn render(&self) -> String {
		match self.color {
			Some(c) => format!(r##" fill="#{}"{}"##, c.rgb_hex(), format_opacity("fill", c.a())),
			None => r#" fill="none""#.to_string(),
		}
	}
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Stroke {
	color: Color,
	width: f32,
}

impl Stroke {
	pub fn new(color: Color, width: f32) -> Self {
		Self { color, width }
	}
	pub fn render(&self) -> String {
		format!(r##" stroke="#{}"{} stroke-width="{}""##, self.color.rgb_hex(), format_opacity("stroke", self.color.a()), self.width)
	}
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct PathStyle {
	stroke: Option<Stroke>,
	fill: Option<Fill>,
}
impl PathStyle {
	pub fn new(stroke: Option<Stroke>, fill: Option<Fill>) -> Self {
		Self { stroke, fill }
	}
	pub fn fill(&self) -> Option<Fill> {
		self.fill
	}
	pub fn stroke(&self) -> Option<Stroke> {
		self.stroke
	}
	pub fn set_fill(&mut self, fill: Fill) {
		self.fill = Some(fill);
	}
	pub fn set_stroke(&mut self, stroke: Stroke) {
		self.stroke = Some(stroke);
	}
	pub fn clear_fill(&mut self) {
		self.fill = None;
	}
	pub fn clear_stroke(&mut self) {
		self.stroke = None;
	}
	pub fn render(&self) -> String {
		format!(
			"{}{}",
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
