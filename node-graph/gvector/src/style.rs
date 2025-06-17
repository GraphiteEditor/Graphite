//! Contains stylistic options for SVG elements.

use dyn_any::DynAny;
use glam::{DAffine2, DVec2};
use graphene_core::Color;
use graphene_core::consts::{LAYER_OUTLINE_STROKE_COLOR, LAYER_OUTLINE_STROKE_WEIGHT};
use graphene_core::gradient::{Gradient, format_transform_matrix};
use std::fmt::Write;

/// Describes the fill of a layer.
///
/// Can be None, a solid [Color], or a linear/radial [Gradient].
///
/// In the future we'll probably also add a pattern fill. This will probably be named "Paint" in the future.
#[repr(C)]
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, DynAny, Hash, specta::Type)]
pub enum Fill {
	#[default]
	None,
	Solid(Color),
	Gradient(Gradient),
}

impl std::fmt::Display for Fill {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::None => write!(f, "None"),
			Self::Solid(color) => write!(f, "#{} (Alpha: {}%)", color.to_rgb_hex_srgb(), color.a() * 100.),
			Self::Gradient(gradient) => write!(f, "{}", gradient),
		}
	}
}

impl Fill {
	/// Construct a new [Fill::Solid] from a [Color].
	pub fn solid(color: Color) -> Self {
		Self::Solid(color)
	}

	/// Construct a new [Fill::Solid] or [Fill::None] from an optional [Color].
	pub fn solid_or_none(color: Option<Color>) -> Self {
		match color {
			Some(color) => Self::Solid(color),
			None => Self::None,
		}
	}

	/// Evaluate the color at some point on the fill. Doesn't currently work for Gradient.
	pub fn color(&self) -> Color {
		match self {
			Self::None => Color::BLACK,
			Self::Solid(color) => *color,
			// TODO: Should correctly sample the gradient the equation here: https://svgwg.org/svg2-draft/pservers.html#Gradients
			Self::Gradient(Gradient { stops, .. }) => stops.0[0].1,
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
				solid_to_gradient.stops.0.iter_mut().for_each(|(_, color)| *color = *a);
				let a = &solid_to_gradient;
				Self::Gradient(a.lerp(b, time))
			}
			(Self::Gradient(a), Self::Solid(b)) => {
				let mut gradient_to_solid = a.clone();
				gradient_to_solid.stops.0.iter_mut().for_each(|(_, color)| *color = *b);
				let b = &gradient_to_solid;
				Self::Gradient(a.lerp(b, time))
			}
			(Self::Gradient(a), Self::Gradient(b)) => Self::Gradient(a.lerp(b, time)),
			_ => Self::None,
		}
	}

	/// Renders the fill, adding necessary defs through mutating the first argument.
	pub fn render(&self, svg_defs: &mut String, element_transform: DAffine2, stroke_transform: DAffine2, bounds: [DVec2; 2], transformed_bounds: [DVec2; 2]) -> String {
		match self {
			Self::None => r#" fill="none""#.to_string(),
			Self::Solid(color) => {
				let mut result = format!(r##" fill="#{}""##, color.to_rgb_hex_srgb_from_gamma());
				if color.a() < 1. {
					let _ = write!(result, r#" fill-opacity="{}""#, (color.a() * 1000.).round() / 1000.);
				}
				result
			}
			Self::Gradient(gradient) => {
				let gradient_id = gradient.render_defs(svg_defs, element_transform, stroke_transform, bounds, transformed_bounds);
				format!(r##" fill="url('#{gradient_id}')""##)
			}
		}
	}

	/// Extract a gradient from the fill
	pub fn as_gradient(&self) -> Option<&Gradient> {
		match self {
			Self::Gradient(gradient) => Some(gradient),
			_ => None,
		}
	}

	/// Extract a solid color from the fill
	pub fn as_solid(&self) -> Option<Color> {
		match self {
			Self::Solid(color) => Some(*color),
			_ => None,
		}
	}
}

impl From<Color> for Fill {
	fn from(color: Color) -> Fill {
		Fill::Solid(color)
	}
}

impl From<Option<Color>> for Fill {
	fn from(color: Option<Color>) -> Fill {
		Fill::solid_or_none(color)
	}
}

impl From<Gradient> for Fill {
	fn from(gradient: Gradient) -> Fill {
		Fill::Gradient(gradient)
	}
}

/// Describes the fill of a layer, but unlike [`Fill`], this doesn't store a [`Gradient`] directly but just its [`GradientStops`].
///
/// Can be None, a solid [Color], or a linear/radial [Gradient].
///
/// In the future we'll probably also add a pattern fill.
#[repr(C)]
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, DynAny, Hash, specta::Type)]
pub enum FillChoice {
	#[default]
	None,
	/// WARNING: Color is gamma, not linear!
	Solid(Color),
	/// WARNING: Color stops are gamma, not linear!
	Gradient(GradientStops),
}

