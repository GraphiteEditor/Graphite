#![allow(clippy::too_many_arguments)]

#[cfg(feature = "alloc")]
use super::curve::{Curve, CurveManipulatorGroup, ValueMapperNode};
#[cfg(feature = "alloc")]
use super::ImageFrame;
use super::{Channel, Color, Node, Pixel, RGBMut};
use crate::registry::types::Percentage;
use crate::vector::style::GradientStops;
use crate::vector::VectorData;
use crate::GraphicGroup;

use dyn_any::{DynAny, StaticType};

use core::cmp::Ordering;
use core::fmt::Debug;
#[cfg(feature = "serde")]
#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::float::Float;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "std", derive(specta::Type))]
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, DynAny, Hash)]
pub enum LuminanceCalculation {
	#[default]
	SRGB,
	Perceptual,
	AverageChannels,
	MinimumChannels,
	MaximumChannels,
}

impl LuminanceCalculation {
	pub fn list() -> [LuminanceCalculation; 5] {
		[
			LuminanceCalculation::SRGB,
			LuminanceCalculation::Perceptual,
			LuminanceCalculation::AverageChannels,
			LuminanceCalculation::MinimumChannels,
			LuminanceCalculation::MaximumChannels,
		]
	}
}

impl core::fmt::Display for LuminanceCalculation {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			LuminanceCalculation::SRGB => write!(f, "sRGB"),
			LuminanceCalculation::Perceptual => write!(f, "Perceptual"),
			LuminanceCalculation::AverageChannels => write!(f, "Average Channels"),
			LuminanceCalculation::MinimumChannels => write!(f, "Minimum Channels"),
			LuminanceCalculation::MaximumChannels => write!(f, "Maximum Channels"),
		}
	}
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "std", derive(specta::Type))]
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, DynAny, Hash)]
#[repr(i32)] // TODO: Enable Int8 capability for SPIR-V so that we don't need this?
pub enum BlendMode {
	// Basic group
	#[default]
	Normal,

	// Darken group
	Darken,
	Multiply,
	ColorBurn,
	LinearBurn,
	DarkerColor,

	// Lighten group
	Lighten,
	Screen,
	ColorDodge,
	LinearDodge,
	LighterColor,

	// Contrast group
	Overlay,
	SoftLight,
	HardLight,
	VividLight,
	LinearLight,
	PinLight,
	HardMix,

	// Inversion group
	Difference,
	Exclusion,
	Subtract,
	Divide,

	// Component group
	Hue,
	Saturation,
	Color,
	Luminosity,

	// Other stuff
	Erase,
	Restore,
	MultiplyAlpha,
}

impl BlendMode {
	/// All standard blend modes ordered by group.
	pub fn list() -> [&'static [BlendMode]; 6] {
		use BlendMode::*;
		[
			// Normal group
			&[Normal],
			// Darken group
			&[Darken, Multiply, ColorBurn, LinearBurn, DarkerColor],
			// Lighten group
			&[Lighten, Screen, ColorDodge, LinearDodge, LighterColor],
			// Contrast group
			&[Overlay, SoftLight, HardLight, VividLight, LinearLight, PinLight, HardMix],
			// Inversion group
			&[Difference, Exclusion, Subtract, Divide],
			// Component group
			&[Hue, Saturation, Color, Luminosity],
		]
	}

	/// The subset of [`BlendMode::list()`] that is supported by SVG.
	pub fn list_svg_subset() -> [&'static [BlendMode]; 6] {
		use BlendMode::*;
		[
			// Normal group
			&[Normal],
			// Darken group
			&[Darken, Multiply, ColorBurn],
			// Lighten group
			&[Lighten, Screen, ColorDodge],
			// Contrast group
			&[Overlay, SoftLight, HardLight],
			// Inversion group
			&[Difference, Exclusion],
			// Component group
			&[Hue, Saturation, Color, Luminosity],
		]
	}

	pub fn index_in_list(&self) -> Option<usize> {
		Self::list().iter().flat_map(|x| x.iter()).position(|&blend_mode| blend_mode == *self)
	}

	pub fn index_in_list_svg_subset(&self) -> Option<usize> {
		Self::list_svg_subset().iter().flat_map(|x| x.iter()).position(|&blend_mode| blend_mode == *self)
	}

	/// Convert the enum to the CSS string for the blend mode.
	/// [Read more](https://developer.mozilla.org/en-US/docs/Web/CSS/blend-mode#values)
	pub fn to_svg_style_name(&self) -> Option<&'static str> {
		match self {
			// Normal group
			BlendMode::Normal => Some("normal"),
			// Darken group
			BlendMode::Darken => Some("darken"),
			BlendMode::Multiply => Some("multiply"),
			BlendMode::ColorBurn => Some("color-burn"),
			// Lighten group
			BlendMode::Lighten => Some("lighten"),
			BlendMode::Screen => Some("screen"),
			BlendMode::ColorDodge => Some("color-dodge"),
			// Contrast group
			BlendMode::Overlay => Some("overlay"),
			BlendMode::SoftLight => Some("soft-light"),
			BlendMode::HardLight => Some("hard-light"),
			// Inversion group
			BlendMode::Difference => Some("difference"),
			BlendMode::Exclusion => Some("exclusion"),
			// Component group
			BlendMode::Hue => Some("hue"),
			BlendMode::Saturation => Some("saturation"),
			BlendMode::Color => Some("color"),
			BlendMode::Luminosity => Some("luminosity"),
			_ => None,
		}
	}

	/// Renders the blend mode CSS style declaration.
	pub fn render(&self) -> String {
		format!(
			r#" mix-blend-mode: {};"#,
			self.to_svg_style_name().unwrap_or_else(|| {
				warn!("Unsupported blend mode {self:?}");
				"normal"
			})
		)
	}
}

