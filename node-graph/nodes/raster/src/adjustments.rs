#![allow(clippy::too_many_arguments)]

use crate::adjust::Adjust;
#[cfg(feature = "std")]
use crate::color_lookup_table::LutCache;
use core::fmt::Debug;
#[cfg(feature = "std")]
use core_types::list::{Item, List};
#[cfg(feature = "std")]
use core_types::transfer_curve::{TransferCurve, TransferCurveEvaluator};
#[cfg(feature = "std")]
use glam::DVec2;
use glam::Vec3;
#[cfg(feature = "std")]
use graphene_resource::Resource;
use no_std_types::color::{Color, linear_to_srgb, set_luminosity, srgb_to_linear};
use no_std_types::context::Ctx;
#[cfg(not(feature = "std"))]
use no_std_types::list::ShaderItem as Item;
#[cfg(feature = "std")]
use no_std_types::registry::types::{Angle, Percentage, SignedPercentage};
use node_macro::BufferStruct;
use num_enum::{FromPrimitive, IntoPrimitive};
#[cfg(not(feature = "std"))]
use num_traits::float::Float;
#[cfg(feature = "std")]
use raster_types::{CPU, Raster};
#[cfg(feature = "std")]
use vector_types::Gradient;

/// Conversion from a color to grayscale.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "std", derive(dyn_any::DynAny))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash, node_macro::ChoiceType, bytemuck::NoUninit, BufferStruct, FromPrimitive, IntoPrimitive)]
#[widget(Dropdown)]
#[repr(u32)]
pub enum DesaturateMethod {
	/// Light level of the color, the Y (luminance) of Rec. 709, which weights the linear-light RGB channels by `0.2126, 0.7152, 0.0722`.
	///
	/// Accessibility contrast ratios and SVG luminance masks use this.
	#[default]
	#[label("Luminance (Rec. 709)")]
	#[cfg_attr(feature = "serde", serde(alias = "SRGB"))]
	LuminanceRec709,
	/// Light level approximation for the color, the Y′ (luma) of Rec. 709, which weights the gamma-encoded RGB channels by `0.2126, 0.7152, 0.0722`.
	///
	/// CSS filter functions such as `grayscale()` use this.
	#[label("Luma (Rec. 709)")]
	LumaRec709,
	/// Light level approximation for the color, the Y′ (luma) of Rec. 601, which weights the gamma-encoded RGB channels by `0.299, 0.587, 0.114`.
	///
	/// The Luminosity family of blend modes uses this, rounded to `0.3, 0.59, 0.11`.
	#[label("Luma (Rec. 601)")]
	LumaRec601,
	/// Perceptually uniform scale from black to white, the L (lightness) of OkLab.
	#[label("Lightness (OkLab)")]
	#[cfg_attr(feature = "serde", serde(alias = "Perceptual"))]
	LightnessOkLab,
	/// Mean of the three linear-light RGB channels.
	#[menu_separator]
	#[cfg_attr(feature = "serde", serde(alias = "AverageChannels"))]
	ChannelsAverage,
	/// Smallest of the three linear-light RGB channels.
	#[cfg_attr(feature = "serde", serde(alias = "MinimumChannels"))]
	ChannelsMinimum,
	/// Largest of the three linear-light RGB channels, the V (value) of HSV.
	#[cfg_attr(feature = "serde", serde(alias = "MaximumChannels"))]
	ChannelsMaximum,
	/// Midpoint of the largest and smallest gamma-encoded RGB channels, the L (lightness) of HSL.
	///
	/// The classic "Desaturate" command of many image editors uses this.
	#[label("Lightness (HSL)")]
	LightnessHsl,
}

#[node_macro::node(category("Raster: Adjustment"), shader_node(PerPixelAdjust))]
fn desaturate<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient)]
	#[gpu_image]
	input: Item<T>,
	method: Item<DesaturateMethod>,
) -> Item<T> {
	let mut input = input;
	let method = method.into_element();

	input.element_mut().adjust(|color| {
		// Gamma-encoded formulas are decoded as if they were a gray
		let gamma = || color.to_gamma_srgb_channels();
		let luminance = match method {
			DesaturateMethod::LuminanceRec709 => color.luminance_rec_709(),
			DesaturateMethod::LumaRec709 => {
				let [r, g, b, _] = gamma();
				srgb_to_linear(0.2126 * r + 0.7152 * g + 0.0722 * b)
			}
			DesaturateMethod::LumaRec601 => {
				let [r, g, b, _] = gamma();
				srgb_to_linear(0.299 * r + 0.587 * g + 0.114 * b)
			}
			DesaturateMethod::LightnessOkLab => {
				// A gray's OkLab lightness is the cube root of its linear value, so cubing gives the gray of equal lightness
				let lightness = color.lightness_oklab();
				lightness * lightness * lightness
			}
			DesaturateMethod::ChannelsAverage => color.average_rgb_channels(),
			DesaturateMethod::ChannelsMinimum => color.minimum_rgb_channels(),
			DesaturateMethod::ChannelsMaximum => color.maximum_rgb_channels(),
			DesaturateMethod::LightnessHsl => {
				// The transfer curve is monotonic, so the extremes are found first and only they are encoded
				let max = linear_to_srgb(color.maximum_rgb_channels());
				let min = linear_to_srgb(color.minimum_rgb_channels());
				srgb_to_linear((max + min) / 2.)
			}
		};
		color.map_rgb(|_| luminance)
	});
	input
}

#[node_macro::node(category("Raster: Adjustment"), shader_node(PerPixelAdjust))]
fn gamma_correction<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient)]
	#[gpu_image]
	input: Item<T>,
	#[default(2.2)]
	#[range]
	#[hard(0.0001..)]
	#[soft(0.01..10)]
	gamma: Item<f64>,
	inverse: Item<bool>,
) -> Item<T> {
	let mut input = input;
	let gamma = gamma.into_element();
	let inverse = inverse.into_element();

	let exponent = if inverse { 1. / gamma } else { gamma };
	input.element_mut().adjust(|color| color.apply_gamma_exponent(exponent as f32));
	input
}

#[node_macro::node(category("Raster: Channels"), shader_node(PerPixelAdjust))]
fn extract_channel<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient)]
	#[gpu_image]
	input: Item<T>,
	channel: Item<RedGreenBlueAlpha>,
) -> Item<T> {
	let mut input = input;
	let channel = channel.into_element();

	input.element_mut().adjust(|color| {
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
	#[implementations(Raster<CPU>, Color, Gradient)]
	#[gpu_image]
	input: Item<T>,
) -> Item<T> {
	let mut input = input;
	input.element_mut().adjust(|color| color.with_alpha(1.));
	input
}

/// Remaps a gamma-space channel through the stages of a Levels adjustment: the input range, the midtones gamma, and the output range.
fn apply_levels(value: f32, input_shadows: f32, input_highlights: f32, inverse_gamma: f32, output_minimum: f32, output_maximum: f32) -> f32 {
	let highlights_minus_shadows = (input_highlights - input_shadows).clamp(f32::EPSILON, 1.);
	let value = ((value - input_shadows).max(0.) / highlights_minus_shadows).min(1.);
	let value = value.powf(inverse_gamma);

	value * (output_maximum - output_minimum) + output_minimum
}

/// The classic Brightness/Contrast algorithm: a Levels remap around the pivot, adding the brightness before a positive
/// contrast stretch and after a negative contrast squeeze.
fn brightness_contrast_classic(value: f32, brightness: f32, contrast: f32, pivot: f32) -> f32 {
	// Full contrast is a hard step, sending values at the pivot or above to white (with half a 16-bit step of slack for float ties)
	if contrast >= 1. {
		return if value + brightness >= pivot - 1. / 65536. { 1. } else { 0. };
	}

	let result = if contrast > 0. {
		let input_shadows = pivot * contrast - brightness;
		apply_levels(value, input_shadows, input_shadows + 1. - contrast, 1., 0., 1.)
	} else {
		let output_minimum = brightness - pivot * contrast;
		apply_levels(value, 0., 1., 1., output_minimum, output_minimum + 1. + contrast)
	};

	result.clamp(0., 1.)
}

/// One brightness curve of the current algorithm, for a magnitude in 0..100: a line of slope 2^(b/110) up to an output of 0.5,
/// continued by a cubic Hermite segment that eases into (1, 1).
struct BrightnessCurve {
	slope: f32,
	knee: f32,
	end_slope: f32,
}

impl BrightnessCurve {
	fn new(brightness: f32) -> Self {
		let slope = 2_f32.powf(brightness / 110.);
		let knee = 0.5 / slope;
		let end_slope = (1. / (1. + 12. * (slope - 1.))).max(0.1);

		Self { slope, knee, end_slope }
	}

	/// Evaluates the Hermite segment at its parameter t in 0..1, returning the value and its derivative with respect to x.
	fn hermite(&self, t: f32) -> (f32, f32) {
		let length = 1. - self.knee;
		let start_tangent = length * self.slope;
		let end_tangent = length * self.end_slope;
		let t2 = t * t;
		let t3 = t2 * t;

		let value = (2. * t3 - 3. * t2 + 1.) * 0.5 + (t3 - 2. * t2 + t) * start_tangent + (-2. * t3 + 3. * t2) + (t3 - t2) * end_tangent;
		let derivative = ((6. * t2 - 6. * t) * 0.5 + (3. * t2 - 4. * t + 1.) * start_tangent + (-6. * t2 + 6. * t) + (3. * t2 - 2. * t) * end_tangent) / length;

		(value, derivative)
	}

	fn forward(&self, x: f32) -> f32 {
		if x < self.knee {
			return self.slope * x;
		}

		let t = ((x - self.knee) / (1. - self.knee)).min(1.);
		self.hermite(t).0.min(1.)
	}

	/// Inverts the curve, solving the monotone Hermite segment with a bracketed Newton iteration.
	fn inverse(&self, y: f32) -> f32 {
		if y <= 0.5 {
			return y / self.slope;
		}

		let mut low = 0.;
		let mut high = 1.;
		let mut t = (y - 0.5) * 2.;
		for _ in 0..8 {
			let (value, derivative) = self.hermite(t);
			let error = value - y;
			if error > 0. {
				high = t;
			} else {
				low = t;
			}

			let step = t - error / (derivative * (1. - self.knee));
			t = if step >= low && step <= high { step } else { (low + high) * 0.5 };
		}

		self.knee + t * (1. - self.knee)
	}
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27brit%27%20%3D%20Brightness/Contrast
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=Padding-,Brightness%20and%20Contrast,-Key%20is%20%27brit
//
// Some further analysis available at:
// https://geraldbakker.nl/psnumbers/brightness-contrast.html
//
// TODO: A Lab-only mode once Graphite supports the CIE Lab color space.
#[node_macro::node(name("Brightness/Contrast"), category("Raster: Adjustment"), properties("brightness_contrast_properties"), shader_node(PerPixelAdjust))]
fn brightness_contrast<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient)]
	#[gpu_image]
	input: Item<T>,
	brightness: Item<SignedPercentage>,
	contrast: Item<SignedPercentage>,
	use_classic: Item<bool>,
	#[default(127.)] classic_pivot: Item<f64>,
) -> Item<T> {
	let mut input = input;
	let brightness = brightness.into_element() as f32;
	let contrast = (contrast.into_element() / 100.) as f32;
	let use_classic = use_classic.into_element();
	let classic_pivot = (classic_pivot.into_element() / 255.) as f32;

	// Beyond a magnitude of 100, the curve for 100 is applied first and the curve for the remainder after it
	let magnitude = brightness.abs().min(150.);
	let first_curve = BrightnessCurve::new(magnitude.min(100.));
	let second_curve = BrightnessCurve::new((magnitude - 100.).max(0.));

	input.element_mut().adjust(|color| {
		color.map_gamma_rgb(|c| {
			if use_classic {
				return brightness_contrast_classic(c, brightness / 255., contrast, classic_pivot);
			}

			// Negative brightness runs the same curves in reverse
			let brightened = if brightness >= 0. {
				second_curve.forward(first_curve.forward(c))
			} else {
				first_curve.inverse(second_curve.inverse(c))
			};

			// Contrast pushes away from (or pulls toward) the midpoint, most strongly at the quarter tones
			let contrasted = brightened + 0.76 * contrast * (2. * brightened - 1.) * brightened.min(1. - brightened);
			contrasted.clamp(0., 1.)
		})
	});

	input
}

#[repr(u32)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "std", derive(dyn_any::DynAny))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, node_macro::ChoiceType, BufferStruct, FromPrimitive, IntoPrimitive)]
#[widget(Dropdown)]
/// The channel whose settings are shown, with RGB adjusting all three color channels together.
pub enum AdjustmentChannel {
	#[default]
	#[label("RGB")]
	Rgb,
	Red,
	Green,
	Blue,
	Alpha,
}

/// One Levels record in the node's units: percentage input and output points and the gamma value.
#[derive(Clone, Copy)]
struct LevelsRecord {
	shadows: f32,
	midtones: f32,
	highlights: f32,
	output_minimums: f32,
	output_maximums: f32,
}

/// A record's input curve followed by its output range.
#[derive(Clone, Copy)]
struct LevelsStage {
	curve: LevelsCurve,
	output_minimum: f32,
	output_maximum: f32,
}

impl LevelsRecord {
	fn new(shadows: f32, midtones: f32, highlights: f32, output_minimums: f32, output_maximums: f32) -> Self {
		Self {
			shadows,
			midtones,
			highlights,
			output_minimums,
			output_maximums,
		}
	}

	fn stage(&self, gamma: f32) -> LevelsStage {
		LevelsStage {
			curve: LevelsCurve::from_points(self.shadows * 2.55, self.highlights * 2.55, gamma),
			output_minimum: self.output_minimums / 100.,
			output_maximum: self.output_maximums / 100.,
		}
	}
}

impl LevelsStage {
	fn apply(&self, value: f32) -> f32 {
		self.curve.apply(value) * (self.output_maximum - self.output_minimum) + self.output_minimum
	}
}

