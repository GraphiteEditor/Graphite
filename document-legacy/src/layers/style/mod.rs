//! Contains stylistic options for SVG elements.

use super::text_layer::FontCache;
use crate::color::Color;
use crate::consts::{LAYER_OUTLINE_STROKE_COLOR, LAYER_OUTLINE_STROKE_WEIGHT};

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Write};

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
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

/// Contains metadata for rendering the document as an svg
#[derive(Debug, Clone, Copy)]
pub struct RenderData<'a> {
	pub view_mode: ViewMode,
	pub font_cache: &'a FontCache,
	pub culling_bounds: Option<[DVec2; 2]>,
}

impl<'a> RenderData<'a> {
	pub fn new(view_mode: ViewMode, font_cache: &'a FontCache, culling_bounds: Option<[DVec2; 2]>) -> Self {
		Self {
			view_mode,
			font_cache,
			culling_bounds,
		}
	}
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, Hash, Serialize, Deserialize)]
pub enum GradientType {
	Linear,
	Radial,
}

impl Default for GradientType {
	fn default() -> Self {
		GradientType::Linear
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
	pub gradient_type: GradientType,
}

impl Gradient {
	/// Constructs a new gradient with the colors at 0 and 1 specified.
	pub fn new(start: DVec2, start_color: Color, end: DVec2, end_color: Color, transform: DAffine2, uuid: u64, gradient_type: GradientType) -> Self {
		Gradient {
			start,
			end,
			positions: vec![(0., Some(start_color)), (1., Some(end_color))],
			transform,
			uuid,
			gradient_type,
		}
	}

