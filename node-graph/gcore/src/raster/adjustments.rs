use super::{
	curve::{Curve, CurveManipulatorGroup, ValueMapperNode},
	Channel, Color,
};
use crate::Node;
use bezier_rs::{Bezier, TValue};

use core::fmt::Debug;
use dyn_any::{DynAny, StaticType};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

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

impl BlendMode {
	pub fn list() -> [BlendMode; 26] {
		[
			BlendMode::Normal,
			BlendMode::Multiply,
			BlendMode::Darken,
			BlendMode::ColorBurn,
			BlendMode::LinearBurn,
			BlendMode::DarkerColor,
			BlendMode::Screen,
			BlendMode::Lighten,
			BlendMode::ColorDodge,
			BlendMode::LinearDodge,
			BlendMode::LighterColor,
			BlendMode::Overlay,
			BlendMode::SoftLight,
			BlendMode::HardLight,
			BlendMode::VividLight,
			BlendMode::LinearLight,
			BlendMode::PinLight,
			BlendMode::HardMix,
			BlendMode::Difference,
			BlendMode::Exclusion,
			BlendMode::Subtract,
			BlendMode::Divide,
			BlendMode::Hue,
			BlendMode::Saturation,
			BlendMode::Color,
			BlendMode::Luminosity,
		]
	}
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "std", derive(specta::Type))]
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, DynAny, Hash)]
pub enum BlendMode {
	#[default]
	// Basic group
	Normal,
	// Not supported by SVG, but we should someday support: Dissolve

	// Darken group
	Multiply,
	Darken,
	ColorBurn,
	LinearBurn,
	DarkerColor,

	// Lighten group
	Screen,
	Lighten,
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
}

impl core::fmt::Display for BlendMode {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			BlendMode::Normal => write!(f, "Normal"),

			BlendMode::Multiply => write!(f, "Multiply"),
			BlendMode::Darken => write!(f, "Darken"),
			BlendMode::ColorBurn => write!(f, "Color Burn"),
			BlendMode::LinearBurn => write!(f, "Linear Burn"),
			BlendMode::DarkerColor => write!(f, "Darker Color"),

			BlendMode::Screen => write!(f, "Screen"),
			BlendMode::Lighten => write!(f, "Lighten"),
			BlendMode::ColorDodge => write!(f, "Color Dodge"),
			BlendMode::LinearDodge => write!(f, "Linear Dodge"),
			BlendMode::LighterColor => write!(f, "Lighter Color"),

			BlendMode::Overlay => write!(f, "Overlay"),
			BlendMode::SoftLight => write!(f, "Soft Light"),
			BlendMode::HardLight => write!(f, "Hard Light"),
			BlendMode::VividLight => write!(f, "Vivid Light"),
			BlendMode::LinearLight => write!(f, "Linear Light"),
			BlendMode::PinLight => write!(f, "Pin Light"),
			BlendMode::HardMix => write!(f, "Hard Mix"),

			BlendMode::Difference => write!(f, "Difference"),
			BlendMode::Exclusion => write!(f, "Exclusion"),
			BlendMode::Subtract => write!(f, "Subtract"),
			BlendMode::Divide => write!(f, "Divide"),

			BlendMode::Hue => write!(f, "Hue"),
			BlendMode::Saturation => write!(f, "Saturation"),
			BlendMode::Color => write!(f, "Color"),
			BlendMode::Luminosity => write!(f, "Luminosity"),
		}
	}
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LuminanceNode<LuminanceCalculation> {
	luminance_calc: LuminanceCalculation,
}

