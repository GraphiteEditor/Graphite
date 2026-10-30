//! Color adjustments as plain data holding their nodes' settings in the nodes' own units, decoded from effect rows, carried out by the kernels of the `raster-kernels` crate.

use crate::effects::EffectKind;
use crate::parameters::ParameterVisitor;
use core_types::Color;
use core_types::blending::BlendMode;
use core_types::color::SRGBA8;
use core_types::graphene_hash::CacheHash;
use core_types::list::Item;
use core_types::transfer_curve::TransferCurve;
use graphene_resource::Resource;
use num_enum::{FromPrimitive, IntoPrimitive};
use vector_types::Gradient;

/// A color adjustment applied to each color on its own, with no regard for its neighbors.
#[derive(Clone, Debug, PartialEq, CacheHash)]
pub enum Adjustment {
	Desaturate {
		method: DesaturateMethod,
	},
	GammaCorrection {
		gamma: f64,
		inverse: bool,
	},
	MakeOpaque,
	BrightnessContrast {
		/// Percent.
		brightness: f64,
		/// Percent.
		contrast: f64,
		use_classic: bool,
		/// On a scale of 0 to 255.
		classic_pivot: f64,
	},
	Levels(Box<LevelsSettings>),
	Curves(Box<CurvesSettings>),
	BlackAndWhite(BlackAndWhiteSettings),
	HueSaturation(Box<HueSaturationSettings>),
	Invert,
	Threshold {
		/// Percent.
		min_luminance: f64,
		/// Percent.
		max_luminance: f64,
	},
	GradientMap {
		/// The gradient, with its whole-ramp settings as its attributes.
		gradient: Item<Gradient>,
		reverse: bool,
	},
	Vibrance {
		/// Percent.
		vibrance: f64,
		/// Percent.
		saturation: f64,
	},
	ChannelMixer(Box<ChannelMixerSettings>),
	SelectiveColor(Box<SelectiveColorSettings>),
	Posterize {
		levels: i64,
	},
	Exposure {
		exposure: f64,
		offset: f64,
		gamma_correction: f64,
	},
	ColorBalance(ColorBalanceSettings),
	PhotoFilter {
		color: Color,
		/// Percent.
		density: f64,
		preserve_luminosity: bool,
	},
	ColorLookup {
		/// The file the lookup table is read from.
		lut_file: Resource,
	},
	ColorOverlay {
		color: Color,
		blend_mode: BlendMode,
		/// Percent.
		opacity: f64,
	},
}

impl Adjustment {
	/// Whether each channel of the result depends only on the same channel of the color, making the adjustment four curves.
	pub fn is_per_channel(&self) -> bool {
		matches!(
			self,
			Self::GammaCorrection { .. }
				| Self::MakeOpaque
				| Self::BrightnessContrast { .. }
				| Self::Levels(_)
				| Self::Curves(_)
				| Self::Invert
				| Self::Posterize { .. }
				| Self::Exposure { .. }
				| Self::ColorBalance(_)
		)
	}

	pub fn kind(&self) -> EffectKind {
		match self {
			Self::Desaturate { .. } => EffectKind::Desaturate,
			Self::GammaCorrection { .. } => EffectKind::GammaCorrection,
			Self::MakeOpaque => EffectKind::MakeOpaque,
			Self::BrightnessContrast { .. } => EffectKind::BrightnessContrast,
			Self::Levels(_) => EffectKind::Levels,
			Self::Curves(_) => EffectKind::Curves,
			Self::BlackAndWhite(_) => EffectKind::BlackAndWhite,
			Self::HueSaturation(_) => EffectKind::HueSaturation,
			Self::Invert => EffectKind::Invert,
			Self::Threshold { .. } => EffectKind::Threshold,
			Self::GradientMap { .. } => EffectKind::GradientMap,
			Self::Vibrance { .. } => EffectKind::Vibrance,
			Self::ChannelMixer(_) => EffectKind::ChannelMixer,
			Self::SelectiveColor(_) => EffectKind::SelectiveColor,
			Self::Posterize { .. } => EffectKind::Posterize,
			Self::Exposure { .. } => EffectKind::Exposure,
			Self::ColorBalance(_) => EffectKind::ColorBalance,
			Self::PhotoFilter { .. } => EffectKind::PhotoFilter,
			Self::ColorLookup { .. } => EffectKind::ColorLookup,
			Self::ColorOverlay { .. } => EffectKind::ColorOverlay,
		}
	}

