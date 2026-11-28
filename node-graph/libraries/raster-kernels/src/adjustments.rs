//! The kernels carrying out each [`Adjustment`], with its settings turned into what its kernel reads by [`visit_kernel`].

use crate::MapColor;
use crate::adjustment_steps::{AdjustmentSteps, StepKind, f32_pair, texel};
use crate::blending::ColorOverlay;
use crate::color_lookup_table::{Lut, parse_cached};
use core_types::Color;
use core_types::color::{linear_to_srgb, set_luminosity, srgb_to_linear};
use core_types::math::midpoint::sanitized_midpoint;
use core_types::transfer_curve::TransferCurveEvaluator;
use glam::Vec3;
use graphic_types::adjustment::{self as settings, Adjustment, DesaturateMethod, RelativeAbsolute};
use raster_types::parallel_rows::fill_rows;
use std::iter::once;
use std::sync::Arc;
use vector_types::gradient::{GradientEvaluator, SplineKnots, interpolation_endpoints};
use vector_types::{GradientInterpolation, GradientSettings, GradientSpace, GradientSpread};

/// How many colors each thread band handles at a time when a long slice is split across threads.
const COLORS_PER_ROW: usize = 4096;

/// Adjusts each color in place, on threads for long slices on native platforms. O(n).
pub fn adjust_colors(adjustment: &Adjustment, colors: &mut [Color]) {
	struct AdjustColors<'a>(&'a mut [Color]);

	impl AdjustmentVisitor for AdjustColors<'_> {
		type Output = ();

		fn visit<K: MapColor>(self, kernel: K) {
			fill_rows(self.0, COLORS_PER_ROW, |_, row| row.iter_mut().for_each(|color| *color = kernel.map_cpu(*color)));
		}

		fn unchanged(self) {}
	}

	visit_kernel(adjustment, AdjustColors(colors));
}

/// The color with `adjustment` carried out on it. To adjust many colors one at a time, prepare a [`PreparedAdjustment`] once instead.
pub fn adjust_color(adjustment: &Adjustment, color: Color) -> Color {
	struct AdjustColor(Color);

	impl AdjustmentVisitor for AdjustColor {
		type Output = Color;

		fn visit<K: MapColor>(self, kernel: K) -> Color {
			kernel.map_cpu(self.0)
		}

		fn unchanged(self) -> Color {
			self.0
		}
	}

	visit_kernel(adjustment, AdjustColor(color))
}

/// An adjustment with its kernel prepared from its settings, which carries it out on any number of colors without preparing it again.
pub struct PreparedAdjustment(Option<Box<dyn MapColor>>);

impl PreparedAdjustment {
	pub fn new(adjustment: &Adjustment) -> Self {
		struct Prepare;

		impl AdjustmentVisitor for Prepare {
			type Output = Option<Box<dyn MapColor>>;

			fn visit<K: MapColor>(self, kernel: K) -> Self::Output {
				Some(Box::new(kernel))
			}

			fn unchanged(self) -> Self::Output {
				None
			}
		}

		Self(visit_kernel(adjustment, Prepare))
	}

	/// The color with the adjustment carried out on it.
	pub fn adjust(&self, color: Color) -> Color {
		self.0.as_ref().map_or(color, |kernel| kernel.map_cpu(color))
	}
}

/// The red, green, blue, and alpha curves of a per-channel adjustment, each sampled at `size` evenly spaced gamma-encoded values from 0 to 1.
pub fn channel_curves(adjustment: &Adjustment, size: usize) -> Option<[Vec<f32>; 4]> {
	if !adjustment.is_per_channel() || size < 2 {
		return None;
	}

	// Each sample feeds the same value to every channel, which the curves map independently
	let mut samples: Vec<Color> = (0..size)
		.map(|index| {
			let value = index as f32 / (size - 1) as f32;
			Color::from_gamma_srgb_channels(value, value, value, value)
		})
		.collect();
	adjust_colors(adjustment, &mut samples);

	let channels: Vec<[f32; 4]> = samples.iter().map(Color::to_gamma_srgb_channels).collect();
	Some(std::array::from_fn(|channel| channels.iter().map(|values| values[channel]).collect()))
}

/// Whether every color keeps its alpha to within half an 8-bit level.
/// Only a per-channel adjustment's alpha curve can change it, since the kernels of the others all keep alpha.
pub fn preserves_alpha(adjustment: &Adjustment) -> bool {
	const LEVELS: usize = 256;

	channel_curves(adjustment, LEVELS).is_none_or(|[.., alpha]| {
		let identity = |index: usize| index as f32 / (LEVELS - 1) as f32;
		alpha.iter().enumerate().all(|(index, value)| (value - identity(index)).abs() <= 0.5 / 255.)
	})
}

/// Receives the kernel carrying out an [`Adjustment`], which has its own type for each adjustment.
pub trait AdjustmentVisitor {
	type Output;
	fn visit<K: MapColor>(self, kernel: K) -> Self::Output;
	/// Stands in for the kernel of an adjustment that leaves every color as it is, such as a lookup table with a file that can't be read.
	fn unchanged(self) -> Self::Output;
}

