//! Contains stylistic options for SVG elements.

use crate::Color;
pub use crate::gradient::*;
use crate::table::Table;
use dyn_any::DynAny;
use glam::DAffine2;

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
			Self::Gradient(gradient) => write!(f, "{gradient}"),
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

	/// Find if fill can be represented with only opaque colors
	pub fn is_opaque(&self) -> bool {
		match self {
			Fill::Solid(color) => color.is_opaque(),
			Fill::Gradient(gradient) => gradient.stops.iter().all(|(_, color)| color.is_opaque()),
			Fill::None => true,
		}
	}

	/// Returns if fill is none
	pub fn is_none(&self) -> bool {
		*self == Self::None
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

impl From<Table<Color>> for Fill {
	fn from(color: Table<Color>) -> Fill {
		Fill::solid_or_none(color.into())
	}
}

impl From<Table<GradientStops>> for Fill {
	fn from(gradient: Table<GradientStops>) -> Fill {
		Fill::Gradient(Gradient {
			stops: gradient.iter().nth(0).map(|row| row.element.clone()).unwrap_or_default(),
			..Default::default()
		})
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
pub enum StrokeCap {
	#[default]
	Butt,
	Round,
	Square,
}

impl StrokeCap {
	pub fn svg_name(&self) -> &'static str {
		match self {
			StrokeCap::Butt => "butt",
			StrokeCap::Round => "round",
			StrokeCap::Square => "square",
		}
	}
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash, DynAny, specta::Type, node_macro::ChoiceType)]
#[widget(Radio)]
pub enum StrokeJoin {
	#[default]
	Miter,
	Bevel,
	Round,
}

impl StrokeJoin {
	pub fn svg_name(&self) -> &'static str {
		match self {
			StrokeJoin::Bevel => "bevel",
			StrokeJoin::Miter => "miter",
			StrokeJoin::Round => "round",
		}
	}
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash, DynAny, specta::Type, node_macro::ChoiceType)]
#[widget(Radio)]
pub enum StrokeAlign {
	#[default]
	Center,
	Inside,
	Outside,
}

impl StrokeAlign {
	pub fn is_not_centered(self) -> bool {
		self != Self::Center
	}
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash, DynAny, specta::Type, node_macro::ChoiceType)]
#[widget(Radio)]
pub enum PaintOrder {
	#[default]
	StrokeAbove,
	StrokeBelow,
}

impl PaintOrder {
	pub fn is_default(self) -> bool {
		self == Self::default()
	}
}

fn daffine2_identity() -> DAffine2 {
	DAffine2::IDENTITY
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, DynAny, specta::Type)]
#[serde(default)]
pub struct Stroke {
	/// Stroke color
	pub color: Option<Color>,
	/// Line thickness
	pub weight: f64,
	pub dash_lengths: Vec<f64>,
	pub dash_offset: f64,
	#[serde(alias = "line_cap")]
	pub cap: StrokeCap,
	#[serde(alias = "line_join")]
	pub join: StrokeJoin,
	#[serde(alias = "line_join_miter_limit")]
	pub join_miter_limit: f64,
	#[serde(default)]
	pub align: StrokeAlign,
	#[serde(default = "daffine2_identity")]
	pub transform: DAffine2,
	#[serde(default)]
	pub non_scaling: bool,
	#[serde(default)]
	pub paint_order: PaintOrder,
}

impl std::hash::Hash for Stroke {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.color.hash(state);
		self.weight.to_bits().hash(state);
		{
			self.dash_lengths.len().hash(state);
			self.dash_lengths.iter().for_each(|length| length.to_bits().hash(state));
		}
		self.dash_offset.to_bits().hash(state);
		self.cap.hash(state);
		self.join.hash(state);
		self.join_miter_limit.to_bits().hash(state);
		self.align.hash(state);
		self.transform.to_cols_array().iter().for_each(|x| x.to_bits().hash(state));
		self.non_scaling.hash(state);
		self.paint_order.hash(state);
	}
}

