//! The kernels of the passes that carry out an item's effects on the pixels of its content.
//! They work on images of premultiplied gamma-encoded channels, as Vello GPU blends, and on coverage held in every channel.

use crate::{RasterKernel, Wgsl, gpu_params};
use core_types::Color;

/// A color as the premultiplied, gamma-encoded channels the passes work with, carried in a [`Color`] to be passed to them.
pub fn premultiplied_gamma_channels(color: Color) -> Color {
	let [red, green, blue, alpha] = color.to_gamma_srgb_channels();
	Color::from_rgbaf32_unchecked(red * alpha, green * alpha, blue * alpha, alpha)
}

gpu_params! {
	/// The straight-alpha linear-light content, placed where its input lies, as the channels the passes work with, multiplied by `amount`.
	#[derive(Clone)]
	pub struct EffectEncode {
		pub amount: f32,
	}
}

impl RasterKernel for EffectEncode {
	const INPUTS: usize = 1;
	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/effect_encode.wgsl"))
	}
}

gpu_params! {
	/// The channels the passes work with, back into straight-alpha linear-light colors.
	#[derive(Clone)]
	pub struct EffectDecode {}
}

impl RasterKernel for EffectDecode {
	const INPUTS: usize = 1;
	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/effect_decode.wgsl"))
	}
}

gpu_params! {
	/// The coverage of the content's silhouette, or of the area outside it if `inverse`, cast by its input's placement.
	#[derive(Clone)]
	pub struct CastSilhouette {
		pub inverse: bool,
	}
}

impl RasterKernel for CastSilhouette {
	const INPUTS: usize = 1;
	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/effect_cast_silhouette.wgsl"))
	}
}

gpu_params! {
	/// The most coverage of its three inputs, the same coverage placed at the pixel and a step to either side along an axis.
	#[derive(Clone)]
	pub struct Dilate {}
}

impl RasterKernel for Dilate {
	const INPUTS: usize = 3;
	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/effect_dilate.wgsl"))
	}
}

gpu_params! {
	/// A silhouette's coverage in `color`, given as [`premultiplied_gamma_channels`].
	#[derive(Clone)]
	pub struct Tint {
		pub color: Color,
	}
}

impl RasterKernel for Tint {
	const INPUTS: usize = 1;
	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/effect_tint.wgsl"))
	}
}

gpu_params! {
	/// Stacks a silhouette's coverage, tinted `color` (given as [`premultiplied_gamma_channels`]), over the shadows and glows beneath the content.
	#[derive(Clone)]
	pub struct Accumulate {
		pub color: Color,
	}
}

impl RasterKernel for Accumulate {
	const INPUTS: usize = 2;
	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/effect_accumulate.wgsl"))
	}
}

gpu_params! {
	/// The image with a silhouette's coverage, tinted `color` (given as [`premultiplied_gamma_channels`]), placed over it within the full-strength content.
	#[derive(Clone)]
	pub struct CompositeWithin {
		pub color: Color,
	}
}

impl RasterKernel for CompositeWithin {
	const INPUTS: usize = 3;
	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/effect_composite_within.wgsl"))
	}
}

gpu_params! {
	/// The image with the shadows and glows beneath it, knocked out by the full-strength content.
	#[derive(Clone)]
	pub struct CompositeBeneath {}
}

impl RasterKernel for CompositeBeneath {
	const INPUTS: usize = 3;
	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/effect_composite_beneath.wgsl"))
	}
}

gpu_params! {
	/// The coverage of everything a coverage doesn't cover.
	#[derive(Clone)]
	pub struct InvertCoverage {}
}

impl RasterKernel for InvertCoverage {
	const INPUTS: usize = 1;
	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/effect_invert_coverage.wgsl"))
	}
}

gpu_params! {
	/// The coverage of satin's shading, from the two casts of the silhouette.
	#[derive(Clone)]
	pub struct SatinShading {
		pub invert: bool,
	}
}

impl RasterKernel for SatinShading {
	const INPUTS: usize = 2;
	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/effect_satin.wgsl"))
	}
}

gpu_params! {
	/// Starts a jump flood from the texels inside the content's silhouette, or outside it if `inward`.
	#[derive(Clone)]
	pub struct JumpFloodSeed {
		pub inward: bool,
	}
}

impl RasterKernel for JumpFloodSeed {
	const INPUTS: usize = 1;
	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/effect_jump_flood_seed.wgsl"))
	}
}

gpu_params! {
	/// One step of a jump flood, spreading the nearest seeds found so far by `step` texels.
	#[derive(Clone)]
	pub struct JumpFloodStep {
		pub step: i32,
	}
}

impl RasterKernel for JumpFloodStep {
	const INPUTS: usize = 1;
	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/effect_jump_flood_step.wgsl"))
	}
}

gpu_params! {
	/// The coverage of the band `width` texels wide along the silhouette's edge, from a finished jump flood.
	#[derive(Clone)]
	pub struct EdgeBand {
		pub width: f32,
	}
}

impl RasterKernel for EdgeBand {
	const INPUTS: usize = 1;
	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/effect_edge_band.wgsl"))
	}
}

gpu_params! {
	/// A bevel's height from 0 to 1, from the two finished jump floods across the silhouette's edge. `style` is a [`graphic_types::BevelStyle`].
	#[derive(Clone)]
	pub struct BevelHeight {
		pub size: f32,
		pub style: u32,
	}
}

impl RasterKernel for BevelHeight {
	const INPUTS: usize = 2;
	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/effect_bevel_height.wgsl"))
	}
}

gpu_params! {
	/// The coverage of a bevel's highlight, or if `shadow` of its shadow, from its height lit from the direction of the unit vector `light`.
	#[derive(Clone)]
	pub struct BevelShading {
		pub slope: f32,
		pub light_x: f32,
		pub light_y: f32,
		pub light_z: f32,
		pub shadow: bool,
	}
}

impl RasterKernel for BevelShading {
	const INPUTS: usize = 1;
	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/effect_bevel_shading.wgsl"))
	}
}
