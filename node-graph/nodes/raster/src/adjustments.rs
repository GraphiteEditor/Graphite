#![allow(clippy::too_many_arguments)]

use crate::effects::ApplyEffect;
use crate::gpu_params;
use crate::raster_kernel::{MapColor, Wgsl, map_raster, raster_kernel_pipeline};
use core::fmt::Debug;
use core_types::color::Color;
use core_types::context::Ctx;
use core_types::list::{Item, List};
use core_types::transfer_curve::TransferCurve;
use core_types::{Interrupt, NodeError};
use glam::DVec2;
use graphene_resource::Resource;
use graphic_types::adjustment::{
	Adjustment, BlackAndWhiteSettings, ChannelMixerRow, ChannelMixerSettings, ColorBalanceSettings, ColorBalanceShifts, CurvesSettings, HueRangeSettings, HueSaturationSettings, HueSaturationSliders,
	LevelsRecord, LevelsSettings, SelectiveColorInks, SelectiveColorSettings,
};
pub use graphic_types::adjustment::{DesaturateMethod, RelativeAbsolute, SelectiveColorChoice};
pub use graphic_types::generator::{CellularDistanceFunction, CellularReturnType, DomainWarpType, FractalType, NoiseType};
use graphic_types::{Graphic, Vector};
use num_enum::{FromPrimitive, IntoPrimitive};
use raster_kernels::color_lookup_table::parse_cached;
use raster_types::{CPU, GPU, Raster};
use vector_types::Gradient;
use wgpu_executor::WgpuPipelineCache;

/// Carries out the adjustment on the item's content.
fn adjust<T: ApplyEffect>(item: Item<T>, adjustment: Adjustment, pipeline: Item<WgpuPipelineCache>) -> Result<Item<T>, Interrupt> {
	Ok(T::apply_adjustment(item, adjustment, pipeline.element())?)
}

#[node_macro::node(category("Raster: Adjustment"), icon("NodeDesaturate"))]
fn desaturate<T: ApplyEffect>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient, Graphic, Vector, Raster<GPU>, String)] input: Item<T>,
	method: Item<DesaturateMethod>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	adjust(input, Adjustment::Desaturate { method: method.into_element() }, pipeline)
}

#[node_macro::node(category("Raster: Adjustment"), icon("NodeGammaCorrection"))]
fn gamma_correction<T: ApplyEffect>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient, Graphic, Vector, Raster<GPU>, String)] input: Item<T>,
	#[default(2.2)]
	#[range]
	#[hard(0.0001..)]
	#[soft(0.01..10)]
	gamma: Item<f64>,
	inverse: Item<bool>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	adjust(
		input,
		Adjustment::GammaCorrection {
			gamma: gamma.into_element(),
			inverse: inverse.into_element(),
		},
		pipeline,
	)
}

/// The red, green, blue, and alpha channels of an image, split into separate node outputs.
#[derive(Debug, Clone, dyn_any::DynAny, node_macro::Destructure)]
pub struct ImageChannels {
	/// The red channel of the image, as a grayscale image.
	pub red: Item<Raster<GPU>>,
	/// The green channel of the image, as a grayscale image.
	pub green: Item<Raster<GPU>>,
	/// The blue channel of the image, as a grayscale image.
	pub blue: Item<Raster<GPU>>,
	/// The alpha channel of the image, as a grayscale image.
	pub alpha: Item<Raster<GPU>>,
}

/// Separates an image into its red, green, blue, and alpha channels, each provided as a grayscale image.
#[node_macro::node(category("Raster"), icon("NodeImageToChannels"), name("Image to Channels"))]
fn image_to_channels(_: impl Ctx, image: Item<Raster<GPU>>, #[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>) -> Result<ImageChannels, Interrupt> {
	let [red, green, blue, alpha] = [0, 1, 2, 3].map(|channel| {
		let (raster, attributes) = image.clone().into_parts();
		map_raster(&ExtractChannel { channel }, &raster, pipeline.element()).map(|channel_raster| Item::from_parts(channel_raster, attributes))
	});

	Ok(ImageChannels {
		red: red?,
		green: green?,
		blue: blue?,
		alpha: alpha?,
	})
}

gpu_params! {
	#[derive(Clone)]
	struct ExtractChannel {
		channel: u32,
	}
}

impl MapColor for ExtractChannel {
	fn map_cpu(&self, color: Color) -> Color {
		let value = [color.r(), color.g(), color.b(), color.a()][self.channel as usize];
		Color::from_rgbaf32_unchecked(value, value, value, 1.)
	}

	fn wgsl() -> Wgsl {
		Wgsl::body(
			r#"
			let value = color[channel];
			return vec4<f32>(value, value, value, 1.);
			"#,
		)
	}
}

#[node_macro::node(category("Raster: Adjustment"), icon("NodeMakeOpaque"))]
fn make_opaque<T: ApplyEffect>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient, Graphic, Vector, Raster<GPU>, String)] input: Item<T>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	adjust(input, Adjustment::MakeOpaque, pipeline)
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27brit%27%20%3D%20Brightness/Contrast
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=Padding-,Brightness%20and%20Contrast,-Key%20is%20%27brit
//
// Some further analysis available at:
// https://geraldbakker.nl/psnumbers/brightness-contrast.html
//
// TODO: A Lab-only mode once Graphite supports the CIE Lab color space.
#[node_macro::node(name("Brightness/Contrast"), category("Raster: Adjustment"), icon("NodeBrightnessContrast"), inspector("brightness_contrast_inspector"))]
fn brightness_contrast<T: ApplyEffect>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient, Graphic, Vector, Raster<GPU>, String)] input: Item<T>,
	#[unit("%")]
	#[range]
	#[hard(-150..150)]
	brightness: Item<f64>,
	#[unit("%")]
	#[range]
	#[hard(-100..100)]
	contrast: Item<f64>,
	use_classic: Item<bool>,
	#[range]
	#[soft(0..255)]
	#[default(127.)]
	classic_pivot: Item<f64>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	adjust(
		input,
		Adjustment::BrightnessContrast {
			brightness: brightness.into_element(),
			contrast: contrast.into_element(),
			use_classic: use_classic.into_element(),
			classic_pivot: classic_pivot.into_element(),
		},
		pipeline,
	)
}