	/// Adds the gradient def with the uuid specified
	fn render_defs(&self, svg_defs: &mut String, multiplied_transform: DAffine2, bounds: [DVec2; 2], transformed_bounds: [DVec2; 2]) {
		let bound_transform = DAffine2::from_scale_angle_translation(bounds[1] - bounds[0], 0., bounds[0]);
		let transformed_bound_transform = DAffine2::from_scale_angle_translation(transformed_bounds[1] - transformed_bounds[0], 0., transformed_bounds[0]);
		let updated_transform = multiplied_transform * bound_transform;

		let positions = self
			.positions
			.iter()
			.filter_map(|(pos, color)| color.map(|color| (pos, color)))
			.map(|(position, color)| format!(r##"<stop offset="{}" stop-color="#{}" />"##, position, color.rgba_hex()))
			.collect::<String>();

		let mod_gradient = transformed_bound_transform.inverse();
		let mod_points = mod_gradient.inverse() * transformed_bound_transform.inverse() * updated_transform;

		let start = mod_points.transform_point2(self.start);
		let end = mod_points.transform_point2(self.end);

		let transform = mod_gradient
			.to_cols_array()
			.iter()
			.enumerate()
			.map(|(i, entry)| entry.to_string() + if i == 5 { "" } else { "," })
			.collect::<String>();

		match self.gradient_type {
			GradientType::Linear => {
				let _ = write!(
					svg_defs,
					r#"<linearGradient id="{}" x1="{}" x2="{}" y1="{}" y2="{}" gradientTransform="matrix({})">{}</linearGradient>"#,
					self.uuid, start.x, end.x, start.y, end.y, transform, positions
				);
			}
			GradientType::Radial => {
				let radius = (f64::powi(start.x - end.x, 2) + f64::powi(start.y - end.y, 2)).sqrt();
				let _ = write!(
					svg_defs,
					r#"<radialGradient id="{}" cx="{}" cy="{}" r="{}" gradientTransform="matrix({})">{}</radialGradient>"#,
					self.uuid, start.x, start.y, radius, transform, positions
				);
			}
		}
	}

	/// Insert a stop into the gradient, the index if successful
	pub fn insert_stop(&mut self, mouse: DVec2, transform: DAffine2) -> Option<usize> {
		// Transform the start and end positions to the same coordinate space as the mouse.
		let (start, end) = (transform.transform_point2(self.start), transform.transform_point2(self.end));

		// Calculate the new position by finding the closest point on the line
		let new_position = ((end - start).angle_between(mouse - start)).cos() * start.distance(mouse) / start.distance(end);

		// Don't insert point past end of line
		if !(0. ..=1.).contains(&new_position) {
			return None;
		}

		// Compute the color of the inserted stop
		let get_color = |index: usize, time: f64| match (self.positions[index].1, self.positions.get(index + 1).and_then(|x| x.1)) {
			// Lerp between the nearest colours if applicable
			(Some(a), Some(b)) => a.lerp(
				b,
				((time - self.positions[index].0) / self.positions.get(index + 1).map(|end| end.0 - self.positions[index].0).unwrap_or_default()) as f32,
			),
			// Use the start or the end colour if applicable
			(Some(v), _) | (_, Some(v)) => Some(v),
			_ => Some(Color::WHITE),
		};

		// Compute the correct index to keep the positions in order
		let mut index = 0;
		while self.positions.len() > index && self.positions[index].0 <= new_position {
			index += 1;
		}

		let new_color = get_color(index - 1, new_position);

		// Insert the new stop
		self.positions.insert(index, (new_position, new_color));

		Some(index)
	}
}

/// Describes the fill of a layer.
///
/// Can be None, a solid [Color], a linear [Gradient], a radial [Gradient] or potentially some sort of image or pattern in the future
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Fill {
	None,
	Solid(Color),
	Gradient(Gradient),
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

	/// Evaluate the color at some point on the fill. Doesn't currently work for Gradient.
	pub fn color(&self) -> Color {
		match self {
			Self::None => Color::BLACK,
			Self::Solid(color) => *color,
			// TODO: Should correctly sample the gradient
			Self::Gradient(Gradient { positions, .. }) => positions[0].1.unwrap_or(Color::BLACK),
		}
	}

	/// Renders the fill, adding necessary defs.
	pub fn render(&self, svg_defs: &mut String, multiplied_transform: DAffine2, bounds: [DVec2; 2], transformed_bounds: [DVec2; 2]) -> String {
		match self {
			Self::None => r#" fill="none""#.to_string(),
			Self::Solid(color) => format!(r##" fill="#{}"{}"##, color.rgb_hex(), format_opacity("fill", color.a())),
			Self::Gradient(gradient) => {
				gradient.render_defs(svg_defs, multiplied_transform, bounds, transformed_bounds);
				format!(r##" fill="url('#{}')""##, gradient.uuid)
			}
		}
	}

	/// Check if the fill is not none
	pub fn is_some(&self) -> bool {
		*self != Self::None
	}

	/// Extract a gradient from the fill
	pub fn as_gradient(&self) -> Option<&Gradient> {
		if let Self::Gradient(gradient) = self {
			Some(gradient)
		} else {
			None
		}
	}
}

/// The stroke (outline) style of an SVG element.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineCap {
	Butt,
	Round,
	Square,
}

impl Display for LineCap {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			LineCap::Butt => write!(f, "butt"),
			LineCap::Round => write!(f, "round"),
			LineCap::Square => write!(f, "square"),
		}
	}
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineJoin {
	Miter,
	Bevel,
	Round,
}

impl Display for LineJoin {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			LineJoin::Bevel => write!(f, "bevel"),
			LineJoin::Miter => write!(f, "miter"),
			LineJoin::Round => write!(f, "round"),
		}
	}
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
	/// Stroke color
	color: Option<Color>,
	/// Line thickness
	weight: f64,
	dash_lengths: Vec<f32>,
	dash_offset: f64,
	line_cap: LineCap,
	line_join: LineJoin,
	line_join_miter_limit: f64,
}

impl Stroke {
	pub fn new(color: Color, weight: f64) -> Self {
		Self {
			color: Some(color),
			weight,
			..Default::default()
		}
	}

	/// Get the current stroke color.
	pub fn color(&self) -> Option<Color> {
		self.color
	}

	/// Get the current stroke weight.
	pub fn weight(&self) -> f64 {
		self.weight
	}

	pub fn dash_lengths(&self) -> String {
		self.dash_lengths.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", ")
	}

	pub fn dash_offset(&self) -> f64 {
		self.dash_offset
	}

	pub fn line_cap_index(&self) -> u32 {
		self.line_cap as u32
	}

	pub fn line_join_index(&self) -> u32 {
		self.line_join as u32
	}

	pub fn line_join_miter_limit(&self) -> f32 {
		self.line_join_miter_limit as f32
	}

