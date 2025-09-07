#![allow(clippy::too_many_arguments)]

use crate::adjust::Adjust;
use crate::cubic_spline::CubicSplines;
use core::fmt::Debug;
use glam::{Vec3, Vec4};
#[cfg(feature = "std")]
use graphene_core::gradient::GradientStops;
#[cfg(feature = "std")]
use graphene_core::raster_types::{CPU, Raster};
#[cfg(feature = "std")]
use graphene_core::table::Table;
use graphene_core_shaders::color::Color;
use graphene_core_shaders::context::Ctx;
use graphene_core_shaders::registry::types::{AngleF32, PercentageF32, SignedPercentageF32};
use node_macro::BufferStruct;
use num_enum::{FromPrimitive, IntoPrimitive};
#[cfg(not(feature = "std"))]
use num_traits::float::Float;

// TODO: Implement the following:
// Color Balance
// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27blnc%27%20%3D%20Color%20Balance
//
// Photo Filter
// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27phfl%27%20%3D%20Photo%20Filter
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=of%20the%20file.-,Photo%20Filter,-Key%20is%20%27phfl
//
// Color Lookup
// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27clrL%27%20%3D%20Color%20Lookup
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=Color%20Lookup%20(Photoshop%20CS6

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash, node_macro::ChoiceType, bytemuck::NoUninit, BufferStruct, FromPrimitive, IntoPrimitive)]
#[cfg_attr(feature = "std", derive(dyn_any::DynAny, specta::Type, serde::Serialize, serde::Deserialize))]
#[widget(Dropdown)]
#[repr(u32)]
pub enum LuminanceCalculation {
	#[default]
	#[label("sRGB")]
	SRGB,
	Perceptual,
	AverageChannels,
	MinimumChannels,
	MaximumChannels,
}

#[node_macro::node(category("Raster: Adjustment"), shader_node(PerPixelAdjust))]
fn luminance<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(
		Table<Raster<CPU>>,
		Table<Color>,
		Table<GradientStops>,
		GradientStops,
	)]
	#[gpu_image]
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

#[node_macro::node(category("Raster"), shader_node(PerPixelAdjust))]
fn gamma_correction<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(
		Table<Raster<CPU>>,
		Table<Color>,
		Table<GradientStops>,
		GradientStops,
	)]
	#[gpu_image]
	mut input: T,
	#[default(2.2)]
	#[range((0.01, 10.))]
	#[hard_min(0.0001)]
	gamma: f32,
	inverse: bool,
) -> T {
	let exponent = if inverse { 1. / gamma } else { gamma };
	input.adjust(|color| color.gamma(exponent));
	input
}

#[node_macro::node(category("Raster: Channels"), shader_node(PerPixelAdjust))]
fn extract_channel<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(
		Table<Raster<CPU>>,
		Table<Color>,
		Table<GradientStops>,
		GradientStops,
	)]
	#[gpu_image]
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
		color.map_rgb(|_| extracted_value).with_alpha(1.)
	});
	input
}

#[node_macro::node(category("Raster: Channels"), shader_node(PerPixelAdjust))]
fn make_opaque<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(
		Table<Raster<CPU>>,
		Table<Color>,
		Table<GradientStops>,
		GradientStops,
	)]
	#[gpu_image]
	mut input: T,
) -> T {
	input.adjust(|color| {
		if color.a() == 0. {
			return color.with_alpha(1.);
		}
		Color::from_rgbaf32_unchecked(color.r() / color.a(), color.g() / color.a(), color.b() / color.a(), 1.)
	});
	input
}

