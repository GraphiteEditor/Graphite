//! Contains stylistic options for SVG elements.

use crate::Color;
use crate::consts::{LAYER_OUTLINE_STROKE_COLOR, LAYER_OUTLINE_STROKE_WEIGHT};
use crate::renderer::{RenderParams, format_transform_matrix};
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};
use std::fmt::Write;

#[derive(Default, PartialEq, Eq, Clone, Copy, Debug, Hash, serde::Serialize, serde::Deserialize, DynAny, specta::Type, node_macro::ChoiceType)]
#[widget(Radio)]
pub enum GradientType {
	#[default]
	Linear,
	Radial,
}

// TODO: Someday we could switch this to a Box[T] to avoid over-allocation
// TODO: Use linear not gamma colors
/// A list of colors associated with positions (in the range 0 to 1) along a gradient.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, DynAny, specta::Type)]
pub struct GradientStops(Vec<(f64, Color)>);

impl std::hash::Hash for GradientStops {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.0.len().hash(state);
		self.0.iter().for_each(|(position, color)| {
			position.to_bits().hash(state);
			color.hash(state);
		});
	}
}

impl Default for GradientStops {
	fn default() -> Self {
		Self(vec![(0., Color::BLACK), (1., Color::WHITE)])
	}
}

impl IntoIterator for GradientStops {
	type Item = (f64, Color);
	type IntoIter = std::vec::IntoIter<(f64, Color)>;

	fn into_iter(self) -> Self::IntoIter {
		self.0.into_iter()
	}
}

impl<'a> IntoIterator for &'a GradientStops {
	type Item = &'a (f64, Color);
	type IntoIter = std::slice::Iter<'a, (f64, Color)>;

	fn into_iter(self) -> Self::IntoIter {
		self.0.iter()
	}
}

impl std::ops::Index<usize> for GradientStops {
	type Output = (f64, Color);

	fn index(&self, index: usize) -> &Self::Output {
		&self.0[index]
	}
}

impl std::ops::Deref for GradientStops {
	type Target = Vec<(f64, Color)>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl std::ops::DerefMut for GradientStops {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl GradientStops {
	pub fn new(stops: Vec<(f64, Color)>) -> Self {
		let mut stops = Self(stops);
		stops.sort();
		stops
	}

	pub fn evaluate(&self, t: f64) -> Color {
		if self.0.is_empty() {
			return Color::BLACK;
		}

		if t <= self.0[0].0 {
			return self.0[0].1;
		}
		if t >= self.0[self.0.len() - 1].0 {
			return self.0[self.0.len() - 1].1;
		}

		for i in 0..self.0.len() - 1 {
			let (t1, c1) = self.0[i];
			let (t2, c2) = self.0[i + 1];
			if t >= t1 && t <= t2 {
				let normalized_t = (t - t1) / (t2 - t1);
				return c1.lerp(&c2, normalized_t as f32);
			}
		}

		Color::BLACK
	}

	pub fn sort(&mut self) {
		self.0.sort_unstable_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
	}

	pub fn reversed(&self) -> Self {
		Self(self.0.iter().rev().map(|(position, color)| (1. - position, *color)).collect())
	}

	pub fn map_colors<F: Fn(&Color) -> Color>(&self, f: F) -> Self {
		Self(self.0.iter().map(|(position, color)| (*position, f(color))).collect())
	}
}

/// A gradient fill.
///
/// Contains the start and end points, along with the colors at varying points along the length.
#[repr(C)]
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, DynAny, specta::Type)]
pub struct Gradient {
	pub stops: GradientStops,
	pub gradient_type: GradientType,
	pub start: DVec2,
	pub end: DVec2,
	pub transform: DAffine2,
}

impl Default for Gradient {
	fn default() -> Self {
		Self {
			stops: GradientStops::default(),
			gradient_type: GradientType::Linear,
			start: DVec2::new(0., 0.5),
			end: DVec2::new(1., 0.5),
			transform: DAffine2::IDENTITY,
		}
	}
}

impl core::hash::Hash for Gradient {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.stops.0.len().hash(state);
		[].iter()
			.chain(self.start.to_array().iter())
			.chain(self.end.to_array().iter())
			.chain(self.transform.to_cols_array().iter())
			.chain(self.stops.0.iter().map(|(position, _)| position))
			.for_each(|x| x.to_bits().hash(state));
		self.stops.0.iter().for_each(|(_, color)| color.hash(state));
		self.gradient_type.hash(state);
	}
}

impl Gradient {
	/// Constructs a new gradient with the colors at 0 and 1 specified.
	pub fn new(start: DVec2, start_color: Color, end: DVec2, end_color: Color, transform: DAffine2, gradient_type: GradientType) -> Self {
		Gradient {
			start,
			end,
			stops: GradientStops::new(vec![(0., start_color.to_gamma_srgb()), (1., end_color.to_gamma_srgb())]),
			transform,
			gradient_type,
		}
	}

	pub fn lerp(&self, other: &Self, time: f64) -> Self {
		let start = self.start + (other.start - self.start) * time;
		let end = self.end + (other.end - self.end) * time;
		let transform = self.transform;
		let stops = self
			.stops
			.0
			.iter()
			.zip(other.stops.0.iter())
			.map(|((a_pos, a_color), (b_pos, b_color))| {
				let position = a_pos + (b_pos - a_pos) * time;
				let color = a_color.lerp(b_color, time as f32);
				(position, color)
			})
			.collect::<Vec<_>>();
		let stops = GradientStops::new(stops);
		let gradient_type = if time < 0.5 { self.gradient_type } else { other.gradient_type };

		Self {
			start,
			end,
			transform,
			stops,
			gradient_type,
		}
	}