impl core::fmt::Display for BlendMode {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			// Normal group
			BlendMode::Normal => write!(f, "Normal"),
			// Darken group
			BlendMode::Darken => write!(f, "Darken"),
			BlendMode::Multiply => write!(f, "Multiply"),
			BlendMode::ColorBurn => write!(f, "Color Burn"),
			BlendMode::LinearBurn => write!(f, "Linear Burn"),
			BlendMode::DarkerColor => write!(f, "Darker Color"),
			// Lighten group
			BlendMode::Lighten => write!(f, "Lighten"),
			BlendMode::Screen => write!(f, "Screen"),
			BlendMode::ColorDodge => write!(f, "Color Dodge"),
			BlendMode::LinearDodge => write!(f, "Linear Dodge"),
			BlendMode::LighterColor => write!(f, "Lighter Color"),
			// Contrast group
			BlendMode::Overlay => write!(f, "Overlay"),
			BlendMode::SoftLight => write!(f, "Soft Light"),
			BlendMode::HardLight => write!(f, "Hard Light"),
			BlendMode::VividLight => write!(f, "Vivid Light"),
			BlendMode::LinearLight => write!(f, "Linear Light"),
			BlendMode::PinLight => write!(f, "Pin Light"),
			BlendMode::HardMix => write!(f, "Hard Mix"),
			// Inversion group
			BlendMode::Difference => write!(f, "Difference"),
			BlendMode::Exclusion => write!(f, "Exclusion"),
			BlendMode::Subtract => write!(f, "Subtract"),
			BlendMode::Divide => write!(f, "Divide"),
			// Component group
			BlendMode::Hue => write!(f, "Hue"),
			BlendMode::Saturation => write!(f, "Saturation"),
			BlendMode::Color => write!(f, "Color"),
			BlendMode::Luminosity => write!(f, "Luminosity"),
			// Other utility blend modes (hidden from the normal list)
			BlendMode::Erase => write!(f, "Erase"),
			BlendMode::Restore => write!(f, "Restore"),
			BlendMode::MultiplyAlpha => write!(f, "Multiply Alpha"),
		}
	}
}

#[cfg(feature = "vello")]
impl From<BlendMode> for vello::peniko::Mix {
	fn from(val: BlendMode) -> Self {
		match val {
			// Normal group
			BlendMode::Normal => vello::peniko::Mix::Normal,
			// Darken group
			BlendMode::Darken => vello::peniko::Mix::Darken,
			BlendMode::Multiply => vello::peniko::Mix::Multiply,
			BlendMode::ColorBurn => vello::peniko::Mix::ColorBurn,
			// Lighten group
			BlendMode::Lighten => vello::peniko::Mix::Lighten,
			BlendMode::Screen => vello::peniko::Mix::Screen,
			BlendMode::ColorDodge => vello::peniko::Mix::ColorDodge,
			// Contrast group
			BlendMode::Overlay => vello::peniko::Mix::Overlay,
			BlendMode::SoftLight => vello::peniko::Mix::SoftLight,
			BlendMode::HardLight => vello::peniko::Mix::HardLight,
			// Inversion group
			BlendMode::Difference => vello::peniko::Mix::Difference,
			BlendMode::Exclusion => vello::peniko::Mix::Exclusion,
			// Component group
			BlendMode::Hue => vello::peniko::Mix::Hue,
			BlendMode::Saturation => vello::peniko::Mix::Saturation,
			BlendMode::Color => vello::peniko::Mix::Color,
			BlendMode::Luminosity => vello::peniko::Mix::Luminosity,
			_ => todo!(),
		}
	}
}

#[node_macro::new_node_fn(category("Adjustments"))]
fn luminance<T: Adjust<Color>>(
	_: (),
	#[expose]
	#[implementations(Color, ImageFrame<Color>)]
	mut input: T,
	luminance_calc: LuminanceCalculation,
) -> T {
	input.adjust(|color| {
		let luminance = match luminance_calc {
			LuminanceCalculation::SRGB => color.luminance_srgb(),
			LuminanceCalculation::Perceptual => color.luminance_perceptual(),
			LuminanceCalculation::AverageChannels => color.average_rgb_channels(),
			LuminanceCalculation::MinimumChannels => color.minimum_rgb_channels(),
			LuminanceCalculation::MaximumChannels => color.maximum_rgb_channels(),
		};
		color.map_rgb(|_| luminance)
	});
	input
}

#[node_macro::new_node_fn(category("Adjustments"))]
fn extract_channel<T: Adjust<Color>>(
	_: (),
	#[expose]
	#[implementations(Color, ImageFrame<Color>)]
	mut input: T,
	channel: RedGreenBlueAlpha,
) -> T {
	input.adjust(|color| {
		let extracted_value = match channel {
			RedGreenBlueAlpha::Red => color.r(),
			RedGreenBlueAlpha::Green => color.g(),
			RedGreenBlueAlpha::Blue => color.b(),
			RedGreenBlueAlpha::Alpha => color.a(),
		};
		color.map_rgba(|_| extracted_value)
	});
	input
}

#[node_macro::new_node_fn(category("Adjustments"))]
fn extract_opaque<T: Adjust<Color>>(
	_: (),
	#[expose]
	#[implementations(Color, ImageFrame<Color>)]
	mut input: T,
) -> T {
	input.adjust(|color| {
		if color.a() == 0. {
			return color.with_alpha(1.);
		}
		Color::from_rgbaf32(color.r() / color.a(), color.g() / color.a(), color.b() / color.a(), 1.).unwrap()
	});
	input
}

// From https://stackoverflow.com/questions/39510072/algorithm-for-adjustment-of-image-levels
#[node_macro::new_node_fn(category("Adjustments"))]
fn levels<T: Adjust<Color>>(
	_: (),
	#[expose]
	#[implementations(Color, ImageFrame<Color>)]
	mut input: T,
	input_start: f64,
	input_mid: f64,
	input_end: f64,
	output_start: f64,
	output_end: f64,
) -> T {
	input.adjust(|color| {
		let color = color.to_gamma_srgb();

		// Input Range (Range: 0-1)
		let input_shadows = (input_start / 100.) as f32;
		let input_midtones = (input_mid / 100.) as f32;
		let input_highlights = (input_end / 100.) as f32;

		// Output Range (Range: 0-1)
		let output_minimums = (output_start / 100.) as f32;
		let output_maximums = (output_end / 100.) as f32;

		// Midtones interpolation factor between minimums and maximums (Range: 0-1)
		let midtones = output_minimums + (output_maximums - output_minimums) * input_midtones;

		// Gamma correction (Range: 0.01-10)
		let gamma = if midtones < 0.5 {
			// Range: 0-1
			let x = 1. - midtones * 2.;
			// Range: 1-10
			1. + 9. * x
		} else {
			// Range: 0-0.5
			let x = 1. - midtones;
			// Range: 0-1
			let x = x * 2.;
			// Range: 0.01-1
			x.max(0.01)
		};

		// Input levels (Range: 0-1)
		let highlights_minus_shadows = (input_highlights - input_shadows).clamp(f32::EPSILON, 1.);
		let color = color.map_rgb(|c| ((c - input_shadows).max(0.) / highlights_minus_shadows).min(1.));

		// Midtones (Range: 0-1)
		let color = color.gamma(gamma);

		// Output levels (Range: 0-1)
		let color = color.map_rgb(|c| c * (output_maximums - output_minimums) + output_minimums);

		color.to_linear_srgb()
	});
	input
}