/// See [`brightness_contrast`]
#[node_macro::node(
	name("Brightness/Contrast classic"),
	category("Raster: Adjustment"),
	properties("brightness_contrast_properties"),
	shader_node(PerPixelAdjust)
)]
fn brightness_contrast_classic<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(
		Table<Raster<CPU>>,
		Table<Color>,
		Table<GradientStops>,
		GradientStops,
	)]
	#[gpu_image]
	mut input: T,
	brightness: SignedPercentageF32,
	contrast: SignedPercentageF32,
) -> T {
	let brightness = brightness / 255.;

	let contrast = contrast / 100.;
	let contrast = if contrast > 0. { (contrast * core::f32::consts::FRAC_PI_2 - 0.01).tan() } else { contrast };

	let offset = brightness * contrast + brightness - contrast / 2.;

	input.adjust(|color| color.to_gamma_srgb().map_rgb(|c| (c + c * contrast + offset).clamp(0., 1.)).to_linear_srgb());

	input
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27brit%27%20%3D%20Brightness/Contrast
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=Padding-,Brightness%20and%20Contrast,-Key%20is%20%27brit
//
// Some further analysis available at:
// https://geraldbakker.nl/psnumbers/brightness-contrast.html
#[node_macro::node(name("Brightness/Contrast"), category("Raster: Adjustment"), properties("brightness_contrast_properties"), cfg(feature = "std"))]
fn brightness_contrast<T: Adjust<Color>>(
	_ctx: impl Ctx,
	#[implementations(
		Table<Raster<CPU>>,
		Table<Color>,
		Table<GradientStops>,
		GradientStops,
	)]
	#[gpu_image]
	mut input: T,
	brightness: SignedPercentageF32,
	contrast: SignedPercentageF32,
	use_classic: bool,
) -> T {
	if use_classic {
		return brightness_contrast_classic(_ctx, input, brightness, contrast);
	}

	const WINDOW_SIZE: usize = 1024;

	// Brightness LUT
	let brightness_is_negative = brightness < 0.;
	// We clamp the brightness before the two curve X-axis points `130 - brightness * 26` and `233 - brightness * 48` intersect.
	// Beyond the point of intersection, the cubic spline fitting becomes invalid and fails an assertion, which we need to avoid.
	// See the intersection of the red lines at x = 103/22*100 = 468.18182 in the graph: https://www.desmos.com/calculator/ekvz4zyd9c
	let brightness = (brightness.abs() / 100.).min(103. / 22. - 0.00001);
	let brightness_curve_points = CubicSplines {
		x: [0., 130. - brightness * 26., 233. - brightness * 48., 255.].map(|x| x / 255.),
		y: [0., 130. + brightness * 51., 233. + brightness * 10., 255.].map(|x| x / 255.),
	};
	let brightness_curve_solutions = brightness_curve_points.solve();
	let mut brightness_lut: [f32; WINDOW_SIZE] = core::array::from_fn(|i| {
		let x = i as f32 / (WINDOW_SIZE as f32 - 1.);
		brightness_curve_points.interpolate(x, &brightness_curve_solutions)
	});
	// Special handling for when brightness is negative
	if brightness_is_negative {
		brightness_lut = core::array::from_fn(|i| {
			let mut x = i;
			while x > 1 && brightness_lut[x] > i as f32 / WINDOW_SIZE as f32 {
				x -= 1;
			}
			x as f32 / WINDOW_SIZE as f32
		});
	}

	// Contrast LUT
	// Unlike with brightness, the X-axis points `64` and `192` don't intersect at any contrast value, because they are constants.
	// So we don't have to worry about clamping the contrast value to avoid invalid cubic spline fitting.
	// See the graph: https://www.desmos.com/calculator/iql9vsca56
	let contrast = contrast / 100.;
	let contrast_curve_points = CubicSplines {
		x: [0., 64., 192., 255.].map(|x| x / 255.),
		y: [0., 64. - contrast * 30., 192. + contrast * 30., 255.].map(|x| x / 255.),
	};
	let contrast_curve_solutions = contrast_curve_points.solve();
	let contrast_lut: [f32; WINDOW_SIZE] = core::array::from_fn(|i| {
		let x = i as f32 / (WINDOW_SIZE as f32 - 1.);
		contrast_curve_points.interpolate(x, &contrast_curve_solutions)
	});

	// Composed brightness and contrast LUTs
	let combined_lut = brightness_lut.map(|brightness| {
		let index_in_contrast_lut = (brightness * (contrast_lut.len() - 1) as f32).round() as usize;
		contrast_lut[index_in_contrast_lut]
	});
	let lut_max = (combined_lut.len() - 1) as f32;

	input.adjust(|color| color.to_gamma_srgb().map_rgb(|c| combined_lut[(c * lut_max).round() as usize]).to_linear_srgb());

	input
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=levl%27%20%3D%20Levels
//
// Algorithm from:
// https://stackoverflow.com/questions/39510072/algorithm-for-adjustment-of-image-levels
//
// Some further analysis available at:
// https://geraldbakker.nl/psnumbers/levels.html
#[node_macro::node(category("Raster: Adjustment"), shader_node(PerPixelAdjust))]
fn levels<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(
		Table<Raster<CPU>>,
		Table<Color>,
		Table<GradientStops>,
		GradientStops,
	)]
	#[gpu_image]
	mut image: T,
	#[default(0.)] shadows: PercentageF32,
	#[default(50.)] midtones: PercentageF32,
	#[default(100.)] highlights: PercentageF32,
	#[default(0.)] output_minimums: PercentageF32,
	#[default(100.)] output_maximums: PercentageF32,
) -> T {
	image.adjust(|color| {
		let color = color.to_gamma_srgb();

		// Input Range (Range: 0-1)
		let input_shadows = shadows / 100.;
		let input_midtones = midtones / 100.;
		let input_highlights = highlights / 100.;

		// Output Range (Range: 0-1)
		let output_minimums = output_minimums / 100.;
		let output_maximums = output_maximums / 100.;

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
	image
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27blwh%27%20%3D%20Black%20and%20White
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=Black%20White%20(Photoshop%20CS3)
//
// Algorithm from:
// https://stackoverflow.com/a/55233732/775283
// Works the same for gamma and linear color
#[node_macro::node(name("Black & White"), category("Raster: Adjustment"), shader_node(PerPixelAdjust))]
fn black_and_white<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(
		Table<Raster<CPU>>,
		Table<Color>,
		Table<GradientStops>,
		GradientStops,
	)]
	#[gpu_image]
	mut image: T,
	#[default(Color::BLACK)] tint: Color,
	#[default(40.)]
	#[range((-200., 300.))]
	reds: PercentageF32,
	#[default(60.)]
	#[range((-200., 300.))]
	yellows: PercentageF32,
	#[default(40.)]
	#[range((-200., 300.))]
	greens: PercentageF32,
	#[default(60.)]
	#[range((-200., 300.))]
	cyans: PercentageF32,
	#[default(20.)]
	#[range((-200., 300.))]
	blues: PercentageF32,
	#[default(80.)]
	#[range((-200., 300.))]
	magentas: PercentageF32,
) -> T {
	image.adjust(|color| {
		let color = color.to_gamma_srgb();

		let reds = reds / 100.;
		let yellows = yellows / 100.;
		let greens = greens / 100.;
		let cyans = cyans / 100.;
		let blues = blues / 100.;
		let magentas = magentas / 100.;

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

		let color = Color::from_rgbaf32_unchecked(color.r(), color.g(), color.b(), alpha_part);

		color.to_linear_srgb()
	});
	image
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27hue%20%27%20%3D%20Old,saturation%2C%20Photoshop%205.0
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=0%20%3D%20Use%20other.-,Hue/Saturation,-Hue/Saturation%20settings
#[node_macro::node(name("Hue/Saturation"), category("Raster: Adjustment"), shader_node(PerPixelAdjust))]
fn hue_saturation<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(
		Table<Raster<CPU>>,
		Table<Color>,
		Table<GradientStops>,
		GradientStops,
	)]
	#[gpu_image]
	mut input: T,
	hue_shift: AngleF32,
	saturation_shift: SignedPercentageF32,
	lightness_shift: SignedPercentageF32,
) -> T {
	input.adjust(|color| {
		let color = color.to_gamma_srgb();

		let [hue, saturation, lightness, alpha] = color.to_hsla();

		let color = Color::from_hsla(
			(hue + hue_shift / 360.) % 1.,
			// TODO: Improve the way saturation works (it's slightly off)
			(saturation + saturation_shift / 100.).clamp(0., 1.),
			// TODO: Fix the way lightness works (it's very off)
			(lightness + lightness_shift / 100.).clamp(0., 1.),
			alpha,
		);

		color.to_linear_srgb()
	});
	input
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27%20%3D%20Color%20Lookup-,%27nvrt%27%20%3D%20Invert,-%27post%27%20%3D%20Posterize
#[node_macro::node(category("Raster: Adjustment"), shader_node(PerPixelAdjust))]
fn invert<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(
		Table<Raster<CPU>>,
		Table<Color>,
		Table<GradientStops>,
		GradientStops,
	)]
	#[gpu_image]
	mut input: T,
) -> T {
	input.adjust(|color| {
		let color = color.to_gamma_srgb();

		let color = color.map_rgb(|c| color.a() - c);

		color.to_linear_srgb()
	});
	input
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=post%27%20%3D%20Posterize-,%27thrs%27%20%3D%20Threshold,-%27grdm%27%20%3D%20Gradient
#[node_macro::node(category("Raster: Adjustment"), shader_node(PerPixelAdjust))]
fn threshold<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(
		Table<Raster<CPU>>,
		Table<Color>,
		Table<GradientStops>,
		GradientStops,
	)]
	#[gpu_image]
	mut image: T,
	#[default(50.)] min_luminance: PercentageF32,
	#[default(100.)] max_luminance: PercentageF32,
	luminance_calc: LuminanceCalculation,
) -> T {
	image.adjust(|color| {
		let min_luminance = Color::srgb_to_linear(min_luminance / 100.);
		let max_luminance = Color::srgb_to_linear(max_luminance / 100.);

		let luminance = match luminance_calc {
			LuminanceCalculation::SRGB => color.luminance_srgb(),
			LuminanceCalculation::Perceptual => color.luminance_perceptual(),
			LuminanceCalculation::AverageChannels => color.average_rgb_channels(),
			LuminanceCalculation::MinimumChannels => color.minimum_rgb_channels(),
			LuminanceCalculation::MaximumChannels => color.maximum_rgb_channels(),
		};

		if luminance >= min_luminance && luminance <= max_luminance { Color::WHITE } else { Color::BLACK }
	});
	image
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27-,vibA%27%20%3D%20Vibrance,-%27hue%20%27%20%3D%20Old
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=Vibrance%20(Photoshop%20CS3)
//
// Algorithm based on:
// https://stackoverflow.com/questions/33966121/what-is-the-algorithm-for-vibrance-filters
// The results of this implementation are very close to correct, but not quite perfect.
//
// Some further analysis available at:
// https://www.photo-mark.com/notes/analyzing-photoshop-vibrance-and-saturation/
//
// This algorithm is currently lacking a "Saturation" parameter which is needed for interoperability.
// It's not the same as the saturation component of Hue/Saturation/Value. Vibrance and Saturation are both separable.
// When both parameters are set, it is equivalent to running this adjustment twice, with only vibrance set and then only saturation set.
// (Except for some noise probably due to rounding error.)
#[node_macro::node(category("Raster: Adjustment"), shader_node(PerPixelAdjust))]
fn vibrance<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(
		Table<Raster<CPU>>,
		Table<Color>,
		Table<GradientStops>,
		GradientStops,
	)]
	#[gpu_image]
	mut image: T,
	vibrance: SignedPercentageF32,
) -> T {
	image.adjust(|color| {
		let vibrance = vibrance / 100.;
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
	image
}

/// Color Channel
#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, node_macro::ChoiceType, BufferStruct, FromPrimitive, IntoPrimitive)]
#[cfg_attr(feature = "std", derive(dyn_any::DynAny, specta::Type, serde::Serialize, serde::Deserialize))]
#[widget(Radio)]
pub enum RedGreenBlue {
	#[default]
	Red,
	Green,
	Blue,
}

/// Color Channel
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, node_macro::ChoiceType, bytemuck::NoUninit, BufferStruct, FromPrimitive, IntoPrimitive)]
#[cfg_attr(feature = "std", derive(dyn_any::DynAny, specta::Type, serde::Serialize, serde::Deserialize))]
#[widget(Radio)]
#[repr(u32)]
pub enum RedGreenBlueAlpha {
	#[default]
	Red,
	Green,
	Blue,
	Alpha,
}

