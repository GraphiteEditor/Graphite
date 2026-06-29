//! Contains stylistic options for SVG elements.

pub use crate::gradient::*;
use core_types::Color;
use core_types::color::SRGBA8;
use core_types::transform::Transform;
use dyn_any::DynAny;
use glam::DAffine2;
use std::f64::consts::{PI, TAU};

/// Describes an editable fill choice, storing color or gradient stops without gradient placement metadata.
///
/// Can be None, a solid [Color], or a linear/radial [GradientStops].
///
/// In the future we'll probably also add a pattern fill.
///
/// Use [`FillChoiceUI`] at the JS boundary.
#[repr(C)]
#[derive(Default, Debug, Clone, PartialEq, graphene_hash::CacheHash, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FillChoice {
	#[default]
	None,
	Solid(Color),
	Gradient(GradientStops),
}

// TODO: Deprecate [`FillChoice`] and keep this, renamed, as the main widget-controlling type
/// JS-boundary version of [`FillChoice`] where the solid color is [`SRGBA8`] and the gradient is [`GradientStopsUI`].
#[cfg_attr(feature = "wasm", derive(tsify::Tsify), tsify(from_wasm_abi))]
#[derive(Default, Debug, Clone, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FillChoiceUI {
	#[default]
	None,
	Solid(SRGBA8),
	Gradient(GradientStopsUI),
}

impl From<&FillChoice> for FillChoiceUI {
	fn from(value: &FillChoice) -> Self {
		match value {
			FillChoice::None => Self::None,
			FillChoice::Solid(color) => Self::Solid(SRGBA8::from(*color)),
			FillChoice::Gradient(stops) => Self::Gradient(GradientStopsUI::from(stops)),
		}
	}
}

impl From<&FillChoiceUI> for FillChoice {
	fn from(value: &FillChoiceUI) -> Self {
		match value {
			FillChoiceUI::None => Self::None,
			FillChoiceUI::Solid(srgba) => Self::Solid(Color::from(*srgba)),
			FillChoiceUI::Gradient(stops) => Self::Gradient(GradientStops::from(stops)),
		}
	}
}

impl FillChoiceUI {
	pub fn as_solid(&self) -> Option<SRGBA8> {
		let Self::Solid(c) = self else { return None };
		Some(*c)
	}

	pub fn as_gradient(&self) -> Option<&GradientStopsUI> {
		let Self::Gradient(g) = self else { return None };
		Some(g)
	}

	/// Build a CSS `background-image` string (always a `linear-gradient(...)`) representing this fill, or `None` if the fill is [`FillChoiceUI::None`].
	/// Solid colors become a degenerate gradient between the same color so the CSS variable can always be assigned to a `background-image`.
	pub fn to_css_background_image(&self) -> Option<String> {
		match self {
			Self::None => None,
			Self::Solid(srgba) => {
				let hex = srgba.to_rgba_hex();
				Some(format!("linear-gradient(#{hex}, #{hex})"))
			}
			Self::Gradient(stops) => Some(stops.to_css_linear_gradient()),
		}
	}
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

	/// Build a CSS `background-image` string (always a `linear-gradient(...)`) representing this fill, or `None` if the fill is [`FillChoice::None`]. Solid colors become a degenerate gradient between the same color so the CSS variable can always be assigned to a `background-image`.
	pub fn to_css_background_image(&self) -> Option<String> {
		match self {
			Self::None => None,
			Self::Solid(color) => {
				let hex = SRGBA8::from(*color).to_rgba_hex();
				Some(format!("linear-gradient(#{hex}, #{hex})"))
			}
			Self::Gradient(stops) => Some(stops.to_css_linear_gradient()),
		}
	}
}

/// The stroke (outline) style of an SVG element.
#[repr(C)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, graphene_hash::CacheHash, DynAny, node_macro::ChoiceType)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[widget(Radio)]
pub enum StrokeCap {
	#[default]
	#[icon("StrokeCapButt")]
	Butt,
	#[icon("StrokeCapRound")]
	Round,
	#[icon("StrokeCapSquare")]
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
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, graphene_hash::CacheHash, DynAny, node_macro::ChoiceType)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[widget(Radio)]
pub enum StrokeJoin {
	#[default]
	#[icon("StrokeJoinMiter")]
	Miter,
	#[icon("StrokeJoinBevel")]
	Bevel,
	#[icon("StrokeJoinRound")]
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
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, graphene_hash::CacheHash, DynAny, node_macro::ChoiceType)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[widget(Radio)]
pub enum StrokeAlign {
	#[default]
	#[icon("StrokeAlignCenter")]
	Center,
	#[icon("StrokeAlignInside")]
	Inside,
	#[icon("StrokeAlignOutside")]
	Outside,
}

