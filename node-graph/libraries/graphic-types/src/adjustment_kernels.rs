//! The CPU kernels that carry out each [`Adjustment`], precomputing what its settings need once and then mapping each color.

use crate::adjustment::*;
use crate::color_lookup_table::parse_cached;
use core_types::Color;
use core_types::blending::apply_blend_mode;
use core_types::color::{linear_to_srgb, set_luminosity, srgb_to_linear};
use core_types::transfer_curve::TransferCurveEvaluator;
use glam::Vec3;
use raster_types::parallel_rows::fill_rows;
use vector_types::GradientSettings;

/// How many colors each thread band handles at a time when a long slice is split across threads.
const COLORS_PER_ROW: usize = 4096;

impl Adjustment {
	/// Adjusts each color in place, on threads for long slices on native platforms. O(n).
	pub fn apply(&self, colors: &mut [Color]) {
		match self {
			Self::Desaturate { method } => map_colors(colors, |color| desaturate(color, *method)),
			Self::GammaCorrection { gamma, inverse } => {
				let exponent = if *inverse { 1. / gamma } else { *gamma };
				map_colors(colors, |color| color.apply_gamma_exponent(exponent as f32));
			}
			Self::MakeOpaque => map_colors(colors, |color| color.with_alpha(1.)),
			Self::BrightnessContrast {
				brightness,
				contrast,
				use_classic,
				classic_pivot,
			} => {
				let kernel = BrightnessContrastKernel::new(*brightness, *contrast, *use_classic, *classic_pivot);
				map_colors(colors, |color| kernel.apply(color));
			}
			Self::Levels(settings) => {
				let kernel = LevelsKernel::new(settings);
				map_colors(colors, |color| kernel.apply(color));
			}
			Self::Curves(settings) => {
				let kernel = CurvesKernel::new(settings);
				map_colors(colors, |color| kernel.apply(color));
			}
			Self::BlackAndWhite(settings) => {
				let kernel = BlackAndWhiteKernel::new(settings);
				map_colors(colors, |color| kernel.apply(color));
			}
			Self::HueSaturation(settings) => {
				let kernel = HueSaturationKernel::new(settings);
				map_colors(colors, |color| kernel.apply(color));
			}
			Self::Invert => map_colors(colors, |color| color.map_gamma_rgb(|channel| 1. - channel)),
			Self::Threshold { min_luminance, max_luminance } => {
				let min_luminance = (min_luminance / 100.) as f32;
				let max_luminance = (max_luminance / 100.) as f32;

				map_colors(colors, |color| {
					// For PSD interop, we compare this 14-bit fixed-point Rec. 601 luma against the level unrounded
					let [r, g, b, _] = color.to_gamma_srgb_channels();
					let luminance = (4915. * r + 9667. * g + 1802. * b) / 16384.;

					let output = if luminance >= min_luminance && luminance <= max_luminance { Color::WHITE } else { Color::BLACK };
					output.with_alpha(color.a())
				});
			}
			Self::GradientMap { gradient, reverse } => {
				let evaluator = gradient.element().evaluator(GradientSettings::from_item_attributes(gradient));

				map_colors(colors, |color| {
					// The classic 0.3/0.59/0.11 luma of the gamma-encoded channels picks the position along the gradient
					let [r, g, b, alpha] = color.to_gamma_srgb_channels();
					let intensity = 0.3 * r + 0.59 * g + 0.11 * b;
					let intensity = if *reverse { 1. - intensity } else { intensity };

					// The source alpha is kept and the gradient's own alpha stops are ignored
					evaluator.evaluate(intensity as f64).with_alpha(alpha)
				});
			}
			Self::Vibrance { vibrance, saturation } => {
				let vibrance = (vibrance / 100.) as f32;
				let saturation_scale = (1. + saturation / 100.) as f32;
				map_colors(colors, |color| apply_vibrance(color, vibrance, saturation_scale));
			}
			Self::ChannelMixer(settings) => {
				let kernel = ChannelMixerKernel::new(settings);
				map_colors(colors, |color| kernel.apply(color));
			}
			Self::SelectiveColor(settings) => {
				let kernel = SelectiveColorKernel::new(settings);
				map_colors(colors, |color| kernel.apply(color));
			}
			Self::Posterize { levels } => {
				let levels = *levels as f32;

				map_colors(colors, |color| {
					color.map_gamma_rgb(|c| {
						// Bins as floor(c * levels) with the outputs spread evenly to white.
						// The sliver of slack keeps an input exactly on an edge in the upper bin despite float ties.
						let bin = ((c + 2e-7) * levels).floor().min(levels - 1.);
						bin / (levels - 1.)
					})
				});
			}
			Self::Exposure { exposure, offset, gamma_correction } => {
				let kernel = ExposureKernel {
					exposure: *exposure as f32,
					offset: *offset as f32,
					gamma_correction: *gamma_correction as f32,
				};
				map_colors(colors, |color| kernel.apply(color));
			}
			Self::ColorBalance(settings) => {
				let kernel = ColorBalanceKernel::new(settings);
				map_colors(colors, |color| kernel.apply(color));
			}
			Self::PhotoFilter { color, density, preserve_luminosity } => {
				let kernel = PhotoFilterKernel::new(*color, *density, *preserve_luminosity);
				map_colors(colors, |pixel| kernel.apply(pixel));
			}
			Self::ColorLookup { lut_file } => {
				// A file that isn't a lookup table leaves the colors as they are
				let Ok(lut) = parse_cached(lut_file) else { return };

				map_colors(colors, |color| {
					// Lookup tables address the gamma-encoded channels
					let [r, g, b, a] = color.to_gamma_srgb_channels();
					let [r, g, b] = lut.apply([r, g, b]);

					Color::from_gamma_srgb_channels(r, g, b, a)
				});
			}
			Self::ColorOverlay { color, blend_mode, opacity } => {
				let opacity = (opacity / 100.) as f32;

				map_colors(colors, |pixel| {
					let overlay = apply_blend_mode(*color, pixel, *blend_mode);
					let mix = |image: f32, overlay: f32| image + (overlay - image) * opacity;

					Color::from_rgbaf32_unchecked(mix(pixel.r(), overlay.r()), mix(pixel.g(), overlay.g()), mix(pixel.b(), overlay.b()), pixel.a())
				});
			}
		}
	}