// From <https://stackoverflow.com/a/55233732/775283>
// Works the same for gamma and linear color
#[node_macro::new_node_fn(category("Adjustments"))]
fn black_and_white_color<T: Adjust<Color>>(
	_: (),
	#[expose]
	#[implementations(Color, ImageFrame<Color>)]
	mut input: T,
	tint: Color,
	reds: f64,
	yellows: f64,
	greens: f64,
	cyans: f64,
	blues: f64,
	magentas: f64,
) -> T {
	input.adjust(|color| {
		let color = color.to_gamma_srgb();

		let reds = reds as f32 / 100.;
		let yellows = yellows as f32 / 100.;
		let greens = greens as f32 / 100.;
		let cyans = cyans as f32 / 100.;
		let blues = blues as f32 / 100.;
		let magentas = magentas as f32 / 100.;

		let gray_base = color.r().min(color.g()).min(color.b());

		let red_part = color.r() - gray_base;
		let green_part = color.g() - gray_base;
		let blue_part = color.b() - gray_base;
		let alpha_part = color.a();

		let additional = if red_part == 0. {
			let cyan_part = green_part.min(blue_part);
			cyan_part * cyans + (green_part - cyan_part) * greens + (blue_part - cyan_part) * blues
		} else if green_part == 0. {
			let magenta_part = red_part.min(blue_part);
			magenta_part * magentas + (red_part - magenta_part) * reds + (blue_part - magenta_part) * blues
		} else {
			let yellow_part = red_part.min(green_part);
			yellow_part * yellows + (red_part - yellow_part) * reds + (green_part - yellow_part) * greens
		};

		let luminance = gray_base + additional;

		// TODO: Fix "Color" blend mode implementation so it matches the expected behavior perfectly (it's currently close)
		let color = tint.with_luminance(luminance);

		let color = Color::from_rgbaf32(color.r(), color.g(), color.b(), alpha_part).unwrap();

		color.to_linear_srgb()
	});
	input
}

#[node_macro::new_node_fn(category("Adjustments"))]
fn hue_shift<T: Adjust<Color>>(
	_: (),
	#[expose]
	#[implementations(Color, ImageFrame<Color>)]
	mut input: T,
	hue_shift: f64,
	saturation_shift: f64,
	lightness_shift: f64,
) -> T {
	input.adjust(|color| {
		let color = color.to_gamma_srgb();

		let [hue, saturation, lightness, alpha] = color.to_hsla();

		let color = Color::from_hsla(
			(hue + hue_shift as f32 / 360.) % 1.,
			// TODO: Improve the way saturation works (it's slightly off)
			(saturation + saturation_shift as f32 / 100.).clamp(0., 1.),
			// TODO: Fix the way lightness works (it's very off)
			(lightness + lightness_shift as f32 / 100.).clamp(0., 1.),
			alpha,
		);

		color.to_linear_srgb()
	});
	input
}

#[node_macro::new_node_fn(category("Adjustments"))]
fn invert<T: Adjust<Color>>(
	_: (),
	#[expose]
	#[implementations(Color, ImageFrame<Color>)]
	mut input: T,
) -> T {
	input.adjust(|color| {
		let color = color.to_gamma_srgb();

		let color = color.map_rgb(|c| color.a() - c);

		color.to_linear_srgb()
	});
	input
}

#[node_macro::new_node_fn(category("Adjustments"))]
fn threshold<T: Adjust<Color>>(
	_: (),
	#[expose]
	#[implementations(Color, ImageFrame<Color>)]
	mut input: T,
	min_luminance: f64,
	max_luminance: f64,
	luminance_calc: LuminanceCalculation,
) -> T {
	input.adjust(|color| {
		let min_luminance = Color::srgb_to_linear(min_luminance as f32 / 100.);
		let max_luminance = Color::srgb_to_linear(max_luminance as f32 / 100.);

		let luminance = match luminance_calc {
			LuminanceCalculation::SRGB => color.luminance_srgb(),
			LuminanceCalculation::Perceptual => color.luminance_perceptual(),
			LuminanceCalculation::AverageChannels => color.average_rgb_channels(),
			LuminanceCalculation::MinimumChannels => color.minimum_rgb_channels(),
			LuminanceCalculation::MaximumChannels => color.maximum_rgb_channels(),
		};

		if luminance >= min_luminance && luminance <= max_luminance {
			Color::WHITE
		} else {
			Color::BLACK
		}
	});
	input
}

trait Blend<P: Pixel> {
	fn blend(&self, under: &Self, blend_fn: impl Fn(P, P) -> P) -> Self;
}

impl Blend<Color> for Color {
	fn blend(&self, under: &Self, blend_fn: impl Fn(Color, Color) -> Color) -> Self {
		blend_fn(*self, *under)
	}
}
impl Blend<Color> for Option<Color> {
	fn blend(&self, under: &Self, blend_fn: impl Fn(Color, Color) -> Color) -> Self {
		match (self, under) {
			(Some(a), Some(b)) => Some(blend_fn(*a, *b)),
			(a, None) => *a,
			(None, b) => *b,
		}
	}
}

impl Blend<Color> for ImageFrame<Color> {
	fn blend(&self, under: &Self, blend_fn: impl Fn(Color, Color) -> Color) -> Self {
		let data = self.image.data.iter().zip(under.image.data.iter()).map(|(a, b)| blend_fn(*a, *b)).collect();

		ImageFrame {
			image: super::Image {
				data,
				width: self.image.width,
				height: self.image.height,
				base64_string: None,
			},
			transform: self.transform,
			alpha_blending: self.alpha_blending,
		}
	}
}