	/// Provide the SVG attributes for the stroke.
	pub fn render(&self) -> String {
		if let Some(color) = self.color {
			format!(
				r##" stroke="#{}"{} stroke-width="{}" stroke-dasharray="{}" stroke-dashoffset="{}" stroke-linecap="{}" stroke-linejoin="{}" stroke-miterlimit="{}" "##,
				color.rgb_hex(),
				format_opacity("stroke", color.a()),
				self.weight,
				self.dash_lengths(),
				self.dash_offset,
				self.line_cap,
				self.line_join,
				self.line_join_miter_limit
			)
		} else {
			String::new()
		}
	}

	pub fn with_color(mut self, color: &Option<Color>) -> Option<Self> {
		self.color = *color;

		Some(self)
	}

	pub fn with_weight(mut self, weight: f64) -> Self {
		self.weight = weight;
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

	pub fn with_dash_offset(mut self, dash_offset: f64) -> Self {
		self.dash_offset = dash_offset;
		self
	}

	pub fn with_line_cap(mut self, line_cap: LineCap) -> Self {
		self.line_cap = line_cap;
		self
	}

	pub fn with_line_join(mut self, line_join: LineJoin) -> Self {
		self.line_join = line_join;
		self
	}

	pub fn with_line_join_miter_limit(mut self, limit: f64) -> Self {
		self.line_join_miter_limit = limit;
		self
	}
}

// Having an alpha of 1 to start with leads to a better experience with the properties panel
impl Default for Stroke {
	fn default() -> Self {
		Self {
			weight: 0.,
			color: Some(Color::from_rgba8(0, 0, 0, 255)),
			dash_lengths: vec![0.],
			dash_offset: 0.,
			line_cap: LineCap::Butt,
			line_join: LineJoin::Miter,
			line_join_miter_limit: 4.,
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
	/// # use graphite_document_legacy::layers::style::{Fill, PathStyle};
	/// # use graphite_document_legacy::color::Color;
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
	/// # use graphite_document_legacy::layers::style::{Fill, Stroke, PathStyle};
	/// # use graphite_document_legacy::color::Color;
	/// let stroke = Stroke::new(Color::GREEN, 42.);
	/// let style = PathStyle::new(Some(stroke.clone()), Fill::None);
	///
	/// assert_eq!(style.stroke(), Some(stroke));
	/// ```
	pub fn stroke(&self) -> Option<Stroke> {
		self.stroke.clone()
	}

	/// Replace the path's [Fill] with a provided one.
	///
	/// # Example
	/// ```
	/// # use graphite_document_legacy::layers::style::{Fill, PathStyle};
	/// # use graphite_document_legacy::color::Color;
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
	/// # use graphite_document_legacy::layers::style::{Stroke, PathStyle};
	/// # use graphite_document_legacy::color::Color;
	/// let mut style = PathStyle::default();
	///
	/// assert_eq!(style.stroke(), None);
	///
	/// let stroke = Stroke::new(Color::GREEN, 42.);
	/// style.set_stroke(stroke.clone());
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
	/// # use graphite_document_legacy::layers::style::{Fill, PathStyle};
	/// # use graphite_document_legacy::color::Color;
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
	/// # use graphite_document_legacy::layers::style::{Fill, Stroke, PathStyle};
	/// # use graphite_document_legacy::color::Color;
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

	pub fn render(&self, view_mode: ViewMode, svg_defs: &mut String, multiplied_transform: DAffine2, bounds: [DVec2; 2], transformed_bounds: [DVec2; 2]) -> String {
		let fill_attribute = match (view_mode, &self.fill) {
			(ViewMode::Outline, _) => Fill::None.render(svg_defs, multiplied_transform, bounds, transformed_bounds),
			(_, fill) => fill.render(svg_defs, multiplied_transform, bounds, transformed_bounds),
		};
		let stroke_attribute = match (view_mode, &self.stroke) {
			(ViewMode::Outline, _) => Stroke::new(LAYER_OUTLINE_STROKE_COLOR, LAYER_OUTLINE_STROKE_WEIGHT).render(),
			(_, Some(stroke)) => stroke.render(),
			(_, None) => String::new(),
		};

		format!("{}{}", fill_attribute, stroke_attribute)
	}
}
