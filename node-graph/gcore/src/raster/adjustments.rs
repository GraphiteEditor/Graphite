use super::Color;
use crate::Node;

use core::fmt::Debug;
use dyn_any::{DynAny, StaticType};

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
	// TODO: Remove conversion to linear when the whole node graph uses linear color
	let color = color.to_linear_srgb();

	let luminance = match luminance_calc {
		LuminanceCalculation::SRGB => color.luminance_srgb(),
		LuminanceCalculation::Perceptual => color.luminance_perceptual(),
		LuminanceCalculation::AverageChannels => color.average_rgb_channels(),
		LuminanceCalculation::MinimumChannels => color.minimum_rgb_channels(),
		LuminanceCalculation::MaximumChannels => color.maximum_rgb_channels(),
	};

	// TODO: Remove conversion to linear when the whole node graph uses linear color
	let luminance = Color::linear_to_srgb(luminance);

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
	color.map_rgb(|c| c * (output_maximums - output_minimums) + output_minimums)
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
	tint.with_luminance(luminance)
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
		let [hue, saturation, lightness, alpha] = color.to_hsla();
		Color::from_hsla(
			(hue + hue_shift as f32 / 360.) % 1.,
			(saturation + saturation_shift as f32 / 100.).clamp(0., 1.),
			(lightness + lightness_shift as f32 / 100.).clamp(0., 1.),
			alpha,
		)
	}
}

#[derive(Debug, Clone, Copy)]
pub struct InvertRGBNode;

#[node_macro::node_fn(InvertRGBNode)]
fn invert_image(color: Color) -> Color {
	color.map_rgb(|c| color.a() - c)
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

	// TODO: Remove conversion to linear when the whole node graph uses linear color
	let color = color.to_linear_srgb();

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

impl<Opacity: dyn_any::StaticTypeSized, Blend: dyn_any::StaticTypeSized> StaticType for BlendNode<Blend, Opacity> {
	type Static = BlendNode<Blend::Static, Opacity::Static>;
}

#[node_macro::node_fn(BlendNode)]
fn blend_node(input: (Color, Color), blend_mode: BlendMode, opacity: f64) -> Color {
	let opacity = opacity / 100.;

	let (foreground, background) = input;
	let foreground = foreground.to_linear_srgb();
	let background = background.to_linear_srgb();

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

	let multiplied_target_color = target_color.to_associated_alpha(opacity as f32);
	let blended = background.alpha_blend(multiplied_target_color);

	blended.to_gamma_srgb()
}

#[derive(Debug, Clone, Copy)]
pub struct VibranceNode<Vibrance> {
	vibrance: Vibrance,
}

// From https://stackoverflow.com/questions/33966121/what-is-the-algorithm-for-vibrance-filters
// The results of this implementation are very close to correct, but not quite perfect
#[node_macro::node_fn(VibranceNode)]
fn vibrance_node(color: Color, vibrance: f64) -> Color {
	// TODO: Remove conversion to linear when the whole node graph uses linear color
	let color = color.to_linear_srgb();

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

	let altered_color = if vibrance >= 0. {
		altered_color
	} else {
		// TODO: The result ends up a bit darker than it should be, further investigation is needed
		let luminance = color.luminance_rec_601();

		// Near -0% vibrance we mostly use `altered_color`.
		// Near -100% vibrance, we mostly use half the desaturated luminance color and half `altered_color`.
		let factor = -slowed_vibrance;
		altered_color.map_rgb(|c| c * (1. - factor) + luminance * factor)
	};

	// TODO: Remove conversion to linear when the whole node graph uses linear color
	altered_color.to_gamma_srgb()
}

#[derive(Debug, Clone, Copy)]
pub struct BrightnessContrastNode<Brightness, Contrast> {
	brightness: Brightness,
	contrast: Contrast,
}

// From https://stackoverflow.com/questions/2976274/adjust-bitmap-image-brightness-contrast-using-c
#[node_macro::node_fn(BrightnessContrastNode)]
fn adjust_image_brightness_and_contrast(color: Color, brightness: f64, contrast: f64) -> Color {
	let (brightness, contrast) = (brightness as f32, contrast as f32);
	let factor = (259. * (contrast + 255.)) / (255. * (259. - contrast));
	let channel = |channel: f32| ((factor * (channel * 255. + brightness - 128.) + 128.) / 255.).clamp(0., 1.);
	color.map_rgb(channel)
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
#[node_macro::node_fn(PosterizeNode)]
fn posterize(color: Color, posterize_value: f64) -> Color {
	let posterize_value = posterize_value as f32;
	let number_of_areas = posterize_value.recip();
	let size_of_areas = (posterize_value - 1.).recip();
	let channel = |channel: f32| (channel / number_of_areas).floor() * size_of_areas;
	color.map_rgb(channel)
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
	// TODO: Remove conversion to linear when the whole node graph uses linear color
	let color = color.to_linear_srgb();

	let result = color
		// Exposure
		.map_rgb(|c: f32| c * 2_f32.powf(exposure as f32))
		// Offset
		.map_rgb(|c: f32| c + offset as f32)
		// Gamma correction
		.gamma(gamma_correction as f32)
		.map_rgb(|c: f32| c.clamp(0., 1.));

	result.to_gamma_srgb()
}
