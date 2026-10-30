//! The kernels carrying out each [`Adjustment`], with its settings turned into what its kernel reads by [`visit_kernel`].

use crate::blending::ColorOverlay;
use crate::color_lookup_table::{Lut, parse_cached};
use crate::{Lut1d, Lut3d, MapColor, ToGpuParams, Wgsl, gpu_param_enum, gpu_params};
use core_types::color::Color;
use core_types::color::{linear_to_srgb, set_luminosity, srgb_to_linear};
use core_types::transfer_curve::TransferCurveEvaluator;
use glam::Vec3;
use graphic_types::EffectOperation;
use graphic_types::adjustment::{self as settings, Adjustment, DesaturateMethod, RelativeAbsolute, SelectiveColorChoice};
use std::sync::Arc;
use vector_types::gradient::GradientInterpolation;
use vector_types::{GradientSettings, GradientSpread};

/// Receives the kernel carrying out an [`Adjustment`], whose type depends on which adjustment it is.
pub trait AdjustmentVisitor {
	type Output;
	fn visit<K: MapColor>(self, kernel: &K) -> Self::Output;
	/// Stands in for the kernel of an adjustment that leaves every color as it is, such as a lookup table whose file can't be read.
	fn unchanged(self) -> Self::Output;
}

/// Hands `visitor` the kernel carrying out `adjustment`, with what the kernel reads worked out once from the adjustment's settings.
pub fn visit_kernel<V: AdjustmentVisitor>(adjustment: &Adjustment, visitor: V) -> V::Output {
	match adjustment {
		Adjustment::Desaturate { method } => visitor.visit(&Desaturate { method: *method }),
		Adjustment::GammaCorrection { gamma, inverse } => {
			let exponent = if *inverse { 1. / gamma } else { *gamma };
			visitor.visit(&GammaCorrection { exponent: exponent as f32 })
		}
		Adjustment::MakeOpaque => visitor.visit(&MakeOpaque {}),
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

			visitor.visit(&BrightnessContrast {
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

			visitor.visit(&Levels {
				red: LevelsChain::new(record(&levels.red), composite),
				green: LevelsChain::new(record(&levels.green), composite),
				blue: LevelsChain::new(record(&levels.blue), composite),
				alpha: alpha.stage(alpha.midtones),
			})
		}
		Adjustment::Curves(curves) => visitor.visit(&Curves {
			composite: curves.composite.evaluator(),
			red: curves.red.evaluator(),
			green: curves.green.evaluator(),
			blue: curves.blue.evaluator(),
			alpha: curves.alpha.evaluator(),
		}),
		Adjustment::BlackAndWhite(black_and_white) => visitor.visit(&BlackAndWhite {
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
			let sliders = |sliders: &settings::HueSaturationSliders| HueSaturationSettings::new(sliders.hue as f32, sliders.saturation as f32, sliders.lightness as f32);
			let range = |range: &settings::HueRangeSettings| {
				HueSaturationRangeSettings::new(
					range.falloff_start as f32,
					range.range_start as f32,
					range.range_end as f32,
					range.falloff_end as f32,
					sliders(&range.sliders),
				)
			};

			visitor.visit(&HueSaturation {
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
		Adjustment::Invert => visitor.visit(&Invert {}),
		Adjustment::Threshold { min_luminance, max_luminance } => visitor.visit(&Threshold {
			min_luminance: (min_luminance / 100.) as f32,
			max_luminance: (max_luminance / 100.) as f32,
		}),
		Adjustment::GradientMap { gradient, reverse } => {
			let settings = GradientSettings::from(gradient);
			let gradient = gradient.element();

			visitor.visit(&GradientMap {
				evaluator: gradient.evaluator(settings),
				padded: gradient.evaluator(GradientSettings {
					spread: GradientSpread::Pad,
					..settings
				}),
				spread: settings.spread,
				stepped: settings.interpolation == GradientInterpolation::Stepped,
				reverse: *reverse,
			})
		}
		Adjustment::Vibrance { vibrance, saturation } => visitor.visit(&Vibrance {
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

			visitor.visit(&ChannelMixer {
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
				(SelectiveColorChoice::Reds, &selective_color.reds),
				(SelectiveColorChoice::Yellows, &selective_color.yellows),
				(SelectiveColorChoice::Greens, &selective_color.greens),
				(SelectiveColorChoice::Cyans, &selective_color.cyans),
				(SelectiveColorChoice::Blues, &selective_color.blues),
				(SelectiveColorChoice::Magentas, &selective_color.magentas),
				(SelectiveColorChoice::Whites, &selective_color.whites),
				(SelectiveColorChoice::Neutrals, &selective_color.neutrals),
				(SelectiveColorChoice::Blacks, &selective_color.blacks),
			]
			.map(|(color_parameter_group, inks)| {
				let [c, m, y, k] = [inks.cyan, inks.magenta, inks.yellow, inks.black].map(|percent| percent as f32);
				SelectiveColorGroup {
					color_parameter_group,
					cmyk: [c, m, y, k],
					inks: [ink(c, k), ink(m, k), ink(y, k)],
				}
			});

			visitor.visit(&SelectiveColor { mode, groups })
		}
		Adjustment::Posterize { levels } => visitor.visit(&Posterize {
			levels: (*levels).clamp(i32::MIN as i64, i32::MAX as i64) as i32,
		}),
		Adjustment::Exposure { exposure, offset, gamma_correction } => visitor.visit(&Exposure {
			exposure: *exposure as f32,
			offset: *offset as f32,
			gamma_correction: *gamma_correction as f32,
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

			visitor.visit(&ColorBalance { red, green, blue })
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

			visitor.visit(&PhotoFilter {
				factor,
				preserve_luminosity: *preserve_luminosity,
			})
		}
		Adjustment::ColorLookup { lut_file } => match parse_cached(lut_file) {
			Ok(lut) => visitor.visit(&ColorLookup { lut }),
			Err(_) => visitor.unchanged(),
		},
		Adjustment::ColorOverlay { color, blend_mode, opacity } => visitor.visit(&ColorOverlay {
			color: *color,
			blend_mode: *blend_mode,
			opacity: (opacity / 100.) as f32,
		}),
	}
}

/// Calls `map` with the CPU mapping of the kernel carrying out `adjustment`, worked out once for all the colors it maps.
pub fn with_color_map<R>(adjustment: &Adjustment, map: impl FnOnce(&dyn Fn(Color) -> Color) -> R) -> R {
	struct WithColorMap<F>(F);

	impl<R, F: FnOnce(&dyn Fn(Color) -> Color) -> R> AdjustmentVisitor for WithColorMap<F> {
		type Output = R;

		fn visit<K: MapColor>(self, kernel: &K) -> R {
			(self.0)(&|color| kernel.map_cpu(color))
		}

		fn unchanged(self) -> R {
			(self.0)(&|color| color)
		}
	}

	visit_kernel(adjustment, WithColorMap(map))
}

/// The color with `adjustment` carried out on it.
pub fn adjust_color(adjustment: &Adjustment, color: Color) -> Color {
	with_color_map(adjustment, |map| map(color))
}

/// The red, green, blue, and alpha curves of a per-channel adjustment, each sampled from its kernel at `size` evenly spaced gamma-encoded values from 0 to 1.
pub fn channel_curves(adjustment: &Adjustment, size: usize) -> Option<[Vec<f32>; 4]> {
	if !adjustment.is_per_channel() || size < 2 {
		return None;
	}

	// Each sample feeds the same value to every channel, which the curves map independently
	let channels: Vec<[f32; 4]> = with_color_map(adjustment, |map| {
		(0..size)
			.map(|index| {
				let value = index as f32 / (size - 1) as f32;
				map(Color::from_gamma_srgb_channels(value, value, value, value)).to_gamma_srgb_channels()
			})
			.collect()
	});

	Some(std::array::from_fn(|channel| channels.iter().map(|values| values[channel]).collect()))
}

/// The adjustments at the start of a content's effects, which can be carried out on its own colors ahead of the rest.
/// A fill fades the content before the effects adjust it, so with one, only adjustments that keep alpha commute with it.
pub fn leading_adjustments(operations: impl IntoIterator<Item = EffectOperation>, fill: f32) -> Vec<Adjustment> {
	operations
		.into_iter()
		.map_while(|operation| match operation {
			EffectOperation::Adjustment(adjustment) if fill == 1. || preserves_alpha(&adjustment) => Some(adjustment),
			_ => None,
		})
		.collect()
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

gpu_param_enum!(u32: DesaturateMethod);

gpu_param_enum!(u32: RelativeAbsolute);

gpu_param_enum!(u32: SelectiveColorChoice);

gpu_params! {
	#[derive(Clone)]
	struct Desaturate {
		method: DesaturateMethod,
	}
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

	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/desaturate.wgsl"))
	}
}

gpu_params! {
	#[derive(Clone)]
	struct GammaCorrection {
		exponent: f32,
	}
}

impl MapColor for GammaCorrection {
	fn map_cpu(&self, color: Color) -> Color {
		color.apply_gamma_exponent(self.exponent)
	}

	fn wgsl() -> Wgsl {
		Wgsl::body("return apply_gamma_exponent(color, exponent);")
	}
}

gpu_params! {
	#[derive(Clone)]
	struct MakeOpaque {}
}

impl MapColor for MakeOpaque {
	fn map_cpu(&self, color: Color) -> Color {
		color.with_alpha(1.)
	}

	fn wgsl() -> Wgsl {
		Wgsl::body("return vec4<f32>(color.rgb, 1.);")
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

gpu_params! {
	/// One brightness curve of the current algorithm, for a magnitude in 0..100: a line of slope 2^(b/110) up to an output of 0.5,
	/// continued by a cubic Hermite segment that eases into (1, 1).
	#[derive(Clone, Copy)]
	struct BrightnessCurve {
		slope: f32,
		knee: f32,
		end_slope: f32,
	}
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

gpu_params! {
	#[derive(Clone)]
	struct BrightnessContrast {
		use_classic: bool,
		brightness: f32,
		contrast: f32,
		classic_pivot: f32,
		first_curve: BrightnessCurve,
		second_curve: BrightnessCurve,
	}
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

	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/brightness_contrast.wgsl"))
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

gpu_params! {
	/// A record's input curve followed by its output range.
	#[derive(Clone, Copy)]
	struct LevelsStage {
		curve: LevelsCurve,
		output_minimum: f32,
		output_maximum: f32,
	}
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

gpu_params! {
	/// A channel's record followed by the composite record.
	#[derive(Clone, Copy)]
	struct LevelsChain {
		first: LevelsStage,
		second: LevelsStage,
		two_stages: bool,
	}
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

gpu_params! {
	#[derive(Clone)]
	struct Levels {
		red: LevelsChain,
		green: LevelsChain,
		blue: LevelsChain,
		alpha: LevelsStage,
	}
}

impl MapColor for Levels {
	fn map_cpu(&self, color: Color) -> Color {
		// Levels math operates in gamma space
		let [r, g, b, a] = color.to_gamma_srgb_channels();

		Color::from_gamma_srgb_channels(self.red.apply(r), self.green.apply(g), self.blue.apply(b), self.alpha.apply(a))
	}

	fn wgsl() -> Wgsl {
		Wgsl::function(concat!(include_str!("wgsl/levels_curve.wgsl"), include_str!("wgsl/levels.wgsl")))
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

gpu_params! {
	struct CurvesTable {
		curves: Lut1d,
	}
}

impl ToGpuParams for Curves {
	type Params = CurvesTable;
	fn to_params(&self) -> CurvesTable {
		const ENTRIES: usize = 4096;
		let entries = (0..ENTRIES)
			.map(|index| {
				let [r, g, b, a] = self.apply([index as f32 / (ENTRIES - 1) as f32; 4]);
				Color::from_rgbaf32_unchecked(r, g, b, a)
			})
			.collect();
		CurvesTable { curves: Lut1d(entries) }
	}
}

impl MapColor for Curves {
	fn map_cpu(&self, color: Color) -> Color {
		// Curves math operates in gamma space
		let [r, g, b, a] = self.apply(color.to_gamma_srgb_channels());
		Color::from_gamma_srgb_channels(r, g, b, a)
	}

	fn wgsl() -> Wgsl {
		Wgsl::body(
			r#"
			let channels = to_gamma_srgb_channels(color);
			return from_gamma_srgb_channels(vec4<f32>(curves(channels.r).r, curves(channels.g).g, curves(channels.b).b, curves(channels.a).a));
			"#,
		)
	}
}

gpu_params! {
	#[derive(Clone)]
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
	}

	fn wgsl() -> Wgsl {
		Wgsl::function(format!("{}\n{}", include_str!("wgsl/luma.wgsl"), include_str!("wgsl/black_and_white.wgsl")))
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

gpu_params! {
	/// One set of Hue/Saturation sliders: a hue shift in degrees and saturation and lightness amounts in -1..1.
	#[derive(Clone, Copy)]
	struct HueSaturationSettings {
		hue: f32,
		saturation: f32,
		saturation_gain: f32,
		lightness: f32,
	}
}

impl HueSaturationSettings {
	fn new(hue: f32, saturation_percent: f32, lightness_percent: f32) -> Self {
		let saturation = saturation_percent / 100.;
		Self {
			hue,
			saturation,
			saturation_gain: saturation_gain(saturation),
			lightness: lightness_percent / 100.,
		}
	}
}

gpu_params! {
	/// A hue range with its falloff: full weight from `range_start` to `range_end`, fading linearly to zero at the falloff ends.
	#[derive(Clone, Copy)]
	struct HueSaturationRangeSettings {
		falloff_start: f32,
		range_start: f32,
		range_end: f32,
		falloff_end: f32,
		settings: HueSaturationSettings,
	}
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

gpu_params! {
	#[derive(Clone, Copy)]
	struct HueSaturation {
		master: HueSaturationSettings,
		colorize: bool,
		colorize_settings: HueSaturationSettings,
		reds: HueSaturationRangeSettings,
		yellows: HueSaturationRangeSettings,
		greens: HueSaturationRangeSettings,
		cyans: HueSaturationRangeSettings,
		blues: HueSaturationRangeSettings,
		magentas: HueSaturationRangeSettings,
	}
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

	fn wgsl() -> Wgsl {
		Wgsl::function(format!("{}\n{}", include_str!("wgsl/hue.wgsl"), include_str!("wgsl/hue_saturation.wgsl")))
	}
}

gpu_params! {
	#[derive(Clone)]
	struct Invert {}
}

impl MapColor for Invert {
	fn map_cpu(&self, color: Color) -> Color {
		color.map_gamma_rgb(|channel| 1. - channel)
	}

	fn wgsl() -> Wgsl {
		Wgsl::body(
			r#"
			let channels = to_gamma_srgb_channels(color);
			return from_gamma_srgb_channels(vec4<f32>(1. - channels.rgb, channels.a));
			"#,
		)
	}
}

gpu_params! {
	#[derive(Clone)]
	struct Threshold {
		min_luminance: f32,
		max_luminance: f32,
	}
}

impl MapColor for Threshold {
	fn map_cpu(&self, color: Color) -> Color {
		// For PSD interop, we compare this 14-bit fixed-point Rec. 601 luma against the level unrounded
		let [r, g, b, _] = color.to_gamma_srgb_channels();
		let luminance = (4915. * r + 9667. * g + 1802. * b) / 16384.;

		let output = if luminance >= self.min_luminance && luminance <= self.max_luminance {
			Color::WHITE
		} else {
			Color::BLACK
		};
		output.with_alpha(color.a())
	}

	fn wgsl() -> Wgsl {
		Wgsl::body(
			r#"
			let channels = to_gamma_srgb_channels(color);
			let luminance = (4915. * channels.r + 9667. * channels.g + 1802. * channels.b) / 16384.;
			let output = select(vec3<f32>(0.), vec3<f32>(1.), luminance >= min_luminance && luminance <= max_luminance);
			return vec4<f32>(output, color.a);
			"#,
		)
	}
}

struct GradientMap {
	evaluator: vector_types::gradient::GradientEvaluator,
	/// The gradient across 0 to 1 without its spread, which the GPU applies before reading the table.
	padded: vector_types::gradient::GradientEvaluator,
	spread: vector_types::GradientSpread,
	stepped: bool,
	reverse: bool,
}

gpu_params! {
	struct GradientMapTable {
		reverse: bool,
		stepped: bool,
		spread: u32,
		gradient: Lut1d,
	}
}

impl ToGpuParams for GradientMap {
	type Params = GradientMapTable;
	fn to_params(&self) -> GradientMapTable {
		const ENTRIES: usize = 1024;
		let entries = (0..ENTRIES).map(|index| self.padded.evaluate(index as f64 / (ENTRIES - 1) as f64)).collect();
		GradientMapTable {
			reverse: self.reverse,
			stepped: self.stepped,
			spread: self.spread as u32,
			gradient: Lut1d(entries),
		}
	}
}

impl MapColor for GradientMap {
	fn map_cpu(&self, color: Color) -> Color {
		// The classic 0.3/0.59/0.11 luma of the gamma-encoded channels picks the position along the gradient
		let [r, g, b, alpha] = color.to_gamma_srgb_channels();
		let intensity = 0.3 * r + 0.59 * g + 0.11 * b;
		let intensity = if self.reverse { 1. - intensity } else { intensity };

		// The source alpha is kept and the gradient's own alpha stops are ignored
		self.evaluator.evaluate(intensity as f64).with_alpha(alpha)
	}

	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/gradient_map.wgsl"))
	}
}

gpu_params! {
	#[derive(Clone)]
	struct Vibrance {
		vibrance: f32,
		saturation_scale: f32,
	}
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

	fn wgsl() -> Wgsl {
		Wgsl::function(format!("{}\n{}", include_str!("wgsl/hue.wgsl"), include_str!("wgsl/vibrance.wgsl")))
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

gpu_params! {
	#[derive(Clone)]
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

	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/channel_mixer.wgsl"))
	}
}

gpu_params! {
	#[derive(Clone, Copy)]
	struct SelectiveColorGroup {
		color_parameter_group: SelectiveColorChoice,
		cmyk: [f32; 4],
		inks: [f32; 3],
	}
}

gpu_params! {
	#[derive(Clone)]
	struct SelectiveColor {
		mode: RelativeAbsolute,
		groups: [SelectiveColorGroup; 9],
	}
}

impl MapColor for SelectiveColor {
	fn map_cpu(&self, color: Color) -> Color {
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
		let (slope_r, slope_g, slope_b) = match self.mode {
			RelativeAbsolute::Relative => (r - 1., g - 1., b - 1.),
			RelativeAbsolute::Absolute => (-1., -1., -1.),
		};

		let mut sum = Vec3::ZERO;
		for SelectiveColorGroup {
			color_parameter_group,
			cmyk: [c, m, y, k],
			inks: [ink_c, ink_m, ink_y],
		} in self.groups
		{
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

	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/selective_color.wgsl"))
	}
}

gpu_params! {
	#[derive(Clone)]
	struct Posterize {
		levels: i32,
	}
}

impl MapColor for Posterize {
	fn map_cpu(&self, color: Color) -> Color {
		let levels = self.levels as f32;

		color.map_gamma_rgb(|c| {
			// Bins as floor(c * levels) with the outputs spread evenly to white.
			// The sliver of slack keeps an input exactly on an edge in the upper bin despite float ties.
			let bin = ((c + 2e-7) * levels).floor().min(levels - 1.);
			bin / (levels - 1.)
		})
	}

	fn wgsl() -> Wgsl {
		Wgsl::body(
			r#"
			let channels = to_gamma_srgb_channels(color);
			let level_count = f32(levels);
			let bin = min(floor((channels.rgb + 2e-7) * level_count), vec3<f32>(level_count - 1.));
			return from_gamma_srgb_channels(vec4<f32>(bin / (level_count - 1.), channels.a));
			"#,
		)
	}
}

gpu_params! {
	#[derive(Clone)]
	struct Exposure {
		exposure: f32,
		offset: f32,
		gamma_correction: f32,
	}
}

impl MapColor for Exposure {
	fn map_cpu(&self, color: Color) -> Color {
		// Linearizes with a 2.2 power above a straight toe of slope 1/32, the two meeting at this constant
		const TOE_END: f32 = 0.05568117; // 32^(-1. / 1.2)

		let decode = |value: f32| if value < TOE_END { value / 32. } else { value.powf(2.2) };
		let encode = |linear: f32| if linear < TOE_END / 32. { linear * 32. } else { linear.powf(1. / 2.2) };
		let adjust = |c: f32| {
			let linear = decode(c) * 2_f32.powf(self.exposure) + self.offset;
			encode(linear.max(0.).powf(1. / self.gamma_correction).min(1.))
		};

		let [r, g, b, a] = color.to_gamma_srgb_channels();
		Color::from_gamma_srgb_channels(adjust(r), adjust(g), adjust(b), a)
	}

	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/exposure.wgsl"))
	}
}

gpu_params! {
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

gpu_params! {
	#[derive(Clone)]
	struct ColorBalance {
		red: LevelsCurve,
		green: LevelsCurve,
		blue: LevelsCurve,
	}
}

impl MapColor for ColorBalance {
	fn map_cpu(&self, color: Color) -> Color {
		// The curves operate on gamma-space channel values
		let [r, g, b, a] = color.to_gamma_srgb_channels();
		Color::from_gamma_srgb_channels(self.red.apply(r), self.green.apply(g), self.blue.apply(b), a)
	}

	fn wgsl() -> Wgsl {
		Wgsl::function(concat!(include_str!("wgsl/levels_curve.wgsl"), include_str!("wgsl/color_balance.wgsl")))
	}
}

gpu_params! {
	#[derive(Clone)]
	struct PhotoFilter {
		factor: [f32; 3],
		preserve_luminosity: bool,
	}
}

impl MapColor for PhotoFilter {
	fn map_cpu(&self, pixel: Color) -> Color {
		let PhotoFilter { factor, preserve_luminosity } = *self;

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
	}

	fn wgsl() -> Wgsl {
		Wgsl::function(format!("{}\n{}", include_str!("wgsl/luma.wgsl"), include_str!("wgsl/photo_filter.wgsl")))
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
}

gpu_params! {
	struct ColorLookupTable {
		domain_min: [f32; 3],
		domain_max: [f32; 3],
		table: Lut3d,
	}
}

impl ToGpuParams for ColorLookup {
	type Params = ColorLookupTable;
	fn to_params(&self) -> ColorLookupTable {
		let grid = self.lut.grid();
		ColorLookupTable {
			domain_min: grid.domain_min,
			domain_max: grid.domain_max,
			table: Lut3d {
				size: grid.size as u32,
				data: grid.entries.into_iter().map(|[r, g, b]| Color::from_rgbaf32_unchecked(r, g, b, 1.)).collect(),
			},
		}
	}
}

impl MapColor for ColorLookup {
	fn map_cpu(&self, color: Color) -> Color {
		// Lookup tables address the gamma-encoded channels
		let [r, g, b, a] = color.to_gamma_srgb_channels();
		let [r, g, b] = self.lut.apply([r, g, b]);

		Color::from_gamma_srgb_channels(r, g, b, a)
	}

	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/color_lookup.wgsl"))
	}
}