	/// The adjustment of a single color.
	pub fn apply_to_color(&self, color: Color) -> Color {
		let mut colors = [color];
		self.apply(&mut colors);
		colors[0]
	}
}

/// Replaces each color with the kernel's result for it.
fn map_colors(colors: &mut [Color], kernel: impl Fn(Color) -> Color + Sync) {
	fill_rows(colors, COLORS_PER_ROW, |_, row| row.iter_mut().for_each(|color| *color = kernel(*color)));
}

fn desaturate(color: Color, method: DesaturateMethod) -> Color {
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
}

// ===================
// Brightness/Contrast
// ===================

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

struct BrightnessContrastKernel {
	use_classic: bool,
	classic_pivot: f32,
	brightness: f32,
	contrast: f32,
	first_curve: BrightnessCurve,
	second_curve: BrightnessCurve,
}

impl BrightnessContrastKernel {
	fn new(brightness: f64, contrast: f64, use_classic: bool, classic_pivot: f64) -> Self {
		let classic_pivot = (classic_pivot / 255.) as f32;

		// Each algorithm has its own slider range, matched here and in the Inspector
		let (brightness_extent, contrast_minimum) = if use_classic { (100., -100.) } else { (150., -50.) };
		let brightness = (brightness as f32).clamp(-brightness_extent, brightness_extent);
		let contrast = (contrast as f32).clamp(contrast_minimum, 100.) / 100.;

		// Beyond a magnitude of 100, the curve for 100 is applied first and the curve for the remainder after it
		let magnitude = brightness.abs();
		let first_curve = BrightnessCurve::new(magnitude.min(100.));
		let second_curve = BrightnessCurve::new((magnitude - 100.).max(0.));

		Self {
			use_classic,
			classic_pivot,
			brightness,
			contrast,
			first_curve,
			second_curve,
		}
	}