#[repr(u32)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(dyn_any::DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, node_macro::ChoiceType, FromPrimitive, IntoPrimitive)]
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

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=levl%27%20%3D%20Levels
//
// Some further analysis available at:
// https://geraldbakker.nl/psnumbers/levels.html
#[node_macro::node(category("Raster: Adjustment"), icon("NodeLevels"), inspector("levels_inspector"))]
fn levels<T: ApplyEffect>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient, Graphic, Vector, Raster<GPU>, String)] image: Item<T>,
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(0.)]
	shadows: Item<f64>,
	#[range]
	#[soft(0.01..9.99)]
	#[default(1.)]
	midtones: Item<f64>,
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(100.)]
	highlights: Item<f64>,
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(0.)]
	output_minimums: Item<f64>,
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(100.)]
	output_maximums: Item<f64>,
	#[name("(Red) Shadows")]
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(0.)]
	red_shadows: Item<f64>,
	#[name("(Red) Midtones")]
	#[range]
	#[soft(0.01..9.99)]
	#[default(1.)]
	red_midtones: Item<f64>,
	#[name("(Red) Highlights")]
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(100.)]
	red_highlights: Item<f64>,
	#[name("(Red) Output Minimums")]
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(0.)]
	red_output_minimums: Item<f64>,
	#[name("(Red) Output Maximums")]
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(100.)]
	red_output_maximums: Item<f64>,
	#[name("(Green) Shadows")]
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(0.)]
	green_shadows: Item<f64>,
	#[name("(Green) Midtones")]
	#[range]
	#[soft(0.01..9.99)]
	#[default(1.)]
	green_midtones: Item<f64>,
	#[name("(Green) Highlights")]
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(100.)]
	green_highlights: Item<f64>,
	#[name("(Green) Output Minimums")]
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(0.)]
	green_output_minimums: Item<f64>,
	#[name("(Green) Output Maximums")]
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(100.)]
	green_output_maximums: Item<f64>,
	#[name("(Blue) Shadows")]
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(0.)]
	blue_shadows: Item<f64>,
	#[name("(Blue) Midtones")]
	#[range]
	#[soft(0.01..9.99)]
	#[default(1.)]
	blue_midtones: Item<f64>,
	#[name("(Blue) Highlights")]
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(100.)]
	blue_highlights: Item<f64>,
	#[name("(Blue) Output Minimums")]
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(0.)]
	blue_output_minimums: Item<f64>,
	#[name("(Blue) Output Maximums")]
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(100.)]
	blue_output_maximums: Item<f64>,
	#[name("(Alpha) Shadows")]
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(0.)]
	alpha_shadows: Item<f64>,
	#[name("(Alpha) Midtones")]
	#[range]
	#[soft(0.01..9.99)]
	#[default(1.)]
	alpha_midtones: Item<f64>,
	#[name("(Alpha) Highlights")]
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(100.)]
	alpha_highlights: Item<f64>,
	#[name("(Alpha) Output Minimums")]
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(0.)]
	alpha_output_minimums: Item<f64>,
	#[name("(Alpha) Output Maximums")]
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(100.)]
	alpha_output_maximums: Item<f64>,
	_channel: Item<AdjustmentChannel>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	let record = |shadows: Item<f64>, midtones: Item<f64>, highlights: Item<f64>, output_minimums: Item<f64>, output_maximums: Item<f64>| LevelsRecord {
		shadows: shadows.into_element(),
		midtones: midtones.into_element(),
		highlights: highlights.into_element(),
		output_minimums: output_minimums.into_element(),
		output_maximums: output_maximums.into_element(),
	};
	let settings = LevelsSettings {
		composite: record(shadows, midtones, highlights, output_minimums, output_maximums),
		red: record(red_shadows, red_midtones, red_highlights, red_output_minimums, red_output_maximums),
		green: record(green_shadows, green_midtones, green_highlights, green_output_minimums, green_output_maximums),
		blue: record(blue_shadows, blue_midtones, blue_highlights, blue_output_minimums, blue_output_maximums),
		alpha: record(alpha_shadows, alpha_midtones, alpha_highlights, alpha_output_minimums, alpha_output_maximums),
	};

	adjust(image, Adjustment::Levels(Box::new(settings)), pipeline)
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27curv%27%20%3D%20Curves
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=Curves%20file%20format
//
// Each curve is any number of (x, y) points on 0..1 joined by a natural cubic spline held flat beyond the outermost
// points, and the per-channel curves apply before the composite one, like Levels. The value between those two stages
// stays exact rather than rounding through an 8-bit table, which can leave results a level away from 8-bit pipelines.
// Needs the heap for its curves, so it stays off the shader build for now.
#[node_macro::node(category("Raster: Adjustment"), icon("NodeCurves"), inspector("transfer_curves_inspector"))]
fn curves<T: ApplyEffect>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient, Graphic, Vector, Raster<GPU>, String)] image: Item<T>,
	curve: Item<TransferCurve>,
	#[name("(Red) Curve")] red_curve: Item<TransferCurve>,
	#[name("(Green) Curve")] green_curve: Item<TransferCurve>,
	#[name("(Blue) Curve")] blue_curve: Item<TransferCurve>,
	#[name("(Alpha) Curve")] alpha_curve: Item<TransferCurve>,
	_channel: Item<AdjustmentChannel>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	let settings = CurvesSettings {
		composite: curve.into_element(),
		red: red_curve.into_element(),
		green: green_curve.into_element(),
		blue: blue_curve.into_element(),
		alpha: alpha_curve.into_element(),
	};

	adjust(image, Adjustment::Curves(Box::new(settings)), pipeline)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("A control point has a coordinate that isn't finite")]
pub struct NonFinitePointError;