	/// Visits each parameter with the attribute it rides, named after the node's parameter, and its implicit default, which is the node's.
	pub(crate) fn visit_parameters(&mut self, visitor: &mut impl ParameterVisitor) {
		match self {
			Self::Desaturate { method } => visitor.parameter("method", method, DesaturateMethod::default()),
			Self::GammaCorrection { gamma, inverse } => {
				visitor.parameter("gamma", gamma, 2.2);
				visitor.parameter("inverse", inverse, false);
			}
			Self::MakeOpaque | Self::Invert => {}
			Self::BrightnessContrast {
				brightness,
				contrast,
				use_classic,
				classic_pivot,
			} => {
				visitor.parameter("brightness", brightness, 0.);
				visitor.parameter("contrast", contrast, 0.);
				visitor.parameter("use_classic", use_classic, false);
				visitor.parameter("classic_pivot", classic_pivot, 127.);
			}
			Self::Levels(settings) => {
				let LevelsSettings { composite, red, green, blue, alpha } = settings.as_mut();
				composite.visit_parameters(visitor, "");
				red.visit_parameters(visitor, "red");
				green.visit_parameters(visitor, "green");
				blue.visit_parameters(visitor, "blue");
				alpha.visit_parameters(visitor, "alpha");
			}
			Self::Curves(settings) => {
				let CurvesSettings { composite, red, green, blue, alpha } = settings.as_mut();
				visitor.parameter("curve", composite, TransferCurve::default());
				visitor.parameter("red_curve", red, TransferCurve::default());
				visitor.parameter("green_curve", green, TransferCurve::default());
				visitor.parameter("blue_curve", blue, TransferCurve::default());
				visitor.parameter("alpha_curve", alpha, TransferCurve::default());
			}
			Self::BlackAndWhite(settings) => {
				visitor.parameter("use_tint", &mut settings.use_tint, false);
				visitor.parameter("tint", &mut settings.tint, Color::from(SRGBA8::new(0xe1, 0xd3, 0xb3, 0xff)));
				visitor.parameter("reds", &mut settings.reds, 40.);
				visitor.parameter("yellows", &mut settings.yellows, 60.);
				visitor.parameter("greens", &mut settings.greens, 40.);
				visitor.parameter("cyans", &mut settings.cyans, 60.);
				visitor.parameter("blues", &mut settings.blues, 20.);
				visitor.parameter("magentas", &mut settings.magentas, 80.);
			}
			Self::HueSaturation(settings) => {
				let HueSaturationSettings {
					master,
					colorize,
					colorize_sliders,
					reds,
					yellows,
					greens,
					cyans,
					blues,
					magentas,
				} = settings.as_mut();

				master.visit_parameters(visitor, "", [0., 0., 0.]);
				visitor.parameter("colorize", colorize, false);
				colorize_sliders.visit_parameters(visitor, "colorize", [24., 25., 0.]);

				reds.visit_parameters(visitor, "reds", [315., 345., 15., 45.]);
				yellows.visit_parameters(visitor, "yellows", [15., 45., 75., 105.]);
				greens.visit_parameters(visitor, "greens", [75., 105., 135., 165.]);
				cyans.visit_parameters(visitor, "cyans", [135., 165., 195., 225.]);
				blues.visit_parameters(visitor, "blues", [195., 225., 255., 285.]);
				magentas.visit_parameters(visitor, "magentas", [255., 285., 315., 345.]);
			}
			Self::Threshold { min_luminance, max_luminance } => {
				visitor.parameter("min_luminance", min_luminance, 50.);
				visitor.parameter("max_luminance", max_luminance, 100.);
			}
			Self::GradientMap { gradient, reverse } => {
				visitor.parameter("gradient", gradient, Item::new_from_element(Gradient::from(vec![Color::BLACK, Color::WHITE])));
				visitor.parameter("reverse", reverse, false);
			}
			Self::Vibrance { vibrance, saturation } => {
				visitor.parameter("vibrance", vibrance, 0.);
				visitor.parameter("saturation", saturation, 0.);
			}
			Self::ChannelMixer(settings) => {
				let ChannelMixerSettings {
					monochrome,
					monochrome_mix,
					red,
					green,
					blue,
				} = settings.as_mut();

				visitor.parameter("monochrome", monochrome, false);
				monochrome_mix.visit_parameters(visitor, "monochrome", [40., 40., 20., 0.]);
				red.visit_parameters(visitor, "red", [100., 0., 0., 0.]);
				green.visit_parameters(visitor, "green", [0., 100., 0., 0.]);
				blue.visit_parameters(visitor, "blue", [0., 0., 100., 0.]);
			}
			Self::SelectiveColor(settings) => {
				let SelectiveColorSettings {
					mode,
					reds,
					yellows,
					greens,
					cyans,
					blues,
					magentas,
					whites,
					neutrals,
					blacks,
				} = settings.as_mut();

				visitor.parameter("mode", mode, RelativeAbsolute::default());
				let groups = [
					("reds", reds),
					("yellows", yellows),
					("greens", greens),
					("cyans", cyans),
					("blues", blues),
					("magentas", magentas),
					("whites", whites),
					("neutrals", neutrals),
					("blacks", blacks),
				];
				for (name, inks) in groups {
					inks.visit_parameters(visitor, name);
				}
			}
			Self::Posterize { levels } => visitor.parameter("levels", levels, 4),
			Self::Exposure { exposure, offset, gamma_correction } => {
				visitor.parameter("exposure", exposure, 0.);
				visitor.parameter("offset", offset, 0.);
				visitor.parameter("gamma_correction", gamma_correction, 1.);
			}
			Self::ColorBalance(settings) => {
				settings.shadows.visit_parameters(visitor, "shadows");
				settings.midtones.visit_parameters(visitor, "midtones");
				settings.highlights.visit_parameters(visitor, "highlights");
				visitor.parameter("preserve_luminosity", &mut settings.preserve_luminosity, true);
			}
			Self::PhotoFilter { color, density, preserve_luminosity } => {
				visitor.parameter("color", color, Color::from(SRGBA8::new(0xec, 0x8a, 0x00, 0xff)));
				visitor.parameter("density", density, 25.);
				visitor.parameter("preserve_luminosity", preserve_luminosity, true);
			}
			Self::ColorLookup { lut_file } => visitor.parameter("lut_file", lut_file, Resource::default()),
			Self::ColorOverlay { color, blend_mode, opacity } => {
				visitor.parameter("color", color, Color::BLACK);
				visitor.parameter("blend_mode", blend_mode, BlendMode::default());
				visitor.parameter("opacity", opacity, 100.);
			}
		}
	}
}