	/// Adds the gradient def through mutating the first argument, returning the gradient ID.
	fn render_defs(&self, svg_defs: &mut String, element_transform: DAffine2, stroke_transform: DAffine2, bounds: [DVec2; 2], transformed_bounds: [DVec2; 2], _render_params: &RenderParams) -> u64 {
		// TODO: Figure out how to use `self.transform` as part of the gradient transform, since that field (`Gradient::transform`) is currently never read from, it's only written to.

		let bound_transform = DAffine2::from_scale_angle_translation(bounds[1] - bounds[0], 0., bounds[0]);
		let transformed_bound_transform = element_transform * DAffine2::from_scale_angle_translation(transformed_bounds[1] - transformed_bounds[0], 0., transformed_bounds[0]);

		let mut stop = String::new();
		for (position, color) in self.stops.0.iter() {
			stop.push_str("<stop");
			if *position != 0. {
				let _ = write!(stop, r#" offset="{}""#, (position * 1_000_000.).round() / 1_000_000.);
			}
			let _ = write!(stop, r##" stop-color="#{}""##, color.to_rgb_hex_srgb_from_gamma());
			if color.a() < 1. {
				let _ = write!(stop, r#" stop-opacity="{}""#, (color.a() * 1000.).round() / 1000.);
			}
			stop.push_str(" />")
		}

		let mod_gradient = if transformed_bound_transform.matrix2.determinant() != 0. {
			transformed_bound_transform.inverse()
		} else {
			DAffine2::IDENTITY // Ignore if the transform cannot be inverted (the bounds are zero). See issue #1944.
		};
		let mod_points = element_transform * stroke_transform * bound_transform;

		let start = mod_points.transform_point2(self.start);
		let end = mod_points.transform_point2(self.end);

		let gradient_id = crate::uuid::generate_uuid();

		let matrix = format_transform_matrix(mod_gradient);
		let gradient_transform = if matrix.is_empty() { String::new() } else { format!(r#" gradientTransform="{}""#, matrix) };

		match self.gradient_type {
			GradientType::Linear => {
				let _ = write!(
					svg_defs,
					r#"<linearGradient id="{}" x1="{}" x2="{}" y1="{}" y2="{}"{gradient_transform}>{}</linearGradient>"#,
					gradient_id, start.x, end.x, start.y, end.y, stop
				);
			}
			GradientType::Radial => {
				let radius = (f64::powi(start.x - end.x, 2) + f64::powi(start.y - end.y, 2)).sqrt();
				let _ = write!(
					svg_defs,
					r#"<radialGradient id="{}" cx="{}" cy="{}" r="{}"{gradient_transform}>{}</radialGradient>"#,
					gradient_id, start.x, start.y, radius, stop
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
		let new_position = ((end - start).angle_to(mouse - start)).cos() * start.distance(mouse) / start.distance(end);

		// Don't insert point past end of line
		if !(0. ..=1.).contains(&new_position) {
			return None;
		}

		// Compute the color of the inserted stop
		let get_color = |index: usize, time: f64| match (self.stops.0[index].1, self.stops.0.get(index + 1).map(|(_, c)| *c)) {
			// Lerp between the nearest colors if applicable
			(a, Some(b)) => a.lerp(
				&b,
				((time - self.stops.0[index].0) / self.stops.0.get(index + 1).map(|end| end.0 - self.stops.0[index].0).unwrap_or_default()) as f32,
			),
			// Use the start or the end color if applicable
			(v, _) => v,
		};

		// Compute the correct index to keep the positions in order
		let mut index = 0;
		while self.stops.0.len() > index && self.stops.0[index].0 <= new_position {
			index += 1;
		}

		let new_color = get_color(index - 1, new_position);

		// Insert the new stop
		self.stops.0.insert(index, (new_position, new_color));

		Some(index)
	}
}

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
	pub fn render(&self, svg_defs: &mut String, element_transform: DAffine2, stroke_transform: DAffine2, bounds: [DVec2; 2], transformed_bounds: [DVec2; 2], render_params: &RenderParams) -> String {
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
				let gradient_id = gradient.render_defs(svg_defs, element_transform, stroke_transform, bounds, transformed_bounds, render_params);
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
	pub fn render(&self, _render_params: &RenderParams) -> String {
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
	pub fn render(&self, svg_defs: &mut String, element_transform: DAffine2, stroke_transform: DAffine2, bounds: [DVec2; 2], transformed_bounds: [DVec2; 2], render_params: &RenderParams) -> String {
		let view_mode = render_params.view_mode;
		match view_mode {
			ViewMode::Outline => {
				let fill_attribute = Fill::None.render(svg_defs, element_transform, stroke_transform, bounds, transformed_bounds, render_params);
				let mut outline_stroke = Stroke::new(Some(LAYER_OUTLINE_STROKE_COLOR), LAYER_OUTLINE_STROKE_WEIGHT);
				// Outline strokes should be non-scaling by default
				outline_stroke.non_scaling = true;
				let stroke_attribute = outline_stroke.render(render_params);
				format!("{fill_attribute}{stroke_attribute}")
			}
			_ => {
				let fill_attribute = self.fill.render(svg_defs, element_transform, stroke_transform, bounds, transformed_bounds, render_params);
				let stroke_attribute = self.stroke.as_ref().map(|stroke| stroke.render(render_params)).unwrap_or_default();
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
