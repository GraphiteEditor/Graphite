//! Contains stylistic options for SVG elements.

use crate::consts::{LAYER_OUTLINE_STROKE_COLOR, LAYER_OUTLINE_STROKE_WEIGHT};
use crate::Color;

use dyn_any::{DynAny, StaticType};
use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Write};

/// Precision of the opacity value in digits after the decimal point.
/// A value of 3 would correspond to a precision of 10^-3.
const OPACITY_PRECISION: usize = 3;

fn format_opacity(attribute: &str, opacity: f32) -> String {
	if (opacity - 1.).abs() > 10_f32.powi(-(OPACITY_PRECISION as i32)) {
		format!(r#" {attribute}="{opacity:.OPACITY_PRECISION$}""#)
	} else {
		String::new()
	}
}

#[derive(Default, PartialEq, Eq, Clone, Copy, Debug, Hash, Serialize, Deserialize, DynAny, specta::Type)]
pub enum GradientType {
	#[default]
	Linear,
	Radial,
}

/// A gradient fill.
///
/// Contains the start and end points, along with the colors at varying points along the length.
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, DynAny, specta::Type)]
pub struct Gradient {
	pub start: DVec2,
	pub end: DVec2,
	pub transform: DAffine2,
	pub positions: Vec<(f64, Color)>,
	pub gradient_type: GradientType,
}

impl core::hash::Hash for Gradient {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.positions.len().hash(state);
		[].iter()
			.chain(self.start.to_array().iter())
			.chain(self.end.to_array().iter())
			.chain(self.transform.to_cols_array().iter())
			.chain(self.positions.iter().map(|(position, _)| position))
			.for_each(|x| x.to_bits().hash(state));
		self.positions.iter().for_each(|(_, color)| color.hash(state));
		self.gradient_type.hash(state);
	}
}

impl Gradient {
	/// Constructs a new gradient with the colors at 0 and 1 specified.
	pub fn new(start: DVec2, start_color: Color, end: DVec2, end_color: Color, transform: DAffine2, gradient_type: GradientType) -> Self {
		Gradient {
			start,
			end,
			positions: vec![(0., start_color), (1., end_color)],
			transform,
			gradient_type,
		}
	}

	pub fn lerp(&self, other: &Self, time: f64) -> Self {
		let start = self.start + (other.start - self.start) * time;
		let end = self.end + (other.end - self.end) * time;
		let transform = self.transform;
		let positions = self
			.positions
			.iter()
			.zip(other.positions.iter())
			.map(|((a_pos, a_color), (b_pos, b_color))| {
				let position = a_pos + (b_pos - a_pos) * time;
				let color = a_color.lerp(b_color, time as f32);
				(position, color)
			})
			.collect::<Vec<_>>();
		let gradient_type = if time < 0.5 { self.gradient_type } else { other.gradient_type };

		Self {
			start,
			end,
			transform,
			positions,
			gradient_type,
		}
	}

	/// Adds the gradient def through mutating the first argument, returning the gradient ID.
	fn render_defs(&self, svg_defs: &mut String, multiplied_transform: DAffine2, bounds: [DVec2; 2], transformed_bounds: [DVec2; 2]) -> u64 {
		let bound_transform = DAffine2::from_scale_angle_translation(bounds[1] - bounds[0], 0., bounds[0]);
		let transformed_bound_transform = DAffine2::from_scale_angle_translation(transformed_bounds[1] - transformed_bounds[0], 0., transformed_bounds[0]);
		let updated_transform = multiplied_transform * bound_transform;

		let mut positions = String::new();
		for (position, color) in self.positions.iter() {
			let _ = write!(positions, r##"<stop offset="{}" stop-color="#{}" />"##, position, color.with_alpha(color.a()).rgba_hex());
		}

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

		let gradient_id = crate::uuid::generate_uuid();
		match self.gradient_type {
			GradientType::Linear => {
				let _ = write!(
					svg_defs,
					r#"<linearGradient id="{}" x1="{}" x2="{}" y1="{}" y2="{}" gradientTransform="matrix({})">{}</linearGradient>"#,
					gradient_id, start.x, end.x, start.y, end.y, transform, positions
				);
			}
			GradientType::Radial => {
				let radius = (f64::powi(start.x - end.x, 2) + f64::powi(start.y - end.y, 2)).sqrt();
				let _ = write!(
					svg_defs,
					r#"<radialGradient id="{}" cx="{}" cy="{}" r="{}" gradientTransform="matrix({})">{}</radialGradient>"#,
					gradient_id, start.x, start.y, radius, transform, positions
				);
			}
		}

		gradient_id
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
		let get_color = |index: usize, time: f64| match (self.positions[index].1, self.positions.get(index + 1).map(|(_, c)| *c)) {
			// Lerp between the nearest colors if applicable
			(a, Some(b)) => a.lerp(
				&b,
				((time - self.positions[index].0) / self.positions.get(index + 1).map(|end| end.0 - self.positions[index].0).unwrap_or_default()) as f32,
			),
			// Use the start or the end color if applicable
			(v, _) => v,
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
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, DynAny, Hash, specta::Type)]
pub enum Fill {
	#[default]
	None,
	Solid(Color),
	Gradient(Gradient),
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
			Self::Gradient(Gradient { positions, .. }) => positions[0].1,
		}
	}

