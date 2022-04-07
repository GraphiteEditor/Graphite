use std::fmt::{self, Display, Write};

use crate::color::Color;
use crate::consts::{LAYER_OUTLINE_STROKE_COLOR, LAYER_OUTLINE_STROKE_WIDTH};

use serde::{Deserialize, Serialize};

use glam::{DAffine2, DVec2};

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
/// Contains the start and end points, along with the colors at varying points along the length.
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Gradient {
	pub start: DVec2,
	pub end: DVec2,
	pub transform: DAffine2,
	pub positions: Vec<(f64, Option<Color>)>,
	uuid: u64,
}
impl Gradient {
	/// Constructs a new gradient with the colors at 0 and 1 specified.
	pub fn new(start: DVec2, start_color: Color, end: DVec2, end_color: Color, transform: DAffine2, uuid: u64) -> Self {
		Gradient {
			start,
			end,
			positions: vec![(0., Some(start_color)), (1., Some(end_color))],
			transform,
			uuid,
		}
	}

	/// Adds the gradient def with the uuid specified
	fn render_defs(&self, svg_defs: &mut String) {
		let positions = self
			.positions
			.iter()
			.filter_map(|(pos, color)| color.map(|color| (pos, color)))
			.map(|(position, color)| format!(r##"<stop offset="{}" stop-color="#{}" />"##, position, color.rgba_hex()))
			.collect::<String>();

		let start = self.transform.inverse().transform_point2(self.start);
		let end = self.transform.inverse().transform_point2(self.end);

		let transform = self
			.transform
			.to_cols_array()
			.iter()
			.enumerate()
			.map(|(i, entry)| entry.to_string() + if i == 5 { "" } else { "," })
			.collect::<String>();

		let _ = write!(
			svg_defs,
			r#"<linearGradient id="{}" x1="{}" x2="{}" y1="{}" y2="{}" gradientTransform="matrix({})">{}</linearGradient>"#,
			self.uuid, start.x, end.x, start.y, end.y, transform, positions
		);
	}
}

/// Describes the fill of a layer.
///
/// Can be None, solid or potentially some sort of image or pattern
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Fill {
	None,
	Solid(Color),
	LinearGradient(Gradient),
}

impl Default for Fill {
	fn default() -> Self {
		Self::None
	}
}

impl Fill {
	/// Construct a new solid fill
	pub fn solid(color: Color) -> Self {
		Self::Solid(color)
	}

	/// Evaluate the color at some point on the fill
	pub fn color(&self) -> Color {
		match self {
			Self::None => Color::BLACK,
			Self::Solid(color) => *color,
			// ToDo: Should correctly sample the gradient
			Self::LinearGradient(Gradient { positions, .. }) => positions[0].1.unwrap_or(Color::BLACK),
		}
	}

	/// Renders the fill, adding necessary defs.
	pub fn render(&self, svg_defs: &mut String) -> String {
		match self {
			Self::None => r#" fill="none""#.to_string(),
			Self::Solid(color) => format!(r##" fill="#{}"{}"##, color.rgb_hex(), format_opacity("fill", color.a())),
			Self::LinearGradient(gradient) => {
				gradient.render_defs(svg_defs);
				format!(r##" fill="url('#{}')""##, gradient.uuid)
			}
		}
	}

	/// Check if the fill is not none
	pub fn is_some(&self) -> bool {
		*self != Self::None
	}
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum LineCap {
	Butt,
	Round,
	Square,
}

impl Display for LineCap {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(match &self {
			LineCap::Butt => "butt",
			LineCap::Round => "round",
			LineCap::Square => "square",
		})
	}
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum LineJoin {
	Bevel,
	Miter,
	Round,
}

impl Display for LineJoin {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(match &self {
			LineJoin::Bevel => "bevel",
			LineJoin::Miter => "miter",
			LineJoin::Round => "round",
		})
	}
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
	color: Option<Color>,
	width: f32,
	dash_lengths: Vec<f32>,
	dash_offset: f32,
	line_cap: LineCap,
	line_join: LineJoin,
	miter_limit: f32,
}

impl Stroke {
	pub fn new(color: Color, width: f32) -> Self {
		Self {
			color: Some(color),
			width,
			..Default::default()
		}
	}

	pub fn color(&self) -> Option<Color> {
		self.color
	}

	pub fn width(&self) -> f32 {
		self.width
	}

	pub fn dash_lengths(&self) -> String {
		self.dash_lengths.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", ")
	}

	pub fn dash_offset(&self) -> f32 {
		self.dash_offset
	}

	pub fn line_cap_index(&self) -> u32 {
		self.line_cap as u32
	}
	pub fn line_join_index(&self) -> u32 {
		self.line_join as u32
	}

	pub fn miterlimit(&self) -> f32 {
		self.miter_limit as f32
	}

	pub fn render(&self) -> String {
		if let Some(color) = self.color {
			format!(
				r##" stroke="#{}"{} stroke-width="{}" stroke-dasharray="{}" stroke-dashoffset="{}" stroke-linecap="{}" stroke-linejoin="{}" stroke-miterlimit="{}" "##,
				color.rgb_hex(),
				format_opacity("stroke", color.a()),
				self.width,
				self.dash_lengths(),
				self.dash_offset,
				self.line_cap,
				self.line_join,
				self.miter_limit
			)
		} else {
			String::new()
		}
	}

	pub fn with_color(mut self, color: &Option<String>) -> Option<Self> {
		if let Some(color) = color {
			Color::from_rgba_str(color).or_else(|| Color::from_rgb_str(color)).map(|color| {
				self.color = Some(color);
				self
			})
		} else {
			self.color = None;
			Some(self)
		}
	}
	pub fn with_width(mut self, width: f32) -> Self {
		self.width = width;
		self
	}
	pub fn with_dash_lengths(mut self, dash_lengths: &str) -> Option<Self> {
		dash_lengths
			.split(&[',', ' '])
			.filter(|x| !x.is_empty())
			.map(str::parse::<f32>)
			.collect::<Result<Vec<_>, _>>()
			.ok()
			.map(|lengths| {
				self.dash_lengths = lengths;
				self
			})
	}
	pub fn with_dash_offset(mut self, dash_offset: f32) -> Self {
		self.dash_offset = dash_offset;
		self
	}
	pub fn with_linecap(mut self, line_cap: LineCap) -> Self {
		self.line_cap = line_cap;
		self
	}
	pub fn with_linejoin(mut self, line_join: LineJoin) -> Self {
		self.line_join = line_join;
		self
	}
	pub fn with_miterlimit(mut self, miterlimit: f32) -> Self {
		self.miter_limit = miterlimit;
		self
	}
}

// Having an alpha of 1 to start with leads to a better experience with the properties panel
impl Default for Stroke {
	fn default() -> Self {
		Self {
			width: 0.,
			color: Some(Color::from_rgba8(0, 0, 0, 255)),
			dash_lengths: vec![0.],
			dash_offset: 0.,
			line_cap: LineCap::Butt,
			line_join: LineJoin::Miter,
			miter_limit: 4.,
		}
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
		self.stroke.clone()
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

	pub fn render(&self, view_mode: ViewMode, svg_defs: &mut String) -> String {
		let fill_attribute = match (view_mode, &self.fill) {
			(ViewMode::Outline, _) => Fill::None.render(svg_defs),
			(_, fill) => fill.render(svg_defs),
		};
		let stroke_attribute = match (view_mode, &self.stroke) {
			(ViewMode::Outline, _) => Stroke::new(LAYER_OUTLINE_STROKE_COLOR, LAYER_OUTLINE_STROKE_WIDTH).render(),
			(_, Some(stroke)) => stroke.render(),
			(_, None) => String::new(),
		};

		format!("{}{}", fill_attribute, stroke_attribute)
	}
}