/// Style of noise pattern
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, node_macro::ChoiceType)]
#[cfg_attr(feature = "std", derive(dyn_any::DynAny, specta::Type, serde::Serialize, serde::Deserialize))]
#[widget(Dropdown)]
pub enum NoiseType {
	#[default]
	Perlin,
	#[label("OpenSimplex2")]
	OpenSimplex2,
	#[label("OpenSimplex2S")]
	OpenSimplex2S,
	Cellular,
	ValueCubic,
	Value,
	WhiteNoise,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, node_macro::ChoiceType)]
#[cfg_attr(feature = "std", derive(dyn_any::DynAny, specta::Type, serde::Serialize, serde::Deserialize))]
/// Style of layered levels of the noise pattern
pub enum FractalType {
	#[default]
	None,
	#[label("Fractional Brownian Motion")]
	FBm,
	Ridged,
	PingPong,
	#[label("Progressive (Domain Warp Only)")]
	DomainWarpProgressive,
	#[label("Independent (Domain Warp Only)")]
	DomainWarpIndependent,
}

/// Distance function used by the cellular noise
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, node_macro::ChoiceType)]
#[cfg_attr(feature = "std", derive(dyn_any::DynAny, specta::Type, serde::Serialize, serde::Deserialize))]
pub enum CellularDistanceFunction {
	#[default]
	Euclidean,
	#[label("Euclidean Squared (Faster)")]
	EuclideanSq,
	Manhattan,
	Hybrid,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, node_macro::ChoiceType)]