/// Conversion from a color to grayscale.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(dyn_any::DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash, CacheHash, node_macro::ChoiceType, FromPrimitive, IntoPrimitive)]
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

#[repr(u32)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(dyn_any::DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, CacheHash, node_macro::ChoiceType, FromPrimitive, IntoPrimitive)]
#[widget(Radio)]
pub enum RelativeAbsolute {
	#[default]
	Relative,
	Absolute,
}

/// A group of colors whose inks the Selective Color adjustment offsets.
#[repr(u32)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(dyn_any::DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, CacheHash, node_macro::ChoiceType, FromPrimitive, IntoPrimitive)]
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

/// One channel's Levels record: percentage input and output points and the midtones gamma.
#[derive(Clone, Copy, Debug, Default, PartialEq, CacheHash)]
pub struct LevelsRecord {
	pub shadows: f64,
	pub midtones: f64,
	pub highlights: f64,
	pub output_minimums: f64,
	pub output_maximums: f64,
}

/// The composite record that the three color channels pass through after their own, and the alpha record that stands apart.
#[derive(Clone, Debug, Default, PartialEq, CacheHash)]
pub struct LevelsSettings {
	pub composite: LevelsRecord,
	pub red: LevelsRecord,
	pub green: LevelsRecord,
	pub blue: LevelsRecord,
	pub alpha: LevelsRecord,
}

