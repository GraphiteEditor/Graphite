//! Color adjustments as plain data, each holding its node's settings in the node's own units, carried out by the kernels of the `raster-kernels` crate.

use core_types::Color;
use core_types::blending::BlendMode;
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

impl std::fmt::Display for Adjustment {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(match self {
			Self::Desaturate { .. } => "Desaturate",
			Self::GammaCorrection { .. } => "Gamma Correction",
			Self::MakeOpaque => "Make Opaque",
			Self::BrightnessContrast { .. } => "Brightness/Contrast",
			Self::Levels(_) => "Levels",
			Self::Curves(_) => "Curves",
			Self::BlackAndWhite(_) => "Black & White",
			Self::HueSaturation(_) => "Hue/Saturation",
			Self::Invert => "Invert",
			Self::Threshold { .. } => "Threshold",
			Self::GradientMap { .. } => "Gradient Map",
			Self::Vibrance { .. } => "Vibrance",
			Self::ChannelMixer(_) => "Channel Mixer",
			Self::SelectiveColor(_) => "Selective Color",
			Self::Posterize { .. } => "Posterize",
			Self::Exposure { .. } => "Exposure",
			Self::ColorBalance(_) => "Color Balance",
			Self::PhotoFilter { .. } => "Photo Filter",
			Self::ColorLookup { .. } => "Color Lookup",
			Self::ColorOverlay { .. } => "Color Overlay",
		})
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
#[derive(Clone, Copy, Debug, PartialEq, CacheHash)]
pub struct LevelsRecord {
	pub shadows: f64,
	pub midtones: f64,
	pub highlights: f64,
	pub output_minimums: f64,
	pub output_maximums: f64,
}

/// The composite record that the three color channels pass through after their own, and the alpha record that stands apart.
#[derive(Clone, Debug, PartialEq, CacheHash)]
pub struct LevelsSettings {
	pub composite: LevelsRecord,
	pub red: LevelsRecord,
	pub green: LevelsRecord,
	pub blue: LevelsRecord,
	pub alpha: LevelsRecord,
}

/// The composite curve that the three color channels pass through after their own, and the alpha curve that stands apart.
#[derive(Clone, Debug, PartialEq, CacheHash)]
pub struct CurvesSettings {
	pub composite: TransferCurve,
	pub red: TransferCurve,
	pub green: TransferCurve,
	pub blue: TransferCurve,
	pub alpha: TransferCurve,
}

/// The percentage each hue contributes to the gray, and the color tinting it when `use_tint` is set.
#[derive(Clone, Debug, PartialEq, CacheHash)]
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
#[derive(Clone, Copy, Debug, PartialEq, CacheHash)]
pub struct HueSaturationSliders {
	pub hue: f64,
	pub saturation: f64,
	pub lightness: f64,
}

/// The sliders for one range of hues, which apply fully from the range start to its end, fading out at the falloff ends. Edges are in degrees.
#[derive(Clone, Copy, Debug, PartialEq, CacheHash)]
pub struct HueRangeSettings {
	pub sliders: HueSaturationSliders,
	pub falloff_start: f64,
	pub range_start: f64,
	pub range_end: f64,
	pub falloff_end: f64,
}

/// The master sliders shift every color, each hue range adds its own, and colorizing replaces them all with an absolute hue and saturation.
#[derive(Clone, Debug, PartialEq, CacheHash)]
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
#[derive(Clone, Copy, Debug, PartialEq, CacheHash)]
pub struct ChannelMixerRow {
	pub red: f64,
	pub green: f64,
	pub blue: f64,
	pub constant: f64,
}

/// A row for each output channel, or for the single gray channel when monochrome.
#[derive(Clone, Debug, PartialEq, CacheHash)]
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
#[derive(Clone, Debug, PartialEq, CacheHash)]
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
#[derive(Clone, Copy, Debug, PartialEq, CacheHash)]
pub struct ColorBalanceShifts {
	pub cyan_red: f64,
	pub magenta_green: f64,
	pub yellow_blue: f64,
}

#[derive(Clone, Debug, PartialEq, CacheHash)]
pub struct ColorBalanceSettings {
	pub shadows: ColorBalanceShifts,
	pub midtones: ColorBalanceShifts,
	pub highlights: ColorBalanceShifts,
	pub preserve_luminosity: bool,
}