impl Blend<Color> for GradientStops {
	fn blend(&self, under: &Self, blend_fn: impl Fn(Color, Color) -> Color) -> Self {
		let mut combined_stops = self.0.iter().map(|(position, _)| position).chain(under.0.iter().map(|(position, _)| position)).collect::<Vec<_>>();
		combined_stops.dedup_by(|&mut a, &mut b| (a - b).abs() < 1e-6);
		combined_stops.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));

		let stops = combined_stops
			.into_iter()
			.map(|&position| {
				let over_color = self.evalute(position);
				let under_color = under.evalute(position);
				let color = blend_fn(over_color, under_color);
				(position, color)
			})
			.collect::<Vec<_>>();

		GradientStops(stops)
	}
}

#[node_macro::new_node_fn(category("Adjustments"))]
fn blend<T: Blend<Color>>(
	_: (),
	#[expose]
	#[implementations(Color, GradientStops, ImageFrame<Color>)]
	over: T,
	#[expose]
	#[implementations(Color, GradientStops, ImageFrame<Color>)]
	under: T,
	blend_mode: BlendMode,
	opacity: crate::registry::types::Percentage,
) -> T {
	Blend::blend(&over, &under, |a, b| blend_colors(a, b, blend_mode, opacity / 100.))
}

#[node_macro::new_node_fn(category("Adjustments"))]
fn blend_pair(input: (Color, Color), blend_mode: BlendMode, opacity: f64) -> Color {
	blend_colors(input.0, input.1, blend_mode, opacity / 100.)
}

pub fn apply_blend_mode(foreground: Color, background: Color, blend_mode: BlendMode) -> Color {
	match blend_mode {
		// Normal group
		BlendMode::Normal => background.blend_rgb(foreground, Color::blend_normal),
		// Darken group
		BlendMode::Darken => background.blend_rgb(foreground, Color::blend_darken),
		BlendMode::Multiply => background.blend_rgb(foreground, Color::blend_multiply),
		BlendMode::ColorBurn => background.blend_rgb(foreground, Color::blend_color_burn),
		BlendMode::LinearBurn => background.blend_rgb(foreground, Color::blend_linear_burn),
		BlendMode::DarkerColor => background.blend_darker_color(foreground),
		// Lighten group
		BlendMode::Lighten => background.blend_rgb(foreground, Color::blend_lighten),
		BlendMode::Screen => background.blend_rgb(foreground, Color::blend_screen),
		BlendMode::ColorDodge => background.blend_rgb(foreground, Color::blend_color_dodge),
		BlendMode::LinearDodge => background.blend_rgb(foreground, Color::blend_linear_dodge),
		BlendMode::LighterColor => background.blend_lighter_color(foreground),
		// Contrast group
		BlendMode::Overlay => foreground.blend_rgb(background, Color::blend_hardlight),
		BlendMode::SoftLight => background.blend_rgb(foreground, Color::blend_softlight),
		BlendMode::HardLight => background.blend_rgb(foreground, Color::blend_hardlight),
		BlendMode::VividLight => background.blend_rgb(foreground, Color::blend_vivid_light),
		BlendMode::LinearLight => background.blend_rgb(foreground, Color::blend_linear_light),
		BlendMode::PinLight => background.blend_rgb(foreground, Color::blend_pin_light),
		BlendMode::HardMix => background.blend_rgb(foreground, Color::blend_hard_mix),
		// Inversion group
		BlendMode::Difference => background.blend_rgb(foreground, Color::blend_difference),
		BlendMode::Exclusion => background.blend_rgb(foreground, Color::blend_exclusion),
		BlendMode::Subtract => background.blend_rgb(foreground, Color::blend_subtract),
		BlendMode::Divide => background.blend_rgb(foreground, Color::blend_divide),
		// Component group
		BlendMode::Hue => background.blend_hue(foreground),
		BlendMode::Saturation => background.blend_saturation(foreground),
		BlendMode::Color => background.blend_color(foreground),
		BlendMode::Luminosity => background.blend_luminosity(foreground),
		// Other utility blend modes (hidden from the normal list) - do not have alpha blend
		_ => panic!("Used blend mode without alpha blend"),
	}
}

trait Adjust<C> {
	fn adjust(&mut self, map_fn: impl Fn(&C) -> C);
}
impl Adjust<Color> for Color {
	fn adjust(&mut self, map_fn: impl Fn(&Color) -> Color) {
		*self = map_fn(self);
	}
}
impl Adjust<Color> for Option<Color> {
	fn adjust(&mut self, map_fn: impl Fn(&Color) -> Color) {
		match self {
			Some(ref mut v) => *v = map_fn(v),
			None => (),
		}
	}
}
impl Adjust<Color> for GradientStops {
	fn adjust(&mut self, map_fn: impl Fn(&Color) -> Color) {
		for (_pos, c) in self.0.iter_mut() {
			*c = map_fn(c);
		}
	}
}
impl<C: Pixel> Adjust<C> for ImageFrame<C> {
	fn adjust(&mut self, map_fn: impl Fn(&C) -> C) {
		for c in self.image.data.iter_mut() {
			*c = map_fn(c);
		}
	}
}

#[inline(always)]
pub fn blend_colors(foreground: Color, background: Color, blend_mode: BlendMode, opacity: f64) -> Color {
	let target_color = match blend_mode {
		// Other utility blend modes (hidden from the normal list) - do not have alpha blend
		BlendMode::Erase => return background.alpha_subtract(foreground),
		BlendMode::Restore => return background.alpha_add(foreground),
		BlendMode::MultiplyAlpha => return background.alpha_multiply(foreground),
		blend_mode => apply_blend_mode(foreground, background, blend_mode),
	};

	background.alpha_blend(target_color.to_associated_alpha(opacity as f32))
}

#[node_macro::new_node_fn(category("Adjustments"))]
fn gradient_map_node(_: (), color: Color, gradient: GradientStops, reverse: bool) -> Color {
	let intensity = color.luminance_srgb();
	let intensity = if reverse { 1. - intensity } else { intensity };
	gradient.evalute(intensity as f64)
}