/// Hands `visitor` the kernel carrying out `adjustment`, with what the kernel reads worked out once from the adjustment's settings.
/// Settings past their nodes' bounds that would divide by zero are clamped to those bounds, so a description from anywhere stays finite.
pub fn visit_kernel<V: AdjustmentVisitor>(adjustment: &Adjustment, visitor: V) -> V::Output {
	match adjustment {
		Adjustment::Desaturate { method } => visitor.visit(Desaturate { method: *method }),
		Adjustment::GammaCorrection { gamma, inverse } => {
			let gamma = gamma.max(0.0001);
			let exponent = if *inverse { 1. / gamma } else { gamma };
			visitor.visit(GammaCorrection { exponent: exponent as f32 })
		}
		Adjustment::MakeOpaque => visitor.visit(MakeOpaque),
		Adjustment::BrightnessContrast {
			brightness,
			contrast,
			use_classic,
			classic_pivot,
		} => {
			let classic_pivot = (classic_pivot / 255.) as f32;

			// Each algorithm has its own slider range, matched here and in the Inspector
			let (brightness_extent, contrast_minimum) = if *use_classic { (100., -100.) } else { (150., -50.) };
			let brightness = (*brightness as f32).clamp(-brightness_extent, brightness_extent);
			let contrast = (*contrast as f32).clamp(contrast_minimum, 100.) / 100.;

			// Beyond a magnitude of 100, the curve for 100 is applied first and the curve for the remainder after it
			let magnitude = brightness.abs();
			let first_curve = BrightnessCurve::new(magnitude.min(100.));
			let second_curve = BrightnessCurve::new((magnitude - 100.).max(0.));

			visitor.visit(BrightnessContrast {
				use_classic: *use_classic,
				brightness,
				contrast,
				classic_pivot,
				first_curve,
				second_curve,
			})
		}
		Adjustment::Levels(levels) => {
			let record = |record: &settings::LevelsRecord| {
				LevelsRecord::new(
					record.shadows as f32,
					record.midtones as f32,
					record.highlights as f32,
					record.output_minimums as f32,
					record.output_maximums as f32,
				)
			};
			let composite = record(&levels.composite);

			// Alpha stands apart from the composite record that the three color channels pass through
			let alpha = record(&levels.alpha);

			visitor.visit(Levels {
				red: LevelsChain::new(record(&levels.red), composite),
				green: LevelsChain::new(record(&levels.green), composite),
				blue: LevelsChain::new(record(&levels.blue), composite),
				alpha: alpha.stage(alpha.midtones),
			})
		}
		Adjustment::Curves(curves) => visitor.visit(Curves {
			composite: curves.composite.evaluator(),
			red: curves.red.evaluator(),
			green: curves.green.evaluator(),
			blue: curves.blue.evaluator(),
			alpha: curves.alpha.evaluator(),
		}),
		Adjustment::BlackAndWhite(black_and_white) => visitor.visit(BlackAndWhite {
			use_tint: black_and_white.use_tint,
			tint: black_and_white.tint,
			reds: (black_and_white.reds / 100.) as f32,
			yellows: (black_and_white.yellows / 100.) as f32,
			greens: (black_and_white.greens / 100.) as f32,
			cyans: (black_and_white.cyans / 100.) as f32,
			blues: (black_and_white.blues / 100.) as f32,
			magentas: (black_and_white.magentas / 100.) as f32,
		}),
		Adjustment::HueSaturation(hue_saturation) => {
			let sliders = |sliders: &settings::HueSaturationSliders| PreparedHueSaturationSliders::new(sliders.hue as f32, sliders.saturation as f32, sliders.lightness as f32);
			let range = |range: &settings::HueRangeSettings| {
				HueSaturationRangeSettings::new(
					range.falloff_start as f32,
					range.range_start as f32,
					range.range_end as f32,
					range.falloff_end as f32,
					sliders(&range.sliders),
				)
			};

			visitor.visit(HueSaturation {
				master: sliders(&hue_saturation.master),
				colorize: hue_saturation.colorize,
				colorize_settings: sliders(&hue_saturation.colorize_sliders),
				reds: range(&hue_saturation.reds),
				yellows: range(&hue_saturation.yellows),
				greens: range(&hue_saturation.greens),
				cyans: range(&hue_saturation.cyans),
				blues: range(&hue_saturation.blues),
				magentas: range(&hue_saturation.magentas),
			})
		}
		Adjustment::Invert => visitor.visit(Invert),
		Adjustment::Solarize => visitor.visit(Solarize),
		Adjustment::Threshold { min_luminance, max_luminance } => visitor.visit(Threshold {
			min_luminance: (min_luminance / 100.) as f32,
			max_luminance: (max_luminance / 100.) as f32,
		}),
		Adjustment::GradientMap { gradient, reverse } => visitor.visit(GradientMap {
			evaluator: gradient.element().evaluator(GradientSettings::from_item_attributes(gradient)),
			reverse: *reverse,
		}),
		Adjustment::Vibrance { vibrance, saturation } => visitor.visit(Vibrance {
			vibrance: (vibrance / 100.) as f32,
			saturation_scale: (1. + saturation / 100.) as f32,
		}),
		Adjustment::ChannelMixer(channel_mixer) => {
			// Weights and constants are 10-bit fixed point truncated toward zero, which PSD interop depends on
			let weight = |percent: f64| (percent as f32 * 1024. / 100.).trunc() / 1024.;
			let settings::ChannelMixerSettings {
				monochrome,
				monochrome_mix,
				red,
				green,
				blue,
			} = &**channel_mixer;

			visitor.visit(ChannelMixer {
				monochrome: *monochrome,
				monochrome_r: weight(monochrome_mix.red),
				monochrome_g: weight(monochrome_mix.green),
				monochrome_b: weight(monochrome_mix.blue),
				monochrome_c: weight(monochrome_mix.constant),
				red_r: weight(red.red),
				red_g: weight(red.green),
				red_b: weight(red.blue),
				red_c: weight(red.constant),
				green_r: weight(green.red),
				green_g: weight(green.green),
				green_b: weight(green.blue),
				green_c: weight(green.constant),
				blue_r: weight(blue.red),
				blue_g: weight(blue.green),
				blue_b: weight(blue.blue),
				blue_c: weight(blue.constant),
			})
		}
		Adjustment::SelectiveColor(selective_color) => {
			let mode = selective_color.mode;

			// For PSD interop, the combined percent (c + k + c k / 100) rounds half up to an integer
			let ink = |color: f32, k: f32| {
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

			let groups = [
				(SelectiveColorGroup::Reds, &selective_color.reds),
				(SelectiveColorGroup::Yellows, &selective_color.yellows),
				(SelectiveColorGroup::Greens, &selective_color.greens),
				(SelectiveColorGroup::Cyans, &selective_color.cyans),
				(SelectiveColorGroup::Blues, &selective_color.blues),
				(SelectiveColorGroup::Magentas, &selective_color.magentas),
				(SelectiveColorGroup::Whites, &selective_color.whites),
				(SelectiveColorGroup::Neutrals, &selective_color.neutrals),
				(SelectiveColorGroup::Blacks, &selective_color.blacks),
			]
			.map(|(group, inks)| {
				let [c, m, y, k] = [inks.cyan, inks.magenta, inks.yellow, inks.black].map(|percent| percent as f32);
				SelectiveColorGroupInks {
					group,
					cmyk: [c, m, y, k],
					inks: [ink(c, k), ink(m, k), ink(y, k)],
				}
			});

			visitor.visit(SelectiveColor { mode, groups })
		}
		Adjustment::Posterize { levels } => visitor.visit(Posterize { levels: (*levels).max(2) as f32 }),
		Adjustment::Exposure { exposure, offset, gamma_correction } => visitor.visit(Exposure {
			exposure: *exposure as f32,
			offset: *offset as f32,
			gamma_correction: gamma_correction.max(0.0001) as f32,
		}),
		Adjustment::ColorBalance(color_balance) => {
			let preserve_luminosity = color_balance.preserve_luminosity;

			// The derivation below is integer arithmetic, so the sliders round to whole percentages first
			let shifts = |shifts: &settings::ColorBalanceShifts| [shifts.cyan_red, shifts.magenta_green, shifts.yellow_blue].map(|value| (value as f32).round() as i32);
			let [s_r, s_g, s_b] = shifts(&color_balance.shadows);
			let [m_r, m_g, m_b] = shifts(&color_balance.midtones);
			let [h_r, h_g, h_b] = shifts(&color_balance.highlights);

			let s_max = s_r.max(s_g).max(s_b);
			let m_max = m_r.max(m_g).max(m_b);
			let m_min = m_r.min(m_g).min(m_b);
			let h_min = h_r.min(h_g).min(h_b);
			let red = color_balance_curve(s_r, m_r, h_r, s_max, m_max, m_min, h_min, preserve_luminosity);
			let green = color_balance_curve(s_g, m_g, h_g, s_max, m_max, m_min, h_min, preserve_luminosity);
			let blue = color_balance_curve(s_b, m_b, h_b, s_max, m_max, m_min, h_min, preserve_luminosity);

			visitor.visit(ColorBalance { red, green, blue })
		}
		Adjustment::PhotoFilter { color, density, preserve_luminosity } => {
			let density = (density / 100.) as f32;

			// The image is multiplied in XYZ by the filter color normalized to the white point, with density easing that multiplier toward 1
			let filter_xyz = multiply_matrix(&SRGB_TO_XYZ_D50, [color.r(), color.g(), color.b()]);
			let factor = [
				1. + density * (filter_xyz[0] / WHITE_XYZ_D50[0] - 1.),
				1. + density * (filter_xyz[1] / WHITE_XYZ_D50[1] - 1.),
				1. + density * (filter_xyz[2] / WHITE_XYZ_D50[2] - 1.),
			];

			visitor.visit(PhotoFilter {
				factor,
				preserve_luminosity: *preserve_luminosity,
			})
		}
		Adjustment::ColorLookup { lut_file } => match parse_cached(lut_file) {
			Ok(lut) => {
				let mut hasher = std::hash::DefaultHasher::new();
				std::hash::Hash::hash(&lut_file.hash(), &mut hasher);
				visitor.visit(ColorLookup {
					lut,
					file_key: std::hash::Hasher::finish(&hasher),
				})
			}
			Err(_) => visitor.unchanged(),
		},
		Adjustment::ColorOverlay { color, blend_mode, opacity } => visitor.visit(ColorOverlay {
			color: *color,
			blend_mode: *blend_mode,
			opacity: (opacity / 100.) as f32,
		}),
	}
}

struct Desaturate {
	method: DesaturateMethod,
}

impl MapColor for Desaturate {
	fn map_cpu(&self, color: Color) -> Color {
		// Gamma-encoded formulas are decoded as if they were a gray
		let gamma = || color.to_gamma_srgb_channels();
		let luminance = match self.method {
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
	}

	fn encode_step(&self, steps: &mut AdjustmentSteps) {
		steps.push(StepKind::Desaturate, [texel(&[desaturate_method_code(self.method) as f32])]);
	}
}

/// The code the shader knows a Desaturate method by.
fn desaturate_method_code(method: DesaturateMethod) -> u32 {
	match method {
		DesaturateMethod::LuminanceRec709 => 0,
		DesaturateMethod::LumaRec709 => 1,
		DesaturateMethod::LumaRec601 => 2,
		DesaturateMethod::LightnessOkLab => 3,
		DesaturateMethod::ChannelsAverage => 4,
		DesaturateMethod::ChannelsMinimum => 5,
		DesaturateMethod::ChannelsMaximum => 6,
		DesaturateMethod::LightnessHsl => 7,
	}
}

struct GammaCorrection {
	exponent: f32,
}

impl MapColor for GammaCorrection {
	fn map_cpu(&self, color: Color) -> Color {
		color.apply_gamma_exponent(self.exponent)
	}

	fn encode_step(&self, steps: &mut AdjustmentSteps) {
		steps.push(StepKind::GammaCorrection, [texel(&[self.exponent])]);
	}
}

struct MakeOpaque;

impl MapColor for MakeOpaque {
	fn map_cpu(&self, color: Color) -> Color {
		color.with_alpha(1.)
	}

	fn encode_step(&self, steps: &mut AdjustmentSteps) {
		steps.push(StepKind::MakeOpaque, []);
	}
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
#[derive(Clone, Copy)]
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

	fn texel(&self) -> [f32; 4] {
		texel(&[self.slope, self.knee, self.end_slope])
	}
}

struct BrightnessContrast {
	use_classic: bool,
	brightness: f32,
	contrast: f32,
	classic_pivot: f32,
	first_curve: BrightnessCurve,
	second_curve: BrightnessCurve,
}

impl MapColor for BrightnessContrast {
	fn map_cpu(&self, color: Color) -> Color {
		color.map_gamma_rgb(|c| {
			if self.use_classic {
				return brightness_contrast_classic(c, self.brightness / 255., self.contrast, self.classic_pivot);
			}

			// Negative brightness runs the same curves in reverse
			let brightened = if self.brightness >= 0. {
				self.second_curve.forward(self.first_curve.forward(c))
			} else {
				self.first_curve.inverse(self.second_curve.inverse(c))
			};

			// Contrast pushes away from (or pulls toward) the midpoint, most strongly at the quarter tones
			let contrasted = brightened + 0.76 * self.contrast * (2. * brightened - 1.) * brightened.min(1. - brightened);
			contrasted.clamp(0., 1.)
		})
	}

	fn encode_step(&self, steps: &mut AdjustmentSteps) {
		let settings = [f32::from(self.use_classic), self.brightness, self.contrast, self.classic_pivot];
		steps.push(StepKind::BrightnessContrast, [settings, self.first_curve.texel(), self.second_curve.texel()]);
	}
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
		let output = self.curve.apply(value) * (self.output_maximum - self.output_minimum) + self.output_minimum;

		// Held within the output range, which rounding could carry the result a hair past
		output.clamp(self.output_minimum.min(self.output_maximum), self.output_minimum.max(self.output_maximum))
	}

	fn texels(&self) -> [[f32; 4]; LEVELS_STAGE_TEXELS] {
		let [points, toe] = self.curve.texels();
		[points, toe, texel(&[self.output_minimum, self.output_maximum])]
	}
}

/// How many texels a Levels curve, a stage holding one and its output range, and a chain of two stages take up in a step.
const LEVELS_CURVE_TEXELS: usize = 2;
const LEVELS_STAGE_TEXELS: usize = LEVELS_CURVE_TEXELS + 1;
const LEVELS_CHAIN_TEXELS: usize = 1 + 2 * LEVELS_STAGE_TEXELS;

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

	fn texels(&self) -> impl Iterator<Item = [f32; 4]> {
		once(texel(&[f32::from(self.two_stages)])).chain(self.first.texels()).chain(self.second.texels())
	}
}