/// Builds a transfer curve from a `Vec2[]` of control points, each mapping the input value at its x to the output value at its y. A smooth spline runs through them, holding the outermost points' values beyond them.
#[node_macro::node(category("Raster: Adjustment"), icon("NodePointsToTransferCurve"), name("Points to Transfer Curve"))]
fn points_to_transfer_curve(
	_: impl Ctx,
	/// The control points, in any order, with both coordinates on the 0 to 1 range.
	points: List<DVec2>,
) -> Result<Item<TransferCurve>, NonFinitePointError> {
	let points: Vec<DVec2> = points.iter_element_values().copied().collect();
	if !points.iter().all(|point| point.is_finite()) {
		return Err(NonFinitePointError);
	}
	Ok(Item::new_from_element(TransferCurve::new(points)))
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27blwh%27%20%3D%20Black%20and%20White
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=Black%20White%20(Photoshop%20CS3)
//
// Algorithm from:
// https://stackoverflow.com/a/55233732/775283
// Works the same for gamma and linear color
#[node_macro::node(name("Black & White"), category("Raster: Adjustment"), icon("NodeBlackAndWhite"), inspector("black_and_white_inspector"))]
fn black_and_white<T: ApplyEffect>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient, Graphic, Vector, Raster<GPU>, String)] image: Item<T>,
	use_tint: Item<bool>,
	#[default("#e1d3b3")] tint: Item<Color>,
	#[unit("%")]
	#[default(40.)]
	#[range]
	#[soft(-200..300)]
	reds: Item<f64>,
	#[unit("%")]
	#[default(60.)]
	#[range]
	#[soft(-200..300)]
	yellows: Item<f64>,
	#[unit("%")]
	#[default(40.)]
	#[range]
	#[soft(-200..300)]
	greens: Item<f64>,
	#[unit("%")]
	#[default(60.)]
	#[range]
	#[soft(-200..300)]
	cyans: Item<f64>,
	#[unit("%")]
	#[default(20.)]
	#[range]
	#[soft(-200..300)]
	blues: Item<f64>,
	#[unit("%")]
	#[default(80.)]
	#[range]
	#[soft(-200..300)]
	magentas: Item<f64>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	let settings = BlackAndWhiteSettings {
		use_tint: use_tint.into_element(),
		tint: tint.into_element(),
		reds: reds.into_element(),
		yellows: yellows.into_element(),
		greens: greens.into_element(),
		cyans: cyans.into_element(),
		blues: blues.into_element(),
		magentas: magentas.into_element(),
	};

	adjust(image, Adjustment::BlackAndWhite(settings), pipeline)
}