// Based on <https://stackoverflow.com/questions/33966121/what-is-the-algorithm-for-vibrance-filters>
// The results of this implementation are very close to correct, but not quite perfect
#[node_macro::new_node_fn(category("Adjustments"))]
fn vibrance_node<T: Adjust<Color>>(
	_: (),
	#[expose]
	#[implementations(Color, ImageFrame<Color>)]
	mut input: T,
	vibrance: f64,
) -> T {
	input.adjust(|color| {
		let vibrance = vibrance as f32 / 100.;
		// Slow the effect down by half when it's negative, since artifacts begin appearing past -50%.
		// So this scales the 0% to -50% range to 0% to -100%.
		let slowed_vibrance = if vibrance >= 0. { vibrance } else { vibrance * 0.5 };

		let channel_max = color.r().max(color.g()).max(color.b());
		let channel_min = color.r().min(color.g()).min(color.b());
		let channel_difference = channel_max - channel_min;

		let scale_multiplier = if channel_max == color.r() {
			let green_blue_difference = (color.g() - color.b()).abs();
			let t = (green_blue_difference / channel_difference).min(1.);
			t * 0.5 + 0.5
		} else {
			1.
		};
		let scale = slowed_vibrance * scale_multiplier * (2. - channel_difference);
		let channel_reduction = channel_min * scale;
		let scale = 1. + scale * (1. - channel_difference);

		let luminance_initial = color.to_linear_srgb().luminance_srgb();
		let altered_color = color.map_rgb(|c| c * scale - channel_reduction).to_linear_srgb();
		let luminance = altered_color.luminance_srgb();
		let altered_color = altered_color.map_rgb(|c| c * luminance_initial / luminance);

		let channel_max = altered_color.r().max(altered_color.g()).max(altered_color.b());
		let altered_color = if Color::linear_to_srgb(channel_max) > 1. {
			let scale = (1. - luminance) / (channel_max - luminance);
			altered_color.map_rgb(|c| (c - luminance) * scale + luminance)
		} else {
			altered_color
		};
		let altered_color = altered_color.to_gamma_srgb();

		if vibrance >= 0. {
			altered_color
		} else {
			// TODO: The result ends up a bit darker than it should be, further investigation is needed
			let luminance = color.luminance_rec_601();

			// Near -0% vibrance we mostly use `altered_color`.
			// Near -100% vibrance, we mostly use half the desaturated luminance color and half `altered_color`.
			let factor = -slowed_vibrance;
			altered_color.map_rgb(|c| c * (1. - factor) + luminance * factor)
		}
	});
	input
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "std", derive(specta::Type))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, DynAny)]
pub enum RedGreenBlue {
	#[default]
	Red,
	Green,
	Blue,
}

impl core::fmt::Display for RedGreenBlue {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			RedGreenBlue::Red => write!(f, "Red"),
			RedGreenBlue::Green => write!(f, "Green"),
			RedGreenBlue::Blue => write!(f, "Blue"),
		}
	}
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "std", derive(specta::Type))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, DynAny)]
pub enum RedGreenBlueAlpha {
	#[default]
	Red,
	Green,
	Blue,
	Alpha,
}

impl core::fmt::Display for RedGreenBlueAlpha {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			RedGreenBlueAlpha::Red => write!(f, "Red"),
			RedGreenBlueAlpha::Green => write!(f, "Green"),
			RedGreenBlueAlpha::Blue => write!(f, "Blue"),
			RedGreenBlueAlpha::Alpha => write!(f, "Alpha"),
		}
	}
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "std", derive(specta::Type))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, DynAny)]
pub enum NoiseType {
	#[default]
	Perlin,
	OpenSimplex2,
	OpenSimplex2S,
	Cellular,
	ValueCubic,
	Value,
	WhiteNoise,
}

impl core::fmt::Display for NoiseType {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			NoiseType::Perlin => write!(f, "Perlin"),
			NoiseType::OpenSimplex2 => write!(f, "OpenSimplex2"),
			NoiseType::OpenSimplex2S => write!(f, "OpenSimplex2S"),
			NoiseType::Cellular => write!(f, "Cellular"),
			NoiseType::ValueCubic => write!(f, "Value Cubic"),
			NoiseType::Value => write!(f, "Value"),
			NoiseType::WhiteNoise => write!(f, "White Noise"),
		}
	}
}

impl NoiseType {
	pub fn list() -> &'static [NoiseType; 7] {
		&[
			NoiseType::Perlin,
			NoiseType::OpenSimplex2,
			NoiseType::OpenSimplex2S,
			NoiseType::Cellular,
			NoiseType::ValueCubic,
			NoiseType::Value,
			NoiseType::WhiteNoise,
		]
	}
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "std", derive(specta::Type))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, DynAny)]
pub enum FractalType {
	#[default]
	None,
	FBm,
	Ridged,
	PingPong,
	DomainWarpProgressive,
	DomainWarpIndependent,
}

impl core::fmt::Display for FractalType {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			FractalType::None => write!(f, "None"),
			FractalType::FBm => write!(f, "Fractional Brownian Motion"),
			FractalType::Ridged => write!(f, "Ridged"),
			FractalType::PingPong => write!(f, "Ping Pong"),
			FractalType::DomainWarpProgressive => write!(f, "Progressive (Domain Warp Only)"),
			FractalType::DomainWarpIndependent => write!(f, "Independent (Domain Warp Only)"),
		}
	}
}

impl FractalType {
	pub fn list() -> &'static [FractalType; 6] {
		&[
			FractalType::None,
			FractalType::FBm,
			FractalType::Ridged,
			FractalType::PingPong,
			FractalType::DomainWarpProgressive,
			FractalType::DomainWarpIndependent,
		]
	}
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "std", derive(specta::Type))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, DynAny)]
pub enum CellularDistanceFunction {
	#[default]
	Euclidean,
	EuclideanSq,
	Manhattan,
	Hybrid,
}

impl core::fmt::Display for CellularDistanceFunction {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			CellularDistanceFunction::Euclidean => write!(f, "Euclidean"),
			CellularDistanceFunction::EuclideanSq => write!(f, "Euclidean Squared (Faster)"),
			CellularDistanceFunction::Manhattan => write!(f, "Manhattan"),
			CellularDistanceFunction::Hybrid => write!(f, "Hybrid"),
		}
	}
}

impl CellularDistanceFunction {
	pub fn list() -> &'static [CellularDistanceFunction; 4] {
		&[
			CellularDistanceFunction::Euclidean,
			CellularDistanceFunction::EuclideanSq,
			CellularDistanceFunction::Manhattan,
			CellularDistanceFunction::Hybrid,
		]
	}
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "std", derive(specta::Type))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, DynAny)]
pub enum CellularReturnType {
	CellValue,
	#[default]
	Nearest,
	NextNearest,
	Average,
	Difference,
	Product,
	Division,
}