struct Levels {
	red: LevelsChain,
	green: LevelsChain,
	blue: LevelsChain,
	alpha: LevelsStage,
}

impl MapColor for Levels {
	fn map_cpu(&self, color: Color) -> Color {
		// Levels math operates in gamma space
		let [r, g, b, a] = color.to_gamma_srgb_channels();

		Color::from_gamma_srgb_channels(self.red.apply(r), self.green.apply(g), self.blue.apply(b), self.alpha.apply(a))
	}

	fn encode_step(&self, steps: &mut AdjustmentSteps) {
		let chains = self.red.texels().chain(self.green.texels()).chain(self.blue.texels());
		steps.push(StepKind::Levels, chains.chain(self.alpha.texels()));
	}
}

struct Curves {
	composite: TransferCurveEvaluator,
	red: TransferCurveEvaluator,
	green: TransferCurveEvaluator,
	blue: TransferCurveEvaluator,
	alpha: TransferCurveEvaluator,
}

impl Curves {
	fn apply(&self, [r, g, b, a]: [f32; 4]) -> [f32; 4] {
		let map = |channel: &TransferCurveEvaluator, value: f32| self.composite.evaluate(channel.evaluate(value as f64).clamp(0., 1.)).clamp(0., 1.) as f32;

		// Alpha stands apart from the composite curve that the three color channels pass through
		let a = self.alpha.evaluate(a as f64).clamp(0., 1.) as f32;

		[map(&self.red, r), map(&self.green, g), map(&self.blue, b), a]
	}
}

impl MapColor for Curves {
	fn map_cpu(&self, color: Color) -> Color {
		// Curves math operates in gamma space
		let [r, g, b, a] = self.apply(color.to_gamma_srgb_channels());
		Color::from_gamma_srgb_channels(r, g, b, a)
	}

	fn encode_step(&self, steps: &mut AdjustmentSteps) {
		let curves = [&self.composite, &self.red, &self.green, &self.blue, &self.alpha];
		steps.push(StepKind::Curves, curves.into_iter().flat_map(transfer_curve_texels));
	}
}

/// A curve's point count, then each point's x (as a pair, so the shader compares inputs against it as the CPU does), y, and second derivative,
/// with the width to the next point worked out in double precision, so points too close to tell apart in single precision keep their order.
fn transfer_curve_texels(curve: &TransferCurveEvaluator) -> Vec<[f32; 4]> {
	let knots: Vec<(glam::DVec2, f64)> = curve.knots().collect();
	let widths = knots.windows(2).map(|pair| (pair[1].0.x - pair[0].0.x).max(f64::EPSILON)).chain(once(0.));

	let points = knots.iter().zip(widths).flat_map(|(&(point, second_derivative), width)| {
		let [x, x_remainder] = f32_pair(point.x);
		[[x, x_remainder, point.y as f32, second_derivative as f32], texel(&[width as f32])]
	});
	once(texel(&[knots.len() as f32])).chain(points).collect()
}