#[repr(u32)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(dyn_any::DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, node_macro::ChoiceType, FromPrimitive, IntoPrimitive)]
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

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27hue%20%27%20%3D%20Old,saturation%2C%20Photoshop%205.0
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=0%20%3D%20Use%20other.-,Hue/Saturation,-Hue/Saturation%20settings
//
// TODO: Residuals in 8-bit PSD interop: the byte-hue colorize table, the colorize lightness slider (up to 2.5 levels), and the range edges (a few tenths of a degree)
#[node_macro::node(name("Hue/Saturation"), category("Raster: Adjustment"), icon("NodeHueSaturation"), inspector("hue_saturation_inspector"))]
fn hue_saturation<T: ApplyEffect>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient, Graphic, Vector, Raster<GPU>, String)] input: Item<T>,
	#[unit("°")]
	#[range]
	#[soft(-180..180)]
	hue: Item<f64>,
	#[unit("%")]
	#[range]
	#[hard(-100..100)]
	saturation: Item<f64>,
	#[unit("%")]
	#[range]
	#[hard(-100..100)]
	lightness: Item<f64>,
	colorize: Item<bool>,
	// Colorize sets an absolute hue rather than shifting one, so its range covers the whole turn
	#[name("(Colorize) Hue")]
	#[unit("°")]
	#[range]
	#[soft(0..360)]
	#[default(24.)]
	colorize_hue: Item<f64>,
	#[name("(Colorize) Saturation")]
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(25.)]
	colorize_saturation: Item<f64>,
	#[name("(Colorize) Lightness")]
	#[unit("%")]
	#[range]
	#[hard(-100..100)]
	colorize_lightness: Item<f64>,
	#[name("(Reds) Hue")]
	#[unit("°")]
	#[range]
	#[soft(-180..180)]
	reds_hue: Item<f64>,
	#[name("(Reds) Saturation")]
	#[unit("%")]
	#[range]
	#[hard(-100..100)]
	reds_saturation: Item<f64>,
	#[name("(Reds) Lightness")]
	#[unit("%")]
	#[range]
	#[hard(-100..100)]
	reds_lightness: Item<f64>,
	#[name("(Reds) Falloff Start")]
	#[unit("°")]
	#[range]
	#[soft(0..360)]
	#[default(315.)]
	reds_falloff_start: Item<f64>,
	#[name("(Reds) Range Start")]
	#[unit("°")]
	#[range]
	#[soft(0..360)]
	#[default(345.)]
	reds_range_start: Item<f64>,
	#[name("(Reds) Range End")]
	#[unit("°")]
	#[range]
	#[soft(0..360)]
	#[default(15.)]
	reds_range_end: Item<f64>,
	#[name("(Reds) Falloff End")]
	#[unit("°")]
	#[range]
	#[soft(0..360)]
	#[default(45.)]
	reds_falloff_end: Item<f64>,
	#[name("(Yellows) Hue")]
	#[unit("°")]
	#[range]
	#[soft(-180..180)]
	yellows_hue: Item<f64>,
	#[name("(Yellows) Saturation")]
	#[unit("%")]
	#[range]
	#[hard(-100..100)]
	yellows_saturation: Item<f64>,
	#[name("(Yellows) Lightness")]
	#[unit("%")]
	#[range]
	#[hard(-100..100)]
	yellows_lightness: Item<f64>,
	#[name("(Yellows) Falloff Start")]
	#[unit("°")]
	#[range]
	#[soft(0..360)]
	#[default(15.)]
	yellows_falloff_start: Item<f64>,
	#[name("(Yellows) Range Start")]
	#[unit("°")]
	#[range]
	#[soft(0..360)]
	#[default(45.)]
	yellows_range_start: Item<f64>,
	#[name("(Yellows) Range End")]
	#[unit("°")]
	#[range]
	#[soft(0..360)]
	#[default(75.)]
	yellows_range_end: Item<f64>,
	#[name("(Yellows) Falloff End")]
	#[unit("°")]
	#[range]
	#[soft(0..360)]
	#[default(105.)]
	yellows_falloff_end: Item<f64>,
	#[name("(Greens) Hue")]
	#[unit("°")]
	#[range]
	#[soft(-180..180)]
	greens_hue: Item<f64>,
	#[name("(Greens) Saturation")]
	#[unit("%")]
	#[range]
	#[hard(-100..100)]
	greens_saturation: Item<f64>,
	#[name("(Greens) Lightness")]
	#[unit("%")]
	#[range]
	#[hard(-100..100)]
	greens_lightness: Item<f64>,
	#[name("(Greens) Falloff Start")]
	#[unit("°")]
	#[range]
	#[soft(0..360)]
	#[default(75.)]
	greens_falloff_start: Item<f64>,
	#[name("(Greens) Range Start")]
	#[unit("°")]
	#[range]
	#[soft(0..360)]
	#[default(105.)]
	greens_range_start: Item<f64>,
	#[name("(Greens) Range End")]
	#[unit("°")]
	#[range]
	#[soft(0..360)]
	#[default(135.)]
	greens_range_end: Item<f64>,
	#[name("(Greens) Falloff End")]
	#[unit("°")]
	#[range]
	#[soft(0..360)]
	#[default(165.)]
	greens_falloff_end: Item<f64>,
	#[name("(Cyans) Hue")]
	#[unit("°")]
	#[range]
	#[soft(-180..180)]
	cyans_hue: Item<f64>,
	#[name("(Cyans) Saturation")]
	#[unit("%")]
	#[range]
	#[hard(-100..100)]
	cyans_saturation: Item<f64>,
	#[name("(Cyans) Lightness")]
	#[unit("%")]
	#[range]
	#[hard(-100..100)]
	cyans_lightness: Item<f64>,
	#[name("(Cyans) Falloff Start")]
	#[unit("°")]
	#[range]
	#[soft(0..360)]
	#[default(135.)]
	cyans_falloff_start: Item<f64>,
	#[name("(Cyans) Range Start")]
	#[unit("°")]
	#[range]
	#[soft(0..360)]
	#[default(165.)]
	cyans_range_start: Item<f64>,
	#[name("(Cyans) Range End")]
	#[unit("°")]
	#[range]
	#[soft(0..360)]
	#[default(195.)]
	cyans_range_end: Item<f64>,
	#[name("(Cyans) Falloff End")]
	#[unit("°")]
	#[range]
	#[soft(0..360)]
	#[default(225.)]
	cyans_falloff_end: Item<f64>,
	#[name("(Blues) Hue")]
	#[unit("°")]
	#[range]
	#[soft(-180..180)]
	blues_hue: Item<f64>,
	#[name("(Blues) Saturation")]
	#[unit("%")]
	#[range]
	#[hard(-100..100)]
	blues_saturation: Item<f64>,
	#[name("(Blues) Lightness")]
	#[unit("%")]
	#[range]
	#[hard(-100..100)]
	blues_lightness: Item<f64>,
	#[name("(Blues) Falloff Start")]
	#[unit("°")]
	#[range]
	#[soft(0..360)]
	#[default(195.)]
	blues_falloff_start: Item<f64>,
	#[name("(Blues) Range Start")]
	#[unit("°")]
	#[range]
	#[soft(0..360)]
	#[default(225.)]
	blues_range_start: Item<f64>,
	#[name("(Blues) Range End")]
	#[unit("°")]
	#[range]
	#[soft(0..360)]
	#[default(255.)]
	blues_range_end: Item<f64>,
	#[name("(Blues) Falloff End")]
	#[unit("°")]
	#[range]
	#[soft(0..360)]
	#[default(285.)]
	blues_falloff_end: Item<f64>,
	#[name("(Magentas) Hue")]
	#[unit("°")]
	#[range]
	#[soft(-180..180)]
	magentas_hue: Item<f64>,
	#[name("(Magentas) Saturation")]
	#[unit("%")]
	#[range]
	#[hard(-100..100)]
	magentas_saturation: Item<f64>,
	#[name("(Magentas) Lightness")]
	#[unit("%")]
	#[range]
	#[hard(-100..100)]
	magentas_lightness: Item<f64>,
	#[name("(Magentas) Falloff Start")]
	#[unit("°")]
	#[range]
	#[soft(0..360)]
	#[default(255.)]
	magentas_falloff_start: Item<f64>,
	#[name("(Magentas) Range Start")]
	#[unit("°")]
	#[range]
	#[soft(0..360)]
	#[default(285.)]
	magentas_range_start: Item<f64>,
	#[name("(Magentas) Range End")]
	#[unit("°")]
	#[range]
	#[soft(0..360)]
	#[default(315.)]
	magentas_range_end: Item<f64>,
	#[name("(Magentas) Falloff End")]
	#[unit("°")]
	#[range]
	#[soft(0..360)]
	#[default(345.)]
	magentas_falloff_end: Item<f64>,
	_range: Item<HueSaturationRange>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	let sliders = |hue: Item<f64>, saturation: Item<f64>, lightness: Item<f64>| HueSaturationSliders {
		hue: hue.into_element(),
		saturation: saturation.into_element(),
		lightness: lightness.into_element(),
	};
	let range = |sliders: HueSaturationSliders, falloff_start: Item<f64>, range_start: Item<f64>, range_end: Item<f64>, falloff_end: Item<f64>| HueRangeSettings {
		sliders,
		falloff_start: falloff_start.into_element(),
		range_start: range_start.into_element(),
		range_end: range_end.into_element(),
		falloff_end: falloff_end.into_element(),
	};
	let settings = HueSaturationSettings {
		master: sliders(hue, saturation, lightness),
		colorize: colorize.into_element(),
		colorize_sliders: sliders(colorize_hue, colorize_saturation, colorize_lightness),
		reds: range(
			sliders(reds_hue, reds_saturation, reds_lightness),
			reds_falloff_start,
			reds_range_start,
			reds_range_end,
			reds_falloff_end,
		),
		yellows: range(
			sliders(yellows_hue, yellows_saturation, yellows_lightness),
			yellows_falloff_start,
			yellows_range_start,
			yellows_range_end,
			yellows_falloff_end,
		),
		greens: range(
			sliders(greens_hue, greens_saturation, greens_lightness),
			greens_falloff_start,
			greens_range_start,
			greens_range_end,
			greens_falloff_end,
		),
		cyans: range(
			sliders(cyans_hue, cyans_saturation, cyans_lightness),
			cyans_falloff_start,
			cyans_range_start,
			cyans_range_end,
			cyans_falloff_end,
		),
		blues: range(
			sliders(blues_hue, blues_saturation, blues_lightness),
			blues_falloff_start,
			blues_range_start,
			blues_range_end,
			blues_falloff_end,
		),
		magentas: range(
			sliders(magentas_hue, magentas_saturation, magentas_lightness),
			magentas_falloff_start,
			magentas_range_start,
			magentas_range_end,
			magentas_falloff_end,
		),
	};

	adjust(input, Adjustment::HueSaturation(Box::new(settings)), pipeline)
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27%20%3D%20Color%20Lookup-,%27nvrt%27%20%3D%20Invert,-%27post%27%20%3D%20Posterize
#[node_macro::node(category("Raster: Adjustment"), icon("NodeInvert"))]
fn invert<T: ApplyEffect>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient, Graphic, Vector, Raster<GPU>, String)] input: Item<T>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	adjust(input, Adjustment::Invert, pipeline)
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=post%27%20%3D%20Posterize-,%27thrs%27%20%3D%20Threshold,-%27grdm%27%20%3D%20Gradient
#[node_macro::node(category("Raster: Adjustment"), icon("NodeThreshold"), inspector("threshold_inspector"))]
fn threshold<T: ApplyEffect>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient, Graphic, Vector, Raster<GPU>, String)] image: Item<T>,
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(50.)]
	min_luminance: Item<f64>,
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(100.)]
	max_luminance: Item<f64>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	adjust(
		image,
		Adjustment::Threshold {
			min_luminance: min_luminance.into_element(),
			max_luminance: max_luminance.into_element(),
		},
		pipeline,
	)
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
#[node_macro::node(category("Raster: Adjustment"), icon("NodeGradientMap"))]
fn gradient_map<T: ApplyEffect>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient, Graphic, Vector, Raster<GPU>, String)] image: Item<T>,
	#[default(Color::BLACK, Color::WHITE)] gradient: Item<Gradient>,
	reverse: Item<bool>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	adjust(
		image,
		Adjustment::GradientMap {
			gradient,
			reverse: reverse.into_element(),
		},
		pipeline,
	)
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27-,vibA%27%20%3D%20Vibrance,-%27hue%20%27%20%3D%20Old
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=Vibrance%20(Photoshop%20CS3)
#[node_macro::node(category("Raster: Adjustment"), icon("NodeVibrance"), inspector("vibrance_inspector"))]
fn vibrance<T: ApplyEffect>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient, Graphic, Vector, Raster<GPU>, String)] image: Item<T>,
	#[unit("%")]
	#[range]
	#[hard(-100..100)]
	vibrance: Item<f64>,
	#[unit("%")]
	#[range]
	#[hard(-100..100)]
	saturation: Item<f64>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	adjust(
		image,
		Adjustment::Vibrance {
			vibrance: vibrance.into_element(),
			saturation: saturation.into_element(),
		},
		pipeline,
	)
}

