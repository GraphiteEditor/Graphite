//! Contains stylistic options for SVG elements.

use crate::color::Color;
use crate::consts::{LAYER_OUTLINE_STROKE_COLOR, LAYER_OUTLINE_STROKE_WIDTH};

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

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

/// Represents different ways of rendering an object
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub enum ViewMode {
	/// Render with normal coloration at the current viewport resolution
	Normal,
	/// Render only the outlines of shapes at the current viewport resolution
	Outline,
	/// Render with normal coloration at the document resolution, showing the pixels when the current viewport resolution is higher
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
	pub positions: Vec<(f64, Color)>,
	uuid: u64,
}
impl Gradient {
	/// Constructs a new gradient with the colors at 0 and 1 specified.
	pub fn new(start: DVec2, start_color: Color, end: DVec2, end_color: Color, transform: DAffine2, uuid: u64) -> Self {
		Gradient {
			start,
			end,
			positions: vec![(0., start_color), (1., end_color)],
			transform,
			uuid,
		}
	}

	/// Adds the gradient def with the uuid specified
	fn render_defs(&self, svg_defs: &mut String) {
		let positions = self
			.positions
			.iter()
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
/// Can be None, a solid [Color], a linear [Gradient], or potentially some sort of image or pattern in the future
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
	/// Construct a new solid [Fill] from a [Color].
	pub fn solid(color: Color) -> Self {
		Self::Solid(color)
	}

	/// Evaluate the color at some point on the fill. Doesn't currently work for LinearGradient.
	pub fn color(&self) -> Color {
		match self {
			Self::None => Color::BLACK,
			Self::Solid(color) => *color,
			// TODO: Should correctly sample the gradient
			Self::LinearGradient(Gradient { positions, .. }) => positions[0].1,
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

/// The stroke (outline) style of an SVG element.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
	/// Stroke color
	color: Color,
	/// Line thickness
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

	/// Provide the SVG attributes for the stroke.
	pub fn render(&self) -> String {
		format!(r##" stroke="#{}"{} stroke-width="{}""##, self.color.rgb_hex(), format_opacity("stroke", self.color.a()), self.width)
	}
}

// Having an alpha of 1 to start with leads to a better experience with the properties panel
impl Default for Stroke {
	fn default() -> Self {
		Self {
			width: 0.,
			color: Color::from_rgba8(0, 0, 0, 255),
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

	/// Get the current path's [Fill].
	///
	/// # Example
	/// ```
	/// # use graphite_graphene::layers::style::{Fill, PathStyle};
	/// # use graphite_graphene::color::Color;
	/// let fill = Fill::solid(Color::RED);
	/// let style = PathStyle::new(None, fill.clone());
	///
	/// assert_eq!(*style.fill(), fill);
	/// ```
	pub fn fill(&self) -> &Fill {
		&self.fill
	}

	/// Get the current path's [Stroke].
	///
	/// # Example
	/// ```
	/// # use graphite_graphene::layers::style::{Fill, Stroke, PathStyle};
	/// # use graphite_graphene::color::Color;
	/// let stroke = Stroke::new(Color::GREEN, 42.);
	/// let style = PathStyle::new(Some(stroke), Fill::None);
	///
	/// assert_eq!(style.stroke(), Some(stroke));
	/// ```
	pub fn stroke(&self) -> Option<Stroke> {
		self.stroke
	}

	/// Replace the path's [Fill] with a provided one.
	///
	/// # Example
	/// ```
	/// # use graphite_graphene::layers::style::{Fill, PathStyle};
	/// # use graphite_graphene::color::Color;
	/// let mut style = PathStyle::default();
	///
	/// assert_eq!(*style.fill(), Fill::None);
	///
	/// let fill = Fill::solid(Color::RED);
	/// style.set_fill(fill.clone());
	///
	/// assert_eq!(*style.fill(), fill);
	/// ```
	pub fn set_fill(&mut self, fill: Fill) {
		self.fill = fill;
	}

	/// Replace the path's [Stroke] with a provided one.
	///
	/// # Example
	/// ```
	/// # use graphite_graphene::layers::style::{Stroke, PathStyle};
	/// # use graphite_graphene::color::Color;
	/// let mut style = PathStyle::default();
	///
	/// assert_eq!(style.stroke(), None);
	///
	/// let stroke = Stroke::new(Color::GREEN, 42.);
	/// style.set_stroke(stroke);
	///
	/// assert_eq!(style.stroke(), Some(stroke));
	/// ```
	pub fn set_stroke(&mut self, stroke: Stroke) {
		self.stroke = Some(stroke);
	}

	/// Set the path's fill to None.
	///
	/// # Example
	/// ```
	/// # use graphite_graphene::layers::style::{Fill, PathStyle};
	/// # use graphite_graphene::color::Color;
	/// let mut style = PathStyle::new(None, Fill::Solid(Color::RED));
	///
	/// assert!(style.fill().is_some());
	///
	/// style.clear_fill();
	///
	/// assert!(!style.fill().is_some());
	/// ```
	pub fn clear_fill(&mut self) {
		self.fill = Fill::None;
	}

	/// Set the path's stroke to None.
	///
	/// # Example
	/// ```
	/// # use graphite_graphene::layers::style::{Fill, Stroke, PathStyle};
	/// # use graphite_graphene::color::Color;
	/// let mut style = PathStyle::new(Some(Stroke::new(Color::GREEN, 42.)), Fill::None);
	///
	/// assert!(style.stroke().is_some());
	///
	/// style.clear_stroke();
	///
	/// assert!(!style.stroke().is_some());
	/// ```
	pub fn clear_stroke(&mut self) {
		self.stroke = None;
	}

	pub fn render(&self, view_mode: ViewMode, svg_defs: &mut String) -> String {
		let fill_attribute = match (view_mode, &self.fill) {
			(ViewMode::Outline, _) => Fill::None.render(svg_defs),
			(_, fill) => fill.render(svg_defs),
		};
		let stroke_attribute = match (view_mode, self.stroke) {
			(ViewMode::Outline, _) => Stroke::new(LAYER_OUTLINE_STROKE_COLOR, LAYER_OUTLINE_STROKE_WIDTH).render(),
			(_, Some(stroke)) => stroke.render(),
			(_, None) => String::new(),
		};

		format!("{}{}", fill_attribute, stroke_attribute)
	}
}