struct BlackAndWhite {
	use_tint: bool,
	tint: Color,
	reds: f32,
	yellows: f32,
	greens: f32,
	cyans: f32,
	blues: f32,
	magentas: f32,
}

impl MapColor for BlackAndWhite {
	fn map_cpu(&self, color: Color) -> Color {
		let BlackAndWhite {
			use_tint,
			tint,
			reds,
			yellows,
			greens,
			cyans,
			blues,
			magentas,
		} = *self;

		// Black & White channel weights are tuned for gamma-space values
		let [r, g, b, alpha_part] = color.to_gamma_srgb_channels();

		let gray_base = r.min(g).min(b);

		let red_part = r - gray_base;
		let green_part = g - gray_base;
		let blue_part = b - gray_base;

		// The smallest channel, which has no part past the gray, picks the pair of hues the others lie between
		let additional = if r <= gray_base {
			let cyan_part = green_part.min(blue_part);
			cyan_part * cyans + (green_part - cyan_part) * greens + (blue_part - cyan_part) * blues
		} else if g <= gray_base {
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
	}

	fn encode_step(&self, steps: &mut AdjustmentSteps) {
		let weights = [f32::from(self.use_tint), self.reds, self.yellows, self.greens];
		let tint = [self.tint.r(), self.tint.g(), self.tint.b(), self.tint.a()];
		steps.push(StepKind::BlackAndWhite, [weights, texel(&[self.cyans, self.blues, self.magentas]), tint]);
	}
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

	let sector = if r >= maximum {
		wrap_positive((g - b) / chroma, 6.)
	} else if g >= maximum {
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
struct PreparedHueSaturationSliders {
	hue: f32,
	saturation: f32,
	saturation_gain: f32,
	lightness: f32,
}

impl PreparedHueSaturationSliders {
	fn new(hue: f32, saturation_percent: f32, lightness_percent: f32) -> Self {
		let saturation = saturation_percent / 100.;
		Self {
			hue,
			saturation,
			saturation_gain: saturation_gain(saturation),
			lightness: lightness_percent / 100.,
		}
	}

	fn texel(&self) -> [f32; 4] {
		[self.hue, self.saturation, self.saturation_gain, self.lightness]
	}
}

/// A hue range with its falloff: full weight from `range_start` to `range_end`, fading linearly to zero at the falloff ends.
#[derive(Clone, Copy)]
struct HueSaturationRangeSettings {
	falloff_start: f32,
	range_start: f32,
	range_end: f32,
	falloff_end: f32,
	settings: PreparedHueSaturationSliders,
}

impl HueSaturationRangeSettings {
	fn new(falloff_start: f32, range_start: f32, range_end: f32, falloff_end: f32, settings: PreparedHueSaturationSliders) -> Self {
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

		// Each falloff is measured from its end of the range, so a hue a hair outside the range keeps nearly its full weight
		let start_falloff = distance(self.falloff_start, self.range_start);
		let end_falloff = distance(self.range_end, self.falloff_end);
		if distance(hue, self.range_start) < start_falloff {
			return 1. - distance(hue, self.range_start) / start_falloff;
		}
		if distance(self.range_end, hue) < end_falloff {
			return 1. - distance(self.range_end, hue) / end_falloff;
		}

		0.
	}

	fn texels(&self) -> [[f32; 4]; 2] {
		[[self.falloff_start, self.range_start, self.range_end, self.falloff_end], self.settings.texel()]
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
			self.saturation_factor *= 1. + (range.settings.saturation_gain - 1.) * weight;
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
	if amount >= 0. { value * (1. - amount) + amount } else { value * (1. + amount) }
}

/// A range's lightness moves the channels toward the color's own maximum (positive) or minimum (negative) instead.
/// The weighted sum lands every channel exactly on it at a full amount, leaving no trace of chroma for a saturation to raise.
fn lightness_toward_max_or_min(rgb: [f32; 3], amount: f32) -> [f32; 3] {
	let maximum = rgb[0].max(rgb[1]).max(rgb[2]);
	let minimum = rgb[0].min(rgb[1]).min(rgb[2]);
	let toward = if amount >= 0. { maximum } else { minimum };
	let blend = |value: f32| value * (1. - amount.abs()) + toward * amount.abs();
	[blend(rgb[0]), blend(rgb[1]), blend(rgb[2])]
}

struct HueSaturation {
	master: PreparedHueSaturationSliders,
	colorize: bool,
	colorize_settings: PreparedHueSaturationSliders,
	reds: HueSaturationRangeSettings,
	yellows: HueSaturationRangeSettings,
	greens: HueSaturationRangeSettings,
	cyans: HueSaturationRangeSettings,
	blues: HueSaturationRangeSettings,
	magentas: HueSaturationRangeSettings,
}

impl MapColor for HueSaturation {
	fn map_cpu(&self, color: Color) -> Color {
		let Self {
			master,
			colorize,
			colorize_settings,
			reds,
			yellows,
			greens,
			cyans,
			blues,
			magentas,
		} = *self;
		let [r, g, b, alpha] = color.to_gamma_srgb_channels();

		if colorize {
			let [_, _, lightness] = gamma_rgb_to_hsl(r, g, b);
			let lightness = lightness_toward_white_or_black(lightness, colorize_settings.lightness);
			let [r, g, b] = hsl_to_gamma_rgb(colorize_settings.hue, colorize_settings.saturation, lightness);
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
			saturation_factor *= master.saturation_gain;
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
	}

	fn encode_step(&self, steps: &mut AdjustmentSteps) {
		let sliders = [texel(&[f32::from(self.colorize)]), self.master.texel(), self.colorize_settings.texel()];
		let ranges = [&self.reds, &self.yellows, &self.greens, &self.cyans, &self.blues, &self.magentas];
		steps.push(StepKind::HueSaturation, sliders.into_iter().chain(ranges.into_iter().flat_map(HueSaturationRangeSettings::texels)));
	}
}

struct Invert;

impl MapColor for Invert {
	fn map_cpu(&self, color: Color) -> Color {
		color.map_gamma_rgb(|channel| 1. - channel)
	}

	fn encode_step(&self, steps: &mut AdjustmentSteps) {
		steps.push(StepKind::Invert, []);
	}
}

/// For PSD interop, inverts each channel past middle gray, folding it back down.
struct Solarize;

impl MapColor for Solarize {
	fn map_cpu(&self, color: Color) -> Color {
		color.map_gamma_rgb(|channel| channel.min(1. - channel))
	}

	fn encode_step(&self, steps: &mut AdjustmentSteps) {
		steps.push(StepKind::Solarize, []);
	}
}

struct Threshold {
	min_luminance: f32,
	max_luminance: f32,
}

/// How far past either bound of a Threshold a luminance may fall and still lie within it (half a step of the fixed-point luma of 8-bit channels, for float ties).
pub(crate) const THRESHOLD_SLACK: f32 = 0.5 / (16384. * 255.);

impl MapColor for Threshold {
	fn map_cpu(&self, color: Color) -> Color {
		// For PSD interop, we compare this 14-bit fixed-point Rec. 601 luma against the level unrounded
		let [r, g, b, _] = color.to_gamma_srgb_channels();
		let luminance = (4915. * r + 9667. * g + 1802. * b) / 16384.;

		let output = if luminance >= self.min_luminance - THRESHOLD_SLACK && luminance <= self.max_luminance + THRESHOLD_SLACK {
			Color::WHITE
		} else {
			Color::BLACK
		};
		output.with_alpha(color.a())
	}

	fn encode_step(&self, steps: &mut AdjustmentSteps) {
		steps.push(StepKind::Threshold, [texel(&[self.min_luminance, self.max_luminance])]);
	}
}

struct GradientMap {
	evaluator: GradientEvaluator,
	reverse: bool,
}

impl MapColor for GradientMap {
	fn map_cpu(&self, color: Color) -> Color {
		// The classic 0.3/0.59/0.11 luma of the gamma-encoded channels picks the position along the gradient
		let [r, g, b, alpha] = color.to_gamma_srgb_channels();
		let intensity = 0.3 * r + 0.59 * g + 0.11 * b;
		let intensity = if self.reverse { 1. - intensity } else { intensity };

		// The source alpha is kept and the gradient's own alpha stops are ignored
		self.evaluator.evaluate_mapped(intensity as f64).with_alpha(alpha)
	}

	fn encode_step(&self, steps: &mut AdjustmentSteps) {
		let GradientSettings {
			spread,
			cyclic,
			space,
			hue_direction,
			interpolation,
		} = self.evaluator.settings();
		let stops = self.evaluator.stops();
		let stop_count = texel(&[stops.len() as f32, f32::from(cyclic)]);
		let color = |color: Color| texel(&[color.r(), color.g(), color.b()]);

		// The ramp's Smooth path, its stops' colors to hold when stepped, or else its stops to interpolate linearly between
		let (evaluation, ramp): (RampEvaluation, Vec<[f32; 4]>) = match self.evaluator.smooth_knots() {
			Some((warp, [first, second, third, _])) => {
				// The channel splines share their knots' positions, and alpha is left out since the map keeps the color's own
				let warp_knots = monotonic_spline_texels(&[&warp]);
				let channel_knots = monotonic_spline_texels(&[&first, &second, &third]);

				let knot_counts = texel(&[warp.len() as f32, first.len() as f32]);
				(RampEvaluation::Smooth, once(knot_counts).chain(warp_knots).chain(channel_knots).collect())
			}
			None if interpolation == GradientInterpolation::Stepped => {
				let stop_texels = stops.iter().flat_map(|stop| [texel(&f32_pair(stop.position)), color(stop.color)]);
				(RampEvaluation::Stepped, once(stop_count).chain(stop_texels).collect())
			}
			None => {
				// Each stop with its interval to the next (the last stop's wrapping around to the first): its width in double precision and the channels it interpolates between
				let stop_texels = stops.iter().enumerate().flat_map(|(index, stop)| {
					let next = &stops[(index + 1) % stops.len()];
					let width = if index + 1 < stops.len() {
						next.position - stop.position
					} else {
						next.position + 1. - stop.position
					};
					let [start, end] = interpolation_endpoints(stop.color, next.color, space, hue_direction);

					let [position, position_remainder] = f32_pair(stop.position);
					let placement = [position, position_remainder, width as f32, sanitized_midpoint(stop.midpoint) as f32];
					[placement, color(stop.color), texel(&start), texel(&end)]
				});
				(RampEvaluation::Linear, once(stop_count).chain(stop_texels).collect())
			}
		};

		let settings = [
			f32::from(self.reverse),
			gradient_spread_code(spread) as f32,
			evaluation as u32 as f32,
			gradient_space_code(space) as f32,
		];
		steps.push(StepKind::GradientMap, once(settings).chain(ramp));
	}
}

/// How the shader evaluates a Gradient Map's ramp, numbered as its constants for them are.
#[derive(Clone, Copy)]
enum RampEvaluation {
	Stepped = 0,
	Linear = 1,
	Smooth = 2,
}

/// The code the shader knows a gradient's spread by.
fn gradient_spread_code(spread: GradientSpread) -> u32 {
	match spread {
		GradientSpread::Pad => 0,
		GradientSpread::Reflect => 1,
		GradientSpread::Repeat => 2,
		GradientSpread::Clear => 3,
	}
}

/// The code the shader knows a gradient's interpolation space by.
fn gradient_space_code(space: GradientSpace) -> u32 {
	match space {
		GradientSpace::OkLab => 0,
		GradientSpace::Lab => 1,
		GradientSpace::OkLCh => 2,
		GradientSpace::LCh => 3,
		GradientSpace::RgbLinear => 4,
		GradientSpace::RgbGamma => 5,
		GradientSpace::Hsv => 6,
		GradientSpace::Hsl => 7,
	}
}

/// Up to three monotonic splines sharing the first one's knot positions. Each knot holds its position (paired, as the shader compares inputs against it)
/// and the run to the next worked out in double precision, then its value in each spline, then its tangent in each.
fn monotonic_spline_texels(splines: &[&SplineKnots]) -> Vec<[f32; 4]> {
	let Some(positions) = splines.first() else { return Vec::new() };

	(0..positions.len())
		.flat_map(|index| {
			let position = positions[index][0];
			let run = positions.get(index + 1).map_or(0., |next| next[0] - position);
			let component = |component: usize| texel(&splines.iter().map(|spline| spline.get(index).map_or(0., |knot| knot[component] as f32)).collect::<Vec<_>>());

			let [position, position_remainder] = f32_pair(position);
			[texel(&[position, position_remainder, run as f32]), component(1), component(2)]
		})
		.collect()
}

struct Vibrance {
	vibrance: f32,
	saturation_scale: f32,
}

impl MapColor for Vibrance {
	fn map_cpu(&self, color: Color) -> Color {
		// Vibrance then saturation, both in linear light, which equals applying each alone in turn
		let (r, g, b) = (color.r(), color.g(), color.b());
		let maximum = r.max(g).max(b);
		let [chroma_factor, brightness_factor] = vibrance_factors(r, g, b, self.vibrance);
		let after_vibrance = Color::from_rgbaf32_unchecked(
			scale_about_maximum(r, maximum, chroma_factor, brightness_factor),
			scale_about_maximum(g, maximum, chroma_factor, brightness_factor),
			scale_about_maximum(b, maximum, chroma_factor, brightness_factor),
			color.a(),
		);

		// For PSD interop, saturation scales each channel's distance from a gray weighted by ProPhoto's luminance coefficients
		let gray = 0.288040 * after_vibrance.r() + 0.711874 * after_vibrance.g() + 0.000086 * after_vibrance.b();
		after_vibrance.map_rgb(|c| (gray + (c - gray) * self.saturation_scale).clamp(0., 1.))
	}

	fn encode_step(&self, steps: &mut AdjustmentSteps) {
		steps.push(StepKind::Vibrance, [texel(&[self.vibrance, self.saturation_scale])]);
	}
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

struct ChannelMixer {
	monochrome: bool,
	monochrome_r: f32,
	monochrome_g: f32,
	monochrome_b: f32,
	monochrome_c: f32,
	red_r: f32,
	red_g: f32,
	red_b: f32,
	red_c: f32,
	green_r: f32,
	green_g: f32,
	green_b: f32,
	green_c: f32,
	blue_r: f32,
	blue_g: f32,
	blue_b: f32,
	blue_c: f32,
}

impl MapColor for ChannelMixer {
	fn map_cpu(&self, color: Color) -> Color {
		let [r, g, b, a] = color.to_gamma_srgb_channels();

		let (out_r, out_g, out_b) = if self.monochrome {
			let gray = (r * self.monochrome_r + g * self.monochrome_g + b * self.monochrome_b + self.monochrome_c).clamp(0., 1.);

			(gray, gray, gray)
		} else {
			let red = (r * self.red_r + g * self.red_g + b * self.red_b + self.red_c).clamp(0., 1.);
			let green = (r * self.green_r + g * self.green_g + b * self.green_b + self.green_c).clamp(0., 1.);
			let blue = (r * self.blue_r + g * self.blue_g + b * self.blue_b + self.blue_c).clamp(0., 1.);

			(red, green, blue)
		};

		Color::from_gamma_srgb_channels(out_r, out_g, out_b, a)
	}

	fn encode_step(&self, steps: &mut AdjustmentSteps) {
		let rows = [
			texel(&[f32::from(self.monochrome)]),
			[self.monochrome_r, self.monochrome_g, self.monochrome_b, self.monochrome_c],
			[self.red_r, self.red_g, self.red_b, self.red_c],
			[self.green_r, self.green_g, self.green_b, self.green_c],
			[self.blue_r, self.blue_g, self.blue_b, self.blue_c],
		];
		steps.push(StepKind::ChannelMixer, rows);
	}
}

/// The groups of colors that Selective Color adjusts, in the order of its settings, numbered as the GPU's shader numbers them.
#[derive(Clone, Copy)]
enum SelectiveColorGroup {
	Reds = 0,
	Yellows = 1,
	Greens = 2,
	Cyans = 3,
	Blues = 4,
	Magentas = 5,
	Whites = 6,
	Neutrals = 7,
	Blacks = 8,
}

/// A group's cyan, magenta, yellow, and black percentages, with the inks they combine into for each of red, green, and blue.
#[derive(Clone, Copy)]
struct SelectiveColorGroupInks {
	group: SelectiveColorGroup,
	cmyk: [f32; 4],
	inks: [f32; 3],
}

struct SelectiveColor {
	mode: RelativeAbsolute,
	groups: [SelectiveColorGroupInks; 9],
}

impl MapColor for SelectiveColor {
	fn map_cpu(&self, color: Color) -> Color {
		let [r, g, b, a] = color.to_gamma_srgb_channels();

		let min = |a: f32, b: f32, c: f32| a.min(b).min(c);
		let max = |a: f32, b: f32, c: f32| a.max(b).max(c);
		let med = |a: f32, b: f32, c: f32| a + b + c - min(a, b, c) - max(a, b, c);

		let max_channel = max(r, g, b);
		let min_channel = min(r, g, b);

		let pixel_color_range = |group| match group {
			SelectiveColorGroup::Reds => r >= max_channel,
			SelectiveColorGroup::Yellows => b <= min_channel,
			SelectiveColorGroup::Greens => g >= max_channel,
			SelectiveColorGroup::Cyans => r <= min_channel,
			SelectiveColorGroup::Blues => b >= max_channel,
			SelectiveColorGroup::Magentas => g <= min_channel,
			SelectiveColorGroup::Whites => r > 0.5 && g > 0.5 && b > 0.5,
			// Every pixel, since the neutrals scale factor already vanishes at black, white, and fully saturated colors
			SelectiveColorGroup::Neutrals => true,
			SelectiveColorGroup::Blacks => r < 0.5 && g < 0.5 && b < 0.5,
		};

		let color_parameter_group_scale_factor_rgb = max(r, g, b) - med(r, g, b);
		let color_parameter_group_scale_factor_cmy = med(r, g, b) - min(r, g, b);

		// Used to apply the r, g, or b channel slope (by multiplying it by 1) in relative mode, or no slope (by multiplying it by 0) in absolute mode
		let (slope_r, slope_g, slope_b) = match self.mode {
			RelativeAbsolute::Relative => (r - 1., g - 1., b - 1.),
			RelativeAbsolute::Absolute => (-1., -1., -1.),
		};

		let mut sum = Vec3::ZERO;
		for SelectiveColorGroupInks {
			group,
			cmyk: [c, m, y, k],
			inks: [ink_c, ink_m, ink_y],
		} in self.groups
		{
			// Skip this group if it's left at zero or this pixel's color isn't in the range it affects
			if (c == 0. && m == 0. && y == 0. && k == 0.) || !pixel_color_range(group) {
				continue;
			}

			let color_parameter_group_scale_factor = match group {
				SelectiveColorGroup::Reds | SelectiveColorGroup::Greens | SelectiveColorGroup::Blues => color_parameter_group_scale_factor_rgb,
				SelectiveColorGroup::Cyans | SelectiveColorGroup::Magentas | SelectiveColorGroup::Yellows => color_parameter_group_scale_factor_cmy,
				SelectiveColorGroup::Whites => min(r, g, b) * 2. - 1.,
				SelectiveColorGroup::Neutrals => 1. - ((max(r, g, b) - 0.5).abs() + (min(r, g, b) - 0.5).abs()),
				SelectiveColorGroup::Blacks => 1. - max(r, g, b) * 2.,
			};

			let offset_r = (ink_c * slope_r).max(-r).min(-r + 1.) * color_parameter_group_scale_factor;
			let offset_g = (ink_m * slope_g).max(-g).min(-g + 1.) * color_parameter_group_scale_factor;
			let offset_b = (ink_y * slope_b).max(-b).min(-b + 1.) * color_parameter_group_scale_factor;

			// An 8-bit PSD document sums the groups' 8-bit offsets, which this float node does not currently attempt to reproduce
			sum += Vec3::new(offset_r, offset_g, offset_b);
		}

		let rgb = Vec3::new(r, g, b);
		let out = (sum + rgb).clamp(Vec3::ZERO, Vec3::ONE);

		Color::from_gamma_srgb_channels(out.x, out.y, out.z, a)
	}

	fn encode_step(&self, steps: &mut AdjustmentSteps) {
		// Groups left at zero offset nothing, so only the others are carried out, in the same order
		let groups: Vec<[f32; 4]> = (self.groups.iter())
			.filter(|group| group.cmyk.iter().any(|&percent| percent != 0.))
			.map(|group| {
				let [ink_c, ink_m, ink_y] = group.inks;
				[group.group as u32 as f32, ink_c, ink_m, ink_y]
			})
			.collect();

		let settings = texel(&[f32::from(self.mode == RelativeAbsolute::Absolute), groups.len() as f32]);
		steps.push(StepKind::SelectiveColor, once(settings).chain(groups));
	}
}

struct Posterize {
	levels: f32,
}

impl MapColor for Posterize {
	fn map_cpu(&self, color: Color) -> Color {
		let levels = self.levels;

		color.map_gamma_rgb(|c| {
			// Bins as floor(c * levels) with the outputs spread evenly to white.
			// The sliver of slack keeps an input exactly on an edge in the upper bin despite float ties.
			let bin = ((c + 2e-7) * levels).floor().min(levels - 1.);
			(bin / (levels - 1.)).min(1.)
		})
	}

	fn encode_step(&self, steps: &mut AdjustmentSteps) {
		steps.push(StepKind::Posterize, [texel(&[self.levels])]);
	}
}

struct Exposure {
	exposure: f32,
	offset: f32,
	gamma_correction: f32,
}

impl MapColor for Exposure {
	fn map_cpu(&self, color: Color) -> Color {
		// Linearizes with a 2.2 power above a straight toe of slope 1/32, the two meeting at this constant
		const TOE_END: f32 = 0.05568117; // 32^(-1. / 1.2)

		let decode = |value: f32| if value < TOE_END { value / 32. } else { value.powf(2.2) };
		let encode = |linear: f32| if linear < TOE_END / 32. { linear * 32. } else { linear.powf(1. / 2.2) };
		let adjust = |c: f32| {
			let linear = decode(c) * 2_f32.powf(self.exposure) + self.offset;
			encode(linear.max(0.).powf(1. / self.gamma_correction).min(1.)).min(1.)
		};

		let [r, g, b, a] = color.to_gamma_srgb_channels();
		Color::from_gamma_srgb_channels(adjust(r), adjust(g), adjust(b), a)
	}

	fn encode_step(&self, steps: &mut AdjustmentSteps) {
		steps.push(StepKind::Exposure, [texel(&[self.exposure, self.offset, self.gamma_correction])]);
	}
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
		(y / 255.).clamp(0., 1.)
	}

	fn texels(&self) -> [[f32; 4]; LEVELS_CURVE_TEXELS] {
		[
			[self.black, self.white, self.exponent, self.toe_end],
			texel(&[self.toe_value, self.toe_slope_start, self.toe_slope_end]),
		]
	}
}

/// One channel's Levels parameters from its own slider values, and (with preserve_luminosity) the slider
/// extremes across all three channels. The halvings truncate toward zero, as PSD interop requires.
#[allow(clippy::too_many_arguments)]
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

struct ColorBalance {
	red: LevelsCurve,
	green: LevelsCurve,
	blue: LevelsCurve,
}

impl MapColor for ColorBalance {
	fn map_cpu(&self, color: Color) -> Color {
		// The curves operate on gamma-space channel values
		let [r, g, b, a] = color.to_gamma_srgb_channels();
		Color::from_gamma_srgb_channels(self.red.apply(r), self.green.apply(g), self.blue.apply(b), a)
	}

	fn encode_step(&self, steps: &mut AdjustmentSteps) {
		steps.push(StepKind::ColorBalance, [self.red, self.green, self.blue].iter().flat_map(LevelsCurve::texels));
	}
}

struct PhotoFilter {
	factor: [f32; 3],
	preserve_luminosity: bool,
}

impl MapColor for PhotoFilter {
	fn map_cpu(&self, pixel: Color) -> Color {
		let PhotoFilter { factor, preserve_luminosity } = *self;

		let [r_in, g_in, b_in, alpha] = pixel.to_gamma_srgb_channels();
		let xyz = multiply_matrix(&SRGB_TO_XYZ_D50, [srgb_to_linear(r_in), srgb_to_linear(g_in), srgb_to_linear(b_in)]);
		let filtered = multiply_matrix(&XYZ_D50_TO_SRGB, [xyz[0] * factor[0], xyz[1] * factor[1], xyz[2] * factor[2]]);
		let mut r = linear_to_srgb(filtered[0].clamp(0., 1.)).min(1.);
		let mut g = linear_to_srgb(filtered[1].clamp(0., 1.)).min(1.);
		let mut b = linear_to_srgb(filtered[2].clamp(0., 1.)).min(1.);

		if preserve_luminosity {
			let luma_in = luma_rec_601_fixed_point(r_in.clamp(0., 1.), g_in.clamp(0., 1.), b_in.clamp(0., 1.));
			[r, g, b] = set_luminosity(r, g, b, luma_rec_601_fixed_point(r, g, b), luma_in);
		}

		Color::from_gamma_srgb_channels(r, g, b, alpha)
	}

	fn encode_step(&self, steps: &mut AdjustmentSteps) {
		let [x, y, z] = self.factor;
		steps.push(StepKind::PhotoFilter, [[x, y, z, f32::from(self.preserve_luminosity)]]);
	}
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

struct ColorLookup {
	lut: Arc<Lut>,
	/// Identifies the file the table was read from, so its upload outlasts changes to the other adjustments around it.
	file_key: u64,
}

impl MapColor for ColorLookup {
	fn map_cpu(&self, color: Color) -> Color {
		// Lookup tables address the gamma-encoded channels
		let [r, g, b, a] = color.to_gamma_srgb_channels();
		let [r, g, b] = self.lut.apply([r, g, b]);

		Color::from_gamma_srgb_channels(r, g, b, a)
	}

	fn encode_step(&self, steps: &mut AdjustmentSteps) {
		let lut = self.lut.clone();
		let table = steps.push_table(self.file_key, move |max_texels| lut.texels_within(max_texels));
		steps.push(StepKind::ColorLookup, [texel(&[table as f32])]);
	}
}

/// The names the shader gives the codes the kernels here encode settings with, and the texel counts it steps over.
pub(crate) fn shader_constants() -> Vec<(&'static str, u32)> {
	let desaturate_methods = [
		("LUMINANCE_REC_709", DesaturateMethod::LuminanceRec709),
		("LUMA_REC_709", DesaturateMethod::LumaRec709),
		("LUMA_REC_601", DesaturateMethod::LumaRec601),
		("LIGHTNESS_OKLAB", DesaturateMethod::LightnessOkLab),
		("CHANNELS_AVERAGE", DesaturateMethod::ChannelsAverage),
		("CHANNELS_MINIMUM", DesaturateMethod::ChannelsMinimum),
		("CHANNELS_MAXIMUM", DesaturateMethod::ChannelsMaximum),
		("LIGHTNESS_HSL", DesaturateMethod::LightnessHsl),
	]
	.map(|(name, method)| (name, desaturate_method_code(method)));

	let selective_color_groups = [
		("REDS", SelectiveColorGroup::Reds),
		("YELLOWS", SelectiveColorGroup::Yellows),
		("GREENS", SelectiveColorGroup::Greens),
		("CYANS", SelectiveColorGroup::Cyans),
		("BLUES", SelectiveColorGroup::Blues),
		("MAGENTAS", SelectiveColorGroup::Magentas),
		("WHITES", SelectiveColorGroup::Whites),
		("NEUTRALS", SelectiveColorGroup::Neutrals),
		("BLACKS", SelectiveColorGroup::Blacks),
	]
	.map(|(name, group)| (name, group as u32));

	let spreads = [
		("PAD", GradientSpread::Pad),
		("REFLECT", GradientSpread::Reflect),
		("REPEAT", GradientSpread::Repeat),
		("CLEAR", GradientSpread::Clear),
	]
	.map(|(name, spread)| (name, gradient_spread_code(spread)));

	let evaluations = [("STEPPED", RampEvaluation::Stepped), ("LINEAR", RampEvaluation::Linear), ("SMOOTH", RampEvaluation::Smooth)].map(|(name, evaluation)| (name, evaluation as u32));

	let spaces = [
		("OKLAB", GradientSpace::OkLab),
		("LAB", GradientSpace::Lab),
		("OKLCH", GradientSpace::OkLCh),
		("LCH", GradientSpace::LCh),
		("RGB_LINEAR", GradientSpace::RgbLinear),
		("RGB_GAMMA", GradientSpace::RgbGamma),
		("HSV", GradientSpace::Hsv),
		("HSL", GradientSpace::Hsl),
	]
	.map(|(name, space)| (name, gradient_space_code(space)));

	let texel_counts = [
		("LEVELS_CURVE_TEXELS", LEVELS_CURVE_TEXELS),
		("LEVELS_STAGE_TEXELS", LEVELS_STAGE_TEXELS),
		("LEVELS_CHAIN_TEXELS", LEVELS_CHAIN_TEXELS),
	]
	.map(|(name, count)| (name, count as u32));

	(desaturate_methods.into_iter())
		.chain(selective_color_groups)
		.chain(spreads)
		.chain(evaluations)
		.chain(spaces)
		.chain(texel_counts)
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	const COLORS: [Color; 2] = [Color::from_rgbaf32_unchecked(0.2, 0.5, 0.9, 1.), Color::from_rgbaf32_unchecked(1., 0., 0.3, 0.5)];

	#[test]
	fn a_prepared_adjustment_matches_preparing_it_for_each_color() {
		let adjustments = [
			Adjustment::Invert,
			Adjustment::Posterize { levels: 3 },
			Adjustment::Exposure {
				exposure: 1.,
				offset: 0.1,
				gamma_correction: 1.2,
			},
		];

		for adjustment in &adjustments {
			let prepared = PreparedAdjustment::new(adjustment);
			for color in COLORS {
				assert_eq!(prepared.adjust(color), adjust_color(adjustment, color), "{adjustment:?}");
			}
		}
	}

	#[test]
	fn settings_past_their_bounds_stay_finite() {
		let adjustments = [
			Adjustment::Posterize { levels: 1 },
			Adjustment::GammaCorrection { gamma: 0., inverse: true },
			Adjustment::Exposure {
				exposure: 0.,
				offset: 0.,
				gamma_correction: 0.,
			},
		];

		for adjustment in &adjustments {
			for color in COLORS {
				let adjusted = adjust_color(adjustment, color);
				assert!([adjusted.r(), adjusted.g(), adjusted.b(), adjusted.a()].iter().all(|channel| channel.is_finite()), "{adjustment:?}");
			}
		}
	}

	fn gray(level: u8) -> Color {
		let value = f32::from(level) / 255.;
		Color::from_gamma_srgb_channels(value, value, value, 1.)
	}

	#[test]
	fn white_stays_exactly_white_through_adjustments_set_to_keep_it() {
		let identity = settings::LevelsRecord {
			shadows: 0.,
			midtones: 1.,
			highlights: 100.,
			output_minimums: 0.,
			output_maximums: 100.,
		};
		let adjustments = [
			Adjustment::Levels(Box::new(settings::LevelsSettings {
				composite: settings::LevelsRecord { midtones: 1.2, ..identity },
				red: identity,
				green: identity,
				blue: identity,
				alpha: identity,
			})),
			Adjustment::ColorBalance(Default::default()),
			Adjustment::Curves(Default::default()),
			Adjustment::Posterize { levels: 4 },
			Adjustment::HueSaturation(Default::default()),
			Adjustment::Exposure {
				exposure: 0.,
				offset: 0.,
				gamma_correction: 1.,
			},
			Adjustment::Threshold {
				min_luminance: 100.,
				max_luminance: 100.,
			},
		];

		for adjustment in &adjustments {
			assert_eq!(adjust_color(adjustment, Color::WHITE), Color::WHITE, "{adjustment:?}");
		}
	}

	#[test]
	fn a_threshold_takes_in_the_grays_exactly_at_its_bounds() {
		for level in 0..=255_u8 {
			let bound = f64::from(level) / 255. * 100.;
			let from_the_gray = Adjustment::Threshold {
				min_luminance: bound,
				max_luminance: 100.,
			};
			let up_to_the_gray = Adjustment::Threshold {
				min_luminance: 0.,
				max_luminance: bound,
			};

			assert_eq!(adjust_color(&from_the_gray, gray(level)), Color::WHITE, "from gray {level}");
			assert_eq!(adjust_color(&up_to_the_gray, gray(level)), Color::WHITE, "up to gray {level}");
		}

		// A color with a fixed-point luma a few steps short of the bound is still out
		let from_82 = Adjustment::Threshold {
			min_luminance: 82. / 255. * 100.,
			max_luminance: 100.,
		};
		let just_short = Color::from_gamma_srgb_channels(1., 0., 50. / 255., 1.);
		assert_eq!(adjust_color(&from_82, just_short), Color::BLACK);
	}

	#[test]
	fn a_range_at_full_lightness_leaves_grays_that_full_saturation_keeps_gray() {
		let sliders = |saturation, lightness| settings::HueSaturationSliders { hue: 0., saturation, lightness };
		let range = |sliders, [falloff_start, range_start, range_end, falloff_end]: [f64; 4]| settings::HueRangeSettings {
			sliders,
			falloff_start,
			range_start,
			range_end,
			falloff_end,
		};

		for lightness in [-100., 100.] {
			let hue_saturation = Adjustment::HueSaturation(Box::new(settings::HueSaturationSettings {
				master: sliders(100., 0.),
				reds: range(sliders(0., lightness), [315., 345., 15., 45.]),
				yellows: range(sliders(0., 0.), [15., 45., 75., 105.]),
				greens: range(sliders(0., 0.), [75., 105., 135., 165.]),
				cyans: range(sliders(0., 0.), [135., 165., 195., 225.]),
				blues: range(sliders(0., 0.), [195., 225., 255., 285.]),
				magentas: range(sliders(0., 0.), [255., 285., 315., 345.]),
				..Default::default()
			}));

			// Reds of every strength with hues well within the range
			for red in (40..=255_u8).step_by(5) {
				for low in (0..red / 2).step_by(7) {
					let color = Color::from_gamma_srgb_channels(f32::from(red) / 255., f32::from(low) / 255., f32::from(low + red / 16) / 255., 1.);
					let [r, g, b, _] = adjust_color(&hue_saturation, color).to_gamma_srgb_channels();
					assert!(r == g && g == b, "{color:?} at a lightness of {lightness} comes out as {:?}", [r, g, b]);
				}
			}
		}
	}

	#[test]
	fn a_hue_a_hair_outside_a_range_keeps_nearly_its_full_weight() {
		let sliders = PreparedHueSaturationSliders::new(0., 0., 0.);

		// Ranges with a falloff on each side, some of them reaching across the turn of the hue circle
		for [falloff_start, range_start, range_end, falloff_end] in [[15., 45., 75., 105.], [350., 10., 40., 60.], [291., 105., 250., 280.], [300., 330., 350., 20.]] {
			let range = HueSaturationRangeSettings::new(falloff_start, range_start, range_end, falloff_end, sliders);

			let just_before = f32::from_bits(range.range_start.to_bits() - 1);
			let just_after = f32::from_bits(range.range_end.to_bits() + 1);
			for hue in [just_before, range.range_start, range.range_end, just_after] {
				let weight = range.weight(hue);
				assert!(weight > 0.9999 && weight <= 1., "A range from {range_start} to {range_end} weighs a hue of {hue} as {weight}");
			}
		}
	}

	#[test]
	fn a_gradient_map_lands_black_white_and_grays_on_the_stops_at_them() {
		use core_types::list::{ATTR_GRADIENT_INTERPOLATION, ATTR_GRADIENT_SPREAD, Item};
		use vector_types::Gradient;
		use vector_types::gradient::GradientStop;

		let stop = |position, value: f32| GradientStop {
			position,
			midpoint: 0.5,
			color: Color::from_gamma_srgb_channels(value, value, value, 1.),
		};
		let map = |stops: Vec<GradientStop>, interpolation: GradientInterpolation, spread: GradientSpread| Adjustment::GradientMap {
			gradient: Item::new_from_element(Gradient::new(stops))
				.with_attribute(ATTR_GRADIENT_INTERPOLATION, interpolation)
				.with_attribute(ATTR_GRADIENT_SPREAD, spread),
			reverse: false,
		};
		let mapped = |map: &Adjustment, level: u8| adjust_color(map, gray(level)).to_gamma_srgb_channels()[0];

		// A stepped ramp gives each gray the color of the stop it lands on, white among them
		let stepped = map(
			vec![stop(0., 0.1), stop(51. / 255., 0.3), stop(128. / 255., 0.5), stop(204. / 255., 0.7), stop(1., 0.9)],
			GradientInterpolation::Stepped,
			GradientSpread::Pad,
		);
		for (level, value) in [(0, 0.1), (50, 0.1), (51, 0.3), (127, 0.3), (128, 0.5), (204, 0.7), (254, 0.7), (255, 0.9)] {
			assert!((mapped(&stepped, level) - value).abs() < 1e-5, "gray {level} steps to {}", mapped(&stepped, level));
		}

		// A repeating ramp keeps white at its end, and a clear one keeps it on the ramp
		for spread in [GradientSpread::Repeat, GradientSpread::Clear] {
			let ramp = map(vec![stop(0., 0.), stop(1., 1.)], GradientInterpolation::Linear, spread);
			assert_eq!((mapped(&ramp, 0), mapped(&ramp, 255)), (0., 1.), "{spread:?}");
		}
	}
}