/// A channel's record followed by the composite record.
#[derive(Clone, Copy)]
struct LevelsChain {
	first: LevelsStage,
	second: LevelsStage,
	two_stages: bool,
}

impl LevelsChain {
	fn new(channel: LevelsRecord, composite: LevelsRecord) -> Self {
		// For PSD interop, two power functions with nothing between them (the composite's input points and the
		// channel's output range at their defaults) merge into one curve with the product of the gammas, toe included
		let nothing_between = composite.shadows == 0. && composite.highlights == 100. && channel.output_minimums == 0. && channel.output_maximums == 100.;
		if nothing_between {
			let merged = LevelsRecord {
				output_minimums: composite.output_minimums,
				output_maximums: composite.output_maximums,
				..channel
			};
			let stage = merged.stage(channel.midtones * composite.midtones);
			Self {
				first: stage,
				second: stage,
				two_stages: false,
			}
		} else {
			Self {
				first: channel.stage(channel.midtones),
				second: composite.stage(composite.midtones),
				two_stages: true,
			}
		}
	}

	fn apply(&self, value: f32) -> f32 {
		let value = self.first.apply(value);
		if self.two_stages { self.second.apply(value) } else { value }
	}
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=levl%27%20%3D%20Levels
//
// Some further analysis available at:
// https://geraldbakker.nl/psnumbers/levels.html
#[node_macro::node(category("Raster: Adjustment"), properties("levels_properties"), shader_node(PerPixelAdjust))]
fn levels<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient)]
	#[gpu_image]
	image: Item<T>,
	#[default(0.)] shadows: Item<Percentage>,
	#[default(1.)] midtones: Item<f64>,
	#[default(100.)] highlights: Item<Percentage>,
	#[default(0.)] output_minimums: Item<Percentage>,
	#[default(100.)] output_maximums: Item<Percentage>,
	#[name("(Red) Shadows")]
	#[default(0.)]
	red_shadows: Item<Percentage>,
	#[name("(Red) Midtones")]
	#[default(1.)]
	red_midtones: Item<f64>,
	#[name("(Red) Highlights")]
	#[default(100.)]
	red_highlights: Item<Percentage>,
	#[name("(Red) Output Minimums")]
	#[default(0.)]
	red_output_minimums: Item<Percentage>,
	#[name("(Red) Output Maximums")]
	#[default(100.)]
	red_output_maximums: Item<Percentage>,
	#[name("(Green) Shadows")]
	#[default(0.)]
	green_shadows: Item<Percentage>,
	#[name("(Green) Midtones")]
	#[default(1.)]
	green_midtones: Item<f64>,
	#[name("(Green) Highlights")]
	#[default(100.)]
	green_highlights: Item<Percentage>,
	#[name("(Green) Output Minimums")]
	#[default(0.)]
	green_output_minimums: Item<Percentage>,
	#[name("(Green) Output Maximums")]
	#[default(100.)]
	green_output_maximums: Item<Percentage>,
	#[name("(Blue) Shadows")]
	#[default(0.)]
	blue_shadows: Item<Percentage>,
	#[name("(Blue) Midtones")]
	#[default(1.)]
	blue_midtones: Item<f64>,
	#[name("(Blue) Highlights")]
	#[default(100.)]
	blue_highlights: Item<Percentage>,
	#[name("(Blue) Output Minimums")]
	#[default(0.)]
	blue_output_minimums: Item<Percentage>,
	#[name("(Blue) Output Maximums")]
	#[default(100.)]
	blue_output_maximums: Item<Percentage>,
	#[name("(Alpha) Shadows")]
	#[default(0.)]
	alpha_shadows: Item<Percentage>,
	#[name("(Alpha) Midtones")]
	#[default(1.)]
	alpha_midtones: Item<f64>,
	#[name("(Alpha) Highlights")]
	#[default(100.)]
	alpha_highlights: Item<Percentage>,
	#[name("(Alpha) Output Minimums")]
	#[default(0.)]
	alpha_output_minimums: Item<Percentage>,
	#[name("(Alpha) Output Maximums")]
	#[default(100.)]
	alpha_output_maximums: Item<Percentage>,
	_channel: Item<AdjustmentChannel>,
) -> Item<T> {
	let mut image = image;
	let composite = LevelsRecord::new(
		shadows.into_element() as f32,
		midtones.into_element() as f32,
		highlights.into_element() as f32,
		output_minimums.into_element() as f32,
		output_maximums.into_element() as f32,
	);
	let red = LevelsChain::new(
		LevelsRecord::new(
			red_shadows.into_element() as f32,
			red_midtones.into_element() as f32,
			red_highlights.into_element() as f32,
			red_output_minimums.into_element() as f32,
			red_output_maximums.into_element() as f32,
		),
		composite,
	);
	let green = LevelsChain::new(
		LevelsRecord::new(
			green_shadows.into_element() as f32,
			green_midtones.into_element() as f32,
			green_highlights.into_element() as f32,
			green_output_minimums.into_element() as f32,
			green_output_maximums.into_element() as f32,
		),
		composite,
	);
	let blue = LevelsChain::new(
		LevelsRecord::new(
			blue_shadows.into_element() as f32,
			blue_midtones.into_element() as f32,
			blue_highlights.into_element() as f32,
			blue_output_minimums.into_element() as f32,
			blue_output_maximums.into_element() as f32,
		),
		composite,
	);

	// Alpha stands apart from the composite record that the three color channels pass through
	let alpha = LevelsRecord::new(
		alpha_shadows.into_element() as f32,
		alpha_midtones.into_element() as f32,
		alpha_highlights.into_element() as f32,
		alpha_output_minimums.into_element() as f32,
		alpha_output_maximums.into_element() as f32,
	);
	let alpha = alpha.stage(alpha.midtones);

	image.element_mut().adjust(|color| {
		// Levels math operates in gamma space
		let [r, g, b, a] = color.to_gamma_srgb_channels();

		Color::from_gamma_srgb_channels(red.apply(r), green.apply(g), blue.apply(b), alpha.apply(a))
	});
	image
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27curv%27%20%3D%20Curves
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=Curves%20file%20format
//
// Each curve is any number of (x, y) points on 0..1 joined by a natural cubic spline held flat beyond the outermost
// points, and the per-channel curves apply before the composite one, like Levels. The value between those two stages
// stays exact rather than rounding through an 8-bit table, which can leave results a level away from 8-bit pipelines.
// Needs the heap for its curves, so it stays off the shader build for now.
#[cfg(feature = "std")]
#[node_macro::node(category("Raster: Adjustment"), properties("transfer_curves_properties"))]
async fn curves<T: Adjust<Color> + Send>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient)] image: Item<T>,
	curve: Item<TransferCurve>,
	#[name("(Red) Curve")] red_curve: Item<TransferCurve>,
	#[name("(Green) Curve")] green_curve: Item<TransferCurve>,
	#[name("(Blue) Curve")] blue_curve: Item<TransferCurve>,
	#[name("(Alpha) Curve")] alpha_curve: Item<TransferCurve>,
	_channel: Item<AdjustmentChannel>,
) -> Item<T> {
	let mut image = image;
	let composite = curve.into_element().evaluator();
	let red = red_curve.into_element().evaluator();
	let green = green_curve.into_element().evaluator();
	let blue = blue_curve.into_element().evaluator();
	let alpha = alpha_curve.into_element().evaluator();
	let map = |channel: &TransferCurveEvaluator, value: f32| composite.evaluate(channel.evaluate(value as f64).clamp(0., 1.)).clamp(0., 1.) as f32;

	image.element_mut().adjust(|color| {
		// Curves math operates in gamma space
		let [r, g, b, a] = color.to_gamma_srgb_channels();

		// Alpha stands apart from the composite curve that the three color channels pass through
		let a = alpha.evaluate(a as f64).clamp(0., 1.) as f32;

		Color::from_gamma_srgb_channels(map(&red, r), map(&green, g), map(&blue, b), a)
	});

	image
}

/// Builds a transfer curve from a `Vec2[]` of control points, each mapping the input value at its x to the output value at its y. A smooth spline runs through them, holding the outermost points' values beyond them.
#[cfg(feature = "std")]
#[node_macro::node(category("Raster: Adjustment"), name("Points to Transfer Curve"))]
fn points_to_transfer_curve(
	_: impl Ctx,
	/// The control points, in any order, with both coordinates on the 0 to 1 range.
	points: List<DVec2>,
) -> Item<TransferCurve> {
	let points: Vec<DVec2> = points.iter_element_values().copied().collect();
	Item::new_from_element(TransferCurve::new(points))
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27blwh%27%20%3D%20Black%20and%20White
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=Black%20White%20(Photoshop%20CS3)
//
// Algorithm from:
// https://stackoverflow.com/a/55233732/775283
// Works the same for gamma and linear color
#[node_macro::node(name("Black & White"), category("Raster: Adjustment"), properties("black_and_white_properties"), shader_node(PerPixelAdjust))]
fn black_and_white<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient)]
	#[gpu_image]
	image: Item<T>,
	use_tint: Item<bool>,
	#[default("#e1d3b3")] tint: Item<Color>,
	#[default(40.)]
	#[range]
	#[soft(-200..300)]
	reds: Item<Percentage>,
	#[default(60.)]
	#[range]
	#[soft(-200..300)]
	yellows: Item<Percentage>,
	#[default(40.)]
	#[range]
	#[soft(-200..300)]
	greens: Item<Percentage>,
	#[default(60.)]
	#[range]
	#[soft(-200..300)]
	cyans: Item<Percentage>,
	#[default(20.)]
	#[range]
	#[soft(-200..300)]
	blues: Item<Percentage>,
	#[default(80.)]
	#[range]
	#[soft(-200..300)]
	magentas: Item<Percentage>,
) -> Item<T> {
	let mut image = image;
	let tint = tint.into_element();
	let use_tint = use_tint.into_element();
	let reds = reds.into_element();
	let yellows = yellows.into_element();
	let greens = greens.into_element();
	let cyans = cyans.into_element();
	let blues = blues.into_element();
	let magentas = magentas.into_element();

	image.element_mut().adjust(|color| {
		// Black & White channel weights are tuned for gamma-space values
		let [r, g, b, alpha_part] = color.to_gamma_srgb_channels();

		let reds = (reds / 100.) as f32;
		let yellows = (yellows / 100.) as f32;
		let greens = (greens / 100.) as f32;
		let cyans = (cyans / 100.) as f32;
		let blues = (blues / 100.) as f32;
		let magentas = (magentas / 100.) as f32;

		let gray_base = r.min(g).min(b);

		let red_part = r - gray_base;
		let green_part = g - gray_base;
		let blue_part = b - gray_base;

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

		let luminance = (gray_base + additional).clamp(0., 1.);
		if !use_tint {
			return Color::from_gamma_srgb_channels(luminance, luminance, luminance, alpha_part);
		}

		// The tint takes on the gray's luminosity the way the Luminosity blend mode would
		let [tint_r, tint_g, tint_b, _] = tint.to_gamma_srgb_channels();
		let tint_luma = luma_rec_601_fixed_point(tint_r, tint_g, tint_b);
		let [tinted_r, tinted_g, tinted_b] = set_luminosity(tint_r, tint_g, tint_b, tint_luma, luminance);

		Color::from_gamma_srgb_channels(tinted_r, tinted_g, tinted_b, alpha_part)
	});
	image
}

#[repr(u32)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "std", derive(dyn_any::DynAny))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, node_macro::ChoiceType, BufferStruct, FromPrimitive, IntoPrimitive)]
#[widget(Dropdown)]
pub enum HueSaturationRange {
	#[default]
	Master,
	Reds,
	Yellows,
	Greens,
	Cyans,
	Blues,
	Magentas,
}

/// HSL of gamma-encoded channels: hue in degrees, saturation and lightness in 0..1.
fn gamma_rgb_to_hsl(r: f32, g: f32, b: f32) -> [f32; 3] {
	let maximum = r.max(g).max(b);
	let minimum = r.min(g).min(b);
	let chroma = maximum - minimum;
	let lightness = (maximum + minimum) / 2.;
	if chroma <= 0. {
		return [0., 0., lightness];
	}

	let saturation = chroma / (1. - (2. * lightness - 1.).abs()).max(1e-6);
	[hexagon_hue_degrees(r, g, b), saturation.min(1.), lightness]
}

/// Hexagon hue in degrees of three channels in any encoding, 0 for gray.
fn hexagon_hue_degrees(r: f32, g: f32, b: f32) -> f32 {
	let maximum = r.max(g).max(b);
	let chroma = maximum - r.min(g).min(b);
	if chroma <= 0. {
		return 0.;
	}

	let sector = if maximum == r {
		wrap_positive((g - b) / chroma, 6.)
	} else if maximum == g {
		(b - r) / chroma + 2.
	} else {
		(r - g) / chroma + 4.
	};
	sector * 60.
}

/// `value` wrapped into the range from 0 to `modulus`.
fn wrap_positive(value: f32, modulus: f32) -> f32 {
	value - (value / modulus).floor() * modulus
}

/// Gamma-encoded channels from a hue in degrees and saturation and lightness in 0..1.
fn hsl_to_gamma_rgb(hue: f32, saturation: f32, lightness: f32) -> [f32; 3] {
	let chroma = (1. - (2. * lightness - 1.).abs()) * saturation;
	let sector = wrap_positive(hue, 360.) / 60.;
	let x = chroma * (1. - (wrap_positive(sector, 2.) - 1.).abs());
	let (r, g, b) = if sector < 1. {
		(chroma, x, 0.)
	} else if sector < 2. {
		(x, chroma, 0.)
	} else if sector < 3. {
		(0., chroma, x)
	} else if sector < 4. {
		(0., x, chroma)
	} else if sector < 5. {
		(x, 0., chroma)
	} else {
		(chroma, 0., x)
	};
	let m = lightness - chroma / 2.;

	[(r + m).clamp(0., 1.), (g + m).clamp(0., 1.), (b + m).clamp(0., 1.)]
}