#[cfg_attr(feature = "std", derive(dyn_any::DynAny, specta::Type, serde::Serialize, serde::Deserialize))]
pub enum CellularReturnType {
	CellValue,
	#[default]
	#[label("Nearest (F1)")]
	Nearest,
	#[label("Next Nearest (F2)")]
	NextNearest,
	#[label("Average (F1 / 2 + F2 / 2)")]
	Average,
	#[label("Difference (F2 - F1)")]
	Difference,
	#[label("Product (F2 * F1 / 2)")]
	Product,
	#[label("Division (F1 / F2)")]
	Division,
}

/// Type of domain warp
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, node_macro::ChoiceType)]
#[cfg_attr(feature = "std", derive(dyn_any::DynAny, specta::Type, serde::Serialize, serde::Deserialize))]
#[widget(Dropdown)]
pub enum DomainWarpType {
	#[default]
	None,
	#[label("OpenSimplex2")]
	OpenSimplex2,
	#[label("OpenSimplex2 Reduced")]
	OpenSimplex2Reduced,
	BasicGrid,
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27mixr%27%20%3D%20Channel%20Mixer
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=Lab%20color%20only-,Channel%20Mixer,-Key%20is%20%27mixr
#[node_macro::node(category("Raster: Adjustment"), properties("channel_mixer_properties"), shader_node(PerPixelAdjust))]
fn channel_mixer<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(
		Table<Raster<CPU>>,
		Table<Color>,
		Table<GradientStops>,
		GradientStops,
	)]
	#[gpu_image]
	mut image: T,

	monochrome: bool,

	#[default(40.)]
	#[name("Red")]
	monochrome_r: f32,
	#[default(40.)]
	#[name("Green")]
	monochrome_g: f32,
	#[default(20.)]
	#[name("Blue")]
	monochrome_b: f32,
	#[default(0.)]
	#[name("Constant")]
	monochrome_c: f32,

	#[default(100.)]
	#[name("(Red) Red")]
	red_r: f32,
	#[default(0.)]
	#[name("(Red) Green")]
	red_g: f32,
	#[default(0.)]
	#[name("(Red) Blue")]
	red_b: f32,
	#[default(0.)]
	#[name("(Red) Constant")]
	red_c: f32,

	#[default(0.)]
	#[name("(Green) Red")]
	green_r: f32,
	#[default(100.)]
	#[name("(Green) Green")]
	green_g: f32,
	#[default(0.)]
	#[name("(Green) Blue")]
	green_b: f32,
	#[default(0.)]
	#[name("(Green) Constant")]
	green_c: f32,

	#[default(0.)]
	#[name("(Blue) Red")]
	blue_r: f32,
	#[default(0.)]
	#[name("(Blue) Green")]
	blue_g: f32,
	#[default(100.)]
	#[name("(Blue) Blue")]
	blue_b: f32,
	#[default(0.)]
	#[name("(Blue) Constant")]
	blue_c: f32,

	// Display-only properties (not used within the node)
	_output_channel: RedGreenBlue,
) -> T {
	image.adjust(|color| {
		let color = color.to_gamma_srgb();

		let (r, g, b, a) = color.components();

		let color = if monochrome {
			let (monochrome_r, monochrome_g, monochrome_b, monochrome_c) = (monochrome_r / 100., monochrome_g / 100., monochrome_b / 100., monochrome_c / 100.);

			let gray = (r * monochrome_r + g * monochrome_g + b * monochrome_b + monochrome_c).clamp(0., 1.);

			Color::from_rgbaf32_unchecked(gray, gray, gray, a)
		} else {
			let (red_r, red_g, red_b, red_c) = (red_r / 100., red_g / 100., red_b / 100., red_c / 100.);
			let (green_r, green_g, green_b, green_c) = (green_r / 100., green_g / 100., green_b / 100., green_c / 100.);
			let (blue_r, blue_g, blue_b, blue_c) = (blue_r / 100., blue_g / 100., blue_b / 100., blue_c / 100.);

			let red = (r * red_r + g * red_g + b * red_b + red_c).clamp(0., 1.);
			let green = (r * green_r + g * green_g + b * green_b + green_c).clamp(0., 1.);
			let blue = (r * blue_r + g * blue_g + b * blue_b + blue_c).clamp(0., 1.);

			Color::from_rgbaf32_unchecked(red, green, blue, a)
		};

		color.to_linear_srgb()
	});
	image
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, node_macro::ChoiceType, BufferStruct, FromPrimitive, IntoPrimitive)]
#[cfg_attr(feature = "std", derive(dyn_any::DynAny, specta::Type, serde::Serialize, serde::Deserialize))]
#[widget(Radio)]
pub enum RelativeAbsolute {
	#[default]
	Relative,
	Absolute,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, node_macro::ChoiceType, BufferStruct, FromPrimitive, IntoPrimitive)]