	fn apply(&self, color: Color) -> Color {
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
}

// ======
// Levels
// ======

/// A record's input curve followed by its output range.
#[derive(Clone, Copy)]
struct LevelsStage {
	curve: LevelsCurve,
	output_minimum: f32,
	output_maximum: f32,
}

impl LevelsRecord {
	fn stage(&self, gamma: f32) -> LevelsStage {
		LevelsStage {
			curve: LevelsCurve::from_points(self.shadows as f32 * 2.55, self.highlights as f32 * 2.55, gamma),
			output_minimum: self.output_minimums as f32 / 100.,
			output_maximum: self.output_maximums as f32 / 100.,
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
		let at = |value: f64, default: f32| value as f32 == default;
		let nothing_between = at(composite.shadows, 0.) && at(composite.highlights, 100.) && at(channel.output_minimums, 0.) && at(channel.output_maximums, 100.);
		if nothing_between {
			let merged = LevelsRecord {
				output_minimums: composite.output_minimums,
				output_maximums: composite.output_maximums,
				..channel
			};
			let stage = merged.stage(channel.midtones as f32 * composite.midtones as f32);
			Self {
				first: stage,
				second: stage,
				two_stages: false,
			}
		} else {
			Self {
				first: channel.stage(channel.midtones as f32),
				second: composite.stage(composite.midtones as f32),
				two_stages: true,
			}
		}
	}

	fn apply(&self, value: f32) -> f32 {
		let value = self.first.apply(value);
		if self.two_stages { self.second.apply(value) } else { value }
	}
}

struct LevelsKernel {
	red: LevelsChain,
	green: LevelsChain,
	blue: LevelsChain,
	alpha: LevelsStage,
}

impl LevelsKernel {
	fn new(settings: &LevelsSettings) -> Self {
		Self {
			red: LevelsChain::new(settings.red, settings.composite),
			green: LevelsChain::new(settings.green, settings.composite),
			blue: LevelsChain::new(settings.blue, settings.composite),
			// Alpha stands apart from the composite record that the three color channels pass through
			alpha: settings.alpha.stage(settings.alpha.midtones as f32),
		}
	}

	fn apply(&self, color: Color) -> Color {
		// Levels math operates in gamma space
		let [r, g, b, a] = color.to_gamma_srgb_channels();

		Color::from_gamma_srgb_channels(self.red.apply(r), self.green.apply(g), self.blue.apply(b), self.alpha.apply(a))
	}
}

// ======
// Curves
// ======

struct CurvesKernel {
	composite: TransferCurveEvaluator,
	red: TransferCurveEvaluator,
	green: TransferCurveEvaluator,
	blue: TransferCurveEvaluator,
	alpha: TransferCurveEvaluator,
}

impl CurvesKernel {
	fn new(settings: &CurvesSettings) -> Self {
		Self {
			composite: settings.composite.evaluator(),
			red: settings.red.evaluator(),
			green: settings.green.evaluator(),
			blue: settings.blue.evaluator(),
			alpha: settings.alpha.evaluator(),
		}
	}

	fn map(&self, channel: &TransferCurveEvaluator, value: f32) -> f32 {
		self.composite.evaluate(channel.evaluate(value as f64).clamp(0., 1.)).clamp(0., 1.) as f32
	}

	fn apply(&self, color: Color) -> Color {
		// Curves math operates in gamma space
		let [r, g, b, a] = color.to_gamma_srgb_channels();

		// Alpha stands apart from the composite curve that the three color channels pass through
		let a = self.alpha.evaluate(a as f64).clamp(0., 1.) as f32;

		Color::from_gamma_srgb_channels(self.map(&self.red, r), self.map(&self.green, g), self.map(&self.blue, b), a)
	}
}

// =============
// Black & White
// =============

struct BlackAndWhiteKernel {
	tint: Option<Color>,
	reds: f32,
	yellows: f32,
	greens: f32,
	cyans: f32,
	blues: f32,
	magentas: f32,
}

impl BlackAndWhiteKernel {
	fn new(settings: &BlackAndWhiteSettings) -> Self {
		let weight = |percent: f64| (percent / 100.) as f32;

		Self {
			tint: settings.use_tint.then_some(settings.tint),
			reds: weight(settings.reds),
			yellows: weight(settings.yellows),
			greens: weight(settings.greens),
			cyans: weight(settings.cyans),
			blues: weight(settings.blues),
			magentas: weight(settings.magentas),
		}
	}

