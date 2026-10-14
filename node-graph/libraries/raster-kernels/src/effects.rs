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
