use super::Color;
use crate::Node;

use core::fmt::Debug;

#[derive(Debug, Clone, Copy, Default)]
pub struct GrayscaleNode;

#[node_macro::node_fn(GrayscaleNode)]
fn grayscale_color_node(color: Color) -> Color {
	// TODO: Remove conversion to linear when the whole node graph uses linear color
	let color = color.to_linear_srgb();

	let luminance = color.luminance();

	// TODO: Remove conversion to linear when the whole node graph uses linear color
	let luminance = Color::linear_to_srgb(luminance);

	color.map_rgb(|_| luminance)
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WeightedGrayscaleNode<RWeight, GWeight, BWeight, CWeight, MWeight, YWeight> {
	r_weight: RWeight,
	g_weight: GWeight,
	b_weight: BWeight,
	c_weight: CWeight,
	m_weight: MWeight,
	y_weight: YWeight,
}

#[node_macro::node_fn(WeightedGrayscaleNode)]
fn weighted_grayscale_color_node(color: Color, r_weight: f64, g_weight: f64, b_weight: f64, c_weight: f64, m_weight: f64, y_weight: f64) -> Color {
	// TODO: Remove conversion to linear when the whole node graph uses linear color
//	let color = color.to_linear_srgb();

	let [hue, saturation, lightness, _alpha] = color.to_hsla();
	/// Calculates the black and white filter for a single pixel.
	let hue_val = hue;
	let v_coeff_values = [r_weight, y_weight, g_weight, c_weight, b_weight, m_weight];
    let v_coeff_values: Vec<_> =v_coeff_values.iter().map(|x|(x - 50.) / 50.).collect();
	let hue_radius = 1. / 6.;
	let hue_values: Vec<_> = (0..v_coeff_values.len()).map(|x| x as f64 / 6.).collect();

	//fn process_pixel_black_white_filter(hue_val: f64, hue_values: &[f64], v_coeff_values: &[f64], hue_radius: f64) -> f64 {
	let mut lum_coeff = 0.0;

	let diff_val = hue_val.min(1.0 - hue_val) as f64;
	lum_coeff += v_coeff_values[0] * (hue_radius - diff_val).max(0.0);

	for (hue_value, coeff) in hue_values.iter().zip(v_coeff_values.iter()).skip(1) {
		lum_coeff += coeff * (hue_radius - (hue_value - hue_val as f64).abs()).max(0.0);
	}
    
	let luminance = lightness * (1.0 + saturation * lum_coeff as f32);

	// TODO: Remove conversion to linear when the whole node graph uses linear color
//	let luminance = Color::linear_to_srgb(luminance);
	
    color.map_rgb(|_| luminance)
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
pub struct ThresholdNode<Threshold> {
	threshold: Threshold,
}

#[node_macro::node_fn(ThresholdNode)]
fn threshold_node(color: Color, threshold: f64) -> Color {
	let threshold = Color::srgb_to_linear(threshold as f32);

	// TODO: Remove conversion to linear when the whole node graph uses linear color
	let color = color.to_linear_srgb();

	if color.luminance() >= threshold {
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
	let opacity_multiplier = opacity_multiplier as f32;
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
