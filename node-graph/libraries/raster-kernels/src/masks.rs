//! The CPU twins of the GPU passes that mask, feather, and blur content by a radius mask.

use crate::effect_kernels::{BlurKernel, NEGLIGIBLE_LENGTH, Plane};
use crate::raster_effects::{blur, premultiplied_gamma_channels};
use core_types::Color;
use glam::DVec2;
use graphic_types::masks::{FEATHER_FOOTPRINT_ALPHA, MaskMode, feather_parts, mask_coverage, mask_density_line, mask_feather_sigma, masked_radius_blur_code, masked_radius_blur_interpolation};
use raster_types::parallel_rows::fill_rows;

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

/// How much a footprint eroded by `erosion` pixels covers a pixel, given the pixel's distance to the area outside the footprint plus half a pixel and its share of that area, relative to its own coverage of the footprint.
fn erode(to_outside: f32, outside_coverage: f32, erosion: f32) -> f32 {
	let coverage = 1. - outside_coverage;
	let eroded = (to_outside - erosion).clamp(0., 1.);
	if coverage > 0. { (eroded / coverage).min(1.) } else { eroded }
}

/// How much a feather of `distance` pixels keeps of the `full`-strength content at each pixel, from 0 to 1, as [`feather_parts`] defines it, matching the GPU's `feather_strength`.
/// O(width × height × log(distance)).
pub(crate) fn feather_strength(full: &Plane<[f32; 4]>, distance: f64) -> Plane<f32> {
	let (width, height) = (full.width, full.height);
	let alpha_at = |x: usize, y: usize| full.data[y * width + x][3];

	// The area outside the footprint, with its edge falling within the pixels bordering it by their alpha relative to the largest beside them
	let mut outside = Plane::<f32>::new(width, height);
	fill_rows(&mut outside.data, width, |y, row| {
		for (x, outside_coverage) in row.iter_mut().enumerate() {
			let alpha = alpha_at(x, y);
			let neighbors = (y.saturating_sub(1)..(y + 2).min(height)).flat_map(|neighbor_y| (x.saturating_sub(1)..(x + 2).min(width)).map(move |neighbor_x| (neighbor_x, neighbor_y)));
			let neighbor_alphas = neighbors.map(|(neighbor_x, neighbor_y)| alpha_at(neighbor_x, neighbor_y));
			let (borders_absence, largest) = neighbor_alphas.fold((false, alpha), |(borders, largest), neighbor| (borders || neighbor <= FEATHER_FOOTPRINT_ALPHA, largest.max(neighbor)));

			*outside_coverage = match (alpha <= FEATHER_FOOTPRINT_ALPHA, borders_absence) {
				(true, _) => 1.,
				(false, true) => 1. - (alpha / largest).min(1.),
				(false, false) => 0.,
			};
		}
	});

	// The footprint eroded in straight lines, from each pixel's distance to the edge plus half a pixel, relative to the pixel's own coverage of the footprint so a feather of nothing keeps the content as it is
	let (erosion, sigma) = feather_parts(distance);
	let to_outside = outside.jump_flood(erosion + 2., |coverage| coverage > 0., |[x, y], coverage| (x as f32).hypot(y as f32) + (0.5 - coverage) + 0.5);
	let eroded = to_outside.zip(&outside, |to_outside, outside_coverage| erode(to_outside, outside_coverage, erosion as f32));

	match sigma >= NEGLIGIBLE_LENGTH {
		true => eroded.blurred(BlurKernel::Gaussian { sigma }, BlurKernel::Gaussian { sigma }),
		false => eroded,
	}
}

/// The 8-bit luminance code of each of a masked radius blur map's `colors`, taken from 8-bit channels as the GPU renders the map: straight, then premultiplied.
pub(crate) fn masked_radius_blur_codes(colors: &[Color]) -> Vec<u8> {
	let eight_bit = |channel: f32| (channel.clamp(0., 1.) * 255.).round() / 255.;
	colors
		.iter()
		.map(|&color| {
			let [red, green, blue, alpha] = color.to_gamma_srgb_channels().map(eight_bit);
			masked_radius_blur_code([eight_bit(red * alpha), eight_bit(green * alpha), eight_bit(blue * alpha), alpha])
		})
		.collect()
}

/// The premultiplied gamma-encoded `image` blurred by the interpolation between `levels` (from `masked_radius_blur_levels`, here in pixels along each axis) each pixel's map luminance `codes` picks, matching the GPU's `masked_radius_blur`.
/// The levels are boxes if `box_blur` and Gaussians otherwise, blurring in linear light if `linear`. O(width × height) for each of the levels.
pub(crate) fn masked_radius_blur(image: &Plane<[f32; 4]>, codes: &[u8], levels: &[DVec2], box_blur: bool, linear: bool) -> Plane<[f32; 4]> {
	// Each code's level below and share of the level above, the same for every level and at any scale
	let lengths: Vec<f64> = levels.iter().map(|level| level.max_element()).collect();
	let interpolations: Vec<(usize, f32)> = (0..=255_u8)
		.map(|code| {
			let (below, above_parts) = masked_radius_blur_interpolation(code, &lengths);
			(below, above_parts as f32 / 255.)
		})
		.collect();

	// Only the levels some pixel takes a share of are blurred, so a map of one luminance blurs once or twice
	let mut weighed = vec![false; levels.len()];
	for &code in codes {
		let (below, above_share) = interpolations[code as usize];
		weighed[below] |= above_share < 1.;
		if let Some(above) = weighed.get_mut(below + 1) {
			*above |= above_share > 0.;
		}
	}

	let mut sum = Plane::<[f32; 4]>::new(image.width, image.height);
	let kernel = |length: f64| match box_blur {
		true => BlurKernel::Box { radius: length },
		false => BlurKernel::Gaussian { sigma: length },
	};
	for (index, &length) in levels.iter().enumerate().filter(|&(index, _)| weighed[index]) {
		let blurred = match length.max_element() >= NEGLIGIBLE_LENGTH {
			true => blur(image.clone(), (kernel(length.x), kernel(length.y)), linear),
			false => image.clone(),
		};

		let weight = |(below, above_share): (usize, f32)| match index {
			_ if index == below => 1. - above_share,
			_ if index == below + 1 => above_share,
			_ => 0.,
		};
		// Summed in gamma on purpose (see `masked_radius_blur_interpolation`)
		for ((sum, pixel), &code) in sum.data.iter_mut().zip(&blurred.data).zip(codes) {
			let weight = weight(interpolations[code as usize]);
			*sum = std::array::from_fn(|channel| sum[channel] + pixel[channel] * weight);
		}
	}
	sum
}