	pub fn lerp(&self, other: &Self, time: f64) -> Self {
		let transparent = Self::solid(Color::TRANSPARENT);
		let a = if *self == Self::None { &transparent } else { self };
		let b = if *other == Self::None { &transparent } else { other };

		match (a, b) {
			(Self::Solid(a), Self::Solid(b)) => Self::Solid(a.lerp(b, time as f32)),
			(Self::Solid(a), Self::Gradient(b)) => {
				let mut solid_to_gradient = b.clone();
				solid_to_gradient.positions.iter_mut().for_each(|(_, color)| *color = *a);
				let a = &solid_to_gradient;
				Self::Gradient(a.lerp(b, time))
			}
			(Self::Gradient(a), Self::Solid(b)) => {
				let mut gradient_to_solid = a.clone();
				gradient_to_solid.positions.iter_mut().for_each(|(_, color)| *color = *b);
				let b = &gradient_to_solid;
				Self::Gradient(a.lerp(b, time))
			}
			(Self::Gradient(a), Self::Gradient(b)) => Self::Gradient(a.lerp(b, time)),
			_ => Self::None,
		}
	}

	/// Renders the fill, adding necessary defs through mutating the first argument.
	pub fn render(&self, svg_defs: &mut String, multiplied_transform: DAffine2, bounds: [DVec2; 2], transformed_bounds: [DVec2; 2]) -> String {
		match self {
			Self::None => r#" fill="none""#.to_string(),
			Self::Solid(color) => format!(r##" fill="#{}"{}"##, color.rgb_hex(), format_opacity("fill-opacity", color.a())),
			Self::Gradient(gradient) => {
				let gradient_id = gradient.render_defs(svg_defs, multiplied_transform, bounds, transformed_bounds);
				format!(r##" fill="url('#{gradient_id}')""##)
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

/// Enum describing the type of [Fill]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, DynAny, Hash, specta::Type)]
pub enum FillType {
	Solid,
	Gradient,
}

/// The stroke (outline) style of an SVG element.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, DynAny, specta::Type)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, DynAny, specta::Type)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, DynAny, specta::Type)]
pub struct Stroke {
	/// Stroke color
	pub color: Option<Color>,
	/// Line thickness
	pub weight: f64,
	pub dash_lengths: Vec<f64>,
	pub dash_offset: f64,
	pub line_cap: LineCap,
	pub line_join: LineJoin,
	pub line_join_miter_limit: f64,
}

impl core::hash::Hash for Stroke {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.color.hash(state);
		self.weight.to_bits().hash(state);
		self.dash_lengths.len().hash(state);
		self.dash_lengths.iter().for_each(|length| length.to_bits().hash(state));
		self.dash_offset.to_bits().hash(state);
		self.line_cap.hash(state);
		self.line_join.hash(state);
		self.line_join_miter_limit.to_bits().hash(state);
	}
}

impl Stroke {
	pub const fn new(color: Option<Color>, weight: f64) -> Self {
		Self {
			color,
			weight,
			dash_lengths: Vec::new(),
			dash_offset: 0.,
			line_cap: LineCap::Butt,
			line_join: LineJoin::Miter,
			line_join_miter_limit: 4.,
		}
	}

	pub fn lerp(&self, other: &Self, time: f64) -> Self {
		Self {
			color: self.color.map(|color| color.lerp(&other.color.unwrap_or(color), time as f32)),
			weight: self.weight + (other.weight - self.weight) * time,
			dash_lengths: self.dash_lengths.iter().zip(other.dash_lengths.iter()).map(|(a, b)| a + (b - a) * time).collect(),
			dash_offset: self.dash_offset + (other.dash_offset - self.dash_offset) * time,
			line_cap: if time < 0.5 { self.line_cap } else { other.line_cap },
			line_join: if time < 0.5 { self.line_join } else { other.line_join },
			line_join_miter_limit: self.line_join_miter_limit + (other.line_join_miter_limit - self.line_join_miter_limit) * time,
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
		if self.dash_lengths.is_empty() {
			"none".to_string()
		} else {
			self.dash_lengths.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", ")
		}
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
				format_opacity("stroke-opacity", color.a()),
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
			.map(str::parse::<f64>)
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
			color: Some(Color::from_rgba8_srgb(0, 0, 0, 255)),
			dash_lengths: Vec::new(),
			dash_offset: 0.,
			line_cap: LineCap::Butt,
			line_join: LineJoin::Miter,
			line_join_miter_limit: 4.,
		}
	}
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, DynAny, specta::Type)]
pub struct PathStyle {
	stroke: Option<Stroke>,
	fill: Fill,
}

impl core::hash::Hash for PathStyle {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.stroke.hash(state);
		self.fill.hash(state);
	}
}

impl PathStyle {
	pub const fn new(stroke: Option<Stroke>, fill: Fill) -> Self {
		Self { stroke, fill }
	}

	pub fn lerp(&self, other: &Self, time: f64) -> Self {
		Self {
			fill: self.fill.lerp(&other.fill, time),
			stroke: match (self.stroke.as_ref(), other.stroke.as_ref()) {
				(Some(a), Some(b)) => Some(a.lerp(b, time)),
				(Some(a), None) => {
					if time < 0.5 {
						Some(a.clone())
					} else {
						None
					}
				}
				(None, Some(b)) => {
					if time < 0.5 {
						Some(b.clone())
					} else {
						None
					}
				}
				(None, None) => None,
			},
		}
	}

	/// Get the current path's [Fill].
	///
	/// # Example
	/// ```
	/// # use graphene_core::vector::style::{Fill, PathStyle};
	/// # use graphene_core::raster::color::Color;
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
	/// # use graphene_core::vector::style::{Fill, Stroke, PathStyle};
	/// # use graphene_core::raster::color::Color;
	/// let stroke = Stroke::new(Some(Color::GREEN), 42.);
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
	/// # use graphene_core::vector::style::{Fill, PathStyle};
	/// # use graphene_core::raster::color::Color;
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
	/// # use graphene_core::vector::style::{Stroke, PathStyle};
	/// # use graphene_core::raster::color::Color;
	/// let mut style = PathStyle::default();
	///
	/// assert_eq!(style.stroke(), None);
	///
	/// let stroke = Stroke::new(Some(Color::GREEN), 42.);
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
	/// # use graphene_core::vector::style::{Fill, PathStyle};
	/// # use graphene_core::raster::color::Color;
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
	/// # use graphene_core::vector::style::{Fill, Stroke, PathStyle};
	/// # use graphene_core::raster::color::Color;
	/// let mut style = PathStyle::new(Some(Stroke::new(Some(Color::GREEN), 42.)), Fill::None);
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

	/// Renders the shape's fill and stroke attributes as a string with them concatenated together.
	pub fn render(&self, view_mode: ViewMode, svg_defs: &mut String, multiplied_transform: DAffine2, bounds: [DVec2; 2], transformed_bounds: [DVec2; 2]) -> String {
		match view_mode {
			ViewMode::Outline => {
				let fill_attribute = Fill::None.render(svg_defs, multiplied_transform, bounds, transformed_bounds);
				let stroke_attribute = Stroke::new(Some(LAYER_OUTLINE_STROKE_COLOR), LAYER_OUTLINE_STROKE_WEIGHT).render();
				format!("{fill_attribute}{stroke_attribute}")
			}
			_ => {
				let fill_attribute = self.fill.render(svg_defs, multiplied_transform, bounds, transformed_bounds);
				let stroke_attribute = self.stroke.as_ref().map(|stroke| stroke.render()).unwrap_or_default();
				format!("{fill_attribute}{stroke_attribute}")
			}
		}
	}
}

/// Represents different ways of rendering an object
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Hash, DynAny, specta::Type)]
pub enum ViewMode {
	/// Render with normal coloration at the current viewport resolution
	#[default]
	Normal,
	/// Render only the outlines of shapes at the current viewport resolution
	Outline,
	/// Render with normal coloration at the document resolution, showing the pixels when the current viewport resolution is higher
	Pixels,
}
