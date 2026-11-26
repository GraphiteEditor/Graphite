//! The definitions shared by every renderer for masking content by another graphic and feathering it toward its edges.

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

/// The alpha at or below which a feather counts content as absent, half an 8-bit level, so the faint tail of a blur doesn't count as content, though an 8-bit image's faintest alpha does.
pub const FEATHER_FOOTPRINT_ALPHA: f32 = 0.5 / 255.;

/// How far a feather of `distance` erodes the content's footprint, and the standard deviation of the Gaussian softening the eroded footprint, in the same units.
///
/// A feather multiplies the content by a strength made this way. The footprint is where the content's alpha exceeds [`FEATHER_FOOTPRINT_ALPHA`].
/// Its edge falls within the pixels bordering the absent area by their alpha relative to the largest beside them, so translucent content places it as opaque content does.
/// It is eroded by half the distance, measured in straight lines, then blurred by a Gaussian reaching three standard deviations, which end at the footprint's edge.
/// Across a straight edge, the strength rises along the normal distribution's cumulative curve from nothing at the edge, through half at half the distance, to full at the distance. Parts thinner than the distance fade away entirely.
/// A gap between parts counts as an edge only once it swallows a whole pixel, so gaps one to two pixels wide feather or not depending on where they fall on the pixel grid.
pub fn feather_parts(distance: f64) -> (f64, f64) {
	let distance = distance.max(0.);
	(distance / 2., distance / 6.)
}