/// The composite curve that the three color channels pass through after their own, and the alpha curve that stands apart.
#[derive(Clone, Debug, Default, PartialEq, CacheHash)]
pub struct CurvesSettings {
	pub composite: TransferCurve,
	pub red: TransferCurve,
	pub green: TransferCurve,
	pub blue: TransferCurve,
	pub alpha: TransferCurve,
}

/// The percentage each hue contributes to the gray, and the color tinting it when `use_tint` is set.
#[derive(Clone, Debug, Default, PartialEq, CacheHash)]
pub struct BlackAndWhiteSettings {
	pub use_tint: bool,
	pub tint: Color,
	pub reds: f64,
	pub yellows: f64,
	pub greens: f64,
	pub cyans: f64,
	pub blues: f64,
	pub magentas: f64,
}

/// One set of Hue/Saturation sliders: a hue in degrees and saturation and lightness percentages.
#[derive(Clone, Copy, Debug, Default, PartialEq, CacheHash)]
pub struct HueSaturationSliders {
	pub hue: f64,
	pub saturation: f64,
	pub lightness: f64,
}

/// The sliders for one range of hues, which apply fully from the range start to its end, fading out at the falloff ends. Edges are in degrees.
#[derive(Clone, Copy, Debug, Default, PartialEq, CacheHash)]
pub struct HueRangeSettings {
	pub sliders: HueSaturationSliders,
	pub falloff_start: f64,
	pub range_start: f64,
	pub range_end: f64,
	pub falloff_end: f64,
}

/// The master sliders shift every color, each hue range adds its own, and colorizing replaces them all with an absolute hue and saturation.
#[derive(Clone, Debug, Default, PartialEq, CacheHash)]
pub struct HueSaturationSettings {
	pub master: HueSaturationSliders,
	pub colorize: bool,
	pub colorize_sliders: HueSaturationSliders,
	pub reds: HueRangeSettings,
	pub yellows: HueRangeSettings,
	pub greens: HueRangeSettings,
	pub cyans: HueRangeSettings,
	pub blues: HueRangeSettings,
	pub magentas: HueRangeSettings,
}

/// The percentages of each source channel, plus a constant, that sum to one output channel.
#[derive(Clone, Copy, Debug, Default, PartialEq, CacheHash)]
pub struct ChannelMixerRow {
	pub red: f64,
	pub green: f64,
	pub blue: f64,
	pub constant: f64,
}

/// A row for each output channel, or for the single gray channel when monochrome.
#[derive(Clone, Debug, Default, PartialEq, CacheHash)]
pub struct ChannelMixerSettings {
	pub monochrome: bool,
	pub monochrome_mix: ChannelMixerRow,
	pub red: ChannelMixerRow,
	pub green: ChannelMixerRow,
	pub blue: ChannelMixerRow,
}

/// Percentage offsets to the cyan, magenta, yellow, and black inks of one group of colors.
#[derive(Clone, Copy, Debug, Default, PartialEq, CacheHash)]
pub struct SelectiveColorInks {
	pub cyan: f64,
	pub magenta: f64,
	pub yellow: f64,
	pub black: f64,
}

/// The ink offsets of each group of colors, and whether they are relative to the inks already present.
#[derive(Clone, Debug, Default, PartialEq, CacheHash)]
pub struct SelectiveColorSettings {
	pub mode: RelativeAbsolute,
	pub reds: SelectiveColorInks,
	pub yellows: SelectiveColorInks,
	pub greens: SelectiveColorInks,
	pub cyans: SelectiveColorInks,
	pub blues: SelectiveColorInks,
	pub magentas: SelectiveColorInks,
	pub whites: SelectiveColorInks,
	pub neutrals: SelectiveColorInks,
	pub blacks: SelectiveColorInks,
}

