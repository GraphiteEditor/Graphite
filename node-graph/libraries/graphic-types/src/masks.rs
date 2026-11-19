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

/// The most levels a masked radius blur interpolates between down to a document pixel, enough to halve from a radius of 1000 document pixels to one.
pub const MASKED_RADIUS_BLUR_MAX_LEVELS: usize = 12;

/// The most levels finer than a document pixel that a masked radius blur adds when zoomed in, enough to reach an output pixel at a zoom of 25600%.
pub const MASKED_RADIUS_BLUR_MAX_FINER_LEVELS: usize = 8;

/// The radii, in document pixels, of the blurs a masked radius blur of `radius` interpolates between: none, then the radius halved while it spans a document pixel, or an output pixel at more `pixels_per_length` than one.
/// Each is a Gaussian's standard deviation or a box's half width. Levels a factor of 2 apart interpolate into what reads as one blur, and under a pixel a blur hardly differs from none.
pub fn masked_radius_blur_levels(radius: f64, pixels_per_length: f64) -> Vec<f64> {
	let finest = 1. / pixels_per_length.max(1.);

	let (mut blurred, mut finer) = (vec![radius.max(0.)], 0);
	loop {
		let next = blurred[0] / 2.;
		let coarse = next >= 1. && blurred.len() + 1 < MASKED_RADIUS_BLUR_MAX_LEVELS;
		let fine = (finest..1.).contains(&next) && finer < MASKED_RADIUS_BLUR_MAX_FINER_LEVELS;
		if !(coarse || fine) {
			break;
		}

		finer += fine as usize;
		blurred.insert(0, next);
	}
	std::iter::once(0.).chain(blurred).collect()
}

/// The 8-bit code of a masked radius blur map's luminance at a pixel, given as premultiplied gamma-encoded channels, which picks the pixel's interpolation between blurs.
pub fn masked_radius_blur_code(channels: [f32; 4]) -> u8 {
	(mask_coverage(channels, MaskMode::Luminance) * 255.).round() as u8
}

/// For a masked radius blur map's luminance as an 8-bit `code`, the level below it among the `levels` (from [`masked_radius_blur_levels`]) and how many of 255 parts of the interpolation the level above takes, the rest going to the level below.
///
/// Each pixel of a masked radius blur is the sum of its levels (its content blurred by Gaussians or boxes of their radii) weighed by their parts, on premultiplied gamma-encoded channels.
/// The map value's fraction of the full radius falls between two levels. The parts are linear in the radius from no blur to the first level, and linear in its logarithm between blurred levels.
/// On opaque hard edges, between Gaussian levels a factor of 2 apart, that strays at most about 20 of 255 sRGB levels from a true Gaussian of the map value's radius, against about 32 for the sum in linear light.
/// Summing in gamma, even when the levels blur in linear light, is deliberate: a power space near 2.7 strays only about 12, but SVG would need a 256-entry table per level for it, where gamma costs nothing.
pub fn masked_radius_blur_interpolation(code: u8, levels: &[f64]) -> (usize, u32) {
	let Some(&radius) = levels.last().filter(|_| levels.len() >= 2) else { return (0, 0) };
	let length = code as f64 / 255. * radius;
	let below = (0..levels.len() - 1).rev().find(|&level| length >= levels[level]).unwrap_or(0);
	let (low, high) = (levels[below], levels[below + 1]);

	let share = match low > 0. {
		true => (length / low).log2() / (high / low).log2(),
		false => length / high,
	};
	(below, (share * 255.).round().clamp(0., 255.) as u32)
}

/// How many of 255 parts of a masked radius blur's interpolation the `level` takes for the map luminance `code`, as [`masked_radius_blur_interpolation`] gives them.
pub fn masked_radius_blur_weight(level: usize, code: u8, levels: &[f64]) -> u32 {
	let (below, above_parts) = masked_radius_blur_interpolation(code, levels);
	match level {
		_ if level == below => 255 - above_parts,
		_ if level == below + 1 => above_parts,
		_ => 0,
	}
}