impl core::fmt::Display for CellularReturnType {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			CellularReturnType::CellValue => write!(f, "Cell Value"),
			CellularReturnType::Nearest => write!(f, "Nearest (F1)"),
			CellularReturnType::NextNearest => write!(f, "Next Nearest (F2)"),
			CellularReturnType::Average => write!(f, "Average (F1 / 2 + F2 / 2)"),
			CellularReturnType::Difference => write!(f, "Difference (F2 - F1)"),
			CellularReturnType::Product => write!(f, "Product (F2 * F1 / 2)"),
			CellularReturnType::Division => write!(f, "Division (F1 / F2)"),
		}
	}
}

impl CellularReturnType {
	pub fn list() -> &'static [CellularReturnType; 7] {
		&[
			CellularReturnType::CellValue,
			CellularReturnType::Nearest,
			CellularReturnType::NextNearest,
			CellularReturnType::Average,
			CellularReturnType::Difference,
			CellularReturnType::Product,
			CellularReturnType::Division,
		]
	}
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "std", derive(specta::Type))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, DynAny)]
pub enum DomainWarpType {
	#[default]
	None,
	OpenSimplex2,
	OpenSimplex2Reduced,
	BasicGrid,
}

impl core::fmt::Display for DomainWarpType {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			DomainWarpType::None => write!(f, "None"),
			DomainWarpType::OpenSimplex2 => write!(f, "OpenSimplex2"),
			DomainWarpType::OpenSimplex2Reduced => write!(f, "OpenSimplex2 Reduced"),
			DomainWarpType::BasicGrid => write!(f, "Basic Grid"),
		}
	}
}

impl DomainWarpType {
	pub fn list() -> &'static [DomainWarpType; 4] {
		&[DomainWarpType::None, DomainWarpType::OpenSimplex2, DomainWarpType::OpenSimplex2Reduced, DomainWarpType::BasicGrid]
	}
}

#[node_macro::new_node_fn(category("Adjustments"))]
fn channel_mixer_node<T: Adjust<Color>>(
	_: (),
	#[expose]
	#[implementations(Color, ImageFrame<Color>)]
	mut input: T,
	monochrome: bool,
	monochrome_r: f64,
	monochrome_g: f64,
	monochrome_b: f64,
	monochrome_c: f64,
	red_r: f64,
	red_g: f64,
	red_b: f64,
	red_c: f64,
	green_r: f64,
	green_g: f64,
	green_b: f64,
	green_c: f64,
	blue_r: f64,
	blue_g: f64,
	blue_b: f64,
	blue_c: f64,
) -> T {
	input.adjust(|color| {
		let color = color.to_gamma_srgb();

		let (r, g, b, a) = color.components();

		let color = if monochrome {
			let (monochrome_r, monochrome_g, monochrome_b, monochrome_c) = (monochrome_r as f32 / 100., monochrome_g as f32 / 100., monochrome_b as f32 / 100., monochrome_c as f32 / 100.);

			let gray = (r * monochrome_r + g * monochrome_g + b * monochrome_b + monochrome_c).clamp(0., 1.);

			Color::from_rgbaf32_unchecked(gray, gray, gray, a)
		} else {
			let (red_r, red_g, red_b, red_c) = (red_r as f32 / 100., red_g as f32 / 100., red_b as f32 / 100., red_c as f32 / 100.);
			let (green_r, green_g, green_b, green_c) = (green_r as f32 / 100., green_g as f32 / 100., green_b as f32 / 100., green_c as f32 / 100.);
			let (blue_r, blue_g, blue_b, blue_c) = (blue_r as f32 / 100., blue_g as f32 / 100., blue_b as f32 / 100., blue_c as f32 / 100.);

			let red = (r * red_r + g * red_g + b * red_b + red_c).clamp(0., 1.);
			let green = (r * green_r + g * green_g + b * green_b + green_c).clamp(0., 1.);
			let blue = (r * blue_r + g * blue_g + b * blue_b + blue_c).clamp(0., 1.);

			Color::from_rgbaf32_unchecked(red, green, blue, a)
		};

		color.to_linear_srgb()
	});
	input
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "std", derive(specta::Type))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, DynAny)]
pub enum RelativeAbsolute {
	#[default]
	Relative,
	Absolute,
}

impl core::fmt::Display for RelativeAbsolute {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			RelativeAbsolute::Relative => write!(f, "Relative"),
			RelativeAbsolute::Absolute => write!(f, "Absolute"),
		}
	}
}

#[repr(C)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "std", derive(specta::Type))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, DynAny)]
pub enum SelectiveColorChoice {
	#[default]
	Reds,
	Yellows,
	Greens,
	Cyans,
	Blues,
	Magentas,
	Whites,
	Neutrals,
	Blacks,
}

impl core::fmt::Display for SelectiveColorChoice {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			SelectiveColorChoice::Reds => write!(f, "Reds"),
			SelectiveColorChoice::Yellows => write!(f, "Yellows"),
			SelectiveColorChoice::Greens => write!(f, "Greens"),
			SelectiveColorChoice::Cyans => write!(f, "Cyans"),
			SelectiveColorChoice::Blues => write!(f, "Blues"),
			SelectiveColorChoice::Magentas => write!(f, "Magentas"),
			SelectiveColorChoice::Whites => write!(f, "Whites"),
			SelectiveColorChoice::Neutrals => write!(f, "Neutrals"),
			SelectiveColorChoice::Blacks => write!(f, "Blacks"),
		}
	}
}

