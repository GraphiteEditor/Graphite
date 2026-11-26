//! The definitions shared by every renderer for masking content by another graphic.

use core_types::graphene_hash::CacheHash;
use dyn_any::DynAny;

/// Which part of a graphic masks the content.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash, CacheHash, DynAny, node_macro::ChoiceType)]
#[widget(Radio)]
pub enum MaskMode {
	/// White shows the content and black hides it, with transparent areas also hiding it.
	#[default]
	Luminance,
	/// Opaque areas show the content and transparent areas hide it, whatever their colors.
	Alpha,
}

/// The weights of the red, green, and blue channels in a mask's luminance, those of SVG's `luminanceToAlpha`, taken on gamma-encoded channels as an SVG mask does.
pub const MASK_LUMINANCE_WEIGHTS: [f32; 3] = [0.2125, 0.7154, 0.0721];

/// How much a pixel of the mask graphic, given as premultiplied gamma-encoded channels, shows the content, from 0 to 1.
/// The luminance of premultiplied channels is that of the straight color times its alpha, so transparent areas hide the content in either mode.
pub fn mask_coverage([red, green, blue, alpha]: [f32; 4], mode: MaskMode) -> f32 {
	let [red_weight, green_weight, blue_weight] = MASK_LUMINANCE_WEIGHTS;
	let coverage = match mode {
		MaskMode::Luminance => red * red_weight + green * green_weight + blue * blue_weight,
		MaskMode::Alpha => alpha,
	};
	coverage.clamp(0., 1.)
}

/// The slope and intercept of the line mapping a mask's feathered coverage to how much it shows the content, flipped if `invert`.
/// For PSD interop, the `density` percentage scales how much the mask hides, so at 50% the content shows at half strength where the mask hides it fully.
pub fn mask_density_line(density: f64, invert: bool) -> (f32, f32) {
	let density = (density / 100.).clamp(0., 1.) as f32;
	match invert {
		false => (density, 1. - density),
		true => (-density, 1.),
	}
}

/// The standard deviation, in document pixels, of the Gaussian blur a mask's `feather` softens it by.
/// For PSD interop, this is the feather radius, within 4 levels on hard edges of PSD's own kernel, which approximates the same Gaussian.
pub fn mask_feather_sigma(feather: f64) -> f64 {
	feather.max(0.)
}
