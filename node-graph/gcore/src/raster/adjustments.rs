use super::Color;
use crate::Node;

use core::fmt::Debug;
use dyn_any::{DynAny, StaticType};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, DynAny, specta::Type, Hash)]
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

impl std::fmt::Display for LuminanceCalculation {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			LuminanceCalculation::SRGB => write!(f, "sRGB"),
			LuminanceCalculation::Perceptual => write!(f, "Perceptual"),
			LuminanceCalculation::AverageChannels => write!(f, "Average Channels"),
			LuminanceCalculation::MinimumChannels => write!(f, "Minimum Channels"),
			LuminanceCalculation::MaximumChannels => write!(f, "Maximum Channels"),
		}
	}
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LuminanceNode<LuminanceCalculation> {
	luma_calculation: LuminanceCalculation,
}

#[node_macro::node_fn(LuminanceNode)]
fn luminance_color_node(color: Color, luma_calculation: LuminanceCalculation) -> Color {
	// TODO: Remove conversion to linear when the whole node graph uses linear color
	let color = color.to_linear_srgb();

	let luminance = match luma_calculation {
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
	// Input Range
	let input_shadows = (input_start / 100.) as f32;
	let input_midtones = (input_mid / 100.) as f32;
	let input_highlights = (input_end / 100.) as f32;

	// Output Range
	let output_minimums = (output_start / 100.) as f32;
	let output_maximums = (output_end / 100.) as f32;

	// Midtones interpolation factor between minimums and maximums
	let midtones = output_minimums + (output_maximums - output_minimums) * input_midtones;

	// Gamma correction
	let gamma = if midtones < 0.5 {
		1. / (1. + (9. * (1. - midtones * 2.))).min(9.99)
	} else {
		1. / ((1. - midtones) * 2.).max(0.01)
	};

	// Input levels
	let color = color.map_rgb(|channel| (channel - input_shadows) / (input_highlights - input_shadows));

	// Midtones
	let color = color.map_rgb(|channel| channel.powf(gamma));

	// Output levels
	color.map_rgb(|channel| channel * (output_maximums - output_minimums) + output_minimums)
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
	color.map_rgb(|c| 1. - c)
}

#[derive(Debug, Clone, Copy)]
pub struct ThresholdNode<LuminanceCalculation, Threshold> {
	luma_calculation: LuminanceCalculation,
	threshold: Threshold,
}

#[node_macro::node_fn(ThresholdNode)]
fn threshold_node(color: Color, luma_calculation: LuminanceCalculation, threshold: f64) -> Color {
	let threshold = Color::srgb_to_linear(threshold as f32 / 100.);

	// TODO: Remove conversion to linear when the whole node graph uses linear color
	let color = color.to_linear_srgb();

	let luminance = match luma_calculation {
		LuminanceCalculation::SRGB => color.luminance_srgb(),
		LuminanceCalculation::Perceptual => color.luminance_perceptual(),
		LuminanceCalculation::AverageChannels => color.average_rgb_channels(),
		LuminanceCalculation::MinimumChannels => color.minimum_rgb_channels(),
		LuminanceCalculation::MaximumChannels => color.maximum_rgb_channels(),
	};

	if luminance >= threshold {
		Color::WHITE
	} else {
		Color::BLACK
	}
}

#[derive(Debug, Clone, Copy)]
pub struct VibranceNode<Vibrance> {
	vibrance: Vibrance,
}

// TODO: The current results are incorrect, try implementing this from https://stackoverflow.com/questions/33966121/what-is-the-algorithm-for-vibrance-filters
#[node_macro::node_fn(VibranceNode)]
fn vibrance_node(color: Color, vibrance: f64) -> Color {
	let [hue, saturation, lightness, alpha] = color.to_hsla();
	let vibrance = vibrance as f32 / 100.;
	let saturation = saturation + vibrance * (1. - saturation);
	Color::from_hsla(hue, saturation, lightness, alpha)
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

// Based on https://stackoverflow.com/questions/12166117/what-is-the-math-behind-exposure-adjustment-on-photoshop
#[node_macro::node_fn(ExposureNode)]
fn exposure(color: Color, exposure: f64, offset: f64, gamma_correction: f64) -> Color {
	let multiplier = 2_f32.powf(exposure as f32);
	color
		// TODO: Fix incorrect behavior of offset
		.map_rgb(|channel: f32| channel + offset as f32)
		// TODO: Fix incorrect behavior of exposure
		.map_rgb(|channel: f32| channel * multiplier)
		// TODO: While gamma correction is correct on its own, determine and implement the correct order of these three operations
		.gamma(gamma_correction as f32)
}