#[repr(u32)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(dyn_any::DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, node_macro::ChoiceType, FromPrimitive, IntoPrimitive)]
#[widget(Radio)]
pub enum RedGreenBlue {
	#[default]
	Red,
	Green,
	Blue,
}

// Kept only to deserialize the channel input of old Extract Channel nodes for document migration
#[derive(dyn_any::DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum RedGreenBlueAlpha {
	#[default]
	Red,
	Green,
	Blue,
	Alpha,
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27mixr%27%20%3D%20Channel%20Mixer
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=Lab%20color%20only-,Channel%20Mixer,-Key%20is%20%27mixr
//
// TODO: CMYK source channels once Graphite supports the CMYK color space.
#[node_macro::node(category("Raster: Adjustment"), icon("NodeChannelMixer"), inspector("channel_mixer_inspector"))]
fn channel_mixer<T: ApplyEffect>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient, Graphic, Vector, Raster<GPU>, String)] image: Item<T>,

	monochrome: Item<bool>,

	#[name("Red")]
	#[unit("%")]
	#[range]
	#[soft(-200..200)]
	#[default(40.)]
	monochrome_r: Item<f64>,
	#[name("Green")]
	#[unit("%")]
	#[range]
	#[soft(-200..200)]
	#[default(40.)]
	monochrome_g: Item<f64>,
	#[name("Blue")]
	#[unit("%")]
	#[range]
	#[soft(-200..200)]
	#[default(20.)]
	monochrome_b: Item<f64>,
	#[name("Constant")]
	#[unit("%")]
	#[range]
	#[soft(-200..200)]
	#[default(0.)]
	monochrome_c: Item<f64>,

	#[name("(Red) Red")]
	#[unit("%")]
	#[range]
	#[soft(-200..200)]
	#[default(100.)]
	red_r: Item<f64>,
	#[name("(Red) Green")]
	#[unit("%")]
	#[range]
	#[soft(-200..200)]
	#[default(0.)]
	red_g: Item<f64>,
	#[name("(Red) Blue")]
	#[unit("%")]
	#[range]
	#[soft(-200..200)]
	#[default(0.)]
	red_b: Item<f64>,
	#[name("(Red) Constant")]
	#[unit("%")]
	#[range]
	#[soft(-200..200)]
	#[default(0.)]
	red_c: Item<f64>,

	#[name("(Green) Red")]
	#[unit("%")]
	#[range]
	#[soft(-200..200)]
	#[default(0.)]
	green_r: Item<f64>,
	#[name("(Green) Green")]
	#[unit("%")]
	#[range]
	#[soft(-200..200)]
	#[default(100.)]
	green_g: Item<f64>,
	#[name("(Green) Blue")]
	#[unit("%")]
	#[range]
	#[soft(-200..200)]
	#[default(0.)]
	green_b: Item<f64>,
	#[name("(Green) Constant")]
	#[unit("%")]
	#[range]
	#[soft(-200..200)]
	#[default(0.)]
	green_c: Item<f64>,

	#[name("(Blue) Red")]
	#[unit("%")]
	#[range]
	#[soft(-200..200)]
	#[default(0.)]
	blue_r: Item<f64>,
	#[name("(Blue) Green")]
	#[unit("%")]
	#[range]
	#[soft(-200..200)]
	#[default(0.)]
	blue_g: Item<f64>,
	#[name("(Blue) Blue")]
	#[unit("%")]
	#[range]
	#[soft(-200..200)]
	#[default(100.)]
	blue_b: Item<f64>,
	#[name("(Blue) Constant")]
	#[unit("%")]
	#[range]
	#[soft(-200..200)]
	#[default(0.)]
	blue_c: Item<f64>,

	// Display-only parameters (not used within the node)
	_output_channel: Item<RedGreenBlue>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	let row = |red: Item<f64>, green: Item<f64>, blue: Item<f64>, constant: Item<f64>| ChannelMixerRow {
		red: red.into_element(),
		green: green.into_element(),
		blue: blue.into_element(),
		constant: constant.into_element(),
	};
	let settings = ChannelMixerSettings {
		monochrome: monochrome.into_element(),
		monochrome_mix: row(monochrome_r, monochrome_g, monochrome_b, monochrome_c),
		red: row(red_r, red_g, red_b, red_c),
		green: row(green_r, green_g, green_b, green_c),
		blue: row(blue_r, blue_g, blue_b, blue_c),
	};

	adjust(image, Adjustment::ChannelMixer(Box::new(settings)), pipeline)
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27selc%27%20%3D%20Selective%20color
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=from%20%2D100...100.%20.-,Selective%20Color,-Selective%20Color%20settings
//
// Algorithm based on:
// https://blog.pkh.me/p/22-understanding-selective-coloring-in-adobe-photoshop.html
#[node_macro::node(category("Raster: Adjustment"), icon("NodeSelectiveColor"), inspector("selective_color_inspector"))]
fn selective_color<T: ApplyEffect>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient, Graphic, Vector, Raster<GPU>, String)] image: Item<T>,

	mode: Item<RelativeAbsolute>,

	#[name("(Reds) Cyan")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	r_c: Item<f64>,
	#[name("(Reds) Magenta")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	r_m: Item<f64>,
	#[name("(Reds) Yellow")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	r_y: Item<f64>,
	#[name("(Reds) Black")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	r_k: Item<f64>,

	#[name("(Yellows) Cyan")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	y_c: Item<f64>,
	#[name("(Yellows) Magenta")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	y_m: Item<f64>,
	#[name("(Yellows) Yellow")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	y_y: Item<f64>,
	#[name("(Yellows) Black")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	y_k: Item<f64>,

	#[name("(Greens) Cyan")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	g_c: Item<f64>,
	#[name("(Greens) Magenta")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	g_m: Item<f64>,
	#[name("(Greens) Yellow")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	g_y: Item<f64>,
	#[name("(Greens) Black")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	g_k: Item<f64>,

	#[name("(Cyans) Cyan")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	c_c: Item<f64>,
	#[name("(Cyans) Magenta")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	c_m: Item<f64>,
	#[name("(Cyans) Yellow")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	c_y: Item<f64>,
	#[name("(Cyans) Black")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	c_k: Item<f64>,

	#[name("(Blues) Cyan")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	b_c: Item<f64>,
	#[name("(Blues) Magenta")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	b_m: Item<f64>,
	#[name("(Blues) Yellow")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	b_y: Item<f64>,
	#[name("(Blues) Black")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	b_k: Item<f64>,

	#[name("(Magentas) Cyan")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	m_c: Item<f64>,
	#[name("(Magentas) Magenta")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	m_m: Item<f64>,
	#[name("(Magentas) Yellow")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	m_y: Item<f64>,
	#[name("(Magentas) Black")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	m_k: Item<f64>,

	#[name("(Whites) Cyan")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	w_c: Item<f64>,
	#[name("(Whites) Magenta")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	w_m: Item<f64>,
	#[name("(Whites) Yellow")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	w_y: Item<f64>,
	#[name("(Whites) Black")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	w_k: Item<f64>,

	#[name("(Neutrals) Cyan")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	n_c: Item<f64>,
	#[name("(Neutrals) Magenta")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	n_m: Item<f64>,
	#[name("(Neutrals) Yellow")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	n_y: Item<f64>,
	#[name("(Neutrals) Black")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	n_k: Item<f64>,

	#[name("(Blacks) Cyan")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	k_c: Item<f64>,
	#[name("(Blacks) Magenta")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	k_m: Item<f64>,
	#[name("(Blacks) Yellow")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	k_y: Item<f64>,
	#[name("(Blacks) Black")]
	#[unit("%")]
	#[range]
	#[soft(-100..100)]
	k_k: Item<f64>,

	_colors: Item<SelectiveColorChoice>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	let inks = |cyan: Item<f64>, magenta: Item<f64>, yellow: Item<f64>, black: Item<f64>| SelectiveColorInks {
		cyan: cyan.into_element(),
		magenta: magenta.into_element(),
		yellow: yellow.into_element(),
		black: black.into_element(),
	};
	let settings = SelectiveColorSettings {
		mode: mode.into_element(),
		reds: inks(r_c, r_m, r_y, r_k),
		yellows: inks(y_c, y_m, y_y, y_k),
		greens: inks(g_c, g_m, g_y, g_k),
		cyans: inks(c_c, c_m, c_y, c_k),
		blues: inks(b_c, b_m, b_y, b_k),
		magentas: inks(m_c, m_m, m_y, m_k),
		whites: inks(w_c, w_m, w_y, w_k),
		neutrals: inks(n_c, n_m, n_y, n_k),
		blacks: inks(k_c, k_m, k_y, k_k),
	};

	adjust(image, Adjustment::SelectiveColor(Box::new(settings)), pipeline)
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=nvrt%27%20%3D%20Invert-,%27post%27%20%3D%20Posterize,-%27thrs%27%20%3D%20Threshold
#[node_macro::node(category("Raster: Adjustment"), icon("NodePosterize"))]
fn posterize<T: ApplyEffect>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient, Graphic, Vector, Raster<GPU>, String)] input: Item<T>,
	#[default(4)]
	#[hard(2..)]
	levels: Item<i64>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	adjust(input, Adjustment::Posterize { levels: levels.into_element() }, pipeline)
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=curv%27%20%3D%20Curves-,%27expA%27%20%3D%20Exposure,-%27vibA%27%20%3D%20Vibrance
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=Flag%20(%20%3D%20128%20)-,Exposure,-Key%20is%20%27expA
//
// The exposure, offset, and gamma operations follow:
// https://geraldbakker.nl/psnumbers/exposure.html
#[node_macro::node(category("Raster: Adjustment"), icon("NodeExposure"), inspector("exposure_inspector"))]
fn exposure<T: ApplyEffect>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient, Graphic, Vector, Raster<GPU>, String)] input: Item<T>,
	#[range]
	#[soft(-20..20)]
	exposure: Item<f64>,
	#[range]
	#[soft(-0.5..0.5)]
	offset: Item<f64>,
	#[default(1.)]
	#[range]
	#[hard(0.0001..)]
	#[soft(0.01..10)]
	gamma_correction: Item<f64>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	adjust(
		input,
		Adjustment::Exposure {
			exposure: exposure.into_element(),
			offset: offset.into_element(),
			gamma_correction: gamma_correction.into_element(),
		},
		pipeline,
	)
}