impl FillChoice {
	pub fn as_solid(&self) -> Option<Color> {
		let Self::Solid(color) = self else { return None };
		Some(*color)
	}

	pub fn as_gradient(&self) -> Option<&GradientStops> {
		let Self::Gradient(gradient) = self else { return None };
		Some(gradient)
	}

	/// Convert this [`FillChoice`] to a [`Fill`] using the provided [`Gradient`] as a base for the positional information of the gradient.
	/// If a gradient isn't provided, default gradient positional information is used in cases where the [`FillChoice`] is a [`Gradient`].
	pub fn to_fill(&self, existing_gradient: Option<&Gradient>) -> Fill {
		match self {
			Self::None => Fill::None,
			Self::Solid(color) => Fill::Solid(*color),
			Self::Gradient(stops) => {
				let mut fill = existing_gradient.cloned().unwrap_or_default();
				fill.stops = stops.clone();
				Fill::Gradient(fill)
			}
		}
	}
}

impl From<Fill> for FillChoice {
	fn from(fill: Fill) -> Self {
		match fill {
			Fill::None => FillChoice::None,
			Fill::Solid(color) => FillChoice::Solid(color),
			Fill::Gradient(gradient) => FillChoice::Gradient(gradient.stops),
		}
	}
}

/// Enum describing the type of [Fill].
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize, DynAny, Hash, specta::Type, node_macro::ChoiceType)]
#[widget(Radio)]
pub enum FillType {
	#[default]
	Solid,
	Gradient,
}

/// The stroke (outline) style of an SVG element.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash, DynAny, specta::Type, node_macro::ChoiceType)]
#[widget(Radio)]
pub enum LineCap {
	#[default]
	Butt,
	Round,
	Square,
}

impl LineCap {
	fn svg_name(&self) -> &'static str {
		match self {
			LineCap::Butt => "butt",
			LineCap::Round => "round",
			LineCap::Square => "square",
		}
	}
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash, DynAny, specta::Type, node_macro::ChoiceType)]
#[widget(Radio)]
pub enum LineJoin {
	#[default]
	Miter,
	Bevel,
	Round,
}

impl LineJoin {
	fn svg_name(&self) -> &'static str {
		match self {
			LineJoin::Bevel => "bevel",
			LineJoin::Miter => "miter",
			LineJoin::Round => "round",
		}
	}
}

fn daffine2_identity() -> DAffine2 {
	DAffine2::IDENTITY
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, DynAny, specta::Type)]
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
	#[serde(default = "daffine2_identity")]
	pub transform: DAffine2,
	#[serde(default)]
	pub non_scaling: bool,
}

impl std::hash::Hash for Stroke {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.color.hash(state);
		self.weight.to_bits().hash(state);
		self.dash_lengths.len().hash(state);
		self.dash_lengths.iter().for_each(|length| length.to_bits().hash(state));
		self.dash_offset.to_bits().hash(state);
		self.line_cap.hash(state);
		self.line_join.hash(state);
		self.line_join_miter_limit.to_bits().hash(state);
		self.non_scaling.hash(state);
	}
}

