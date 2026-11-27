//! The CPU twins of the GPU passes that mask, feather, and blur content by a radius mask.

use crate::effect_kernels::{BlurKernel, NEGLIGIBLE_LENGTH, Plane, VARIABLE_BOX_MAX_VALUE, variable_box_scale};
use crate::raster_effects::{blur, premultiplied_gamma_channels, transferred};
use core_types::Color;
use core_types::color::{linear_to_srgb, srgb_to_linear};
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

/// A masked radius blur map's color in 8-bit channels as the GPU renders the map: straight, then premultiplied.
fn map_channels(color: Color) -> [f32; 4] {
	let eight_bit = |channel: f32| (channel.clamp(0., 1.) * 255.).round() / 255.;
	let [red, green, blue, alpha] = color.to_gamma_srgb_channels().map(eight_bit);
	[eight_bit(red * alpha), eight_bit(green * alpha), eight_bit(blue * alpha), alpha]
}

/// The 8-bit luminance code of each of a masked radius blur map's `colors` (see [`map_channels`]).
pub(crate) fn masked_radius_blur_codes(colors: &[Color]) -> Vec<u8> {
	colors.iter().map(|&color| masked_radius_blur_code(map_channels(color))).collect()
}

/// The luminance from 0 to 1 of each of a masked box blur map's `colors`, unrounded like the GPU's precise map texture, in the half floats the GPU keeps it in, which size the boxes finer than 8-bit codes could.
pub(crate) fn masked_box_blur_luminances(colors: &[Color]) -> Vec<f32> {
	let luminance = |color: Color| {
		let [red, green, blue, alpha] = color.to_gamma_srgb_channels();
		mask_coverage([red * alpha, green * alpha, blue * alpha, alpha], MaskMode::Luminance).clamp(0., 1.)
	};
	colors.iter().map(|&color| half::f16::from_f32(luminance(color)).to_f32()).collect()
}