/// One set of Hue/Saturation sliders: a hue shift in degrees and saturation and lightness amounts in -1..1.
#[derive(Clone, Copy)]
struct HueSaturationSettings {
	hue: f32,
	saturation: f32,
	lightness: f32,
}

impl HueSaturationSettings {
	fn new(hue: f32, saturation_percent: f32, lightness_percent: f32) -> Self {
		Self {
			hue,
			saturation: (saturation_percent / 100.).clamp(-1., 1.),
			lightness: (lightness_percent / 100.).clamp(-1., 1.),
		}
	}
}

/// A hue range with its falloff: full weight from `range_start` to `range_end`, fading linearly to zero at the falloff ends.
#[derive(Clone, Copy)]
struct HueSaturationRangeSettings {
	falloff_start: f32,
	range_start: f32,
	range_end: f32,
	falloff_end: f32,
	settings: HueSaturationSettings,
}

impl HueSaturationRangeSettings {
	fn new(falloff_start: f32, range_start: f32, range_end: f32, falloff_end: f32, settings: HueSaturationSettings) -> Self {
		// For PSD interop, each edge rounds to 1536 hue units per turn over 359 rather than 360 degrees, landing up to a degree late
		let edge = |degrees: f32| (degrees * 1536. / 359.).round() * 360. / 1536.;

		Self {
			falloff_start: edge(falloff_start),
			range_start: edge(range_start),
			range_end: edge(range_end),
			falloff_end: edge(falloff_end),
			settings,
		}
	}

	fn weight(&self, hue: f32) -> f32 {
		let distance = |from: f32, to: f32| wrap_positive(to - from, 360.);
		if distance(self.range_start, hue) <= distance(self.range_start, self.range_end) {
			return 1.;
		}
		let start_falloff = distance(self.falloff_start, self.range_start);
		let end_falloff = distance(self.range_end, self.falloff_end);
		if distance(self.falloff_start, hue) < start_falloff {
			return distance(self.falloff_start, hue) / start_falloff;
		}
		if distance(self.range_end, hue) < end_falloff {
			return 1. - distance(self.range_end, hue) / end_falloff;
		}
		0.
	}
}

/// The six ranges' combined effect on one pixel, gathered before the master sliders apply.
struct HueSaturationRangeEffect {
	hue_shift: f32,
	saturation_factor: f32,
	fully_saturate: bool,
	rgb: [f32; 3],
}

impl HueSaturationRangeEffect {
	fn apply(&mut self, range: &HueSaturationRangeSettings, original_hue: f32, original_saturation: f32) {
		// Range weights come from the original hue, and grays belong to no range
		let weight = if original_saturation > 0. { range.weight(original_hue) } else { 0. };
		if weight <= 0. {
			return;
		}

		self.hue_shift += range.settings.hue * weight;
		// For PSD interop, +100 saturates fully from the very edge of the falloff rather than scaling with the weight
		if range.settings.saturation >= 1. {
			self.fully_saturate = true;
		} else {
			self.saturation_factor *= 1. + (saturation_gain(range.settings.saturation) - 1.) * weight;
		}
		self.rgb = lightness_toward_max_or_min(self.rgb, range.settings.lightness * weight);
	}
}

/// The factor a saturation amount in -1..1 applies to HSL saturation, quantized for PSD interop: 1 - trunc(256 a) / 256 below zero and floor(65280 / (255 - trunc(254 a))) / 256 above.
fn saturation_gain(amount: f32) -> f32 {
	if amount < 0. {
		1. - (-amount * 256.).trunc() / 256.
	} else {
		(65280. / (255. - (amount * 254.).trunc())).floor() / 256.
	}
}

/// Blends toward white for a positive amount and toward black for a negative one, as the master lightness slider does.
fn lightness_toward_white_or_black(value: f32, amount: f32) -> f32 {
	if amount >= 0. { value + (1. - value) * amount } else { value * (1. + amount) }
}

/// A range's lightness moves the channels toward the color's own maximum (positive) or minimum (negative) instead.
fn lightness_toward_max_or_min(rgb: [f32; 3], amount: f32) -> [f32; 3] {
	let maximum = rgb[0].max(rgb[1]).max(rgb[2]);
	let minimum = rgb[0].min(rgb[1]).min(rgb[2]);
	let toward = if amount >= 0. { maximum } else { minimum };
	let blend = |value: f32| value + (toward - value) * amount.abs();
	[blend(rgb[0]), blend(rgb[1]), blend(rgb[2])]
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27hue%20%27%20%3D%20Old,saturation%2C%20Photoshop%205.0
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=0%20%3D%20Use%20other.-,Hue/Saturation,-Hue/Saturation%20settings
//
// TODO: Residuals in 8-bit PSD interop: the byte-hue colorize table, the colorize lightness slider (up to 2.5 levels), and the range edges (a few tenths of a degree)
#[node_macro::node(name("Hue/Saturation"), category("Raster: Adjustment"), properties("hue_saturation_properties"), shader_node(PerPixelAdjust))]
fn hue_saturation<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient)]
	#[gpu_image]
	input: Item<T>,
	hue: Item<Angle>,
	saturation: Item<SignedPercentage>,
	lightness: Item<SignedPercentage>,
	colorize: Item<bool>,
	#[name("(Colorize) Hue")]
	#[default(24.)]
	colorize_hue: Item<Angle>,
	#[name("(Colorize) Saturation")]
	#[default(25.)]
	colorize_saturation: Item<Percentage>,
	#[name("(Colorize) Lightness")] colorize_lightness: Item<SignedPercentage>,
	#[name("(Reds) Hue")] reds_hue: Item<Angle>,
	#[name("(Reds) Saturation")] reds_saturation: Item<SignedPercentage>,
	#[name("(Reds) Lightness")] reds_lightness: Item<SignedPercentage>,
	#[name("(Reds) Falloff Start")]
	#[default(315.)]
	reds_falloff_start: Item<f64>,
	#[name("(Reds) Range Start")]
	#[default(345.)]
	reds_range_start: Item<f64>,
	#[name("(Reds) Range End")]
	#[default(15.)]
	reds_range_end: Item<f64>,
	#[name("(Reds) Falloff End")]
	#[default(45.)]
	reds_falloff_end: Item<f64>,
	#[name("(Yellows) Hue")] yellows_hue: Item<Angle>,
	#[name("(Yellows) Saturation")] yellows_saturation: Item<SignedPercentage>,
	#[name("(Yellows) Lightness")] yellows_lightness: Item<SignedPercentage>,
	#[name("(Yellows) Falloff Start")]
	#[default(15.)]
	yellows_falloff_start: Item<f64>,
	#[name("(Yellows) Range Start")]
	#[default(45.)]
	yellows_range_start: Item<f64>,
	#[name("(Yellows) Range End")]
	#[default(75.)]
	yellows_range_end: Item<f64>,
	#[name("(Yellows) Falloff End")]
	#[default(105.)]
	yellows_falloff_end: Item<f64>,
	#[name("(Greens) Hue")] greens_hue: Item<Angle>,
	#[name("(Greens) Saturation")] greens_saturation: Item<SignedPercentage>,
	#[name("(Greens) Lightness")] greens_lightness: Item<SignedPercentage>,
	#[name("(Greens) Falloff Start")]
	#[default(75.)]
	greens_falloff_start: Item<f64>,
	#[name("(Greens) Range Start")]
	#[default(105.)]
	greens_range_start: Item<f64>,
	#[name("(Greens) Range End")]
	#[default(135.)]
	greens_range_end: Item<f64>,
	#[name("(Greens) Falloff End")]
	#[default(165.)]
	greens_falloff_end: Item<f64>,
	#[name("(Cyans) Hue")] cyans_hue: Item<Angle>,
	#[name("(Cyans) Saturation")] cyans_saturation: Item<SignedPercentage>,
	#[name("(Cyans) Lightness")] cyans_lightness: Item<SignedPercentage>,
	#[name("(Cyans) Falloff Start")]
	#[default(135.)]
	cyans_falloff_start: Item<f64>,
	#[name("(Cyans) Range Start")]
	#[default(165.)]
	cyans_range_start: Item<f64>,
	#[name("(Cyans) Range End")]
	#[default(195.)]
	cyans_range_end: Item<f64>,
	#[name("(Cyans) Falloff End")]
	#[default(225.)]
	cyans_falloff_end: Item<f64>,
	#[name("(Blues) Hue")] blues_hue: Item<Angle>,
	#[name("(Blues) Saturation")] blues_saturation: Item<SignedPercentage>,
	#[name("(Blues) Lightness")] blues_lightness: Item<SignedPercentage>,
	#[name("(Blues) Falloff Start")]
	#[default(195.)]
	blues_falloff_start: Item<f64>,
	#[name("(Blues) Range Start")]
	#[default(225.)]
	blues_range_start: Item<f64>,
	#[name("(Blues) Range End")]
	#[default(255.)]
	blues_range_end: Item<f64>,
	#[name("(Blues) Falloff End")]
	#[default(285.)]
	blues_falloff_end: Item<f64>,
	#[name("(Magentas) Hue")] magentas_hue: Item<Angle>,
	#[name("(Magentas) Saturation")] magentas_saturation: Item<SignedPercentage>,
	#[name("(Magentas) Lightness")] magentas_lightness: Item<SignedPercentage>,
	#[name("(Magentas) Falloff Start")]
	#[default(255.)]
	magentas_falloff_start: Item<f64>,
	#[name("(Magentas) Range Start")]
	#[default(285.)]
	magentas_range_start: Item<f64>,
	#[name("(Magentas) Range End")]
	#[default(315.)]
	magentas_range_end: Item<f64>,
	#[name("(Magentas) Falloff End")]
	#[default(345.)]
	magentas_falloff_end: Item<f64>,
	_range: Item<HueSaturationRange>,
) -> Item<T> {
	let mut input = input;
	let master = HueSaturationSettings::new(hue.into_element() as f32, saturation.into_element() as f32, lightness.into_element() as f32);
	let colorize = colorize.into_element();
	let colorize_settings = HueSaturationSettings::new(colorize_hue.into_element() as f32, colorize_saturation.into_element() as f32, colorize_lightness.into_element() as f32);
	let (reds, yellows, greens, cyans, blues, magentas) = (
		HueSaturationRangeSettings::new(
			reds_falloff_start.into_element() as f32,
			reds_range_start.into_element() as f32,
			reds_range_end.into_element() as f32,
			reds_falloff_end.into_element() as f32,
			HueSaturationSettings::new(reds_hue.into_element() as f32, reds_saturation.into_element() as f32, reds_lightness.into_element() as f32),
		),
		HueSaturationRangeSettings::new(
			yellows_falloff_start.into_element() as f32,
			yellows_range_start.into_element() as f32,
			yellows_range_end.into_element() as f32,
			yellows_falloff_end.into_element() as f32,
			HueSaturationSettings::new(yellows_hue.into_element() as f32, yellows_saturation.into_element() as f32, yellows_lightness.into_element() as f32),
		),
		HueSaturationRangeSettings::new(
			greens_falloff_start.into_element() as f32,
			greens_range_start.into_element() as f32,
			greens_range_end.into_element() as f32,
			greens_falloff_end.into_element() as f32,
			HueSaturationSettings::new(greens_hue.into_element() as f32, greens_saturation.into_element() as f32, greens_lightness.into_element() as f32),
		),
		HueSaturationRangeSettings::new(
			cyans_falloff_start.into_element() as f32,
			cyans_range_start.into_element() as f32,
			cyans_range_end.into_element() as f32,
			cyans_falloff_end.into_element() as f32,
			HueSaturationSettings::new(cyans_hue.into_element() as f32, cyans_saturation.into_element() as f32, cyans_lightness.into_element() as f32),
		),
		HueSaturationRangeSettings::new(
			blues_falloff_start.into_element() as f32,
			blues_range_start.into_element() as f32,
			blues_range_end.into_element() as f32,
			blues_falloff_end.into_element() as f32,
			HueSaturationSettings::new(blues_hue.into_element() as f32, blues_saturation.into_element() as f32, blues_lightness.into_element() as f32),
		),
		HueSaturationRangeSettings::new(
			magentas_falloff_start.into_element() as f32,
			magentas_range_start.into_element() as f32,
			magentas_range_end.into_element() as f32,
			magentas_falloff_end.into_element() as f32,
			HueSaturationSettings::new(magentas_hue.into_element() as f32, magentas_saturation.into_element() as f32, magentas_lightness.into_element() as f32),
		),
	);

	input.element_mut().adjust(|color| {
		let [r, g, b, alpha] = color.to_gamma_srgb_channels();

		if colorize {
			let [_, _, lightness] = gamma_rgb_to_hsl(r, g, b);
			let lightness = lightness_toward_white_or_black(lightness, colorize_settings.lightness);
			let saturation = colorize_settings.saturation.max(0.);
			let [r, g, b] = hsl_to_gamma_rgb(colorize_settings.hue, saturation, lightness);
			return Color::from_gamma_srgb_channels(r, g, b, alpha);
		}

		// Each range weights its sliders by its falloff around the original hue: hue shifts add and saturation gains multiply
		let [original_hue, original_saturation, _] = gamma_rgb_to_hsl(r, g, b);
		let mut effect = HueSaturationRangeEffect {
			hue_shift: master.hue,
			saturation_factor: 1.,
			fully_saturate: false,
			rgb: [r, g, b],
		};
		effect.apply(&reds, original_hue, original_saturation);
		effect.apply(&yellows, original_hue, original_saturation);
		effect.apply(&greens, original_hue, original_saturation);
		effect.apply(&cyans, original_hue, original_saturation);
		effect.apply(&blues, original_hue, original_saturation);
		effect.apply(&magentas, original_hue, original_saturation);
		let HueSaturationRangeEffect {
			hue_shift,
			mut saturation_factor,
			mut fully_saturate,
			rgb,
		} = effect;
		if master.saturation >= 1. {
			fully_saturate = true;
		} else {
			saturation_factor *= saturation_gain(master.saturation);
		}

		// The master lightness blends toward white or black before the hue and saturation, which work in HSL of the gamma channels
		let rgb = [
			lightness_toward_white_or_black(rgb[0], master.lightness),
			lightness_toward_white_or_black(rgb[1], master.lightness),
			lightness_toward_white_or_black(rgb[2], master.lightness),
		];
		let [hue, saturation, lightness] = gamma_rgb_to_hsl(rgb[0], rgb[1], rgb[2]);
		let saturation = if saturation <= 0. {
			0.
		} else if fully_saturate {
			1.
		} else {
			(saturation * saturation_factor).min(1.)
		};
		let [r, g, b] = hsl_to_gamma_rgb(hue + hue_shift, saturation, lightness);

		Color::from_gamma_srgb_channels(r, g, b, alpha)
	});
	input
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27%20%3D%20Color%20Lookup-,%27nvrt%27%20%3D%20Invert,-%27post%27%20%3D%20Posterize
#[node_macro::node(category("Raster: Adjustment"), shader_node(PerPixelAdjust))]
fn invert<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient)]
	#[gpu_image]
	input: Item<T>,
) -> Item<T> {
	let mut input = input;
	input.element_mut().adjust(|color| color.map_gamma_rgb(|channel| 1. - channel));
	input
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=post%27%20%3D%20Posterize-,%27thrs%27%20%3D%20Threshold,-%27grdm%27%20%3D%20Gradient
#[node_macro::node(category("Raster: Adjustment"), properties("threshold_properties"), shader_node(PerPixelAdjust))]
fn threshold<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient)]
	#[gpu_image]
	image: Item<T>,
	#[default(50.)] min_luminance: Item<Percentage>,
	#[default(100.)] max_luminance: Item<Percentage>,
) -> Item<T> {
	let mut image = image;
	let min_luminance = (min_luminance.into_element() / 100.) as f32;
	let max_luminance = (max_luminance.into_element() / 100.) as f32;

	image.element_mut().adjust(|color| {
		// For PSD interop, we compare this 14-bit fixed-point Rec. 601 luma against the level unrounded
		let [r, g, b, _] = color.to_gamma_srgb_channels();
		let luminance = (4915. * r + 9667. * g + 1802. * b) / 16384.;

		let output = if luminance >= min_luminance && luminance <= max_luminance { Color::WHITE } else { Color::BLACK };
		output.with_alpha(color.a())
	});
	image
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27grdm%27%20%3D%20Gradient%20Map
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=Gradient%20settings%20(Photoshop%206.0)
//
// TODO: Full PSD interop needs a compatibility variant of `GradientInterpolation` with its own midpoint semantics, position warp,
// TODO: and smoothing (a `gradient_smoothness` attribute), plus noise gradients, which we don't yet support.
// TODO: Its axes differ from ours: its midpoint is always a knee in the position warp and its smoothness blends the curve over
// TODO: that fixed warp, while each variant here picks warp and curve together, so neither end of the blend is Linear or Smooth.
// TODO: Per channel in the gradient space (measured on gamma RGB):
// TODO: - Position t maps to a parameter p by a piecewise-linear knee through (stop position, index) and (midpoint, index - 0.5).
// TODO: - Linear lerps the interval's stop colors by the fraction of p. Smooth is a cubic Hermite over the stop index with tangent
// TODO:   `(c[i + 1] - c[i - 1]) / 2`, the end stops repeated past the ends, so two stops give `0.5 p + 1.5 p^2 - p^3`.
// TODO: - The ramp is `(1 - s) * linear + s * smooth` for smoothness s, clamped per interval to its two stop colors.
#[cfg(feature = "std")]
#[node_macro::node(category("Raster: Adjustment"))]
async fn gradient_map<T: Adjust<Color> + Send>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient)] image: Item<T>,
	#[default(Color::BLACK, Color::WHITE)] gradient: Item<Gradient>,
	reverse: Item<bool>,
) -> Item<T> {
	let mut image = image;
	let settings = vector_types::GradientSettings::from(&gradient);
	let evaluator = gradient.into_element().evaluator(settings);
	let reverse = reverse.into_element();

	image.element_mut().adjust(|color| {
		// The classic 0.3/0.59/0.11 luma of the gamma-encoded channels picks the position along the gradient
		let [r, g, b, alpha] = color.to_gamma_srgb_channels();
		let intensity = 0.3 * r + 0.59 * g + 0.11 * b;
		let intensity = if reverse { 1. - intensity } else { intensity };

		// The source alpha is kept and the gradient's own alpha stops are ignored
		evaluator.evaluate(intensity as f64).with_alpha(alpha)
	});

	image
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27-,vibA%27%20%3D%20Vibrance,-%27hue%20%27%20%3D%20Old
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=Vibrance%20(Photoshop%20CS3)
#[node_macro::node(category("Raster: Adjustment"), properties("vibrance_properties"), shader_node(PerPixelAdjust))]
fn vibrance<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient)]
	#[gpu_image]
	image: Item<T>,
	vibrance: Item<SignedPercentage>,
	saturation: Item<SignedPercentage>,
) -> Item<T> {
	let mut image = image;
	let vibrance = (vibrance.into_element().clamp(-100., 100.) / 100.) as f32;
	let saturation_scale = (1. + saturation.into_element().clamp(-100., 100.) / 100.) as f32;

	// Vibrance then saturation, both in linear light, which equals applying each alone in turn
	image.element_mut().adjust(|color| {
		let (r, g, b) = (color.r(), color.g(), color.b());
		let maximum = r.max(g).max(b);
		let [chroma_factor, brightness_factor] = vibrance_factors(r, g, b, vibrance);
		let after_vibrance = Color::from_rgbaf32_unchecked(
			scale_about_maximum(r, maximum, chroma_factor, brightness_factor),
			scale_about_maximum(g, maximum, chroma_factor, brightness_factor),
			scale_about_maximum(b, maximum, chroma_factor, brightness_factor),
			color.a(),
		);

		// For PSD interop, saturation scales each channel's distance from a gray weighted by ProPhoto's luminance coefficients
		let gray = 0.288040 * after_vibrance.r() + 0.711874 * after_vibrance.g() + 0.000086 * after_vibrance.b();
		after_vibrance.map_rgb(|c| (gray + (c - gray) * saturation_scale).clamp(0., 1.))
	});
	image
}

