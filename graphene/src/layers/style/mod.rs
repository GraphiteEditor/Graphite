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

pub const WIRE_FRAME_STROKE_WIDTH: f32 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub enum ViewMode {
	Normal,
	WireFrame,
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
	color: Option<Color>,
}
impl Fill {
	pub fn new(color: Color) -> Self {
		Self { color: Some(color) }
	}
	pub fn color(&self) -> Option<Color> {
		self.color
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
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct PathStyle {
	stroke: Option<Stroke>,
	fill: Option<Fill>,

	#[serde(skip)]
	view_mode: ViewMode,
}
impl PathStyle {
	pub fn new(stroke: Option<Stroke>, fill: Option<Fill>) -> Self {
		Self {
			stroke,
			fill,
			view_mode: ViewMode::default(),
		}
	}
	pub fn with_mode(stroke: Option<Stroke>, fill: Option<Fill>, mode: ViewMode) -> Self {
		Self { stroke, fill, view_mode: mode }
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
	pub fn view_mode(&mut self, new_mode: ViewMode) {
		self.view_mode = new_mode;
	}
	pub fn render(&self) -> String {
		// change stroke rendering so solid paths don't dissapear
		// in wireframe view mode extra Stroke/Fill allocations are done
		format!(
			"{}{}",
			match (self.view_mode, self.fill) {
				(ViewMode::WireFrame, _) => Fill::none().render(),
				(_, Some(fill)) => fill.render(),
				(_, None) => String::new(),
			},
			match (self.view_mode, self.stroke) {
				(ViewMode::WireFrame, _) => Stroke::new(Color::BLACK, WIRE_FRAME_STROKE_WIDTH).render(),
				(_, Some(stroke)) => stroke.render(),
				(_, None) => String::new(),
			},
		)
	}
}