#[node_macro::node_fn(LuminanceNode)]
fn luminance_color_node(color: Color, luminance_calc: LuminanceCalculation) -> Color {
	let luminance = match luminance_calc {
		LuminanceCalculation::SRGB => color.luminance_srgb(),
		LuminanceCalculation::Perceptual => color.luminance_perceptual(),
		LuminanceCalculation::AverageChannels => color.average_rgb_channels(),
		LuminanceCalculation::MinimumChannels => color.minimum_rgb_channels(),
		LuminanceCalculation::MaximumChannels => color.maximum_rgb_channels(),
	};
	color.map_rgb(|_| luminance)
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LevelsNode<InputStart, InputMid, InputEnd, OutputStart, OutputEnd> {
	input_start: InputStart,
	input_mid: InputMid,
	input_end: InputEnd,
	output_start: OutputStart,
	output_end: OutputEnd,
}

// From https://stackoverflow.com/questions/39510072/algorithm-for-adjustment-of-image-levels
#[node_macro::node_fn(LevelsNode)]
fn levels_node(color: Color, input_start: f64, input_mid: f64, input_end: f64, output_start: f64, output_end: f64) -> Color {
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
	let highlights_minus_shadows = (input_highlights - input_shadows).max(f32::EPSILON).min(1.);
	let color = color.map_rgb(|c| (c - input_shadows).max(0.) / highlights_minus_shadows);

	// Midtones (Range: 0-1)
	let color = color.gamma(gamma);

	// Output levels (Range: 0-1)
	let color = color.map_rgb(|c| c * (output_maximums - output_minimums) + output_minimums);

	color.to_linear_srgb()
}

#[derive(Debug, Clone, Copy, Default)]
pub struct GrayscaleNode<Tint, Reds, Yellows, Greens, Cyans, Blues, Magentas> {
	tint: Tint,
	reds: Reds,
	yellows: Yellows,
	greens: Greens,
	cyans: Cyans,
	blues: Blues,
	magentas: Magentas,
}

// From <https://stackoverflow.com/a/55233732/775283>
// Works the same for gamma and linear color
#[node_macro::node_fn(GrayscaleNode)]
fn grayscale_color_node(color: Color, tint: Color, reds: f64, yellows: f64, greens: f64, cyans: f64, blues: f64, magentas: f64) -> Color {
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

	color.to_linear_srgb()
}

#[cfg(not(target_arch = "spirv"))]
pub use hue_shift::HueSaturationNode;

// TODO: Make this work on GPU so it can be removed from the wrapper module that excludes GPU (it doesn't work because of the modulo)
#[cfg(not(target_arch = "spirv"))]
mod hue_shift {
	use super::*;

	#[derive(Debug)]
	pub struct HueSaturationNode<Hue, Saturation, Lightness> {
		hue_shift: Hue,
		saturation_shift: Saturation,
		lightness_shift: Lightness,
	}

	#[node_macro::node_fn(HueSaturationNode)]
	fn hue_shift_color_node(color: Color, hue_shift: f64, saturation_shift: f64, lightness_shift: f64) -> Color {
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
	}
}

#[derive(Debug, Clone, Copy)]
pub struct InvertRGBNode;

#[node_macro::node_fn(InvertRGBNode)]
fn invert_image(color: Color) -> Color {
	let color = color.to_gamma_srgb();

	let color = color.map_rgb(|c| color.a() - c);

	color.to_linear_srgb()
}

#[derive(Debug, Clone, Copy)]
pub struct ThresholdNode<MinLuminance, MaxLuminance, LuminanceCalc> {
	min_luminance: MinLuminance,
	max_luminance: MaxLuminance,
	luminance_calc: LuminanceCalc,
}

#[node_macro::node_fn(ThresholdNode)]
fn threshold_node(color: Color, min_luminance: f64, max_luminance: f64, luminance_calc: LuminanceCalculation) -> Color {
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
}

#[derive(Debug, Clone, Copy)]
pub struct BlendNode<BlendMode, Opacity> {
	blend_mode: BlendMode,
	opacity: Opacity,
}

#[node_macro::node_fn(BlendNode)]
fn blend_node(input: (Color, Color), blend_mode: BlendMode, opacity: f64) -> Color {
	let opacity = opacity / 100.;

	let (foreground, background) = input;

	let target_color = match blend_mode {
		BlendMode::Normal => background.blend_rgb(foreground, Color::blend_normal),
		BlendMode::Multiply => background.blend_rgb(foreground, Color::blend_multiply),
		BlendMode::Darken => background.blend_rgb(foreground, Color::blend_darken),
		BlendMode::ColorBurn => background.blend_rgb(foreground, Color::blend_color_burn),
		BlendMode::LinearBurn => background.blend_rgb(foreground, Color::blend_linear_burn),
		BlendMode::DarkerColor => background.blend_darker_color(foreground),

		BlendMode::Screen => background.blend_rgb(foreground, Color::blend_screen),
		BlendMode::Lighten => background.blend_rgb(foreground, Color::blend_lighten),
		BlendMode::ColorDodge => background.blend_rgb(foreground, Color::blend_color_dodge),
		BlendMode::LinearDodge => background.blend_rgb(foreground, Color::blend_linear_dodge),
		BlendMode::LighterColor => background.blend_lighter_color(foreground),

		BlendMode::Overlay => foreground.blend_rgb(background, Color::blend_hardlight),
		BlendMode::SoftLight => background.blend_rgb(foreground, Color::blend_softlight),
		BlendMode::HardLight => background.blend_rgb(foreground, Color::blend_hardlight),
		BlendMode::VividLight => background.blend_rgb(foreground, Color::blend_vivid_light),
		BlendMode::LinearLight => background.blend_rgb(foreground, Color::blend_linear_light),
		BlendMode::PinLight => background.blend_rgb(foreground, Color::blend_pin_light),
		BlendMode::HardMix => background.blend_rgb(foreground, Color::blend_hard_mix),

		BlendMode::Difference => background.blend_rgb(foreground, Color::blend_exclusion),
		BlendMode::Exclusion => background.blend_rgb(foreground, Color::blend_exclusion),
		BlendMode::Subtract => background.blend_rgb(foreground, Color::blend_subtract),
		BlendMode::Divide => background.blend_rgb(foreground, Color::blend_divide),

		BlendMode::Hue => background.blend_hue(foreground),
		BlendMode::Saturation => background.blend_saturation(foreground),
		BlendMode::Color => background.blend_color(foreground),
		BlendMode::Luminosity => background.blend_luminosity(foreground),
	};

	background.alpha_blend(target_color.to_associated_alpha(opacity as f32))
}

#[derive(Debug, Clone, Copy)]
pub struct VibranceNode<Vibrance> {
	vibrance: Vibrance,
}

// Modified from https://stackoverflow.com/questions/33966121/what-is-the-algorithm-for-vibrance-filters
// The results of this implementation are very close to correct, but not quite perfect
#[node_macro::node_fn(VibranceNode)]
fn vibrance_node(color: Color, vibrance: f64) -> Color {
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
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, DynAny)]
pub enum RedGreenBlue {
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

#[derive(Debug, Clone, Copy)]
pub struct ChannelMixerNode<Monochrome, MonochromeR, MonochromeG, MonochromeB, MonochromeC, RedR, RedG, RedB, RedC, GreenR, GreenG, GreenB, GreenC, BlueR, BlueG, BlueB, BlueC> {
	monochrome: Monochrome,
	monochrome_r: MonochromeR,
	monochrome_g: MonochromeG,
	monochrome_b: MonochromeB,
	monochrome_c: MonochromeC,
	red_r: RedR,
	red_g: RedG,
	red_b: RedB,
	red_c: RedC,
	green_r: GreenR,
	green_g: GreenG,
	green_b: GreenB,
	green_c: GreenC,
	blue_r: BlueR,
	blue_g: BlueG,
	blue_b: BlueB,
	blue_c: BlueC,
}

#[node_macro::node_fn(ChannelMixerNode)]
fn channel_mixer_node(
	color: Color,
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
) -> Color {
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
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, DynAny)]
pub enum RelativeAbsolute {
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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, DynAny)]
pub enum SelectiveColorChoice {
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

#[derive(Debug, Clone, Copy)]
pub struct SelectiveColorNode<Absolute, RC, RM, RY, RK, YC, YM, YY, YK, GC, GM, GY, GK, CC, CM, CY, CK, BC, BM, BY, BK, MC, MM, MY, MK, WC, WM, WY, WK, NC, NM, NY, NK, KC, KM, KY, KK> {
	mode: Absolute,
	r_c: RC,
	r_m: RM,
	r_y: RY,
	r_k: RK,
	y_c: YC,
	y_m: YM,
	y_y: YY,
	y_k: YK,
	g_c: GC,
	g_m: GM,
	g_y: GY,
	g_k: GK,
	c_c: CC,
	c_m: CM,
	c_y: CY,
	c_k: CK,
	b_c: BC,
	b_m: BM,
	b_y: BY,
	b_k: BK,
	m_c: MC,
	m_m: MM,
	m_y: MY,
	m_k: MK,
	w_c: WC,
	w_m: WM,
	w_y: WY,
	w_k: WK,
	n_c: NC,
	n_m: NM,
	n_y: NY,
	n_k: NK,
	k_c: KC,
	k_m: KM,
	k_y: KY,
	k_k: KK,
}

// Based on https://blog.pkh.me/p/22-understanding-selective-coloring-in-adobe-photoshop.html
#[node_macro::node_fn(SelectiveColorNode)]
fn selective_color_node(
	color: Color,
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
) -> Color {
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
		(SelectiveColorChoice::Reds, (r_c, r_m, r_y, r_k)),
		(SelectiveColorChoice::Yellows, (y_c, y_m, y_y, y_k)),
		(SelectiveColorChoice::Greens, (g_c, g_m, g_y, g_k)),
		(SelectiveColorChoice::Cyans, (c_c, c_m, c_y, c_k)),
		(SelectiveColorChoice::Blues, (b_c, b_m, b_y, b_k)),
		(SelectiveColorChoice::Magentas, (m_c, m_m, m_y, m_k)),
		(SelectiveColorChoice::Whites, (w_c, w_m, w_y, w_k)),
		(SelectiveColorChoice::Neutrals, (n_c, n_m, n_y, n_k)),
		(SelectiveColorChoice::Blacks, (k_c, k_m, k_y, k_k)),
	]
	.into_iter()
	.fold((0., 0., 0.), |acc, (color_parameter_group, (c, m, y, k))| {
		// Skip this color parameter group...
		// ...if it's unchanged from the default of zero offset on all CMYK paramters, or...
		// ...if this pixel's color isn't in the range affected by this color parameter group
		if (c < f64::EPSILON && m < f64::EPSILON && y < f64::EPSILON && k < f64::EPSILON) || (!pixel_color_range(color_parameter_group)) {
			return acc;
		}

		let (c, m, y, k) = (c as f32 / 100., m as f32 / 100., y as f32 / 100., k as f32 / 100.);

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
}

#[derive(Debug, Clone, Copy)]
pub struct OpacityNode<O> {
	opacity_multiplier: O,
}

#[node_macro::node_fn(OpacityNode)]
fn image_opacity(color: Color, opacity_multiplier: f64) -> Color {
	let opacity_multiplier = opacity_multiplier as f32 / 100.;
	Color::from_rgbaf32_unchecked(color.r(), color.g(), color.b(), color.a() * opacity_multiplier)
}

#[derive(Debug, Clone, Copy)]
pub struct PosterizeNode<P> {
	posterize_value: P,
}

// Based on http://www.axiomx.com/posterize.htm
// This algorithm is perfectly accurate.
#[node_macro::node_fn(PosterizeNode)]
fn posterize(color: Color, posterize_value: f64) -> Color {
	let color = color.to_gamma_srgb();

	let posterize_value = posterize_value as f32;
	let number_of_areas = posterize_value.recip();
	let size_of_areas = (posterize_value - 1.).recip();
	let channel = |channel: f32| (channel / number_of_areas).floor() * size_of_areas;
	let color = color.map_rgb(channel);

	color.to_linear_srgb()
}

#[derive(Debug, Clone, Copy)]
pub struct ExposureNode<Exposure, Offset, GammaCorrection> {
	exposure: Exposure,
	offset: Offset,
	gamma_correction: GammaCorrection,
}

// Based on https://geraldbakker.nl/psnumbers/exposure.html
#[node_macro::node_fn(ExposureNode)]
fn exposure(color: Color, exposure: f64, offset: f64, gamma_correction: f64) -> Color {
	let adjusted = color
		// Exposure
		.map_rgb(|c: f32| c * 2_f32.powf(exposure as f32))
		// Offset
		.map_rgb(|c: f32| c + offset as f32)
		// Gamma correction
		.gamma(gamma_correction as f32);

	adjusted.map_rgb(|c: f32| c.clamp(0., 1.))
}

const WINDOW_SIZE: usize = 1024;

#[derive(Debug, Clone, Copy)]
pub struct GenerateCurvesNode<OutputChannel, Curve> {
	curve: Curve,
	_channel: core::marker::PhantomData<OutputChannel>,
}

#[node_macro::node_fn(GenerateCurvesNode<_Channel>)]
fn generate_curves<_Channel: Channel>(_primary: (), curve: Curve) -> ValueMapperNode<_Channel> {
	let [mut pos, mut param]: [[f32; 2]; 2] = [[0.0; 2], curve.first_handle];
	let mut lut = vec![_Channel::zero(); WINDOW_SIZE];
	let end = CurveManipulatorGroup {
		anchor: [1.0; 2],
		handles: [curve.last_handle, [0.0; 2]],
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
				// Bezier::intersections is probably not the best solution for this.
				// Maybe it's the best adding a function y_coordinate_from_x or so to bezier_rs::Bezier.
				// This can be easily done with bezier_rs::utils::solve_cubic.
				let ts = bezier.intersections(&Bezier::from_linear_coordinates(x, -1., x, 2.), None, None);
				ts.first()
					.map(|&t| bezier.evaluate(TValue::Parametric(t)).y)
					// a very bad approximation if bezier_rs failes
					.unwrap_or_else(|| (x - x0) / (x3 - x0) * (y3 - y0) + y0)
			};
			lut[index] = _Channel::from_f64(y);
		}

		pos = sample.anchor;
		param = sample.handles[1];
	}
	ValueMapperNode::new(lut)
}

#[cfg(feature = "alloc")]
pub use index_node::IndexNode;

#[cfg(feature = "alloc")]
mod index_node {
	use crate::raster::{Color, ImageFrame};
	use crate::Node;

	#[derive(Debug)]
	pub struct IndexNode<Index> {
		pub index: Index,
	}

	#[node_macro::node_fn(IndexNode)]
	pub fn index_node(input: Vec<ImageFrame<Color>>, index: u32) -> ImageFrame<Color> {
		if (index as usize) < input.len() {
			input[index as usize].clone()
		} else {
			warn!("The number of segments is {} and the requested segment is {}!", input.len(), index);
			ImageFrame::empty()
		}
	}
}