fn scale_about_maximum(channel: f32, maximum: f32, chroma_factor: f32, brightness_factor: f32) -> f32 {
	(brightness_factor * (maximum + chroma_factor * (channel - maximum))).clamp(0., 1.)
}

/// Share of the vibrance boost the skin-tone protection removes at full weight, a fitted constant.
const VIBRANCE_PROTECTION_LOSS: f32 = 0.4857;

/// Vibrance on linear SDR channels as `[chroma factor about the max, brightness multiply]` for an amount in -1..1, both fading out toward black.
/// Negative desaturates and darkens low-chroma colors most. Positive boosts them, brightens a little, and spares reds.
fn vibrance_factors(r: f32, g: f32, b: f32, amount: f32) -> [f32; 2] {
	let maximum = r.max(g).max(b);
	let minimum = r.min(g).min(b);
	if maximum <= 0. {
		return [1., 1.];
	}
	let ratio = minimum / maximum;
	let saturation = 1. - ratio;
	let q = ratio * saturation;
	let toe = 1. - (16. * maximum).min(1.);
	let rolloff = 1. - toe * toe;
	let brightness = (2. * q - q * q) * (1. - maximum) * rolloff;

	if amount < 0. {
		let amount = -amount;
		let chroma_factor = (1. - amount / 4.) * (1. - amount * (1. - rolloff * saturation * (1. + saturation) / 2.));
		return [chroma_factor, 1. - amount * brightness];
	}

	let protection = skin_tone_window(hexagon_hue_degrees(r, g, b)) * (1. - saturation * saturation);
	let amount = amount * (1. - protection * (1. - amount));
	let boost = (5. / 6.) * (1. - VIBRANCE_PROTECTION_LOSS * protection) * amount * ratio * (1. - minimum) * rolloff;
	[1. / (1. - boost), 1. + amount * brightness / 4.]
}

/// How fully a hue falls under the skin-tone protection: all of it from red to 30 degrees, fading out by 45, and back in from 300.
fn skin_tone_window(hue: f32) -> f32 {
	if hue < 45. {
		((45. - hue) / 15.).min(1.)
	} else if hue >= 300. {
		(hue - 300.) / 60.
	} else {
		0.
	}
}

#[repr(u32)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "std", derive(dyn_any::DynAny))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, node_macro::ChoiceType, BufferStruct, FromPrimitive, IntoPrimitive)]
#[widget(Radio)]
pub enum RedGreenBlue {
	#[default]
	Red,
	Green,
	Blue,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "std", derive(dyn_any::DynAny))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, node_macro::ChoiceType, bytemuck::NoUninit, BufferStruct, FromPrimitive, IntoPrimitive)]
#[widget(Radio)]
#[repr(u32)]
pub enum RedGreenBlueAlpha {
	#[default]
	Red,
	Green,
	Blue,
	Alpha,
}

/// Style of noise pattern.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "std", derive(dyn_any::DynAny))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, node_macro::ChoiceType)]
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

/// Style of layered levels of the noise pattern.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "std", derive(dyn_any::DynAny))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, node_macro::ChoiceType)]
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

/// Distance function used by the cellular noise.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "std", derive(dyn_any::DynAny))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, node_macro::ChoiceType)]
pub enum CellularDistanceFunction {
	#[default]
	Euclidean,
	#[label("Euclidean Squared (Faster)")]
	EuclideanSq,
	Manhattan,
	Hybrid,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "std", derive(dyn_any::DynAny))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, node_macro::ChoiceType)]
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

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "std", derive(dyn_any::DynAny))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, node_macro::ChoiceType)]
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
//
// TODO: CMYK source channels once Graphite supports the CMYK color space.
#[node_macro::node(category("Raster: Adjustment"), properties("channel_mixer_properties"), shader_node(PerPixelAdjust))]
fn channel_mixer<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient)]
	#[gpu_image]
	image: Item<T>,

	monochrome: Item<bool>,

	#[default(40.)]
	#[name("Red")]
	monochrome_r: Item<f64>,
	#[default(40.)]
	#[name("Green")]
	monochrome_g: Item<f64>,
	#[default(20.)]
	#[name("Blue")]
	monochrome_b: Item<f64>,
	#[default(0.)]
	#[name("Constant")]
	monochrome_c: Item<f64>,

	#[default(100.)]
	#[name("(Red) Red")]
	red_r: Item<f64>,
	#[default(0.)]
	#[name("(Red) Green")]
	red_g: Item<f64>,
	#[default(0.)]
	#[name("(Red) Blue")]
	red_b: Item<f64>,
	#[default(0.)]
	#[name("(Red) Constant")]
	red_c: Item<f64>,

	#[default(0.)]
	#[name("(Green) Red")]
	green_r: Item<f64>,
	#[default(100.)]
	#[name("(Green) Green")]
	green_g: Item<f64>,
	#[default(0.)]
	#[name("(Green) Blue")]
	green_b: Item<f64>,
	#[default(0.)]
	#[name("(Green) Constant")]
	green_c: Item<f64>,

	#[default(0.)]
	#[name("(Blue) Red")]
	blue_r: Item<f64>,
	#[default(0.)]
	#[name("(Blue) Green")]
	blue_g: Item<f64>,
	#[default(100.)]
	#[name("(Blue) Blue")]
	blue_b: Item<f64>,
	#[default(0.)]
	#[name("(Blue) Constant")]
	blue_c: Item<f64>,

	// Display-only properties (not used within the node)
	_output_channel: Item<RedGreenBlue>,
) -> Item<T> {
	let mut image = image;
	let monochrome = monochrome.into_element();
	let (monochrome_r, monochrome_g, monochrome_b, monochrome_c) = (
		monochrome_r.into_element() as f32,
		monochrome_g.into_element() as f32,
		monochrome_b.into_element() as f32,
		monochrome_c.into_element() as f32,
	);
	let (red_r, red_g, red_b, red_c) = (red_r.into_element() as f32, red_g.into_element() as f32, red_b.into_element() as f32, red_c.into_element() as f32);
	let (green_r, green_g, green_b, green_c) = (
		green_r.into_element() as f32,
		green_g.into_element() as f32,
		green_b.into_element() as f32,
		green_c.into_element() as f32,
	);
	let (blue_r, blue_g, blue_b, blue_c) = (blue_r.into_element() as f32, blue_g.into_element() as f32, blue_b.into_element() as f32, blue_c.into_element() as f32);

	image.element_mut().adjust(|color| {
		let [r, g, b, a] = color.to_gamma_srgb_channels();

		// Weights and constants are 10-bit fixed point truncated toward zero, which PSD interop depends on
		let weight = |percent: f32| (percent * 1024. / 100.).trunc() / 1024.;

		let (out_r, out_g, out_b) = if monochrome {
			let (monochrome_r, monochrome_g, monochrome_b, monochrome_c) = (weight(monochrome_r), weight(monochrome_g), weight(monochrome_b), weight(monochrome_c));

			let gray = (r * monochrome_r + g * monochrome_g + b * monochrome_b + monochrome_c).clamp(0., 1.);

			(gray, gray, gray)
		} else {
			let (red_r, red_g, red_b, red_c) = (weight(red_r), weight(red_g), weight(red_b), weight(red_c));
			let (green_r, green_g, green_b, green_c) = (weight(green_r), weight(green_g), weight(green_b), weight(green_c));
			let (blue_r, blue_g, blue_b, blue_c) = (weight(blue_r), weight(blue_g), weight(blue_b), weight(blue_c));

			let red = (r * red_r + g * red_g + b * red_b + red_c).clamp(0., 1.);
			let green = (r * green_r + g * green_g + b * green_b + green_c).clamp(0., 1.);
			let blue = (r * blue_r + g * blue_g + b * blue_b + blue_c).clamp(0., 1.);

			(red, green, blue)
		};

		Color::from_gamma_srgb_channels(out_r, out_g, out_b, a)
	});
	image
}