	fn apply(&self, color: Color) -> Color {
		// Black & White channel weights are tuned for gamma-space values
		let [r, g, b, alpha_part] = color.to_gamma_srgb_channels();

		let gray_base = r.min(g).min(b);

		let red_part = r - gray_base;
		let green_part = g - gray_base;
		let blue_part = b - gray_base;

		let additional = if red_part == 0. {
			let cyan_part = green_part.min(blue_part);
			cyan_part * self.cyans + (green_part - cyan_part) * self.greens + (blue_part - cyan_part) * self.blues
		} else if green_part == 0. {
			let magenta_part = red_part.min(blue_part);
			magenta_part * self.magentas + (red_part - magenta_part) * self.reds + (blue_part - magenta_part) * self.blues
		} else {
			let yellow_part = red_part.min(green_part);
			yellow_part * self.yellows + (red_part - yellow_part) * self.reds + (green_part - yellow_part) * self.greens
		};

		let luminance = (gray_base + additional).clamp(0., 1.);
		let Some(tint) = self.tint else {
			return Color::from_gamma_srgb_channels(luminance, luminance, luminance, alpha_part);
		};

		// The tint takes on the gray's luminosity the way the Luminosity blend mode would
		let [tint_r, tint_g, tint_b, _] = tint.to_gamma_srgb_channels();
		let tint_luma = luma_rec_601_fixed_point(tint_r, tint_g, tint_b);
		let [tinted_r, tinted_g, tinted_b] = set_luminosity(tint_r, tint_g, tint_b, tint_luma, luminance);

		Color::from_gamma_srgb_channels(tinted_r, tinted_g, tinted_b, alpha_part)
	}
}

// ==============
// Hue/Saturation
// ==============

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
struct HueSaturationAmounts {
	hue: f32,
	saturation: f32,
	lightness: f32,
}

impl From<HueSaturationSliders> for HueSaturationAmounts {
	fn from(sliders: HueSaturationSliders) -> Self {
		Self {
			hue: sliders.hue as f32,
			saturation: sliders.saturation as f32 / 100.,
			lightness: sliders.lightness as f32 / 100.,
		}
	}
}

/// A hue range with its falloff: full weight from `range_start` to `range_end`, fading linearly to zero at the falloff ends.
#[derive(Clone, Copy)]
struct HueRangeKernel {
	falloff_start: f32,
	range_start: f32,
	range_end: f32,
	falloff_end: f32,
	amounts: HueSaturationAmounts,
}

impl From<HueRangeSettings> for HueRangeKernel {
	fn from(range: HueRangeSettings) -> Self {
		// For PSD interop, each edge rounds to 1536 hue units per turn over 359 rather than 360 degrees, landing up to a degree late
		let edge = |degrees: f64| (degrees as f32 * 1536. / 359.).round() * 360. / 1536.;

		Self {
			falloff_start: edge(range.falloff_start),
			range_start: edge(range.range_start),
			range_end: edge(range.range_end),
			falloff_end: edge(range.falloff_end),
			amounts: range.sliders.into(),
		}
	}
}

impl HueRangeKernel {
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
	fn apply(&mut self, range: &HueRangeKernel, original_hue: f32, original_saturation: f32) {
		// Range weights come from the original hue, and grays belong to no range
		let weight = if original_saturation > 0. { range.weight(original_hue) } else { 0. };
		if weight <= 0. {
			return;
		}

		self.hue_shift += range.amounts.hue * weight;
		// For PSD interop, +100 saturates fully from the very edge of the falloff rather than scaling with the weight
		if range.amounts.saturation >= 1. {
			self.fully_saturate = true;
		} else {
			self.saturation_factor *= 1. + (saturation_gain(range.amounts.saturation) - 1.) * weight;
		}
		self.rgb = lightness_toward_max_or_min(self.rgb, range.amounts.lightness * weight);
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

struct HueSaturationKernel {
	master: HueSaturationAmounts,
	colorize: Option<HueSaturationAmounts>,
	ranges: [HueRangeKernel; 6],
}

impl HueSaturationKernel {
	fn new(settings: &HueSaturationSettings) -> Self {
		Self {
			master: settings.master.into(),
			colorize: settings.colorize.then(|| settings.colorize_sliders.into()),
			ranges: [settings.reds, settings.yellows, settings.greens, settings.cyans, settings.blues, settings.magentas].map(Into::into),
		}
	}

	fn apply(&self, color: Color) -> Color {
		let [r, g, b, alpha] = color.to_gamma_srgb_channels();

		if let Some(colorize) = self.colorize {
			let [_, _, lightness] = gamma_rgb_to_hsl(r, g, b);
			let lightness = lightness_toward_white_or_black(lightness, colorize.lightness);
			let [r, g, b] = hsl_to_gamma_rgb(colorize.hue, colorize.saturation, lightness);
			return Color::from_gamma_srgb_channels(r, g, b, alpha);
		}

		// Each range weights its sliders by its falloff around the original hue: hue shifts add and saturation gains multiply
		let [original_hue, original_saturation, _] = gamma_rgb_to_hsl(r, g, b);
		let mut effect = HueSaturationRangeEffect {
			hue_shift: self.master.hue,
			saturation_factor: 1.,
			fully_saturate: false,
			rgb: [r, g, b],
		};
		for range in &self.ranges {
			effect.apply(range, original_hue, original_saturation);
		}
		let HueSaturationRangeEffect {
			hue_shift,
			mut saturation_factor,
			mut fully_saturate,
			rgb,
		} = effect;
		if self.master.saturation >= 1. {
			fully_saturate = true;
		} else {
			saturation_factor *= saturation_gain(self.master.saturation);
		}

		// The master lightness blends toward white or black before the hue and saturation, which work in HSL of the gamma channels
		let rgb = rgb.map(|channel| lightness_toward_white_or_black(channel, self.master.lightness));
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
}

// ========
// Vibrance
// ========

/// Vibrance then saturation, both in linear light, which equals applying each alone in turn.
fn apply_vibrance(color: Color, vibrance: f32, saturation_scale: f32) -> Color {
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

// =============
// Channel Mixer
// =============

/// A row's weights and constant in 10-bit fixed point truncated toward zero, which PSD interop depends on.
#[derive(Clone, Copy)]
struct ChannelMixerWeights([f32; 4]);

impl From<ChannelMixerRow> for ChannelMixerWeights {
	fn from(row: ChannelMixerRow) -> Self {
		let weight = |percent: f64| (percent as f32 * 1024. / 100.).trunc() / 1024.;
		Self([weight(row.red), weight(row.green), weight(row.blue), weight(row.constant)])
	}
}

impl ChannelMixerWeights {
	fn mix(&self, r: f32, g: f32, b: f32) -> f32 {
		let [red, green, blue, constant] = self.0;
		(r * red + g * green + b * blue + constant).clamp(0., 1.)
	}
}

struct ChannelMixerKernel {
	monochrome: Option<ChannelMixerWeights>,
	red: ChannelMixerWeights,
	green: ChannelMixerWeights,
	blue: ChannelMixerWeights,
}

impl ChannelMixerKernel {
	fn new(settings: &ChannelMixerSettings) -> Self {
		Self {
			monochrome: settings.monochrome.then(|| settings.monochrome_mix.into()),
			red: settings.red.into(),
			green: settings.green.into(),
			blue: settings.blue.into(),
		}
	}

	fn apply(&self, color: Color) -> Color {
		let [r, g, b, a] = color.to_gamma_srgb_channels();

		let (out_r, out_g, out_b) = match &self.monochrome {
			Some(monochrome) => {
				let gray = monochrome.mix(r, g, b);
				(gray, gray, gray)
			}
			None => (self.red.mix(r, g, b), self.green.mix(r, g, b), self.blue.mix(r, g, b)),
		};

		Color::from_gamma_srgb_channels(out_r, out_g, out_b, a)
	}
}

// ===============
// Selective Color
// ===============

/// The groups of colors that Selective Color adjusts, in the order of its settings.
#[derive(Clone, Copy)]
enum SelectiveColorGroup {
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

struct SelectiveColorKernel {
	mode: RelativeAbsolute,
	/// Each group with its cyan, magenta, yellow, and black offsets, leaving out the groups left at zero.
	groups: Vec<(SelectiveColorGroup, [f32; 4])>,
}

impl SelectiveColorKernel {
	fn new(settings: &SelectiveColorSettings) -> Self {
		let groups = [
			(SelectiveColorGroup::Reds, settings.reds),
			(SelectiveColorGroup::Yellows, settings.yellows),
			(SelectiveColorGroup::Greens, settings.greens),
			(SelectiveColorGroup::Cyans, settings.cyans),
			(SelectiveColorGroup::Blues, settings.blues),
			(SelectiveColorGroup::Magentas, settings.magentas),
			(SelectiveColorGroup::Whites, settings.whites),
			(SelectiveColorGroup::Neutrals, settings.neutrals),
			(SelectiveColorGroup::Blacks, settings.blacks),
		];
		let groups = groups
			.into_iter()
			.map(|(group, inks)| (group, [inks.cyan as f32, inks.magenta as f32, inks.yellow as f32, inks.black as f32]))
			.filter(|(_, inks)| inks.iter().any(|&ink| ink != 0.))
			.collect();

		Self { mode: settings.mode, groups }
	}

	fn apply(&self, color: Color) -> Color {
		let [r, g, b, a] = color.to_gamma_srgb_channels();

		let min = |a: f32, b: f32, c: f32| a.min(b).min(c);
		let max = |a: f32, b: f32, c: f32| a.max(b).max(c);
		let med = |a: f32, b: f32, c: f32| a + b + c - min(a, b, c) - max(a, b, c);

		let max_channel = max(r, g, b);
		let min_channel = min(r, g, b);

		let pixel_color_range = |group| match group {
			SelectiveColorGroup::Reds => max_channel == r,
			SelectiveColorGroup::Yellows => min_channel == b,
			SelectiveColorGroup::Greens => max_channel == g,
			SelectiveColorGroup::Cyans => min_channel == r,
			SelectiveColorGroup::Blues => max_channel == b,
			SelectiveColorGroup::Magentas => min_channel == g,
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
		for &(group, [c, m, y, k]) in &self.groups {
			// Skip this group if this pixel's color isn't in the range it affects
			if !pixel_color_range(group) {
				continue;
			}

			let color_parameter_group_scale_factor = match group {
				SelectiveColorGroup::Reds | SelectiveColorGroup::Greens | SelectiveColorGroup::Blues => color_parameter_group_scale_factor_rgb,
				SelectiveColorGroup::Cyans | SelectiveColorGroup::Magentas | SelectiveColorGroup::Yellows => color_parameter_group_scale_factor_cmy,
				SelectiveColorGroup::Whites => min(r, g, b) * 2. - 1.,
				SelectiveColorGroup::Neutrals => 1. - ((max(r, g, b) - 0.5).abs() + (min(r, g, b) - 0.5).abs()),
				SelectiveColorGroup::Blacks => 1. - max(r, g, b) * 2.,
			};

			// For PSD interop, the combined percent (c + k + c k / 100) rounds half up to an integer
			let ink = |color: f32| {
				let percent = ((2. * (100. * (color + k) + color * k) + 100.) / 200.).floor();
				match self.mode {
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
	}
}

// ========
// Exposure
// ========

struct ExposureKernel {
	exposure: f32,
	offset: f32,
	gamma_correction: f32,
}

impl ExposureKernel {
	fn adjust_channel(&self, c: f32) -> f32 {
		// Linearizes with a 2.2 power above a straight toe of slope 1/32, the two meeting at this constant
		const TOE_END: f32 = 0.05568117; // 32^(-1. / 1.2)

		let decode = |value: f32| if value < TOE_END { value / 32. } else { value.powf(2.2) };
		let encode = |linear: f32| if linear < TOE_END / 32. { linear * 32. } else { linear.powf(1. / 2.2) };

		let linear = decode(c) * 2_f32.powf(self.exposure) + self.offset;
		encode(linear.max(0.).powf(1. / self.gamma_correction).min(1.))
	}

	fn apply(&self, color: Color) -> Color {
		let [r, g, b, a] = color.to_gamma_srgb_channels();
		Color::from_gamma_srgb_channels(self.adjust_channel(r), self.adjust_channel(g), self.adjust_channel(b), a)
	}
}

// =============
// Color Balance
// =============

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

struct ColorBalanceKernel {
	red: LevelsCurve,
	green: LevelsCurve,
	blue: LevelsCurve,
}

impl ColorBalanceKernel {
	fn new(settings: &ColorBalanceSettings) -> Self {
		// The derivation below is integer arithmetic, so the sliders round to whole percentages first
		let sliders = |shifts: ColorBalanceShifts| [shifts.cyan_red, shifts.magenta_green, shifts.yellow_blue].map(|value| (value as f32).round() as i32);
		let [s_r, s_g, s_b] = sliders(settings.shadows);
		let [m_r, m_g, m_b] = sliders(settings.midtones);
		let [h_r, h_g, h_b] = sliders(settings.highlights);

		let s_max = s_r.max(s_g).max(s_b);
		let m_max = m_r.max(m_g).max(m_b);
		let m_min = m_r.min(m_g).min(m_b);
		let h_min = h_r.min(h_g).min(h_b);
		let preserve_luminosity = settings.preserve_luminosity;

		Self {
			red: color_balance_curve(s_r, m_r, h_r, s_max, m_max, m_min, h_min, preserve_luminosity),
			green: color_balance_curve(s_g, m_g, h_g, s_max, m_max, m_min, h_min, preserve_luminosity),
			blue: color_balance_curve(s_b, m_b, h_b, s_max, m_max, m_min, h_min, preserve_luminosity),
		}
	}

	fn apply(&self, color: Color) -> Color {
		// The curves operate on gamma-space channel values
		let [r, g, b, a] = color.to_gamma_srgb_channels();
		Color::from_gamma_srgb_channels(self.red.apply(r), self.green.apply(g), self.blue.apply(b), a)
	}
}

// ============
// Photo Filter
// ============

struct PhotoFilterKernel {
	factor: [f32; 3],
	preserve_luminosity: bool,
}

impl PhotoFilterKernel {
	fn new(color: Color, density: f64, preserve_luminosity: bool) -> Self {
		let density = (density / 100.) as f32;

		// The image is multiplied in XYZ by the filter color normalized to the white point, with density easing that multiplier toward 1
		let filter_xyz = multiply_matrix(&SRGB_TO_XYZ_D50, [color.r(), color.g(), color.b()]);
		let factor = [
			1. + density * (filter_xyz[0] / WHITE_XYZ_D50[0] - 1.),
			1. + density * (filter_xyz[1] / WHITE_XYZ_D50[1] - 1.),
			1. + density * (filter_xyz[2] / WHITE_XYZ_D50[2] - 1.),
		];

		Self { factor, preserve_luminosity }
	}

	fn apply(&self, pixel: Color) -> Color {
		let [r_in, g_in, b_in, alpha] = pixel.to_gamma_srgb_channels();
		let xyz = multiply_matrix(&SRGB_TO_XYZ_D50, [srgb_to_linear(r_in), srgb_to_linear(g_in), srgb_to_linear(b_in)]);
		let filtered = multiply_matrix(&XYZ_D50_TO_SRGB, [xyz[0] * self.factor[0], xyz[1] * self.factor[1], xyz[2] * self.factor[2]]);
		let mut r = linear_to_srgb(filtered[0].clamp(0., 1.));
		let mut g = linear_to_srgb(filtered[1].clamp(0., 1.));
		let mut b = linear_to_srgb(filtered[2].clamp(0., 1.));

		if self.preserve_luminosity {
			let luma_in = luma_rec_601_fixed_point(r_in.clamp(0., 1.), g_in.clamp(0., 1.), b_in.clamp(0., 1.));
			[r, g, b] = set_luminosity(r, g, b, luma_rec_601_fixed_point(r, g, b), luma_in);
		}

		Color::from_gamma_srgb_channels(r, g, b, alpha)
	}
}

// sRGB colorants adapted to D50 as in the sRGB IEC61966-2.1 ICC profile, row major, and their inverse
pub const SRGB_TO_XYZ_D50: [[f32; 3]; 3] = [[0.43607, 0.38515, 0.14307], [0.22249, 0.71687, 0.06061], [0.01392, 0.09708, 0.71410]];
pub const XYZ_D50_TO_SRGB: [[f32; 3]; 3] = [[3.134096, -1.6174, -0.490638], [-0.978793, 1.916295, 0.033454], [0.071971, -0.228987, 1.40538]];
pub const WHITE_XYZ_D50: [f32; 3] = [0.96420, 1., 0.82491];

pub fn multiply_matrix(matrix: &[[f32; 3]; 3], vector: [f32; 3]) -> [f32; 3] {
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
