//! The CPU twins of the GPU passes that mask content by another graphic.

use crate::effect_kernels::{BlurKernel, NEGLIGIBLE_LENGTH, Plane};
use crate::raster_effects::premultiplied_gamma_channels;
use core_types::Color;
use glam::DVec2;
use graphic_types::masks::{MaskMode, mask_coverage, mask_density_line, mask_feather_sigma};

/// How much the mask shows the content at each pixel, from 0 to 1, given the `colors` its graphic draws over a plane of `width` by `height` pixels.
/// Its coverage is softened by the `feather` (in document pixels, `pixels_per_length` pixels each), then mapped through its `density` percentage, flipped if `invert`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn mask_strength(colors: &[Color], width: usize, height: usize, mode: MaskMode, density: f64, feather: f64, invert: bool, pixels_per_length: DVec2) -> Plane<f32> {
	let coverage = Plane {
		width,
		height,
		data: colors.iter().map(|&color| mask_coverage(premultiplied_gamma_channels(color), mode)).collect(),
	};

	let sigma = mask_feather_sigma(feather) * pixels_per_length;
	let coverage = match sigma.max_element() >= NEGLIGIBLE_LENGTH {
		true => coverage.blurred(BlurKernel::Gaussian { sigma: sigma.x }, BlurKernel::Gaussian { sigma: sigma.y }),
		false => coverage,
	};

	let (slope, intercept) = mask_density_line(density, invert);
	coverage.map(|coverage| (slope * coverage + intercept).clamp(0., 1.))
}

/// The premultiplied image faded at each pixel by the mask's `strength` there.
pub(crate) fn masked(image: &Plane<[f32; 4]>, strength: &Plane<f32>) -> Plane<[f32; 4]> {
	image.zip(strength, |pixel, strength| pixel.map(|channel| channel * strength))
}