/// The premultiplied gamma-encoded `image` blurred by the interpolation between Gaussian `levels` (from `masked_radius_blur_levels`, here in pixels along each axis) each pixel's map luminance `codes` picks,
/// matching the GPU's `masked_radius_blur`, blurring in linear light if `linear`. O(width × height) for each of the levels.
pub(crate) fn masked_radius_blur(image: &Plane<[f32; 4]>, codes: &[u8], levels: &[DVec2], linear: bool) -> Plane<[f32; 4]> {
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
	for (index, &length) in levels.iter().enumerate().filter(|&(index, _)| weighed[index]) {
		let blurred = match length.max_element() >= NEGLIGIBLE_LENGTH {
			true => blur(image.clone(), (BlurKernel::Gaussian { sigma: length.x }, BlurKernel::Gaussian { sigma: length.y }), linear),
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

/// The premultiplied gamma-encoded `image` box blurred at each pixel by its own half width along each axis, the fraction of the `radius` in pixels its map's `luminances` give,
/// matching the GPU's `variable_box_blur`. Each box weighs pixels like [`BlurKernel::Box`], holding the edge pixels past the edges, in linear light if `linear`.
/// It sums each box from running sums (a summed-area table) of whole-number counts, exact at any size, so O(width × height) at any radius.
pub fn variable_box_blur(image: &Plane<[f32; 4]>, luminances: &[f32], radius: DVec2, linear: bool) -> Plane<[f32; 4]> {
	let (width, height) = (image.width, image.height);
	let summed = match linear {
		true => std::borrow::Cow::Owned(image.map(transferred(srgb_to_linear))),
		false => std::borrow::Cow::Borrowed(image),
	};
	let scale = variable_box_scale(width * height);
	let count = |value: f32| (value.clamp(0., VARIABLE_BOX_MAX_VALUE) * scale as f32).round_ties_even() as u64;

	// Two channels at a time, as the GPU sums them, keeping the running sums to 16 bytes a pixel
	let mut averages = Plane::<[f32; 4]>::new(width, height);
	for first in [0, 2] {
		// Each channel counted in whole steps, summed over the pixels above and left of each corner between pixels, in rows of one more corner than pixels
		let corners = width + 1;
		let mut sums = vec![[0_u64; 2]; corners * (height + 1)];
		for y in 0..height {
			let mut row = [0_u64; 2];
			for x in 0..width {
				let pixel = summed.data[y * width + x];
				row = [row[0] + count(pixel[first]), row[1] + count(pixel[first + 1])];

				let above = sums[y * corners + x + 1];
				sums[(y + 1) * corners + x + 1] = [above[0] + row[0], above[1] + row[1]];
			}
		}
		let sum_at = |x: usize, y: usize| sums[y * corners + x];

		// The counts over the pixels from `start` up to `end`, with those past the edges repeating the nearest edge pixel
		let counts_continued = |start: [i64; 2], end: [i64; 2]| -> [f64; 2] {
			let spans = |axis: usize, size: usize| {
				let size = size as i64;
				let (inner_start, inner_end) = (start[axis].clamp(0, size), end[axis].clamp(0, size));
				[(0, 1, inner_start - start[axis]), (inner_start, inner_end, 1), (size - 1, size, end[axis] - inner_end)]
			};

			let mut sum = [0.; 2];
			for (start_x, end_x, count_x) in spans(0, width) {
				for (start_y, end_y, count_y) in spans(1, height) {
					if count_x <= 0 || count_y <= 0 {
						continue;
					}

					let [start_x, end_x, start_y, end_y] = [start_x, end_x, start_y, end_y].map(|corner| corner as usize);
					let [top_left, top_right] = [sum_at(start_x, start_y), sum_at(end_x, start_y)];
					let [bottom_left, bottom_right] = [sum_at(start_x, end_y), sum_at(end_x, end_y)];
					let count = (count_x * count_y) as f64;
					sum = std::array::from_fn(|channel| sum[channel] + count * ((bottom_right[channel] + top_left[channel]) - (bottom_left[channel] + top_right[channel])) as f64);
				}
			}
			sum
		};

		fill_rows(&mut averages.data, width, |y, row| {
			for (x, target) in row.iter_mut().enumerate() {
				let index = y * width + x;
				if luminances[index] <= 0. {
					continue;
				}

				// A box of a fractional half width is the mix of the boxes of the whole half widths either side, weighed by that fraction along each axis
				let half_width = radius * luminances[index] as f64;
				let (whole, fraction) = (half_width.floor(), half_width - half_width.floor());
				let mut sum = [0.; 2];
				for wider in [[false, false], [true, false], [false, true], [true, true]] {
					let along = |axis: usize| if wider[axis] { fraction[axis] } else { 1. - fraction[axis] };
					let weight = along(0) * along(1);
					if weight <= 0. {
						continue;
					}

					let reach = [0, 1].map(|axis| whole[axis] as i64 + wider[axis] as i64);
					let rectangle = counts_continued([x as i64 - reach[0], y as i64 - reach[1]], [x as i64 + reach[0] + 1, y as i64 + reach[1] + 1]);
					sum = std::array::from_fn(|channel| sum[channel] + weight * rectangle[channel]);
				}

				let total = (2. * half_width.x + 1.) * (2. * half_width.y + 1.) * scale;
				target[first..first + 2].copy_from_slice(&sum.map(|channel| (channel / total) as f32));
			}
		});
	}

	// Pixels not blurred keep their color, and the rest return to gamma from linear light
	let encode = transferred(linear_to_srgb);
	for ((average, &original), &luminance) in averages.data.iter_mut().zip(&image.data).zip(luminances) {
		*average = match luminance > 0. {
			false => original,
			true if linear => encode(*average),
			true => *average,
		};
	}
	averages
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::effect_kernels::box_weight;

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

	/// Premultiplied colors varying across the plane up to its edges, every seventh pixel transparent and some translucent.
	fn colorful(width: usize, height: usize) -> Plane<[f32; 4]> {
		let data = (0..width * height)
			.map(|index| {
				let (x, y) = ((index % width) as f32 / width as f32, (index / width) as f32 / height as f32);
				let alpha = [0., 0.5, 0.5, 1., 1., 1., 1.][index % 7];
				[x * alpha, y * alpha, (1. - x * y) * alpha, alpha]
			})
			.collect();
		Plane { width, height, data }
	}

	/// The box of `half_width` along each axis around a pixel of the plane, summed pixel by pixel with the edge pixels held past the edges.
	fn summed_box(plane: &Plane<[f32; 4]>, x: usize, y: usize, half_width: DVec2) -> [f32; 4] {
		let reach = half_width.ceil().as_i64vec2();
		let (mut sum, mut total) = ([0.; 4], 0.);
		for offset_y in -reach.y..=reach.y {
			for offset_x in -reach.x..=reach.x {
				let weight = box_weight(half_width.x, offset_x.unsigned_abs() as usize) * box_weight(half_width.y, offset_y.unsigned_abs() as usize);
				let source_x = (x as i64 + offset_x).clamp(0, plane.width as i64 - 1) as usize;
				let source_y = (y as i64 + offset_y).clamp(0, plane.height as i64 - 1) as usize;
				let pixel = plane.data[source_y * plane.width + source_x];

				sum = std::array::from_fn(|channel| sum[channel] + weight * pixel[channel] as f64);
				total += weight;
			}
		}
		sum.map(|channel| (channel / total) as f32)
	}

	#[test]
	fn variable_box_blurs_box_each_pixel_by_its_own_fraction_of_the_radius() {
		let image = colorful(37, 29);
		// Luminances finer than 8-bit codes, with every 41st pixel unblurred
		let luminances: Vec<f32> = (0..37 * 29).map(|index| if index % 41 == 0 { 0. } else { ((index * 37) % 1000) as f32 / 1000. }).collect();
		let radius = DVec2::new(9.3, 6.);

		for linear in [false, true] {
			let blurred = variable_box_blur(&image, &luminances, radius, linear);
			let decoded = if linear { image.map(transferred(srgb_to_linear)) } else { image.clone() };

			// Counting in 4096 steps rounds by up to half a step, which gamma encoding stretches up to 13 times near black
			let tolerance = if linear { 2e-3 } else { 2e-4 };

			for (index, &luminance) in luminances.iter().enumerate() {
				// A luminance of 0 leaves its pixel exactly as it is
				if luminance <= 0. {
					assert_eq!(blurred.data[index], image.data[index]);
					continue;
				}

				let summed = summed_box(&decoded, index % 37, index / 37, radius * luminance as f64);
				let expected = if linear { transferred(linear_to_srgb)(summed) } else { summed };
				for channel in 0..4 {
					let difference = (blurred.data[index][channel] - expected[channel]).abs();
					assert!(
						difference < tolerance,
						"pixel {index} of luminance {luminance} differs by {difference} in channel {channel} (linear: {linear})"
					);
				}
			}
		}
	}

	#[test]
	fn variable_box_blurs_of_one_fraction_of_the_radius_are_the_box_blur_of_that_half_width() {
		let image = colorful(48, 40);
		let luminances = vec![0.6; 48 * 40];
		let radius = DVec2::new(20., 12.5);
		let half_width = radius * 0.6_f32 as f64;

		for linear in [false, true] {
			let variable = variable_box_blur(&image, &luminances, radius, linear);
			let even = blur(image.clone(), (BlurKernel::Box { radius: half_width.x }, BlurKernel::Box { radius: half_width.y }), linear);
			for (index, (variable, even)) in variable.data.iter().zip(&even.data).enumerate() {
				for channel in 0..4 {
					let difference = (variable[channel] - even[channel]).abs();
					assert!(difference < 2e-4, "pixel {index} differs by {difference} in channel {channel} (linear: {linear})");
				}
			}
		}
	}
}