// Based on https://blog.pkh.me/p/22-understanding-selective-coloring-in-adobe-photoshop.html
#[node_macro::new_node_fn(category("Adjustments"))]
fn selective_color<T: Adjust<Color>>(
	_: (),
	#[expose]
	#[implementations(Color, ImageFrame<Color>)]
	mut input: T,
	mode: RelativeAbsolute,
	r_c: f64,
	r_m: f64,
	r_y: f64,
	r_k: f64,
	y_c: f64,
	y_m: f64,
	y_y: f64,
	y_k: f64,
	g_c: f64,
	g_m: f64,
	g_y: f64,
	g_k: f64,
	c_c: f64,
	c_m: f64,
	c_y: f64,
	c_k: f64,
	b_c: f64,
	b_m: f64,
	b_y: f64,
	b_k: f64,
	m_c: f64,
	m_m: f64,
	m_y: f64,
	m_k: f64,
	w_c: f64,
	w_m: f64,
	w_y: f64,
	w_k: f64,
	n_c: f64,
	n_m: f64,
	n_y: f64,
	n_k: f64,
	k_c: f64,
	k_m: f64,
	k_y: f64,
	k_k: f64,
) -> T {
	input.adjust(|color| {
		let color = color.to_gamma_srgb();

		let (r, g, b, a) = color.components();

		let min = |a: f32, b: f32, c: f32| a.min(b).min(c);
		let max = |a: f32, b: f32, c: f32| a.max(b).max(c);
		let med = |a: f32, b: f32, c: f32| a + b + c - min(a, b, c) - max(a, b, c);

		let max_channel = max(r, g, b);
		let min_channel = min(r, g, b);

		let pixel_color_range = |choice| match choice {
			SelectiveColorChoice::Reds => max_channel == r,
			SelectiveColorChoice::Yellows => min_channel == b,
			SelectiveColorChoice::Greens => max_channel == g,
			SelectiveColorChoice::Cyans => min_channel == r,
			SelectiveColorChoice::Blues => max_channel == b,
			SelectiveColorChoice::Magentas => min_channel == g,
			SelectiveColorChoice::Whites => r > 0.5 && g > 0.5 && b > 0.5,
			SelectiveColorChoice::Neutrals => r > 0. && g > 0. && b > 0. && r < 1. && g < 1. && b < 1.,
			SelectiveColorChoice::Blacks => r < 0.5 && g < 0.5 && b < 0.5,
		};

		let color_parameter_group_scale_factor_rgb = max(r, g, b) - med(r, g, b);
		let color_parameter_group_scale_factor_cmy = med(r, g, b) - min(r, g, b);

		// Used to apply the r, g, or b channel slope (by multiplying it by 1) in relative mode, or no slope (by multiplying it by 0) in absolute mode
		let (slope_r, slope_g, slope_b) = match mode {
			RelativeAbsolute::Relative => (r - 1., g - 1., b - 1.),
			RelativeAbsolute::Absolute => (-1., -1., -1.),
		};

		let (sum_r, sum_g, sum_b) = [
			(SelectiveColorChoice::Reds, (r_c as f32, r_m as f32, r_y as f32, r_k as f32)),
			(SelectiveColorChoice::Yellows, (y_c as f32, y_m as f32, y_y as f32, y_k as f32)),
			(SelectiveColorChoice::Greens, (g_c as f32, g_m as f32, g_y as f32, g_k as f32)),
			(SelectiveColorChoice::Cyans, (c_c as f32, c_m as f32, c_y as f32, c_k as f32)),
			(SelectiveColorChoice::Blues, (b_c as f32, b_m as f32, b_y as f32, b_k as f32)),
			(SelectiveColorChoice::Magentas, (m_c as f32, m_m as f32, m_y as f32, m_k as f32)),
			(SelectiveColorChoice::Whites, (w_c as f32, w_m as f32, w_y as f32, w_k as f32)),
			(SelectiveColorChoice::Neutrals, (n_c as f32, n_m as f32, n_y as f32, n_k as f32)),
			(SelectiveColorChoice::Blacks, (k_c as f32, k_m as f32, k_y as f32, k_k as f32)),
		]
		.into_iter()
		.fold((0., 0., 0.), |acc, (color_parameter_group, (c, m, y, k))| {
			// Skip this color parameter group...
			// ...if it's unchanged from the default of zero offset on all CMYK parameters, or...
			// ...if this pixel's color isn't in the range affected by this color parameter group
			if (c < f32::EPSILON && m < f32::EPSILON && y < f32::EPSILON && k < f32::EPSILON) || (!pixel_color_range(color_parameter_group)) {
				return acc;
			}

			let (c, m, y, k) = (c / 100., m / 100., y / 100., k / 100.);

			let color_parameter_group_scale_factor = match color_parameter_group {
				SelectiveColorChoice::Reds | SelectiveColorChoice::Greens | SelectiveColorChoice::Blues => color_parameter_group_scale_factor_rgb,
				SelectiveColorChoice::Cyans | SelectiveColorChoice::Magentas | SelectiveColorChoice::Yellows => color_parameter_group_scale_factor_cmy,
				SelectiveColorChoice::Whites => min(r, g, b) * 2. - 1.,
				SelectiveColorChoice::Neutrals => 1. - ((max(r, g, b) - 0.5).abs() + (min(r, g, b) - 0.5).abs()),
				SelectiveColorChoice::Blacks => 1. - max(r, g, b) * 2.,
			};

			let offset_r = ((c + k * (c + 1.)) * slope_r).clamp(-r, -r + 1.) * color_parameter_group_scale_factor;
			let offset_g = ((m + k * (m + 1.)) * slope_g).clamp(-g, -g + 1.) * color_parameter_group_scale_factor;
			let offset_b = ((y + k * (y + 1.)) * slope_b).clamp(-b, -b + 1.) * color_parameter_group_scale_factor;

			(acc.0 + offset_r, acc.1 + offset_g, acc.2 + offset_b)
		});

		let color = Color::from_rgbaf32_unchecked((r + sum_r).clamp(0., 1.), (g + sum_g).clamp(0., 1.), (b + sum_b).clamp(0., 1.), a);

		color.to_linear_srgb()
	});
	input
}

pub(super) trait MultiplyAlpha {
	fn multiply_alpha(&mut self, factor: f64);
}

impl MultiplyAlpha for Color {
	fn multiply_alpha(&mut self, factor: f64) {
		*self = Color::from_rgbaf32_unchecked(self.r(), self.g(), self.b(), (self.a() * factor as f32).clamp(0., 1.))
	}
}
impl MultiplyAlpha for VectorData {
	fn multiply_alpha(&mut self, factor: f64) {
		self.alpha_blending.opacity *= factor as f32;
	}
}
impl MultiplyAlpha for GraphicGroup {
	fn multiply_alpha(&mut self, factor: f64) {
		self.alpha_blending.opacity *= factor as f32;
	}
}
impl<P: Pixel> MultiplyAlpha for ImageFrame<P> {
	fn multiply_alpha(&mut self, factor: f64) {
		self.alpha_blending.opacity *= factor as f32;
	}
}

