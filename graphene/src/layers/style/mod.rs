use std::default;

use crate::color::Color;
use crate::consts::{LAYER_OUTLINE_STROKE_COLOR, LAYER_OUTLINE_STROKE_WIDTH};

use serde::{Deserialize, Serialize};

use glam::DVec2;

const OPACITY_PRECISION: usize = 3;

fn format_opacity(name: &str, opacity: f32) -> String {
	if (opacity - 1.).abs() > 10_f32.powi(-(OPACITY_PRECISION as i32)) {
		format!(r#" {}-opacity="{:.precision$}""#, name, opacity, precision = OPACITY_PRECISION)
	} else {
		String::new()
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub enum ViewMode {
	Normal,
	Outline,
	Pixels,
}

impl Default for ViewMode {
	fn default() -> Self {
		ViewMode::Normal
	}
}

/// A gradient fill.
///
/// Contains the start and end points, along with the colours at varying points along the length.
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Gradient {
	pub start: DVec2,
	pub end: DVec2,
	pub positions: Vec<(f64, Color)>,
}

/// Describes the fill of a layer.
///
/// Can be None, flat or potentially some sort of image or pattern
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Fill {
	None,
	Flat(Color),
	LinearGradient(Gradient),
}

impl Default for Fill {
	fn default() -> Self {
		Self::None
	}
}

impl Fill {
	pub fn flat(color: Color) -> Self {
		Self::Flat(color)
	}

	/// Evaluate the colour at some point on the fill
	pub fn color(&self) -> Color {
		match self {
			Self::None => Color::BLACK,
			Self::Flat(color) => *color,
			// ToDo: Should correctly sample the gradient
			Self::LinearGradient(Gradient { positions, .. }) => positions[0].1,
		}
	}

	pub fn render(&self) -> String {
		match self {
			Self::None => r#" fill="none""#.to_string(),
			Self::Flat(color) => format!(r##" fill="#{}"{}"##, color.rgb_hex(), format_opacity("fill", color.a())),
			Self::LinearGradient(gradient) => "".to_string(),
		}
	}

	pub fn is_some(&self) -> bool {
		*self != Self::None
	}
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Stroke {
	color: Color,
	width: f32,
}

impl Stroke {
	pub const fn new(color: Color, width: f32) -> Self {
		Self { color, width }
	}

	pub fn color(&self) -> Color {
		self.color
	}

	pub fn width(&self) -> f32 {
		self.width
	}

	pub fn render(&self) -> String {
		format!(r##" stroke="#{}"{} stroke-width="{}""##, self.color.rgb_hex(), format_opacity("stroke", self.color.a()), self.width)
	}
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PathStyle {
	stroke: Option<Stroke>,
	fill: Fill,
}

impl PathStyle {
	pub fn new(stroke: Option<Stroke>, fill: Fill) -> Self {
		Self { stroke, fill }
	}

	pub fn fill(&self) -> &Fill {
		&self.fill
	}

	pub fn stroke(&self) -> Option<Stroke> {
		self.stroke
	}

	pub fn set_fill(&mut self, fill: Fill) {
		self.fill = fill;
	}

	pub fn set_stroke(&mut self, stroke: Stroke) {
		self.stroke = Some(stroke);
	}

	pub fn clear_fill(&mut self) {
		self.fill = Fill::None;
	}

	pub fn clear_stroke(&mut self) {
		self.stroke = None;
	}

	pub fn render(&self, view_mode: ViewMode) -> String {
		let fill_attribute = match (view_mode, &self.fill) {
			(ViewMode::Outline, _) => Fill::None.render(),
			(_, fill) => fill.render(),
		};
		let stroke_attribute = match (view_mode, self.stroke) {
			(ViewMode::Outline, _) => Stroke::new(LAYER_OUTLINE_STROKE_COLOR, LAYER_OUTLINE_STROKE_WIDTH).render(),
			(_, Some(stroke)) => stroke.render(),
			(_, None) => String::new(),
		};

		format!("{}{}", fill_attribute, stroke_attribute)
	}
}