#[repr(u32)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "std", derive(dyn_any::DynAny))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, node_macro::ChoiceType, BufferStruct, FromPrimitive, IntoPrimitive)]
#[widget(Radio)]
pub enum RelativeAbsolute {
	#[default]
	Relative,
	Absolute,
}

#[repr(u32)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "std", derive(dyn_any::DynAny))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, node_macro::ChoiceType, BufferStruct, FromPrimitive, IntoPrimitive)]
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
	#[implementations(Raster<CPU>, Color, Gradient)]
	#[gpu_image]
	image: Item<T>,

	mode: Item<RelativeAbsolute>,

	#[name("(Reds) Cyan")] r_c: Item<f64>,
	#[name("(Reds) Magenta")] r_m: Item<f64>,
	#[name("(Reds) Yellow")] r_y: Item<f64>,
	#[name("(Reds) Black")] r_k: Item<f64>,

	#[name("(Yellows) Cyan")] y_c: Item<f64>,
	#[name("(Yellows) Magenta")] y_m: Item<f64>,
	#[name("(Yellows) Yellow")] y_y: Item<f64>,
	#[name("(Yellows) Black")] y_k: Item<f64>,

	#[name("(Greens) Cyan")] g_c: Item<f64>,
	#[name("(Greens) Magenta")] g_m: Item<f64>,
	#[name("(Greens) Yellow")] g_y: Item<f64>,
	#[name("(Greens) Black")] g_k: Item<f64>,

	#[name("(Cyans) Cyan")] c_c: Item<f64>,
	#[name("(Cyans) Magenta")] c_m: Item<f64>,
	#[name("(Cyans) Yellow")] c_y: Item<f64>,
	#[name("(Cyans) Black")] c_k: Item<f64>,

	#[name("(Blues) Cyan")] b_c: Item<f64>,
	#[name("(Blues) Magenta")] b_m: Item<f64>,
	#[name("(Blues) Yellow")] b_y: Item<f64>,
	#[name("(Blues) Black")] b_k: Item<f64>,

	#[name("(Magentas) Cyan")] m_c: Item<f64>,
	#[name("(Magentas) Magenta")] m_m: Item<f64>,
	#[name("(Magentas) Yellow")] m_y: Item<f64>,
	#[name("(Magentas) Black")] m_k: Item<f64>,

	#[name("(Whites) Cyan")] w_c: Item<f64>,
	#[name("(Whites) Magenta")] w_m: Item<f64>,
	#[name("(Whites) Yellow")] w_y: Item<f64>,
	#[name("(Whites) Black")] w_k: Item<f64>,

	#[name("(Neutrals) Cyan")] n_c: Item<f64>,
	#[name("(Neutrals) Magenta")] n_m: Item<f64>,
	#[name("(Neutrals) Yellow")] n_y: Item<f64>,
	#[name("(Neutrals) Black")] n_k: Item<f64>,

	#[name("(Blacks) Cyan")] k_c: Item<f64>,
	#[name("(Blacks) Magenta")] k_m: Item<f64>,
	#[name("(Blacks) Yellow")] k_y: Item<f64>,
	#[name("(Blacks) Black")] k_k: Item<f64>,

	_colors: Item<SelectiveColorChoice>,
) -> Item<T> {
	let mut image = image;
	let mode = mode.into_element();
	let (r_c, r_m, r_y, r_k) = (r_c.into_element() as f32, r_m.into_element() as f32, r_y.into_element() as f32, r_k.into_element() as f32);
	let (y_c, y_m, y_y, y_k) = (y_c.into_element() as f32, y_m.into_element() as f32, y_y.into_element() as f32, y_k.into_element() as f32);
	let (g_c, g_m, g_y, g_k) = (g_c.into_element() as f32, g_m.into_element() as f32, g_y.into_element() as f32, g_k.into_element() as f32);
	let (c_c, c_m, c_y, c_k) = (c_c.into_element() as f32, c_m.into_element() as f32, c_y.into_element() as f32, c_k.into_element() as f32);
	let (b_c, b_m, b_y, b_k) = (b_c.into_element() as f32, b_m.into_element() as f32, b_y.into_element() as f32, b_k.into_element() as f32);
	let (m_c, m_m, m_y, m_k) = (m_c.into_element() as f32, m_m.into_element() as f32, m_y.into_element() as f32, m_k.into_element() as f32);
	let (w_c, w_m, w_y, w_k) = (w_c.into_element() as f32, w_m.into_element() as f32, w_y.into_element() as f32, w_k.into_element() as f32);
	let (n_c, n_m, n_y, n_k) = (n_c.into_element() as f32, n_m.into_element() as f32, n_y.into_element() as f32, n_k.into_element() as f32);
	let (k_c, k_m, k_y, k_k) = (k_c.into_element() as f32, k_m.into_element() as f32, k_y.into_element() as f32, k_k.into_element() as f32);

	image.element_mut().adjust(|color| {
		let [r, g, b, a] = color.to_gamma_srgb_channels();

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
			// Every pixel, since the neutrals scale factor already vanishes at black, white, and fully saturated colors
			SelectiveColorChoice::Neutrals => true,
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
		// Indexed because the shader compiler cannot lower array iterators
		#[allow(clippy::needless_range_loop)]
		for i in 0..array.len() {
			let (color_parameter_group, (c, m, y, k)) = array[i];

			// Skip this color parameter group...
			// ...if it's unchanged from the default of zero offset on all CMYK parameters, or...
			// ...if this pixel's color isn't in the range affected by this color parameter group
			if (c == 0. && m == 0. && y == 0. && k == 0.) || !pixel_color_range(color_parameter_group) {
				continue;
			}

			let color_parameter_group_scale_factor = match color_parameter_group {
				SelectiveColorChoice::Reds | SelectiveColorChoice::Greens | SelectiveColorChoice::Blues => color_parameter_group_scale_factor_rgb,
				SelectiveColorChoice::Cyans | SelectiveColorChoice::Magentas | SelectiveColorChoice::Yellows => color_parameter_group_scale_factor_cmy,
				SelectiveColorChoice::Whites => min(r, g, b) * 2. - 1.,
				SelectiveColorChoice::Neutrals => 1. - ((max(r, g, b) - 0.5).abs() + (min(r, g, b) - 0.5).abs()),
				SelectiveColorChoice::Blacks => 1. - max(r, g, b) * 2.,
			};

			// For PSD interop, the combined percent (c + k + c k / 100) rounds half up to an integer
			let ink = |color: f32| {
				let percent = ((2. * (100. * (color + k) + color * k) + 100.) / 200.).floor();
				match mode {
					// The multiplier is stored as one byte, 255 / b above 1 and b / 255 below, so 99% and 100% both act as 127/128
					RelativeAbsolute::Relative => {
						let multiplier = 1. + percent / 100.;
						if multiplier >= 1. {
							255. / (255. / multiplier).round() - 1.
						} else {
							(255. * multiplier).round() / 255. - 1.
						}
					}
					RelativeAbsolute::Absolute => percent / 100.,
				}
			};

			let offset_r = f32::clamp(ink(c) * slope_r, -r, -r + 1.) * color_parameter_group_scale_factor;
			let offset_g = f32::clamp(ink(m) * slope_g, -g, -g + 1.) * color_parameter_group_scale_factor;
			let offset_b = f32::clamp(ink(y) * slope_b, -b, -b + 1.) * color_parameter_group_scale_factor;

			// An 8-bit PSD document sums the groups' 8-bit offsets, which this float node does not currently attempt to reproduce
			sum += Vec3::new(offset_r, offset_g, offset_b);
		}

		let rgb = Vec3::new(r, g, b);
		let out = (sum + rgb).clamp(Vec3::ZERO, Vec3::ONE);

		Color::from_gamma_srgb_channels(out.x, out.y, out.z, a)
	});
	image
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=nvrt%27%20%3D%20Invert-,%27post%27%20%3D%20Posterize,-%27thrs%27%20%3D%20Threshold
#[node_macro::node(category("Raster: Adjustment"), shader_node(PerPixelAdjust))]
fn posterize<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient)]
	#[gpu_image]
	input: Item<T>,
	#[default(4)]
	#[hard(2..)]
	levels: Item<i64>,
) -> Item<T> {
	let mut input = input;
	let levels = levels.into_element() as f32;

	input.element_mut().adjust(|color| {
		color.map_gamma_rgb(|c| {
			// Bins as floor(c * levels) with the outputs spread evenly to white.
			// The sliver of slack keeps an input exactly on an edge in the upper bin despite float ties.
			let bin = ((c + 2e-7) * levels).floor().min(levels - 1.);
			bin / (levels - 1.)
		})
	});
	input
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=curv%27%20%3D%20Curves-,%27expA%27%20%3D%20Exposure,-%27vibA%27%20%3D%20Vibrance
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=Flag%20(%20%3D%20128%20)-,Exposure,-Key%20is%20%27expA
//
// The exposure, offset, and gamma operations follow:
// https://geraldbakker.nl/psnumbers/exposure.html
#[node_macro::node(category("Raster: Adjustment"), properties("exposure_properties"), shader_node(PerPixelAdjust))]
fn exposure<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient)]
	#[gpu_image]
	input: Item<T>,
	exposure: Item<f64>,
	offset: Item<f64>,
	#[default(1.)]
	#[range]
	#[hard(0.0001..)]
	#[soft(0.01..10)]
	gamma_correction: Item<f64>,
) -> Item<T> {
	let mut input = input;
	let exposure = exposure.into_element() as f32;
	let offset = offset.into_element() as f32;
	let gamma_correction = gamma_correction.into_element() as f32;

	// Linearizes with a 2.2 power above a straight toe of slope 1/32, the two meeting at this constant
	const TOE_END: f32 = 0.05568117; // 32^(-1. / 1.2)

	let decode = |value: f32| if value < TOE_END { value / 32. } else { value.powf(2.2) };
	let encode = |linear: f32| if linear < TOE_END / 32. { linear * 32. } else { linear.powf(1. / 2.2) };
	let adjust = |c: f32| {
		let linear = decode(c) * 2_f32.powf(exposure) + offset;
		encode(linear.max(0.).powf(1. / gamma_correction).min(1.))
	};

	input.element_mut().adjust(|color| {
		let [r, g, b, a] = color.to_gamma_srgb_channels();
		Color::from_gamma_srgb_channels(adjust(r), adjust(g), adjust(b), a)
	});
	input
}

#[repr(u32)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "std", derive(dyn_any::DynAny))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, node_macro::ChoiceType, BufferStruct, FromPrimitive, IntoPrimitive)]
#[widget(Dropdown)]
pub enum TonalRange {
	Shadows,
	#[default]
	Midtones,
	Highlights,
}

/// A Levels-style tone curve: input black and white points on a 0..255 scale and a gamma exponent. For gamma above 1 the
/// power curve's slope is unbounded at black, so a cubic toe holds it to 2^gamma until past where that line meets the curve.
#[derive(Debug, Clone, Copy)]
struct LevelsCurve {
	black: f32,
	white: f32,
	exponent: f32,
	toe_end: f32,
	toe_value: f32,
	toe_slope_start: f32,
	toe_slope_end: f32,
}

impl LevelsCurve {
	fn new(black: i32, white: i32, gamma: f32) -> Self {
		Self::from_points(black as f32, white as f32, gamma)
	}

	/// `gamma` is the Levels dialog value (pixel exponent 1/gamma).
	fn from_points(black: f32, white: f32, gamma: f32) -> Self {
		let gamma = gamma.max(0.01);
		let black = black.min(white - 1.);
		let exponent = 1. / gamma;

		let mut curve = Self {
			black,
			white,
			exponent,
			toe_end: 0.,
			toe_value: 0.,
			toe_slope_start: 0.,
			toe_slope_end: 0.,
		};
		if gamma > 1. {
			let slope = 2_f32.powf(gamma);
			let intersection = 255. * slope.powf(-1. / (1. - exponent));
			// The cubic toe joins the power curve at twice the intersection
			if intersection > 1e-3 {
				let toe_end = 2. * intersection;
				curve.toe_end = toe_end;
				curve.toe_value = 255. * (toe_end / 255.).powf(exponent);
				curve.toe_slope_start = slope;
				curve.toe_slope_end = exponent * (toe_end / 255.).powf(exponent - 1.);
			}
		}
		curve
	}

	/// Maps one gamma-space channel value in 0..1.
	fn apply(&self, value: f32) -> f32 {
		let t = ((value * 255. - self.black) * 255. / (self.white - self.black)).clamp(0., 255.);
		let y = if t < self.toe_end {
			let u = t / self.toe_end;
			let hermite_start = u * u * u - 2. * u * u + u;
			let hermite_end_value = 3. * u * u - 2. * u * u * u;
			let hermite_end_slope = u * u * u - u * u;
			hermite_start * self.toe_end * self.toe_slope_start + hermite_end_value * self.toe_value + hermite_end_slope * self.toe_end * self.toe_slope_end
		} else {
			255. * (t / 255.).powf(self.exponent)
		};
		y / 255.
	}
}