impl From<Color> for Stroke {
	fn from(color: Color) -> Self {
		Self::new(Some(color), 1.)
	}
}
impl From<Option<Color>> for Stroke {
	fn from(color: Option<Color>) -> Self {
		Self::new(color, 1.)
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
			transform: DAffine2::IDENTITY,
			non_scaling: false,
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
			transform: DAffine2::from_mat2_translation(
				time * self.transform.matrix2 + (1. - time) * other.transform.matrix2,
				self.transform.translation * time + other.transform.translation * (1. - time),
			),
			non_scaling: if time < 0.5 { self.non_scaling } else { other.non_scaling },
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
		// Don't render a stroke at all if it would be invisible
		let Some(color) = self.color else { return String::new() };
		if self.weight <= 0. || color.a() == 0. {
			return String::new();
		}

		// Set to None if the value is the SVG default
		let weight = (self.weight != 1.).then_some(self.weight);
		let dash_array = (!self.dash_lengths.is_empty()).then_some(self.dash_lengths());
		let dash_offset = (self.dash_offset != 0.).then_some(self.dash_offset);
		let line_cap = (self.line_cap != LineCap::Butt).then_some(self.line_cap);
		let line_join = (self.line_join != LineJoin::Miter).then_some(self.line_join);
		let line_join_miter_limit = (self.line_join_miter_limit != 4.).then_some(self.line_join_miter_limit);

		// Render the needed stroke attributes
		let mut attributes = format!(r##" stroke="#{}""##, color.to_rgb_hex_srgb_from_gamma());
		if color.a() < 1. {
			let _ = write!(&mut attributes, r#" stroke-opacity="{}""#, (color.a() * 1000.).round() / 1000.);
		}
		if let Some(weight) = weight {
			let _ = write!(&mut attributes, r#" stroke-width="{}""#, weight);
		}
		if let Some(dash_array) = dash_array {
			let _ = write!(&mut attributes, r#" stroke-dasharray="{}""#, dash_array);
		}
		if let Some(dash_offset) = dash_offset {
			let _ = write!(&mut attributes, r#" stroke-dashoffset="{}""#, dash_offset);
		}
		if let Some(line_cap) = line_cap {
			let _ = write!(&mut attributes, r#" stroke-linecap="{}""#, line_cap.svg_name());
		}
		if let Some(line_join) = line_join {
			let _ = write!(&mut attributes, r#" stroke-linejoin="{}""#, line_join.svg_name());
		}
		if let Some(line_join_miter_limit) = line_join_miter_limit {
			let _ = write!(&mut attributes, r#" stroke-miterlimit="{}""#, line_join_miter_limit);
		}
		// Add vector-effect attribute to make strokes non-scaling
		if self.non_scaling {
			let _ = write!(&mut attributes, r#" vector-effect="non-scaling-stroke""#);
		}
		attributes
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

	pub fn with_non_scaling(mut self, non_scaling: bool) -> Self {
		self.non_scaling = non_scaling;
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
			transform: DAffine2::IDENTITY,
			non_scaling: false,
		}
	}
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize, DynAny, specta::Type)]
pub struct PathStyle {
	stroke: Option<Stroke>,
	fill: Fill,
}

impl std::hash::Hash for PathStyle {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.stroke.hash(state);
		self.fill.hash(state);
	}
}

impl std::fmt::Display for PathStyle {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let fill = &self.fill;

		let stroke = match &self.stroke {
			Some(stroke) => format!("#{} (Weight: {} px)", stroke.color.map_or("None".to_string(), |c| c.to_rgba_hex_srgb()), stroke.weight),
			None => "None".to_string(),
		};

		write!(f, "Fill: {fill}\nStroke: {stroke}")
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

	pub fn set_stroke_transform(&mut self, transform: DAffine2) {
		if let Some(stroke) = &mut self.stroke {
			stroke.transform = transform;
		}
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
	/// assert_ne!(*style.fill(), Fill::None);
	///
	/// style.clear_fill();
	///
	/// assert_eq!(*style.fill(), Fill::None);
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
	pub fn render(&self, view_mode: ViewMode, svg_defs: &mut String, element_transform: DAffine2, stroke_transform: DAffine2, bounds: [DVec2; 2], transformed_bounds: [DVec2; 2]) -> String {
		match view_mode {
			ViewMode::Outline => {
				let fill_attribute = Fill::None.render(svg_defs, element_transform, stroke_transform, bounds, transformed_bounds);
				let mut outline_stroke = Stroke::new(Some(LAYER_OUTLINE_STROKE_COLOR), LAYER_OUTLINE_STROKE_WEIGHT);
				// Outline strokes should be non-scaling by default
				outline_stroke.non_scaling = true;
				let stroke_attribute = outline_stroke.render();
				format!("{fill_attribute}{stroke_attribute}")
			}
			_ => {
				let fill_attribute = self.fill.render(svg_defs, element_transform, stroke_transform, bounds, transformed_bounds);
				let stroke_attribute = self.stroke.as_ref().map(|stroke| stroke.render()).unwrap_or_default();
				format!("{fill_attribute}{stroke_attribute}")
			}
		}
	}
}

/// Represents different ways of rendering an object
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash, DynAny, specta::Type)]
pub enum ViewMode {
	/// Render with normal coloration at the current viewport resolution
	#[default]
	Normal,
	/// Render only the outlines of shapes at the current viewport resolution
	Outline,
	/// Render with normal coloration at the document resolution, showing the pixels when the current viewport resolution is higher
	Pixels,
}