impl Stroke {
	pub const fn new(color: Option<Color>, weight: f64) -> Self {
		Self {
			color,
			weight,
			dash_lengths: Vec::new(),
			dash_offset: 0.,
			cap: StrokeCap::Butt,
			join: StrokeJoin::Miter,
			join_miter_limit: 4.,
			align: StrokeAlign::Center,
			transform: DAffine2::IDENTITY,
			non_scaling: false,
			paint_order: PaintOrder::StrokeAbove,
		}
	}

	pub fn lerp(&self, other: &Self, time: f64) -> Self {
		Self {
			color: self.color.map(|color| color.lerp(&other.color.unwrap_or(color), time as f32)),
			weight: self.weight + (other.weight - self.weight) * time,
			dash_lengths: self.dash_lengths.iter().zip(other.dash_lengths.iter()).map(|(a, b)| a + (b - a) * time).collect(),
			dash_offset: self.dash_offset + (other.dash_offset - self.dash_offset) * time,
			cap: if time < 0.5 { self.cap } else { other.cap },
			join: if time < 0.5 { self.join } else { other.join },
			join_miter_limit: self.join_miter_limit + (other.join_miter_limit - self.join_miter_limit) * time,
			align: if time < 0.5 { self.align } else { other.align },
			transform: DAffine2::from_mat2_translation(
				time * self.transform.matrix2 + (1. - time) * other.transform.matrix2,
				self.transform.translation * time + other.transform.translation * (1. - time),
			),
			non_scaling: if time < 0.5 { self.non_scaling } else { other.non_scaling },
			paint_order: if time < 0.5 { self.paint_order } else { other.paint_order },
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

	/// Get the effective stroke weight.
	pub fn effective_width(&self) -> f64 {
		self.weight
			* match self.align {
				StrokeAlign::Center => 1.,
				StrokeAlign::Inside => 0.,
				StrokeAlign::Outside => 2.,
			}
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

	pub fn cap_index(&self) -> u32 {
		self.cap as u32
	}

	pub fn join_index(&self) -> u32 {
		self.join as u32
	}

	pub fn join_miter_limit(&self) -> f32 {
		self.join_miter_limit as f32
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

	pub fn with_stroke_cap(mut self, stroke_cap: StrokeCap) -> Self {
		self.cap = stroke_cap;
		self
	}

	pub fn with_stroke_join(mut self, stroke_join: StrokeJoin) -> Self {
		self.join = stroke_join;
		self
	}

	pub fn with_stroke_join_miter_limit(mut self, limit: f64) -> Self {
		self.join_miter_limit = limit;
		self
	}

	pub fn with_stroke_align(mut self, stroke_align: StrokeAlign) -> Self {
		self.align = stroke_align;
		self
	}

	pub fn with_non_scaling(mut self, non_scaling: bool) -> Self {
		self.non_scaling = non_scaling;
		self
	}

	pub fn has_renderable_stroke(&self) -> bool {
		self.weight > 0. && self.color.is_some_and(|color| color.a() != 0.)
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
			cap: StrokeCap::Butt,
			join: StrokeJoin::Miter,
			join_miter_limit: 4.,
			align: StrokeAlign::Center,
			transform: DAffine2::IDENTITY,
			non_scaling: false,
			paint_order: PaintOrder::default(),
		}
	}
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize, DynAny, specta::Type)]
pub struct PathStyle {
	pub stroke: Option<Stroke>,
	pub fill: Fill,
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
}

/// Ways the user can choose to view the artwork in the viewport.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash, DynAny, specta::Type)]
pub enum RenderMode {
	/// Render with normal coloration at the current viewport resolution
	#[default]
	Normal = 0,
	/// Render only the outlines of shapes at the current viewport resolution
	Outline,
	// /// Render with normal coloration at the document resolution, showing the pixels when the current viewport resolution is higher
	// PixelPreview,
	// /// Render a preview of how the object would be exported as an SVG.
	// SvgPreview,
}