impl StrokeAlign {
	pub fn is_not_centered(self) -> bool {
		self != Self::Center
	}
}

#[repr(C)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, graphene_hash::CacheHash, DynAny, node_macro::ChoiceType)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[widget(Radio)]
pub enum PaintOrder {
	#[default]
	#[icon("StrokeOrderAbove")]
	StrokeAbove,
	#[icon("StrokeOrderBelow")]
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
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, PartialEq, graphene_hash::CacheHash, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct Stroke {
	/// Line thickness
	pub weight: f64,
	pub dash_lengths: Vec<f64>,
	pub dash_offset: f64,
	#[cfg_attr(feature = "serde", serde(alias = "line_cap"))]
	pub cap: StrokeCap,
	#[cfg_attr(feature = "serde", serde(alias = "line_join"))]
	pub join: StrokeJoin,
	#[cfg_attr(feature = "serde", serde(alias = "line_join_miter_limit"))]
	pub join_miter_limit: f64,
	#[cfg_attr(feature = "serde", serde(default))]
	pub align: StrokeAlign,
	#[cfg_attr(feature = "serde", serde(default = "daffine2_identity"))]
	pub transform: DAffine2,
	#[cfg_attr(feature = "serde", serde(default))]
	pub paint_order: PaintOrder,
}

impl Stroke {
	pub const fn new(weight: f64) -> Self {
		Self {
			weight,
			dash_lengths: Vec::new(),
			dash_offset: 0.,
			cap: StrokeCap::Butt,
			join: StrokeJoin::Miter,
			join_miter_limit: 4.,
			align: StrokeAlign::Center,
			transform: DAffine2::IDENTITY,
			paint_order: PaintOrder::StrokeAbove,
		}
	}