/// One channel's Levels parameters from its own slider values, and (with preserve_luminosity) the slider
/// extremes across all three channels. The halvings truncate toward zero, as PSD interop requires.
fn color_balance_curve(s: i32, m: i32, h: i32, s_max: i32, m_max: i32, m_min: i32, h_min: i32, preserve_luminosity: bool) -> LevelsCurve {
	let (black, white, tone) = if preserve_luminosity {
		(s_max - s, 255 - (h - h_min), m - (m_max + m_min) / 2)
	} else {
		(0.max(-s), 255 - 0.max(h), (s + h) / 2 + m)
	};

	// Rounding the derived gamma to hundredths, the precision of a PSD Levels record, is needed for compatible results
	let gamma = (2_f32.powf(tone as f32 / 100.) * 100.).round() / 100.;
	LevelsCurve::new(black, white, gamma)
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27blnc%27%20%3D%20Color%20Balance
//
// Every channel is a Levels curve whose black point, white point, and two-decimal gamma are derived from
// the nine sliders, see `color_balance_curve`.
#[node_macro::node(category("Raster: Adjustment"), properties("color_balance_properties"), shader_node(PerPixelAdjust))]
fn color_balance<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient)]
	#[gpu_image]
	image: Item<T>,

	#[name("(Shadows) Cyan-Red")] shadows_cyan_red: Item<SignedPercentage>,
	#[name("(Shadows) Magenta-Green")] shadows_magenta_green: Item<SignedPercentage>,
	#[name("(Shadows) Yellow-Blue")] shadows_yellow_blue: Item<SignedPercentage>,

	#[name("(Midtones) Cyan-Red")] midtones_cyan_red: Item<SignedPercentage>,
	#[name("(Midtones) Magenta-Green")] midtones_magenta_green: Item<SignedPercentage>,
	#[name("(Midtones) Yellow-Blue")] midtones_yellow_blue: Item<SignedPercentage>,

	#[name("(Highlights) Cyan-Red")] highlights_cyan_red: Item<SignedPercentage>,
	#[name("(Highlights) Magenta-Green")] highlights_magenta_green: Item<SignedPercentage>,
	#[name("(Highlights) Yellow-Blue")] highlights_yellow_blue: Item<SignedPercentage>,

	#[default(true)] preserve_luminosity: Item<bool>,

	// Display-only property (not used within the node)
	_tone: Item<TonalRange>,
) -> Item<T> {
	let mut image = image;
	let preserve_luminosity = preserve_luminosity.into_element();

	// The derivation below is integer arithmetic, so the sliders round to whole percentages first
	let slider = |value: f32| value.clamp(-100., 100.).round() as i32;
	let (s_r, s_g, s_b) = (
		slider(shadows_cyan_red.into_element() as f32),
		slider(shadows_magenta_green.into_element() as f32),
		slider(shadows_yellow_blue.into_element() as f32),
	);
	let (m_r, m_g, m_b) = (
		slider(midtones_cyan_red.into_element() as f32),
		slider(midtones_magenta_green.into_element() as f32),
		slider(midtones_yellow_blue.into_element() as f32),
	);
	let (h_r, h_g, h_b) = (
		slider(highlights_cyan_red.into_element() as f32),
		slider(highlights_magenta_green.into_element() as f32),
		slider(highlights_yellow_blue.into_element() as f32),
	);

	let s_max = s_r.max(s_g).max(s_b);
	let m_max = m_r.max(m_g).max(m_b);
	let m_min = m_r.min(m_g).min(m_b);
	let h_min = h_r.min(h_g).min(h_b);
	let red = color_balance_curve(s_r, m_r, h_r, s_max, m_max, m_min, h_min, preserve_luminosity);
	let green = color_balance_curve(s_g, m_g, h_g, s_max, m_max, m_min, h_min, preserve_luminosity);
	let blue = color_balance_curve(s_b, m_b, h_b, s_max, m_max, m_min, h_min, preserve_luminosity);

	image.element_mut().adjust(|color| {
		// The curves operate on gamma-space channel values
		let [r, g, b, a] = color.to_gamma_srgb_channels();
		Color::from_gamma_srgb_channels(red.apply(r), green.apply(g), blue.apply(b), a)
	});
	image
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27phfl%27%20%3D%20Photo%20Filter
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=of%20the%20file.-,Photo%20Filter,-Key%20is%20%27phfl
#[node_macro::node(category("Raster: Adjustment"), shader_node(PerPixelAdjust))]
fn photo_filter<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient)]
	#[gpu_image]
	image: Item<T>,
	#[default("#ec8a00")] color: Item<Color>,
	#[default(25.)] density: Item<Percentage>,
	#[default(true)] preserve_luminosity: Item<bool>,
) -> Item<T> {
	let mut image = image;
	let color = color.into_element();
	let density = (density.into_element() / 100.).clamp(0., 1.) as f32;
	let preserve_luminosity = preserve_luminosity.into_element();

	// The image is multiplied in XYZ by the filter color normalized to the white point, with density easing that multiplier toward 1
	let filter_xyz = multiply_matrix(&SRGB_TO_XYZ_D50, [color.r(), color.g(), color.b()]);
	let factor = [
		1. + density * (filter_xyz[0] / WHITE_XYZ_D50[0] - 1.),
		1. + density * (filter_xyz[1] / WHITE_XYZ_D50[1] - 1.),
		1. + density * (filter_xyz[2] / WHITE_XYZ_D50[2] - 1.),
	];

	image.element_mut().adjust(|pixel| {
		let [r_in, g_in, b_in, alpha] = pixel.to_gamma_srgb_channels();
		let xyz = multiply_matrix(&SRGB_TO_XYZ_D50, [srgb_to_linear(r_in), srgb_to_linear(g_in), srgb_to_linear(b_in)]);
		let filtered = multiply_matrix(&XYZ_D50_TO_SRGB, [xyz[0] * factor[0], xyz[1] * factor[1], xyz[2] * factor[2]]);
		let mut r = linear_to_srgb(filtered[0].clamp(0., 1.));
		let mut g = linear_to_srgb(filtered[1].clamp(0., 1.));
		let mut b = linear_to_srgb(filtered[2].clamp(0., 1.));

		if preserve_luminosity {
			let luma_in = luma_rec_601_fixed_point(r_in.clamp(0., 1.), g_in.clamp(0., 1.), b_in.clamp(0., 1.));
			[r, g, b] = set_luminosity(r, g, b, luma_rec_601_fixed_point(r, g, b), luma_in);
		}

		Color::from_gamma_srgb_channels(r, g, b, alpha)
	});
	image
}

// sRGB colorants adapted to D50 as in the sRGB IEC61966-2.1 ICC profile, row major, and their inverse
pub(crate) const SRGB_TO_XYZ_D50: [[f32; 3]; 3] = [[0.43607, 0.38515, 0.14307], [0.22249, 0.71687, 0.06061], [0.01392, 0.09708, 0.71410]];
pub(crate) const XYZ_D50_TO_SRGB: [[f32; 3]; 3] = [[3.134096, -1.6174, -0.490638], [-0.978793, 1.916295, 0.033454], [0.071971, -0.228987, 1.40538]];
pub(crate) const WHITE_XYZ_D50: [f32; 3] = [0.96420, 1., 0.82491];

pub(crate) fn multiply_matrix(matrix: &[[f32; 3]; 3], vector: [f32; 3]) -> [f32; 3] {
	[
		matrix[0][0] * vector[0] + matrix[0][1] * vector[1] + matrix[0][2] * vector[2],
		matrix[1][0] * vector[0] + matrix[1][1] * vector[1] + matrix[1][2] * vector[2],
		matrix[2][0] * vector[0] + matrix[2][1] * vector[1] + matrix[2][2] * vector[2],
	]
}