#[repr(u32)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(dyn_any::DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, node_macro::ChoiceType, FromPrimitive, IntoPrimitive)]
#[widget(Dropdown)]
pub enum TonalRange {
	Shadows,
	#[default]
	Midtones,
	Highlights,
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27blnc%27%20%3D%20Color%20Balance
//
// Every channel is a Levels curve whose black point, white point, and two-decimal gamma are derived from
// the nine sliders, see `color_balance_curve`.
#[node_macro::node(category("Raster: Adjustment"), icon("NodeColorBalance"), inspector("color_balance_inspector"))]
fn color_balance<T: ApplyEffect>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient, Graphic, Vector, Raster<GPU>, String)] image: Item<T>,

	#[name("(Shadows) Cyan-Red")]
	#[unit("%")]
	#[range]
	#[hard(-100..100)]
	shadows_cyan_red: Item<f64>,
	#[name("(Shadows) Magenta-Green")]
	#[unit("%")]
	#[range]
	#[hard(-100..100)]
	shadows_magenta_green: Item<f64>,
	#[name("(Shadows) Yellow-Blue")]
	#[unit("%")]
	#[range]
	#[hard(-100..100)]
	shadows_yellow_blue: Item<f64>,

	#[name("(Midtones) Cyan-Red")]
	#[unit("%")]
	#[range]
	#[hard(-100..100)]
	midtones_cyan_red: Item<f64>,
	#[name("(Midtones) Magenta-Green")]
	#[unit("%")]
	#[range]
	#[hard(-100..100)]
	midtones_magenta_green: Item<f64>,
	#[name("(Midtones) Yellow-Blue")]
	#[unit("%")]
	#[range]
	#[hard(-100..100)]
	midtones_yellow_blue: Item<f64>,

	#[name("(Highlights) Cyan-Red")]
	#[unit("%")]
	#[range]
	#[hard(-100..100)]
	highlights_cyan_red: Item<f64>,
	#[name("(Highlights) Magenta-Green")]
	#[unit("%")]
	#[range]
	#[hard(-100..100)]
	highlights_magenta_green: Item<f64>,
	#[name("(Highlights) Yellow-Blue")]
	#[unit("%")]
	#[range]
	#[hard(-100..100)]
	highlights_yellow_blue: Item<f64>,

	#[default(true)] preserve_luminosity: Item<bool>,

	// Display-only parameter (not used within the node)
	_tone: Item<TonalRange>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	let shifts = |cyan_red: Item<f64>, magenta_green: Item<f64>, yellow_blue: Item<f64>| ColorBalanceShifts {
		cyan_red: cyan_red.into_element(),
		magenta_green: magenta_green.into_element(),
		yellow_blue: yellow_blue.into_element(),
	};
	let settings = ColorBalanceSettings {
		shadows: shifts(shadows_cyan_red, shadows_magenta_green, shadows_yellow_blue),
		midtones: shifts(midtones_cyan_red, midtones_magenta_green, midtones_yellow_blue),
		highlights: shifts(highlights_cyan_red, highlights_magenta_green, highlights_yellow_blue),
		preserve_luminosity: preserve_luminosity.into_element(),
	};

	adjust(image, Adjustment::ColorBalance(settings), pipeline)
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27phfl%27%20%3D%20Photo%20Filter
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=of%20the%20file.-,Photo%20Filter,-Key%20is%20%27phfl
#[node_macro::node(category("Raster: Adjustment"), icon("NodePhotoFilter"))]
fn photo_filter<T: ApplyEffect>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Color, Gradient, Graphic, Vector, Raster<GPU>, String)] image: Item<T>,
	#[default("#ec8a00")] color: Item<Color>,
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(25.)]
	density: Item<f64>,
	#[default(true)] preserve_luminosity: Item<bool>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	adjust(
		image,
		Adjustment::PhotoFilter {
			color: color.into_element(),
			density: density.into_element(),
			preserve_luminosity: preserve_luminosity.into_element(),
		},
		pipeline,
	)
}

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27clrL%27%20%3D%20Color%20Lookup
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=Color%20Lookup%20(Photoshop%20CS6
//
// TODO: Support dither, which needs the pixel position that a per-color adjustment never sees.
// TODO: Verify the tetrahedral interpolation, which is unconfirmed against other implementations.
#[node_macro::node(category("Raster: Adjustment"), icon("NodeColorLookup"))]
fn color_lookup<T: ApplyEffect>(
	_: impl Ctx,
	/// The image whose colors are remapped by the LUT (lookup table).
	#[implementations(Raster<CPU>, Color, Gradient, Graphic, Vector, Raster<GPU>, String)]
	image: Item<T>,
	/// A LUT (lookup table) file in the `.cube`, `.3dl`, `.look`, `.csp`, or `.icc` (*abstract* or *device link* ICC profile) format.
	#[name("LUT File")]
	#[widget(ParsedWidgetOverride::Custom = "lut_file")]
	lut_file: Item<Resource>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	let lut_file = lut_file.into_element();
	if lut_file.is_empty() {
		return Ok(image);
	}

	// Parsed here to report a file that isn't a lookup table, then the kernel reuses the cached result
	parse_cached(&lut_file).map_err(NodeError::new)?;

	adjust(image, Adjustment::ColorLookup { lut_file }, pipeline)
}

mod _graphene_hash_impls {
	use super::{AdjustmentChannel, HueSaturationRange, RedGreenBlue, RedGreenBlueAlpha, TonalRange};
	graphene_hash::impl_via_hash!(RedGreenBlue, RedGreenBlueAlpha, AdjustmentChannel, TonalRange, HueSaturationRange);
}

#[cfg(test)]
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
		let result = brightness_contrast(
			(),
			Item::new_from_element(pixel),
			brightness.into(),
			contrast.into(),
			use_classic.into(),
			127_f64.into(),
			Item::default(),
		);
		result.unwrap().into_element().to_gamma_srgb_channels()[0] * 255.
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
			Item::default(),
		);
		let [r, g, _, _] = result.unwrap().into_element().to_gamma_srgb_channels();
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
			Item::default(),
		)
		.unwrap();
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
			Item::default(),
		);
		let [r, g, b, _] = result.unwrap().into_element().to_gamma_srgb_channels();
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

		let inverted = invert((), Item::new_from_element(color), Item::default()).unwrap().into_element();

		let [r, g, b, a] = inverted.to_gamma_srgb_channels();
		assert!((r - 0.).abs() < 1e-5 && (g - 0.75).abs() < 1e-5 && (b - 1.).abs() < 1e-5, "inverted channels were {r} {g} {b}");
		assert!((a - 0.5).abs() < 1e-5, "alpha was {a}");
	}

	/// Whether one gamma-space RGB value (0..255) ends up white at the given threshold level (0..255).
	fn threshold_is_white(input: [f32; 3], level: f64) -> bool {
		let pixel = Color::from_gamma_srgb_channels(input[0] / 255., input[1] / 255., input[2] / 255., 1.);
		let result = threshold((), Item::new_from_element(pixel), (level / 255. * 100.).into(), 100_f64.into(), Item::default()).unwrap();
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
		let result = vibrance((), Item::new_from_element(pixel), vibrance_amount.into(), saturation.into(), Item::default());
		let [r, g, b, _] = result.unwrap().into_element().to_gamma_srgb_channels();
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
			SelectiveColorChoice::Reds.into(), Item::default(),
		);
		let [r, g, b, _] = result.unwrap().into_element().to_gamma_srgb_channels();
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

	#[test]
	fn selective_color_survives_a_channel_that_is_not_a_number() {
		let mut groups = [[0.; 4]; 9];
		groups[7] = [10., 0., 0., 0.];
		for mode in [RelativeAbsolute::Relative, RelativeAbsolute::Absolute] {
			run_selective_color([f32::NAN, 100., 130.], mode, groups);
		}
	}

	/// Runs Posterize on one gamma-space gray value (0..255) and returns the gamma-space result on the same scale.
	fn run_posterize(value: f32, levels: i64) -> f32 {
		let pixel = Color::from_gamma_srgb_channels(value / 255., value / 255., value / 255., 1.);
		posterize((), Item::new_from_element(pixel), levels.into(), Item::default())
			.unwrap()
			.into_element()
			.to_gamma_srgb_channels()[0]
			* 255.
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
		let result = super::exposure((), Item::new_from_element(pixel), exposure.into(), offset.into(), gamma_correction.into(), Item::default());
		result.unwrap().into_element().to_gamma_srgb_channels()[0] * 255.
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
			Item::default(),
		);
		let [r, g, b, _] = result.unwrap().into_element().to_gamma_srgb_channels();
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
		let result = photo_filter((), Item::new_from_element(pixel), filter.into(), density.into(), preserve_luminosity.into(), Item::default()).unwrap();
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

	#[test]
	fn graphics_stamp_their_adjustments_while_colors_change_at_once() {
		use core_types::list::ATTR_EFFECTS;
		use graphic_types::{EffectKind, Effects};

		let color = Color::from_rgbaf32_unchecked(0.2, 0.4, 0.6, 1.);
		let inverted = invert((), Item::new_from_element(color), Item::default()).unwrap();
		assert!(inverted.attribute::<Effects>(ATTR_EFFECTS).is_none(), "a color has nothing to read a stamp");
		assert_ne!(*inverted.element(), color);

		let vector = posterize((), invert((), Item::new_from_element(Vector::default()), Item::default()).unwrap(), 3_i64.into(), Item::default()).unwrap();
		let effects = vector.attribute::<Effects>(ATTR_EFFECTS).expect("the vector carries its adjustments");
		assert_eq!(effects.effects().map(|effect| effect.kind()).collect::<Vec<_>>(), [EffectKind::Invert, EffectKind::Posterize]);
		assert_eq!(effects.effects().nth(1).and_then(|row| row.0.attribute::<i64>("levels")), Some(&3));
	}

	#[test]
	fn posterizing_a_gradient_bands_its_ramp() {
		use vector_types::GradientSettings;

		// Adjusting only the two end stops would leave a smooth ramp from black to white
		let result = posterize((), Item::new_from_element(Gradient::from(vec![Color::BLACK, Color::WHITE])), 2_i64.into(), Item::default()).unwrap();
		let settings = GradientSettings::from_item_attributes(&result);

		for t in [0.05, 0.2] {
			assert_eq!(result.element().evaluate(t, settings), Color::BLACK, "dark band at {t}");
		}
		for t in [0.8, 0.95] {
			assert_eq!(result.element().evaluate(t, settings), Color::WHITE, "light band at {t}");
		}
	}
}