#[cfg_attr(feature = "std", derive(dyn_any::DynAny, specta::Type, serde::Serialize, serde::Deserialize))]
pub enum SelectiveColorChoice {
	#[default]
	Reds,
	Yellows,
	Greens,
	Cyans,
	Blues,
	Magentas,

	#[menu_separator]
	Whites,
	Neutrals,
	Blacks,
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27selc%27%20%3D%20Selective%20color
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=from%20%2D100...100.%20.-,Selective%20Color,-Selective%20Color%20settings
//
// Algorithm based on:
// https://blog.pkh.me/p/22-understanding-selective-coloring-in-adobe-photoshop.html
#[node_macro::node(category("Raster: Adjustment"), properties("selective_color_properties"), shader_node(PerPixelAdjust))]
fn selective_color<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(
		Table<Raster<CPU>>,
		Table<Color>,
		Table<GradientStops>,
		GradientStops,
	)]
	#[gpu_image]
	mut image: T,

	mode: RelativeAbsolute,

	#[name("(Reds) Cyan")] r_c: f32,
	#[name("(Reds) Magenta")] r_m: f32,
	#[name("(Reds) Yellow")] r_y: f32,
	#[name("(Reds) Black")] r_k: f32,

	#[name("(Yellows) Cyan")] y_c: f32,
	#[name("(Yellows) Magenta")] y_m: f32,
	#[name("(Yellows) Yellow")] y_y: f32,
	#[name("(Yellows) Black")] y_k: f32,

	#[name("(Greens) Cyan")] g_c: f32,
	#[name("(Greens) Magenta")] g_m: f32,
	#[name("(Greens) Yellow")] g_y: f32,
	#[name("(Greens) Black")] g_k: f32,

	#[name("(Cyans) Cyan")] c_c: f32,
	#[name("(Cyans) Magenta")] c_m: f32,
	#[name("(Cyans) Yellow")] c_y: f32,
	#[name("(Cyans) Black")] c_k: f32,

	#[name("(Blues) Cyan")] b_c: f32,
	#[name("(Blues) Magenta")] b_m: f32,
	#[name("(Blues) Yellow")] b_y: f32,
	#[name("(Blues) Black")] b_k: f32,

	#[name("(Magentas) Cyan")] m_c: f32,
	#[name("(Magentas) Magenta")] m_m: f32,
	#[name("(Magentas) Yellow")] m_y: f32,
	#[name("(Magentas) Black")] m_k: f32,

	#[name("(Whites) Cyan")] w_c: f32,
	#[name("(Whites) Magenta")] w_m: f32,
	#[name("(Whites) Yellow")] w_y: f32,
	#[name("(Whites) Black")] w_k: f32,

	#[name("(Neutrals) Cyan")] n_c: f32,
	#[name("(Neutrals) Magenta")] n_m: f32,
	#[name("(Neutrals) Yellow")] n_y: f32,
	#[name("(Neutrals) Black")] n_k: f32,

	#[name("(Blacks) Cyan")] k_c: f32,
	#[name("(Blacks) Magenta")] k_m: f32,
	#[name("(Blacks) Yellow")] k_y: f32,
	#[name("(Blacks) Black")] k_k: f32,

	_colors: SelectiveColorChoice,
) -> T {
	image.adjust(|color| {
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

		let array = [
			(SelectiveColorChoice::Reds, (r_c, r_m, r_y, r_k)),
			(SelectiveColorChoice::Yellows, (y_c, y_m, y_y, y_k)),
			(SelectiveColorChoice::Greens, (g_c, g_m, g_y, g_k)),
			(SelectiveColorChoice::Cyans, (c_c, c_m, c_y, c_k)),
			(SelectiveColorChoice::Blues, (b_c, b_m, b_y, b_k)),
			(SelectiveColorChoice::Magentas, (m_c, m_m, m_y, m_k)),
			(SelectiveColorChoice::Whites, (w_c, w_m, w_y, w_k)),
			(SelectiveColorChoice::Neutrals, (n_c, n_m, n_y, n_k)),
			(SelectiveColorChoice::Blacks, (k_c, k_m, k_y, k_k)),
		];
		let mut sum = Vec3::ZERO;
		for i in 0..array.len() {
			let (color_parameter_group, (c, m, y, k)) = array[i];

			// Skip this color parameter group...
			// ...if it's unchanged from the default of zero offset on all CMYK parameters, or...
			// ...if this pixel's color isn't in the range affected by this color parameter group
			if (c < f32::EPSILON && m < f32::EPSILON && y < f32::EPSILON && k < f32::EPSILON) || (!pixel_color_range(color_parameter_group)) {
				continue;
			}

			let (c, m, y, k) = (c / 100., m / 100., y / 100., k / 100.);

			let color_parameter_group_scale_factor = match color_parameter_group {
				SelectiveColorChoice::Reds | SelectiveColorChoice::Greens | SelectiveColorChoice::Blues => color_parameter_group_scale_factor_rgb,
				SelectiveColorChoice::Cyans | SelectiveColorChoice::Magentas | SelectiveColorChoice::Yellows => color_parameter_group_scale_factor_cmy,
				SelectiveColorChoice::Whites => min(r, g, b) * 2. - 1.,
				SelectiveColorChoice::Neutrals => 1. - ((max(r, g, b) - 0.5).abs() + (min(r, g, b) - 0.5).abs()),
				SelectiveColorChoice::Blacks => 1. - max(r, g, b) * 2.,
			};

			let offset_r = f32::clamp((c + k * (c + 1.)) * slope_r, -r, -r + 1.) * color_parameter_group_scale_factor;
			let offset_g = f32::clamp((m + k * (m + 1.)) * slope_g, -g, -g + 1.) * color_parameter_group_scale_factor;
			let offset_b = f32::clamp((y + k * (y + 1.)) * slope_b, -b, -b + 1.) * color_parameter_group_scale_factor;

			sum += Vec3::new(offset_r, offset_g, offset_b);
		}

		let rgb = Vec3::new(r, g, b);
		let color = Color::from_vec4(Vec4::from(((sum + rgb).clamp(Vec3::ZERO, Vec3::ONE), a)));

		color.to_linear_srgb()
	});
	image
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=nvrt%27%20%3D%20Invert-,%27post%27%20%3D%20Posterize,-%27thrs%27%20%3D%20Threshold
//
// Algorithm based on:
// https://www.axiomx.com/posterize.htm
// This algorithm produces fully accurate output in relation to the industry standard.
#[node_macro::node(category("Raster: Adjustment"), shader_node(PerPixelAdjust))]
fn posterize<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(
		Table<Raster<CPU>>,
		Table<Color>,
		Table<GradientStops>,
		GradientStops,
	)]
	#[gpu_image]
	mut input: T,
	#[default(4)]
	#[hard_min(2.)]
	levels: u32,
) -> T {
	input.adjust(|color| {
		let color = color.to_gamma_srgb();

		let levels = levels as f32;
		let number_of_areas = levels.recip();
		let size_of_areas = (levels - 1.).recip();
		let channel = |channel: f32| (channel / number_of_areas).floor() * size_of_areas;
		let color = color.map_rgb(channel);

		color.to_linear_srgb()
	});
	input
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=curv%27%20%3D%20Curves-,%27expA%27%20%3D%20Exposure,-%27vibA%27%20%3D%20Vibrance
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=Flag%20(%20%3D%20128%20)-,Exposure,-Key%20is%20%27expA
//
// Algorithm based on:
// https://geraldbakker.nl/psnumbers/exposure.html
#[node_macro::node(category("Raster: Adjustment"), properties("exposure_properties"), shader_node(PerPixelAdjust))]
fn exposure<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(
		Table<Raster<CPU>>,
		Table<Color>,
		Table<GradientStops>,
		GradientStops,
	)]
	#[gpu_image]
	mut input: T,
	exposure: f32,
	offset: f32,
	#[default(1.)]
	#[range((0.01, 10.))]
	#[hard_min(0.0001)]
	gamma_correction: f32,
) -> T {
	input.adjust(|color| {
		let adjusted = color
			// Exposure
			.map_rgb(|c: f32| c * 2_f32.powf(exposure))
			// Offset
			.map_rgb(|c: f32| c + offset)
			// Gamma correction
			.gamma(gamma_correction);

		adjusted.map_rgb(|c: f32| c.clamp(0., 1.))
	});
	input
}