	pub fn lerp(&self, other: &Self, time: f64) -> Self {
		Self {
			weight: self.weight + (other.weight - self.weight) * time,
			dash_lengths: self.dash_lengths.iter().zip(other.dash_lengths.iter()).map(|(a, b)| a + (b - a) * time).collect(),
			dash_offset: self.dash_offset + (other.dash_offset - self.dash_offset) * time,
			cap: if time < 0.5 { self.cap } else { other.cap },
			join: if time < 0.5 { self.join } else { other.join },
			join_miter_limit: self.join_miter_limit + (other.join_miter_limit - self.join_miter_limit) * time,
			align: if time < 0.5 { self.align } else { other.align },
			transform: {
				// Decompose into scale/rotation/skew and interpolate each component separately.
				// We do this instead of linear matrix interpolation because that passes through a zero matrix
				// (and thus a division by 0 when rendering) when transforms have opposing rotations (e.g. 0° vs 180°).

				let (s_angle, s_scale, s_skew) = self.transform.decompose_rotation_scale_skew();
				let (t_angle, t_scale, t_skew) = other.transform.decompose_rotation_scale_skew();

				let lerp = |a: f64, b: f64| a + (b - a) * time;
				let lerped_translation = self.transform.translation * (1. - time) + other.transform.translation * time;

				// Shortest-arc rotation interpolation
				let mut rotation_diff = t_angle - s_angle;
				if rotation_diff > PI {
					rotation_diff -= TAU;
				} else if rotation_diff < -PI {
					rotation_diff += TAU;
				}
				let lerped_angle = s_angle + rotation_diff * time;

				let trs = DAffine2::from_scale_angle_translation(s_scale.lerp(t_scale, time), lerped_angle, lerped_translation);
				let skew = DAffine2::from_cols_array(&[1., 0., lerp(s_skew, t_skew), 1., 0., 0.]);
				trs * skew
			},
			paint_order: if time < 0.5 { self.paint_order } else { other.paint_order },
		}
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

	/// Worst-case upper bound on the perpendicular extent (per side) of the visible stroke from the path
	/// centerline, accounting for stroke alignment, miter join overshoot, and square cap diagonal extent.
	/// Used as a cheap, safe inflation amount for renderer clip rects so alignment compositing layers
	/// don't crop the actual stroke geometry. Constant-time — no path traversal.
	///
	/// `path_is_closed` indicates whether every subpath of the vector being measured is closed. The renderer
	/// only honors stroke alignment for fully-closed paths and falls back to drawing a Center-aligned
	/// `weight`-wide stroke otherwise, so callers must pass `false` when any subpath is open or an
	/// `Inside`-aligned stroke would silently get an inflation of `0` and crop at the blend layer.
	///
	/// Tight for round/bevel joins with butt/round caps. Otherwise overestimates: miter joins are assumed
	/// to reach the miter limit at every join (most don't), and square caps are assumed to sit at 45° to
	/// the axes (rarely the case). For an exact bound, use `Vector::stroke_inclusive_bounding_box_with_transform`
	/// at the cost of running kurbo to compute the stroke's outline path.
	pub fn max_aabb_inflation(&self, path_is_closed: bool) -> f64 {
		// Match the renderer: stroke alignment only applies to closed paths; open paths render as Center
		let half_width = if self.align != StrokeAlign::Center && path_is_closed {
			self.effective_width()
		} else {
			self.weight
		} * 0.5;
		let join_factor = if self.join == StrokeJoin::Miter { self.join_miter_limit.max(1.) } else { 1. };
		let cap_factor = if self.cap == StrokeCap::Square { core::f64::consts::SQRT_2 } else { 1. };
		half_width * join_factor.max(cap_factor)
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

	pub fn has_renderable_stroke(&self) -> bool {
		self.weight > 0.
	}
}

// Having an alpha of 1 to start with leads to a better experience with the properties panel
impl Default for Stroke {
	fn default() -> Self {
		Self {
			weight: 0.,
			dash_lengths: Vec::new(),
			dash_offset: 0.,
			cap: StrokeCap::Butt,
			join: StrokeJoin::Miter,
			join_miter_limit: 4.,
			align: StrokeAlign::Center,
			transform: DAffine2::IDENTITY,
			paint_order: PaintOrder::default(),
		}
	}
}

#[repr(C)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, PartialEq, Default, graphene_hash::CacheHash, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PathStyle {
	pub stroke: Option<Stroke>,
}

impl std::fmt::Display for PathStyle {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let stroke = match &self.stroke {
			Some(stroke) => format!("#(Weight: {} px)", stroke.weight),
			None => "None".to_string(),
		};

		write!(f, "Stroke: {stroke}")
	}
}

impl PathStyle {
	pub const fn new(stroke: Option<Stroke>) -> Self {
		Self { stroke }
	}

	/// Get the current path's [Stroke].
	///
	/// # Example
	/// ```
/// # use vector_types::vector::style::{Stroke, PathStyle};
/// let stroke = Stroke::new(42.);
/// let style = PathStyle::new(Some(stroke.clone()));
	///
	/// assert_eq!(style.stroke(), Some(stroke));
	/// ```
	pub fn stroke(&self) -> Option<Stroke> {
		self.stroke.clone()
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
	/// # use vector_types::vector::style::{Stroke, PathStyle};
	/// let mut style = PathStyle::default();
	///
	/// assert_eq!(style.stroke(), None);
	///
	/// let stroke = Stroke::new(42.);
	/// style.set_stroke(stroke.clone());
	///
	/// assert_eq!(style.stroke(), Some(stroke));
	/// ```
	pub fn set_stroke(&mut self, stroke: Stroke) {
		self.stroke = Some(stroke);
	}

	/// Set the path's stroke to None.
	///
	/// # Example
	/// ```
/// # use vector_types::vector::style::{Stroke, PathStyle};
/// let mut style = PathStyle::new(Some(Stroke::new(42.)));
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
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, graphene_hash::CacheHash, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RenderMode {
	/// Render with normal coloration at the current viewport resolution
	#[default]
	Normal = 0,
	/// Render only the outlines of shapes at the current viewport resolution
	Outline,
	/// Render with normal coloration at the document export resolution; at zoom > 100% this shows individual export pixels upscaled with nearest-neighbor filtering
	PixelPreview,
	/// Render a preview of how the object would be exported as an SVG.
	SvgPreview,
}