#[cfg(test)]
mod tests {
	use super::*;

	/// The standard normal cumulative distribution, from the Abramowitz and Stegun approximation of the error function, accurate to 1e-7.
	fn normal_cdf(x: f64) -> f64 {
		let z = x.abs() / std::f64::consts::SQRT_2;
		let t = 1. / (1. + 0.327_591_1 * z);
		let polynomial = t * (0.254_829_592 + t * (-0.284_496_736 + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
		let erf = 1. - polynomial * (-z * z).exp();
		if x >= 0. { 0.5 * (1. + erf) } else { 0.5 * (1. - erf) }
	}

	/// A plane of white content with the alpha `coverage` gives at each pixel's center.
	fn content(size: usize, coverage: impl Fn(f64, f64) -> f32) -> Plane<[f32; 4]> {
		let mut plane = Plane::<[f32; 4]>::new(size, size);
		for (index, pixel) in plane.data.iter_mut().enumerate() {
			let alpha = coverage((index % size) as f64 + 0.5, (index / size) as f64 + 0.5);
			*pixel = [alpha; 4];
		}
		plane
	}

	/// The feather's strength across a straight edge, as the definition gives it for a pixel center at `depth` within the edge.
	fn expected(depth: f64, distance: f64) -> f64 {
		normal_cdf((depth - distance / 2.) / (distance / 6.))
	}

	#[test]
	fn feathers_follow_the_definition_across_straight_and_round_edges() {
		let distance = 16.;

		// A square from 8 to 56, measured across its left edge on its middle row
		let square = content(64, |x, y| if (8. ..56.).contains(&x) && (8. ..56.).contains(&y) { 1. } else { 0. });
		let strength = feather_strength(&square, distance);
		for x in 8..30 {
			let depth = x as f64 + 0.5 - 8.;
			let actual = strength.data[32 * 64 + x] as f64;
			assert!((actual - expected(depth, distance)).abs() < 0.03, "at depth {depth} the square keeps {actual}");
		}

		// A disk of radius 28 with an antialiased edge, measured along a diagonal, where distances by chamfer steps would stray from straight lines
		let disk = content(80, |x, y| (28.5 - (x - 40.).hypot(y - 40.)).clamp(0., 1.) as f32);
		let strength = feather_strength(&disk, distance);
		for step in 0..14 {
			let (x, y) = (40 + 19 - step, 40 + 19 - step);
			let depth = 28. - (x as f64 + 0.5 - 40.).hypot(y as f64 + 0.5 - 40.);
			let actual = strength.data[y * 80 + x] as f64;
			assert!((actual - expected(depth, distance)).abs() < 0.04, "at depth {depth} on the diagonal the disk keeps {actual}");
		}
	}

	#[test]
	fn feathers_fade_thin_parts_and_hole_edges_and_keep_translucent_edges_where_opaque_ones_are() {
		let distance = 12.;

		// A strip thinner than the distance fades away entirely
		let strip = content(48, |x, _| if (20. ..26.).contains(&x) { 1. } else { 0. });
		let strength = feather_strength(&strip, distance);
		assert!(strength.data[24 * 48 + 23] < 0.01, "the strip's middle keeps {}", strength.data[24 * 48 + 23]);

		// A hole's edge fades like the outline's
		let holed = content(64, |x, y| if (x - 32.).hypot(y - 32.) < 4. { 0. } else { 1. });
		let strength = feather_strength(&holed, distance);
		assert!(strength.data[32 * 64 + 36] < 0.05, "beside the hole the content keeps {}", strength.data[32 * 64 + 36]);

		// Content at 40% places its edge as opaque content does
		let opaque = feather_strength(&content(48, |x, _| if x >= 16. { 1. } else { 0. }), distance);
		let translucent = feather_strength(&content(48, |x, _| if x >= 16. { 0.4 } else { 0. }), distance);
		for x in 16..32 {
			assert!(
				(opaque.data[24 * 48 + x] - translucent.data[24 * 48 + x]).abs() < 1e-4,
				"at {x} the translucent content is feathered differently"
			);
		}
	}

	#[test]
	fn a_feather_of_almost_nothing_keeps_the_content_as_it_is() {
		let disk = content(32, |x, y| (10.5 - (x - 16.).hypot(y - 16.)).clamp(0., 1.) as f32);
		let strength = feather_strength(&disk, 0.01);
		for (pixel, strength) in disk.data.iter().zip(&strength.data) {
			if pixel[3] > 0. {
				assert!(*strength > 0.98, "a pixel of alpha {} keeps only {strength}", pixel[3]);
			}
		}
	}
}