/// The Rec. 601 luma in the 14-bit fixed point that PSD interop depends on.
fn luma_rec_601_fixed_point(r: f32, g: f32, b: f32) -> f32 {
	(4915. * r + 9667. * g + 1802. * b) / 16384.
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27clrL%27%20%3D%20Color%20Lookup
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=Color%20Lookup%20(Photoshop%20CS6
//
// TODO: Support dither, which needs the pixel position that a per-color adjustment never sees.
// TODO: Verify the tetrahedral interpolation, which is unconfirmed against other implementations.
#[cfg(feature = "std")]
#[node_macro::node(category("Raster: Adjustment"))]
async fn color_lookup<T: Adjust<Color> + Send>(
	_: impl Ctx,
	/// The image whose colors are remapped by the LUT (lookup table).
	#[implementations(Raster<CPU>, Color, Gradient)]
	image: Item<T>,
	/// A LUT (lookup table) file in the `.cube`, `.3dl`, `.look`, `.csp`, or `.icc` (*abstract* or *device link* ICC profile) format.
	#[name("LUT File")]
	#[widget(ParsedWidgetOverride::Custom = "lut_file")]
	lut_file: Item<Resource>,
	#[data] lut_cache: LutCache,
) -> Item<T> {
	let mut image = image;
	let Ok(lut) = lut_cache.parse(lut_file.element()) else { return image };

	image.element_mut().adjust(|color| {
		// Lookup tables address the gamma-encoded channels
		let [r, g, b, a] = color.to_gamma_srgb_channels();
		let [r, g, b] = lut.apply([r, g, b]);

		Color::from_gamma_srgb_channels(r, g, b, a)
	});

	image
}

#[cfg(feature = "std")]
mod _graphene_hash_impls {
	use super::{
		AdjustmentChannel, CellularDistanceFunction, CellularReturnType, DesaturateMethod, DomainWarpType, FractalType, HueSaturationRange, NoiseType, RedGreenBlue, RedGreenBlueAlpha,
		RelativeAbsolute, SelectiveColorChoice, TonalRange,
	};
	graphene_hash::impl_via_hash!(
		DesaturateMethod,
		RedGreenBlue,
		RedGreenBlueAlpha,
		NoiseType,
		FractalType,
		CellularDistanceFunction,
		CellularReturnType,
		DomainWarpType,
		RelativeAbsolute,
		SelectiveColorChoice,
		AdjustmentChannel,
		TonalRange,
		HueSaturationRange
	);
}

#[cfg(all(feature = "std", test))]
mod tests {
	use super::*;

	/// Matched to within one 8-bit level.
	fn assert_close(actual: [f32; 3], expected: [f32; 3]) {
		for (actual, expected) in actual.iter().zip(expected) {
			assert!((actual - expected).abs() <= 1., "expected {expected}, got {actual}");
		}
	}

	fn assert_close_with_label(actual: [f32; 3], expected: [f32; 3], label: &str) {
		for channel in 0..3 {
			assert!((actual[channel] - expected[channel]).abs() <= 1.5, "{label}: expected {expected:?}, got {actual:?}");
		}
	}

	/// Runs the node on one gamma-space gray value (0..255) and returns the gamma-space result on the same scale.
	fn run_brightness_contrast(value: f32, brightness: f64, contrast: f64, use_classic: bool) -> f32 {
		let pixel = Color::from_gamma_srgb_channels(value / 255., value / 255., value / 255., 1.);
		let result = brightness_contrast((), Item::new_from_element(pixel), brightness.into(), contrast.into(), use_classic.into(), 127_f64.into());
		result.into_element().to_gamma_srgb_channels()[0] * 255.
	}

	#[test]
	fn brightness_contrast_curves_brightness_and_pivots_contrast_at_the_midpoint() {
		for (value, brightness, contrast, expected) in [
			(16., 100., 0., 30.),
			(64., 100., 0., 120.),
			(128., 100., 0., 209.),
			(192., 100., 0., 245.),
			(128., 20., 0., 145.),
			(64., -100., 0., 34.),
			(128., -100., 0., 68.),
			(192., -100., 0., 111.),
			(240., -100., 0., 177.),
			(64., 150., 0., 162.),
			(128., 150., 0., 239.),
			(128., -150., 0., 50.),
			(240., -150., 0., 131.),
			(32., 0., 100., 14.),
			(64., 0., 100., 40.),
			(192., 0., 100., 216.),
			(64., 0., -50., 76.),
			(64., 0., 25., 58.),
			(64., 50., 30., 81.),
			(128., 50., 30., 178.),
			(192., 50., 30., 233.),
			(64., -60., -20., 48.),
			(128., -60., -20., 92.),
			(192., -60., -20., 139.),
		] {
			let actual = run_brightness_contrast(value, brightness, contrast, false);
			assert!(
				(actual - expected).abs() <= 1.,
				"{value} at brightness {brightness}, contrast {contrast}: expected {expected}, got {actual}"
			);
		}
	}

	#[test]
	fn brightness_contrast_classic_remaps_levels_around_the_pivot() {
		for (value, brightness, contrast, expected) in [
			(0., 0., -50., 64.),
			(100., 0., -50., 114.),
			(255., 0., -50., 191.),
			(64., 0., 50., 1.),
			(100., 0., 50., 73.),
			(200., 0., 50., 255.),
			(50., 40., 40., 65.),
			(100., 40., 40., 148.),
			(50., -40., -40., 41.),
			(200., -40., -40., 131.),
			(126., 0., 100., 0.),
			(128., 0., 100., 255.),
		] {
			let actual = run_brightness_contrast(value, brightness, contrast, true);
			assert!(
				(actual - expected).abs() <= 1.,
				"{value} at brightness {brightness}, contrast {contrast}: expected {expected}, got {actual}"
			);
		}
	}

	/// Runs Levels with composite and red records given as [black, white, gamma, output black, output white] with 0..255 points
	/// on one gamma-space gray value (0..255), returning the red and green results on the same scale.
	fn run_levels(value: f32, composite: [f64; 5], red: [f64; 5]) -> [f32; 2] {
		let pixel = Color::from_gamma_srgb_channels(value / 255., value / 255., value / 255., 1.);
		let percent = |level: f64| level / 2.55;
		let result = levels(
			(),
			Item::new_from_element(pixel),
			percent(composite[0]).into(),
			composite[2].into(),
			percent(composite[1]).into(),
			percent(composite[3]).into(),
			percent(composite[4]).into(),
			percent(red[0]).into(),
			red[2].into(),
			percent(red[1]).into(),
			percent(red[3]).into(),
			percent(red[4]).into(),
			0_f64.into(),
			1_f64.into(),
			100_f64.into(),
			0_f64.into(),
			100_f64.into(),
			0_f64.into(),
			1_f64.into(),
			100_f64.into(),
			0_f64.into(),
			100_f64.into(),
			0_f64.into(),
			1_f64.into(),
			100_f64.into(),
			0_f64.into(),
			100_f64.into(),
			AdjustmentChannel::Rgb.into(),
		);
		let [r, g, _, _] = result.into_element().to_gamma_srgb_channels();
		[r * 255., g * 255.]
	}

	#[test]
	fn levels_records_merge_into_one_gamma_only_when_nothing_lies_between() {
		const DEFAULT: [f64; 5] = [0., 255., 1., 0., 255.];
		for (value, composite, red, expected_red, expected_green) in [
			// Two gammas with nothing between them act as one gamma of 2.25, toe included
			(5., [0., 255., 1.5, 0., 255.], [0., 255., 1.5, 0., 255.], 23., 14.),
			(25., [0., 255., 1.5, 0., 255.], [0., 255., 1.5, 0., 255.], 89., 54.),
			(100., [0., 255., 1.5, 0., 255.], [0., 255., 1.5, 0., 255.], 168., 137.),
			// A black point in each record keeps them as two curves
			(40., [30., 255., 1.5, 0., 255.], [20., 255., 1.5, 0., 255.], 49., 28.),
			(100., [30., 255., 1.5, 0., 255.], [20., 255., 1.5, 0., 255.], 144., 117.),
			// Input and output points only
			(100., [30., 220., 1., 0., 255.], [50., 255., 1., 0., 200.], 26., 94.),
			(150., [30., 220., 1., 0., 255.], [50., 255., 1., 0., 200.], 91., 161.),
			// A pure channel gamma under a composite with points stays a separate stage
			(5., [0., 200., 1.2, 10., 255.], [0., 255., 3., 0., 255.], 70., 21.),
			(50., [0., 200., 1.2, 10., 255.], [0., 255., 3., 0., 255.], 201., 88.),
			(128., DEFAULT, DEFAULT, 128., 128.),
		] {
			let [red_actual, green_actual] = run_levels(value, composite, red);
			assert!((red_actual - expected_red).abs() <= 1.5, "{value} red: expected {expected_red}, got {red_actual}");
			assert!((green_actual - expected_green).abs() <= 1.5, "{value} green: expected {expected_green}, got {green_actual}");
		}
	}

	/// Runs Black & White with the default sliders on one gamma-space RGB value (0..255) and returns the gamma-space result on the same scale.
	fn run_black_and_white(input: [f32; 3], tint: [f32; 3]) -> [f32; 3] {
		let pixel = Color::from_gamma_srgb_channels(input[0] / 255., input[1] / 255., input[2] / 255., 1.);
		let tint = Color::from_gamma_srgb_channels(tint[0] / 255., tint[1] / 255., tint[2] / 255., 1.);
		let result = black_and_white(
			(),
			Item::new_from_element(pixel),
			true.into(),
			tint.into(),
			40_f64.into(),
			60_f64.into(),
			40_f64.into(),
			60_f64.into(),
			20_f64.into(),
			80_f64.into(),
		);
		let [r, g, b, _] = result.into_element().to_gamma_srgb_channels();
		[r * 255., g * 255., b * 255.]
	}

	#[test]
	fn black_and_white_tint_takes_the_grays_luminosity() {
		for (input, tint, expected) in [
			([200., 200., 200.], [225., 211., 179.], [213., 199., 167.]),
			([50., 50., 50.], [225., 211., 179.], [63., 49., 17.]),
			([200., 100., 50.], [225., 211., 179.], [133., 119., 87.]),
			([200., 200., 200.], [30., 60., 120.], [176., 202., 255.]),
			([50., 50., 50.], [30., 60., 120.], [22., 52., 112.]),
			([200., 100., 50.], [30., 60., 120.], [92., 122., 182.]),
		] {
			let actual = run_black_and_white(input, tint);
			for (actual, expected) in actual.iter().zip(expected) {
				assert!((actual - expected).abs() <= 1., "{input:?} tinted {tint:?}: expected {expected}, got {actual}");
			}
		}
	}

	#[test]
	fn black_and_white_clipped_channels_are_pulled_toward_the_luminosity() {
		// A pure red tint over grays, where the shifted channels run out of range
		for (gray, expected) in [
			(1., [3.33, 0., 0.]),
			(38., [126.67, 0., 0.]),
			(75., [250.01, 0., 0.]),
			(78., [255., 2.15, 2.15]),
			(129., [255., 75., 75.]),
			(200., [255., 176.43, 176.43]),
		] {
			let actual = run_black_and_white([gray, gray, gray], [255., 0., 0.]);
			for (actual, expected) in actual.iter().zip(expected) {
				assert!((actual - expected).abs() <= 0.51, "gray {gray} tinted red: expected {expected}, got {actual}");
			}
		}
	}

	/// Runs the node on one gamma-space RGB value (0..255) with the master sliders, colorize, and one range's sliders at
	/// its default range values, returning the gamma-space result on the same scale.
	fn run_hue_saturation(input: [f32; 3], master: [f64; 3], colorize: Option<[f64; 3]>, range: Option<(HueSaturationRange, [f64; 3])>) -> [f32; 3] {
		let pixel = Color::from_gamma_srgb_channels(input[0] / 255., input[1] / 255., input[2] / 255., 1.);
		let colorize_values = colorize.unwrap_or([24., 25., 0.]);
		let range_values = |which: HueSaturationRange| match range {
			Some((selected, values)) if selected == which => values,
			_ => [0., 0., 0.],
		};
		let [reds, yellows, greens, cyans, blues, magentas] = [
			range_values(HueSaturationRange::Reds),
			range_values(HueSaturationRange::Yellows),
			range_values(HueSaturationRange::Greens),
			range_values(HueSaturationRange::Cyans),
			range_values(HueSaturationRange::Blues),
			range_values(HueSaturationRange::Magentas),
		];
		let result = hue_saturation(
			(),
			Item::new_from_element(pixel),
			master[0].into(),
			master[1].into(),
			master[2].into(),
			colorize.is_some().into(),
			colorize_values[0].into(),
			colorize_values[1].into(),
			colorize_values[2].into(),
			reds[0].into(),
			reds[1].into(),
			reds[2].into(),
			315_f64.into(),
			345_f64.into(),
			15_f64.into(),
			45_f64.into(),
			yellows[0].into(),
			yellows[1].into(),
			yellows[2].into(),
			15_f64.into(),
			45_f64.into(),
			75_f64.into(),
			105_f64.into(),
			greens[0].into(),
			greens[1].into(),
			greens[2].into(),
			75_f64.into(),
			105_f64.into(),
			135_f64.into(),
			165_f64.into(),
			cyans[0].into(),
			cyans[1].into(),
			cyans[2].into(),
			135_f64.into(),
			165_f64.into(),
			195_f64.into(),
			225_f64.into(),
			blues[0].into(),
			blues[1].into(),
			blues[2].into(),
			195_f64.into(),
			225_f64.into(),
			255_f64.into(),
			285_f64.into(),
			magentas[0].into(),
			magentas[1].into(),
			magentas[2].into(),
			255_f64.into(),
			285_f64.into(),
			315_f64.into(),
			345_f64.into(),
			HueSaturationRange::Master.into(),
		);
		let [r, g, b, _] = result.into_element().to_gamma_srgb_channels();
		[r * 255., g * 255., b * 255.]
	}

	#[test]
	fn hue_saturation_master_sliders_rotate_scale_and_lighten() {
		assert_close_with_label(run_hue_saturation([200., 50., 50.], [30., 0., 0.], None, None), [200., 125., 50.], "hue +30");
		assert_close_with_label(run_hue_saturation([60., 120., 200.], [30., 0., 0.], None, None), [70., 60., 200.], "hue +30 on blue");
		assert_close_with_label(run_hue_saturation([200., 50., 50.], [0., 50., 0.], None, None), [250., 0., 0.], "saturation +50");
		assert_close_with_label(run_hue_saturation([60., 120., 200.], [0., 50., 0.], None, None), [5., 111., 255.], "saturation +50 on blue");
		assert_close_with_label(run_hue_saturation([200., 50., 50.], [0., -50., 0.], None, None), [162., 87., 87.], "saturation -50");
		assert_close_with_label(run_hue_saturation([30., 200., 90.], [0., -50., 0.], None, None), [72., 157., 102.], "saturation -50 on green");
		assert_close_with_label(run_hue_saturation([200., 50., 50.], [0., 0., 50.], None, None), [227., 152., 152.], "lightness +50");
		assert_close_with_label(run_hue_saturation([250., 0., 130.], [0., 0., 50.], None, None), [252., 127., 192.], "lightness +50 on magenta");
		assert_close_with_label(run_hue_saturation([60., 120., 200.], [0., 0., -50.], None, None), [30., 60., 100.], "lightness -50");
		assert_close_with_label(run_hue_saturation([200., 50., 50.], [90., 60., -40.], None, None), [74., 150., 0.], "combined");
		assert_close_with_label(run_hue_saturation([30., 200., 90.], [90., 60., -40.], None, None), [0., 20., 138.], "combined on green");
		assert_close_with_label(run_hue_saturation([150., 150., 150.], [90., 60., -40.], None, None), [90., 90., 90.], "combined on gray");
	}

	#[test]
	fn hue_saturation_colorize_rebuilds_the_exact_hsl_color() {
		assert_close_with_label(run_hue_saturation([200., 50., 50.], [0., 0., 0.], Some([240., 100., 0.]), None), [0., 0., 250.], "colorize 240/100/0");
		assert_close_with_label(run_hue_saturation([150., 150., 150.], [0., 0., 0.], Some([240., 100., 0.]), None), [45., 45., 255.], "colorize on gray");
		assert_close_with_label(run_hue_saturation([30., 200., 90.], [0., 0., 0.], Some([60., 100., 0.]), None), [230., 230., 0.], "colorize 60/100/0");
		assert_close_with_label(
			run_hue_saturation([150., 150., 150.], [0., 0., 0.], Some([30., 60., -30.]), None),
			[168., 104., 42.],
			"colorize 30/60/-30",
		);
		assert_close_with_label(run_hue_saturation([120., 0., 30.], [0., 0., 0.], Some([30., 60., -30.]), None), [67., 42., 17.], "colorize dark");
		assert_close_with_label(
			run_hue_saturation([255., 0., 0.], [0., 0., 0.], Some([20., 100., 0.]), None),
			[255., 85., 0.],
			"colorize 20 is the exact HSL color",
		);
		assert_close_with_label(run_hue_saturation([255., 0., 0.], [0., 0., 0.], Some([160., 100., 0.]), None), [0., 255., 170.], "colorize 160");
		assert_close_with_label(run_hue_saturation([255., 0., 0.], [0., 0., 0.], Some([340., 100., 0.]), None), [255., 0., 85.], "colorize 340");
		assert_close_with_label(
			run_hue_saturation([255., 0., 0.], [0., 0., 0.], Some([-20., 100., 0.]), None),
			[255., 0., 85.],
			"colorize -20 wraps to 340",
		);
	}

	#[test]
	fn hue_saturation_ranges_weight_their_sliders_by_falloff() {
		let reds = HueSaturationRange::Reds;
		assert_close_with_label(
			run_hue_saturation([200., 50., 50.], [0., 0., 0.], None, Some((reds, [60., 0., 0.]))),
			[199., 200., 50.],
			"reds hue +60 inside",
		);
		assert_close_with_label(
			run_hue_saturation([60., 120., 200.], [0., 0., 0.], None, Some((reds, [60., 0., 0.]))),
			[60., 120., 200.],
			"reds hue +60 outside",
		);
		assert_close_with_label(
			run_hue_saturation([200., 50., 50.], [0., 0., 0.], None, Some((reds, [0., 100., 0.]))),
			[250., 0., 0.],
			"reds saturation +100",
		);
		assert_close_with_label(
			run_hue_saturation([200., 50., 50.], [0., 0., 0.], None, Some((reds, [0., 0., -50.]))),
			[125., 50., 50.],
			"reds lightness -50",
		);
		assert_close_with_label(
			run_hue_saturation([255., 65., 0.], [0., 0., 0.], None, Some((reds, [0., 0., -50.]))),
			[129., 33., 0.],
			"reds lightness -50 near the edge",
		);
		assert_close_with_label(
			run_hue_saturation([30., 200., 90.], [0., 0., 0.], None, Some((HueSaturationRange::Greens, [0., 50., -25.]))),
			[0., 196., 69.],
			"greens saturation and lightness",
		);
		assert_close_with_label(
			run_hue_saturation([200., 50., 50.], [30., 0., 0.], None, Some((reds, [0., 50., 0.]))),
			[250., 125., 0.],
			"master hue with a range saturation",
		);
	}

	#[test]
	fn invert_flips_straight_channels_and_keeps_alpha() {
		let color = Color::from_gamma_srgb_channels(1., 0.25, 0., 0.5);

		let inverted = invert((), Item::new_from_element(color)).into_element();

		let [r, g, b, a] = inverted.to_gamma_srgb_channels();
		assert!((r - 0.).abs() < 1e-5 && (g - 0.75).abs() < 1e-5 && (b - 1.).abs() < 1e-5, "inverted channels were {r} {g} {b}");
		assert!((a - 0.5).abs() < 1e-5, "alpha was {a}");
	}

	/// Whether one gamma-space RGB value (0..255) ends up white at the given threshold level (0..255).
	fn threshold_is_white(input: [f32; 3], level: f64) -> bool {
		let pixel = Color::from_gamma_srgb_channels(input[0] / 255., input[1] / 255., input[2] / 255., 1.);
		let result = threshold((), Item::new_from_element(pixel), (level / 255. * 100.).into(), 100_f64.into());
		result.into_element().r() == 1.
	}

	#[test]
	fn threshold_compares_rec_601_luma_as_an_8_bit_level() {
		assert!(!threshold_is_white([200., 100., 40.], 128.));
		assert!(!threshold_is_white([125., 130., 120.], 128.));
		assert!(threshold_is_white([0., 255., 0.], 128.));
		assert!(!threshold_is_white([255., 0., 0.], 128.));
		assert!(threshold_is_white([128., 128., 128.], 128.));
		assert!(!threshold_is_white([127., 127., 127.], 128.));
		assert!(threshold_is_white([200., 100., 40.], 123.));
		assert!(!threshold_is_white([200., 100., 40.], 124.));
	}

	#[test]
	fn threshold_ties_follow_the_unrounded_fixed_point_luma() {
		// Half-level lumas in 0.3/0.59/0.11 stay below the level either way, and the 14-bit weights pull a whole-level red or blue luma just under it
		assert!(!threshold_is_white([189., 120., 0.], 128.));
		assert!(!threshold_is_white([248., 90., 0.], 128.));
		assert!(!threshold_is_white([135., 100., 0.], 100.));
		assert!(!threshold_is_white([255., 0., 50.], 82.));
		assert!(threshold_is_white([0., 200., 0.], 118.));
	}

	/// Runs the node on one gamma-space RGB value (0..255) and returns the gamma-space result on the same scale.
	fn run_vibrance(input: [f32; 3], vibrance_amount: f64, saturation: f64) -> [f32; 3] {
		let pixel = Color::from_gamma_srgb_channels(input[0] / 255., input[1] / 255., input[2] / 255., 1.);
		let result = vibrance((), Item::new_from_element(pixel), vibrance_amount.into(), saturation.into());
		let [r, g, b, _] = result.into_element().to_gamma_srgb_channels();
		[r * 255., g * 255., b * 255.]
	}

	#[test]
	fn vibrance_saturation_scales_chroma_around_a_prophoto_weighted_gray() {
		for (input, saturation, expected) in [
			([255., 0., 0.], -100., [146., 146., 146.]),
			([0., 255., 0.], -100., [219., 219., 219.]),
			([0., 0., 255.], -100., [0., 0., 0.]),
			([200., 100., 50.], -100., [139., 139., 139.]),
			([0., 255., 0.], -50., [161., 238., 161.]),
			([200., 100., 50.], 50., [223., 71., 0.]),
			([100., 150., 200.], 100., [3., 161., 244.]),
			([200., 180., 170.], 100., [213., 174., 152.]),
		] {
			let actual = run_vibrance(input, 0., saturation);
			for (actual, expected) in actual.iter().zip(expected) {
				assert!((actual - expected).abs() <= 1., "{input:?} at {saturation}: expected {expected}, got {actual}");
			}
		}
	}

	#[test]
	fn vibrance_boosts_low_chroma_colors_most_and_spares_reds() {
		for (input, amount, expected) in [
			([200., 100., 100.], -100., [188., 148., 148.]),
			([200., 160., 160.], -50., [192., 172., 172.]),
			([50., 0., 0.], -100., [50., 31., 31.]),
			([100., 50., 0.], -100., [100., 67., 50.]),
			([200., 100., 100.], 100., [203., 71., 71.]),
			([255., 125., 125.], 100., [255., 91., 91.]),
			([125., 255., 125.], 100., [80., 255., 80.]),
			([255., 200., 205.], 100., [255., 190., 196.]),
			([60., 120., 200.], 75., [38., 115., 201.]),
		] {
			let actual = run_vibrance(input, amount, 0.);
			for (actual, expected) in actual.iter().zip(expected) {
				assert!((actual - expected).abs() <= 1., "{input:?} at {amount}: expected {expected}, got {actual}");
			}
		}
	}

	/// Runs Selective Color on one gamma-space RGB value (0..255) with the given group values
	/// (Reds through Blacks, each cyan, magenta, yellow, black) and returns the gamma-space result on the same scale.
	fn run_selective_color(input: [f32; 3], mode: RelativeAbsolute, groups: [[f64; 4]; 9]) -> [f32; 3] {
		let pixel = Color::from_gamma_srgb_channels(input[0] / 255., input[1] / 255., input[2] / 255., 1.);
		let g = |group: usize, component: usize| Item::new_from_element(groups[group][component]);
		#[rustfmt::skip]
		let result = selective_color(
			(), Item::new_from_element(pixel), mode.into(),
			g(0, 0), g(0, 1), g(0, 2), g(0, 3), g(1, 0), g(1, 1), g(1, 2), g(1, 3), g(2, 0), g(2, 1), g(2, 2), g(2, 3),
			g(3, 0), g(3, 1), g(3, 2), g(3, 3), g(4, 0), g(4, 1), g(4, 2), g(4, 3), g(5, 0), g(5, 1), g(5, 2), g(5, 3),
			g(6, 0), g(6, 1), g(6, 2), g(6, 3), g(7, 0), g(7, 1), g(7, 2), g(7, 3), g(8, 0), g(8, 1), g(8, 2), g(8, 3),
			SelectiveColorChoice::Reds.into(),
		);
		let [r, g, b, _] = result.into_element().to_gamma_srgb_channels();
		[r * 255., g * 255., b * 255.]
	}

	#[test]
	fn selective_color_applies_negative_values() {
		let mut groups = [[0.; 4]; 9];
		groups[0] = [-100., 0., 0., 0.];
		assert_close(run_selective_color([125., 0., 0.], RelativeAbsolute::Relative, groups), [189., 0., 0.]);
		assert_close(run_selective_color([120., 100., 5.], RelativeAbsolute::Relative, groups), [131., 100., 5.]);

		let mut groups = [[0.; 4]; 9];
		groups[0] = [0., 0., 0., -100.];
		assert_close(run_selective_color([110., 65., 25.], RelativeAbsolute::Absolute, groups), [136., 99., 66.]);
	}

	#[test]
	fn selective_color_neutrals_include_pixels_with_an_empty_channel() {
		let mut groups = [[0.; 4]; 9];
		groups[7] = [0., -100., 0., 0.];
		assert_close(run_selective_color([100., 0., 130.], RelativeAbsolute::Relative, groups), [100., 125., 130.]);
		assert_close(run_selective_color([100., 50., 130.], RelativeAbsolute::Relative, groups), [100., 191., 130.]);
	}

	/// Runs Posterize on one gamma-space gray value (0..255) and returns the gamma-space result on the same scale.
	fn run_posterize(value: f32, levels: i64) -> f32 {
		let pixel = Color::from_gamma_srgb_channels(value / 255., value / 255., value / 255., 1.);
		posterize((), Item::new_from_element(pixel), levels.into()).into_element().to_gamma_srgb_channels()[0] * 255.
	}

	#[test]
	fn posterize_bins_by_floor_with_levels_spread_to_white() {
		for (value, levels, expected) in [
			(84., 3, 0.),
			(85., 3, 127.5),
			(169., 3, 127.5),
			(170., 3, 255.),
			(36., 7, 0.),
			(37., 7, 42.5),
			(110., 7, 127.5),
			(255., 7, 255.),
		] {
			let actual = run_posterize(value, levels);
			assert!((actual - expected).abs() <= 0.01, "{value} at {levels} levels: expected {expected}, got {actual}");
		}
	}

	/// Runs Exposure on one gamma-space gray value (0..255) and returns the gamma-space result on the same scale.
	fn run_exposure(value: f32, exposure: f64, offset: f64, gamma_correction: f64) -> f32 {
		let pixel = Color::from_gamma_srgb_channels(value / 255., value / 255., value / 255., 1.);
		let result = super::exposure((), Item::new_from_element(pixel), exposure.into(), offset.into(), gamma_correction.into());
		result.into_element().to_gamma_srgb_channels()[0] * 255.
	}

	#[test]
	fn exposure_linearizes_through_the_toe_and_power_curve() {
		for (value, exposure, offset, gamma_correction, expected) in [
			(1., 1., 0., 1., 2.),
			(8., 1., 0., 1., 15.),
			(16., 1., 0., 1., 22.),
			(128., 1., 0., 1., 175.),
			(200., 1., 0., 1., 255.),
			(1., 0., 0., 2., 33.),
			(16., 0., 0., 2., 64.),
			(128., 0., 0., 2., 181.),
			(200., 0., 0., 2., 226.),
			(0., -2., 0.2, 1.5, 157.),
			(100., -2., 0.2, 1.5, 164.),
			(200., -2., 0.2, 1.5, 185.),
			(100., 0., -0.25, 1., 0.),
			(200., 0., -0.25, 1., 155.),
		] {
			let actual = run_exposure(value, exposure, offset, gamma_correction);
			assert!(
				(actual - expected).abs() <= 1.,
				"{value} at exposure {exposure}, offset {offset}, gamma {gamma_correction}: expected {expected}, got {actual}"
			);
		}
	}

	#[test]
	fn exposure_clamps_negative_offsets_before_the_gamma_power() {
		assert_eq!(run_exposure(50., 0., -0.5, 2.), 0.);
	}

	/// Runs Color Balance on one gamma-space RGB value (0..255) and returns the gamma-space result on the same scale.
	fn run_color_balance(input: [f32; 3], shadows: [f64; 3], midtones: [f64; 3], highlights: [f64; 3], preserve_luminosity: bool) -> [f32; 3] {
		let color = Color::from_gamma_srgb_channels(input[0] / 255., input[1] / 255., input[2] / 255., 1.);
		let result = color_balance(
			(),
			Item::new_from_element(color),
			shadows[0].into(),
			shadows[1].into(),
			shadows[2].into(),
			midtones[0].into(),
			midtones[1].into(),
			midtones[2].into(),
			highlights[0].into(),
			highlights[1].into(),
			highlights[2].into(),
			preserve_luminosity.into(),
			TonalRange::Midtones.into(),
		);
		let [r, g, b, _] = result.into_element().to_gamma_srgb_channels();
		[r * 255., g * 255., b * 255.]
	}

	#[test]
	fn color_balance_midtones_are_a_gamma_with_a_toe() {
		let none = [0., 0., 0.];
		assert_close(run_color_balance([100., 100., 100.], none, [100., 0., 0.], none, false), [160., 100., 100.]);
		assert_close(run_color_balance([200., 200., 200.], none, [100., 0., 0.], none, false), [226., 200., 200.]);
		assert_close(run_color_balance([4., 4., 4.], none, [100., 0., 0.], none, false), [16., 4., 4.]);
		assert_close(run_color_balance([1., 1., 1.], none, [100., 0., 0.], none, false), [4., 1., 1.]);
		assert_close(run_color_balance([100., 100., 100.], none, [-100., 0., 0.], none, false), [39., 100., 100.]);
	}

	#[test]
	fn color_balance_shadows_and_highlights_move_the_end_points() {
		let none = [0., 0., 0.];
		assert_close(run_color_balance([100., 100., 100.], [-100., 0., 0.], none, none, false), [0., 100., 100.]);
		assert_close(run_color_balance([150., 150., 150.], [-100., 0., 0.], none, none, false), [52., 150., 150.]);
		assert_close(run_color_balance([200., 200., 200.], [-100., 0., 0.], none, none, false), [138., 200., 200.]);
		assert_close(run_color_balance([100., 100., 100.], none, none, [100., 0., 0.], false), [187., 100., 100.]);
		assert_close(run_color_balance([155., 155., 155.], none, none, [100., 0., 0.], false), [255., 155., 155.]);
	}

	#[test]
	fn color_balance_preserve_luminosity_makes_sliders_relative() {
		let none = [0., 0., 0.];
		assert_close(run_color_balance([90., 90., 90.], none, [-100., 0., 0.], none, true), [59., 122., 122.]);
		assert_close(run_color_balance([90., 90., 90.], [50., 0., 0.], none, none, true), [90., 50., 50.]);
		assert_close(run_color_balance([120., 120., 120.], none, none, [-100., 0., 0.], true), [120., 197., 197.]);
		assert_close(run_color_balance([90., 90., 90.], [100., 100., 100.], [100., 100., 100.], [100., 100., 100.], true), [90., 90., 90.]);
	}

	#[test]
	fn color_balance_combined_tones_use_integer_arithmetic() {
		// Red: black 39, white 235, gamma 1.09; green: gamma 0.91; blue: white 210, gamma 1.13
		let result = run_color_balance([128., 128., 128.], [-39., 6., 42.], [21., 0., -25.], [20., -35., 45.], false);
		assert_close(result, [124., 120., 164.]);
	}

	/// Runs Photo Filter on one gamma-space RGB value (0..255) and returns the gamma-space result on the same scale.
	fn run_photo_filter(input: [f32; 3], filter: [f32; 3], density: f64, preserve_luminosity: bool) -> [f32; 3] {
		let pixel = Color::from_gamma_srgb_channels(input[0] / 255., input[1] / 255., input[2] / 255., 1.);
		let filter = Color::from_gamma_srgb_channels(filter[0] / 255., filter[1] / 255., filter[2] / 255., 1.);
		let result = photo_filter((), Item::new_from_element(pixel), filter.into(), density.into(), preserve_luminosity.into());
		let [r, g, b, _] = result.into_element().to_gamma_srgb_channels();
		[r * 255., g * 255., b * 255.]
	}

	#[test]
	fn photo_filter_multiplies_in_xyz() {
		assert_close(run_photo_filter([0., 255., 0.], [255., 0., 0.], 100., false), [146., 103., 0.]);
		assert_close(run_photo_filter([0., 0., 255.], [255., 0., 0.], 100., false), [116., 0., 37.]);
		assert_close(run_photo_filter([100., 100., 100.], [128., 128., 128.], 100., false), [46., 46., 46.]);
		assert_close(run_photo_filter([100., 100., 100.], [236., 138., 0.], 25., false), [98., 91., 87.]);
		assert_close(run_photo_filter([200., 200., 200.], [255., 255., 255.], 100., false), [200., 200., 200.]);
	}

	#[test]
	fn photo_filter_preserve_luminosity_shifts_then_clips_toward_luminosity() {
		assert_close(run_photo_filter([20., 20., 20.], [255., 0., 0.], 100., true), [34., 14., 14.]);
		assert_close(run_photo_filter([160., 160., 160.], [255., 0., 0.], 100., true), [255., 120., 120.]);
		assert_close(run_photo_filter([90., 90., 90.], [236., 138., 0.], 25., true), [95., 88., 85.]);
	}

	#[test]
	fn photo_filter_clips_light_brighter_than_white_when_preserving_luminosity() {
		// Above white, where the luma to take on falls outside the 0..1 domain the construction needs
		// A white filter alters nothing, so the pixel keeps its channels once the above-white red is clipped
		assert_close(run_photo_filter([300., 200., 100.], [255., 255., 255.], 100., true), [255., 200., 100.]);
	}
}