/// Percentage shifts along the three opponent color axes for one tonal range.
#[derive(Clone, Copy, Debug, Default, PartialEq, CacheHash)]
pub struct ColorBalanceShifts {
	pub cyan_red: f64,
	pub magenta_green: f64,
	pub yellow_blue: f64,
}

#[derive(Clone, Debug, Default, PartialEq, CacheHash)]
pub struct ColorBalanceSettings {
	pub shadows: ColorBalanceShifts,
	pub midtones: ColorBalanceShifts,
	pub highlights: ColorBalanceShifts,
	pub preserve_luminosity: bool,
}

/// The attribute key of a parameter in one of a node's groups of parameters, which the node names with the group's prefix.
fn grouped_key(group: &str, parameter: &str) -> String {
	if group.is_empty() { parameter.to_string() } else { format!("{group}_{parameter}") }
}

impl LevelsRecord {
	fn visit_parameters(&mut self, visitor: &mut impl ParameterVisitor, group: &str) {
		visitor.parameter(&grouped_key(group, "shadows"), &mut self.shadows, 0.);
		visitor.parameter(&grouped_key(group, "midtones"), &mut self.midtones, 1.);
		visitor.parameter(&grouped_key(group, "highlights"), &mut self.highlights, 100.);
		visitor.parameter(&grouped_key(group, "output_minimums"), &mut self.output_minimums, 0.);
		visitor.parameter(&grouped_key(group, "output_maximums"), &mut self.output_maximums, 100.);
	}
}

impl HueSaturationSliders {
	fn visit_parameters(&mut self, visitor: &mut impl ParameterVisitor, group: &str, [hue, saturation, lightness]: [f64; 3]) {
		visitor.parameter(&grouped_key(group, "hue"), &mut self.hue, hue);
		visitor.parameter(&grouped_key(group, "saturation"), &mut self.saturation, saturation);
		visitor.parameter(&grouped_key(group, "lightness"), &mut self.lightness, lightness);
	}
}

impl HueRangeSettings {
	fn visit_parameters(&mut self, visitor: &mut impl ParameterVisitor, group: &str, [falloff_start, range_start, range_end, falloff_end]: [f64; 4]) {
		self.sliders.visit_parameters(visitor, group, [0.; 3]);
		visitor.parameter(&grouped_key(group, "falloff_start"), &mut self.falloff_start, falloff_start);
		visitor.parameter(&grouped_key(group, "range_start"), &mut self.range_start, range_start);
		visitor.parameter(&grouped_key(group, "range_end"), &mut self.range_end, range_end);
		visitor.parameter(&grouped_key(group, "falloff_end"), &mut self.falloff_end, falloff_end);
	}
}

impl ChannelMixerRow {
	fn visit_parameters(&mut self, visitor: &mut impl ParameterVisitor, group: &str, [red, green, blue, constant]: [f64; 4]) {
		visitor.parameter(&grouped_key(group, "red"), &mut self.red, red);
		visitor.parameter(&grouped_key(group, "green"), &mut self.green, green);
		visitor.parameter(&grouped_key(group, "blue"), &mut self.blue, blue);
		visitor.parameter(&grouped_key(group, "constant"), &mut self.constant, constant);
	}
}

impl SelectiveColorInks {
	fn visit_parameters(&mut self, visitor: &mut impl ParameterVisitor, group: &str) {
		visitor.parameter(&grouped_key(group, "cyan"), &mut self.cyan, 0.);
		visitor.parameter(&grouped_key(group, "magenta"), &mut self.magenta, 0.);
		visitor.parameter(&grouped_key(group, "yellow"), &mut self.yellow, 0.);
		visitor.parameter(&grouped_key(group, "black"), &mut self.black, 0.);
	}
}

impl ColorBalanceShifts {
	fn visit_parameters(&mut self, visitor: &mut impl ParameterVisitor, group: &str) {
		visitor.parameter(&grouped_key(group, "cyan_red"), &mut self.cyan_red, 0.);
		visitor.parameter(&grouped_key(group, "magenta_green"), &mut self.magenta_green, 0.);
		visitor.parameter(&grouped_key(group, "yellow_blue"), &mut self.yellow_blue, 0.);
	}
}