type PosterizeValue = f64;
// Based on https://www.axiomx.com/posterize.htm
// This algorithm produces fully accurate output in relation to the industry standard.
#[node_macro::new_node_fn(category("Adjustments"))]
fn posterize<T: Adjust<Color>>(
	_: (),
	#[expose]
	#[implementations(Color, ImageFrame<Color>)]
	mut input: T,
	levels: PosterizeValue,
) -> T {
	input.adjust(|color| {
		let color = color.to_gamma_srgb();

		let number_of_areas = levels.recip() as f32;
		let size_of_areas = (levels - 1.).recip() as f32;
		let channel = |channel: f32| (channel / number_of_areas).floor() * size_of_areas;
		let color = color.map_rgb(channel);

		color.to_linear_srgb()
	});
	input
}

// Based on https://geraldbakker.nl/psnumbers/exposure.html
#[node_macro::new_node_fn(category("Adjustments"))]
fn exposure<T: Adjust<Color>>(
	_: (),
	#[expose]
	#[implementations(Color, ImageFrame<Color>)]
	mut input: T,
	exposure: f64,
	offset: f64,
	gamma_correction: f64,
) -> T {
	input.adjust(|color| {
		let adjusted = color
		// Exposure
		.map_rgb(|c: f32| c * 2_f32.powf(exposure as f32))
		// Offset
		.map_rgb(|c: f32| c + offset as f32)
		// Gamma correction
		.gamma(gamma_correction as f32);

		adjusted.map_rgb(|c: f32| c.clamp(0., 1.))
	});
	input
}

const WINDOW_SIZE: usize = 1024;

#[cfg(feature = "alloc")]
#[node_macro::new_node_fn(category("Adjustments"))]
fn generate_curves<C: Channel + super::Linear>(_: (), curve: Curve, #[implementations(f32)] _target_format: C) -> ValueMapperNode<C> {
	use bezier_rs::{Bezier, TValue};
	let [mut pos, mut param]: [[f32; 2]; 2] = [[0.; 2], curve.first_handle];
	let mut lut = vec![C::from_f64(0.); WINDOW_SIZE];
	let end = CurveManipulatorGroup {
		anchor: [1.; 2],
		handles: [curve.last_handle, [0.; 2]],
	};
	for sample in curve.manipulator_groups.iter().chain(core::iter::once(&end)) {
		let [x0, y0, x1, y1, x2, y2, x3, y3] = [pos[0], pos[1], param[0], param[1], sample.handles[0][0], sample.handles[0][1], sample.anchor[0], sample.anchor[1]].map(f64::from);

		let bezier = Bezier::from_cubic_coordinates(x0, y0, x1, y1, x2, y2, x3, y3);

		let [left, right] = [pos[0], sample.anchor[0]].map(|c| c.clamp(0., 1.));
		let lut_index_left: usize = (left * (lut.len() - 1) as f32).floor() as _;
		let lut_index_right: usize = (right * (lut.len() - 1) as f32).ceil() as _;
		for index in lut_index_left..=lut_index_right {
			let x = index as f64 / (lut.len() - 1) as f64;
			let y = if x <= x0 {
				y0
			} else if x >= x3 {
				y3
			} else {
				bezier.find_tvalues_for_x(x)
					.next()
					.map(|t| bezier.evaluate(TValue::Parametric(t.clamp(0., 1.))).y)
					// Fall back to a very bad approximation if Bezier-rs fails
					.unwrap_or_else(|| (x - x0) / (x3 - x0) * (y3 - y0) + y0)
			};
			lut[index] = C::from_f64(y);
		}

		pos = sample.anchor;
		param = sample.handles[1];
	}
	ValueMapperNode::new(lut)
}

#[cfg(feature = "alloc")]
#[node_macro::new_node_fn(category("Adjustments"))]
pub fn color_fill_node(mut image_frame: ImageFrame<Color>, color: Color) -> ImageFrame<Color> {
	for pixel in &mut image_frame.image.data {
		pixel.set_red(color.r());
		pixel.set_blue(color.b());
		pixel.set_green(color.g());
		pixel.alpha_multiply(color);
	}

	image_frame
}

#[cfg(feature = "alloc")]
#[node_macro::new_node_fn(category("Adjustments"))]
pub fn color_overlay_node(mut image: ImageFrame<Color>, color: Color, blend_mode: BlendMode, opacity: f64) -> ImageFrame<Color> {
	let opacity = (opacity as f32 / 100.).clamp(0., 1.);
	for pixel in &mut image.image.data {
		let image = pixel.map_rgb(|channel| channel * (1. - opacity));

		// The apply blend mode function divides rgb by the alpha channel for the background. This undoes that.
		let associated_pixel = Color::from_rgbaf32_unchecked(pixel.r() * pixel.a(), pixel.g() * pixel.a(), pixel.b() * pixel.a(), pixel.a());
		let overlay = apply_blend_mode(color, associated_pixel, blend_mode).map_rgb(|channel| channel * opacity);

		*pixel = Color::from_rgbaf32(image.r() + overlay.r(), image.g() + overlay.g(), image.b() + overlay.b(), pixel.a()).unwrap();
	}

	image
}

#[test]
fn color_overlay_multiply() {
	use crate::raster::Image;

	let image_color = Color::from_rgbaf32_unchecked(0.7, 0.6, 0.5, 0.4);
	let image = ImageFrame {
		image: Image::new(1, 1, image_color),
		..Default::default()
	};

	// Color { red: 0., green: 1., blue: 0., alpha: 1. }
	let overlay_color = Color::GREEN;

	// 100% of the output should come from the multiplied value
	let opacity = 100_f64;

	let result = color_overlay_node(image, overlay_color, BlendMode::Multiply, opacity);

	// The output should just be the original green and alpha channels (as we multiply them by 1 and other channels by 0)
	assert_eq!(result.image.data[0], Color::from_rgbaf32_unchecked(0., image_color.g(), 0., image_color.a()));
}

#[cfg(feature = "alloc")]
pub use index_node::IndexNode;

#[cfg(feature = "alloc")]
mod index_node {
	use crate::raster::{Color, ImageFrame};
	use crate::Node;

	#[node_macro::new_node_fn(category("Adjustments"))]
	pub fn index<T: Default + Clone>(_: (), #[implementations(Vec<ImageFrame<Color>>, Vec<Color>)] input: Vec<T>, index: u32) -> T {
		if (index as usize) < input.len() {
			input[index as usize].clone()
		} else {
			warn!("The number of segments is {} and the requested segment is {}!", input.len(), index);
			Default::default()
		}
	}
}
