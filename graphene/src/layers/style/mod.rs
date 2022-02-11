//! Contains stylistic options for SVG Elements.
use crate::color::Color;
use crate::consts::{LAYER_OUTLINE_STROKE_COLOR, LAYER_OUTLINE_STROKE_WIDTH};

use serde::{Deserialize, Serialize};

/// Precision of the opacity value in digits after the decimal point.
/// A value of 3 would correspond to a precision of 10^-3.
const OPACITY_PRECISION: usize = 3;

fn format_opacity(name: &str, opacity: f32) -> String {
	if (opacity - 1.).abs() > 10_f32.powi(-(OPACITY_PRECISION as i32)) {
		format!(r#" {}-opacity="{:.precision$}""#, name, opacity, precision = OPACITY_PRECISION)
	} else {
		String::new()
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
/// Represents different ways of rendering an object
pub enum ViewMode {
	/// Render everything.
	Normal,
	/// Only render the outline.
	Outline,
	Pixels,
}

impl Default for ViewMode {
	fn default() -> Self {
		ViewMode::Normal
	}
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Fill {
	/// The fill color
	color: Color,
}

impl Fill {
	/// Create a new [Fill] from a [Color].
	pub fn new(color: Color) -> Self {
		Self { color }
	}

	/// Set the fill color
	pub fn color(&self) -> Color {
		self.color
	}

	/// Render a fill to a string
	pub fn render(fill: Option<Fill>) -> String {
		match fill {
			Some(c) => format!(r##" fill="#{}"{}"##, c.color.rgb_hex(), format_opacity("fill", c.color.a())),
			None => r#" fill="none""#.to_string(),
		}
	}
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Stroke {
	/// The stroke color
	color: Color,
	/// The line width
	width: f32,
}

impl Stroke {
	pub const fn new(color: Color, width: f32) -> Self {
		Self { color, width }
	}

	/// Get the current stroke color.
	pub fn color(&self) -> Color {
		self.color
	}

	/// Get the current stroke width.
	pub fn width(&self) -> f32 {
		self.width
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

	/// Get the current path fill
	pub fn fill(&self) -> Option<Fill> {
		self.fill
	}

	/// Get the current path stroke
	pub fn stroke(&self) -> Option<Stroke> {
		self.stroke
	}

	/// Set the path fill
	pub fn set_fill(&mut self, fill: Fill) {
		self.fill = Some(fill);
	}

	/// Set the path stroke
	pub fn set_stroke(&mut self, stroke: Stroke) {
		self.stroke = Some(stroke);
	}

	/// Clear the path fill
	pub fn clear_fill(&mut self) {
		self.fill = None;
	}

	/// Clear the path stroke
	pub fn clear_stroke(&mut self) {
		self.stroke = None;
	}

	pub fn render(&self, view_mode: ViewMode) -> String {
		let fill_attribute = match (view_mode, self.fill) {
			(ViewMode::Outline, _) => Fill::render(None),
			(_, fill) => Fill::render(fill),
		};
		let stroke_attribute = match (view_mode, self.stroke) {
			(ViewMode::Outline, _) => Stroke::new(LAYER_OUTLINE_STROKE_COLOR, LAYER_OUTLINE_STROKE_WIDTH).render(),
			(_, Some(stroke)) => stroke.render(),
			(_, None) => String::new(),
		};

		format!("{}{}", fill_attribute, stroke_attribute)
	}
}
