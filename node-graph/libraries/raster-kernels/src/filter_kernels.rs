//! The pixel filters on the CPU, which mirror the renderer's GPU passes on premultiplied gamma-encoded channels.

use crate::effect_kernels::{BlurKernel, Plane};
use crate::generators::pcg3d;
use glam::{DAffine2, DMat2, DVec2, IVec2};
use graphic_types::effects::spans_along_axes;
use graphic_types::filters::{
	Convolution, FieldSampling, GAUSSIAN_NOISE_LIMIT, HIGH_PASS_GRAY, NoiseDistribution, RandomField, SHARPEN_NEIGHBORS, SOBEL_TAPS, Surroundings, UnsharpBlur, VISIBLE_ALPHA, average_weight,
	edge_gain, fill_shifts, neighborhood_radii, noise_extent, unsharp, unsharp_change,
};
use graphic_types::{EffectOperation, MinimumMaximumMode};
use raster_types::parallel_rows::fill_rows;

/// Carries out a filter effect on premultiplied gamma-encoded pixels, which `pixels_to_effect` places in the effect space, or returns them as they are for any other operation.
/// The pixels visible in the `full`-strength content (the plane itself for `None`) are those whose colors the filter reads.
pub fn filter(plane: Plane<[f32; 4]>, full: Option<&Plane<[f32; 4]>>, operation: &EffectOperation, pixels_to_effect: DAffine2) -> Plane<[f32; 4]> {
	let document_to_pixels = pixels_to_effect.matrix2.inverse();
	if let Some(convolution) = operation.convolution() {
		return convolve(&plane, full.unwrap_or(&plane), &convolution, document_to_pixels);
	}

	if let Some((blur, sharpening)) = unsharp(operation) {
		let sharpening = sharpening.map(|[amount, threshold]| (amount as f32, threshold as f32));
		return unsharp_mask(&plane, full.unwrap_or(&plane), blur, sharpening, document_to_pixels);
	}

	match *operation {
		EffectOperation::FindEdges => find_edges(&plane, document_to_pixels),
		EffectOperation::MinimumMaximum { radius, mode } => neighborhood_extreme(&plane, radius, mode == MinimumMaximumMode::Maximum, document_to_pixels),
		EffectOperation::AddNoise {
			amount,
			distribution,
			monochromatic,
			seed,
		} => add_noise(&plane, amount / 100., distribution, monochromatic, seed, pixels_to_effect),
		EffectOperation::Dissolve { opacity, seed } => dissolve(&plane, opacity / 100., seed, pixels_to_effect),
		_ => plane,
	}
}

/// The whole-pixel shifts, nearest first, that a faint pixel looks along for a visible one within `reach` document pixels, measured through `document_to_pixels`. O(reach² log reach).
pub fn pixel_fill_shifts(reach: f64, document_to_pixels: DMat2) -> Vec<[isize; 2]> {
	let mut seen = std::collections::HashSet::new();
	let pixel_shift = |[x, y]: [i32; 2]| (document_to_pixels * DVec2::new(x as f64, y as f64)).round();
	(fill_shifts(reach.ceil() as i32).into_iter().map(pixel_shift))
		.map(|shift| [shift.x as isize, shift.y as isize])
		.filter(|&shift| seen.insert(shift))
		.collect()
}

/// Each pixel's straight color, as the filters read it, with an alpha of 1 where there is one: its own if it's visible in the `full`-strength content, otherwise that of the first visible pixel along `shifts`,
/// or failing that its own if it has any coverage. O(width × height × shifts) for the faint pixels.
pub(crate) fn visible_colors(plane: &Plane<[f32; 4]>, full: &Plane<[f32; 4]>, shifts: &[[isize; 2]]) -> Plane<[f32; 4]> {
	let (columns, rows) = (plane.width as isize, plane.height as isize);
	let covered = |x: isize, y: isize, least_alpha: f32| {
		let index = (x >= 0 && y >= 0 && x < columns && y < rows).then_some((y * columns + x) as usize)?;
		let [red, green, blue, alpha] = plane.data[index];
		(alpha > 0. && full.data[index][3] >= least_alpha).then(|| [red / alpha, green / alpha, blue / alpha, 1.])
	};

	let mut colors = Plane::new(plane.width, plane.height);
	fill_rows(&mut colors.data, plane.width, |y, row| {
		for (x, color) in row.iter_mut().enumerate() {
			let (x, y) = (x as isize, y as isize);
			let nearest_visible = shifts.iter().find_map(|&[shift_x, shift_y]| covered(x + shift_x, y + shift_y, VISIBLE_ALPHA));
			*color = nearest_visible.or_else(|| covered(x, y, 0.)).unwrap_or_default();
		}
	});
	colors
}

/// A weighted copy of a plane shifted by a fixed fraction of a pixel, read as the four pixels around each position with their bilinear weights.
struct BilinearTap {
	/// The whole-pixel part of the shift.
	whole: [isize; 2],
	/// The bilinear weights of the pixels at the whole shift and one past it along x, then the same one past it along y.
	bilinear: [f32; 4],
	weight: f32,
}

impl BilinearTap {
	fn new(shift: DVec2, weight: f32) -> Self {
		let whole = shift.floor();
		let [x, y] = (shift - whole).as_vec2().to_array();
		Self {
			whole: [whole.x as isize, whole.y as isize],
			bilinear: [(1. - x) * (1. - y), x * (1. - y), (1. - x) * y, x * y],
			weight,
		}
	}

	/// The weighted color between the four `corners` (straight colors with an alpha of 1 where there is one), interpolated among those with a color, matching the GPU's `color_between`.
	/// Leaving out those without one continues the colors past the edges between pixels too, rather than fading toward black.
	fn weighted(&self, corners: [[f32; 4]; 4]) -> [f32; 3] {
		let (mut color, mut covered) = ([0.; 3], 0.);
		for (corner, bilinear) in corners.into_iter().zip(self.bilinear) {
			(0..3).for_each(|channel| color[channel] += corner[channel] * bilinear);
			covered += corner[3] * bilinear;
		}
		if covered > 0. { color.map(|channel| channel / covered * self.weight) } else { [0.; 3] }
	}
}

/// The opaque colors a filter reads, as its surroundings describe, with the color it reads beyond their edges.
struct SurroundedColors {
	colors: Plane<[f32; 4]>,
	beyond: [f32; 4],
}

impl SurroundedColors {
	/// The colors of the premultiplied pixels that a filter reading within `reach` document pixels sees, measured through `document_to_pixels`, those visible judged by the `full`-strength content.
	fn new(plane: &Plane<[f32; 4]>, full: &Plane<[f32; 4]>, surroundings: Surroundings, reach: f64, document_to_pixels: DMat2) -> Self {
		match surroundings {
			Surroundings::Continued => Self {
				colors: visible_colors(plane, full, &pixel_fill_shifts(reach, document_to_pixels)),
				beyond: [0.; 4],
			},
			Surroundings::Matted(gray) => {
				let gray = gray as f32;
				let over_gray = |channel: f32, alpha: f32| channel + gray * (1. - alpha);
				Self {
					colors: plane.map(|[red, green, blue, alpha]| [over_gray(red, alpha), over_gray(green, alpha), over_gray(blue, alpha), 1.]),
					beyond: [gray, gray, gray, 1.],
				}
			}
		}
	}

	/// The coverage of the premultiplied pixels in each channel, read with none beyond the plane's edges and every pixel counted when reading between them.
	fn coverage(plane: &Plane<[f32; 4]>) -> Self {
		Self {
			colors: plane.map(|[.., alpha]| [alpha, alpha, alpha, 1.]),
			beyond: [0., 0., 0., 1.],
		}
	}

	fn at(&self, x: isize, y: isize) -> [f32; 4] {
		let (columns, rows) = (self.colors.width as isize, self.colors.height as isize);
		match x >= 0 && y >= 0 && x < columns && y < rows {
			true => self.colors.data[(y * columns + x) as usize],
			false => self.beyond,
		}
	}

	/// The weighted sum of the colors that the `taps` read around a pixel.
	fn sum(&self, x: isize, y: isize, taps: &[BilinearTap]) -> [f32; 3] {
		let mut sum = [0.; 3];
		for tap in taps {
			let [shift_x, shift_y] = [x + tap.whole[0], y + tap.whole[1]];
			let corners = [
				self.at(shift_x, shift_y),
				self.at(shift_x + 1, shift_y),
				self.at(shift_x, shift_y + 1),
				self.at(shift_x + 1, shift_y + 1),
			];
			let weighted = tap.weighted(corners);
			(0..3).for_each(|channel| sum[channel] += weighted[channel]);
		}
		sum
	}
}

/// A filtered pixel as a premultiplied color: its straight `colors` given the coverage of `alpha`, or for matted surroundings, colors filtered over the gray and lifted back off it within that coverage.
fn uncovered(colors: [f32; 3], alpha: f32, surroundings: Surroundings) -> [f32; 4] {
	let [red, green, blue] = match surroundings {
		Surroundings::Continued => colors.map(|channel| channel.clamp(0., 1.) * alpha),
		Surroundings::Matted(gray) => colors.map(|channel| (channel.clamp(0., 1.) - gray as f32 * (1. - alpha)).max(0.).min(alpha)),
	};
	[red, green, blue, alpha]
}

/// The premultiplied pixels with each one's color replaced by the weighted sum `convolution` describes, with its shifts measured through `document_to_pixels`.
/// It reads the colors as the convolution's surroundings describe, those visible judged by the `full`-strength content, then divides, raises, and clamps the sum, keeping each pixel's coverage unless it weighs that too. O(width × height × taps).
pub fn convolve(plane: &Plane<[f32; 4]>, full: &Plane<[f32; 4]>, convolution: &Convolution, document_to_pixels: DMat2) -> Plane<[f32; 4]> {
	let taps: Vec<BilinearTap> = convolution
		.taps
		.iter()
		.map(|&(shift, weight)| BilinearTap::new(document_to_pixels * shift, (weight / convolution.divisor) as f32))
		.collect();
	let offset = convolution.offset as f32;
	let colors = SurroundedColors::new(plane, full, convolution.surroundings, convolution.reach(), document_to_pixels);
	let coverage = convolution.weighs_coverage.then(|| SurroundedColors::coverage(plane));

	let mut result = Plane::new(plane.width, plane.height);
	fill_rows(&mut result.data, plane.width, |y, row| {
		for (x, pixel) in row.iter_mut().enumerate() {
			let (x, y) = (x as isize, y as isize);
			let alpha = match &coverage {
				Some(coverage) => coverage.sum(x, y, &taps)[0].clamp(0., 1.),
				None => plane.data[y as usize * plane.width + x as usize][3],
			};
			if alpha <= 0. {
				continue;
			}

			let sum = colors.sum(x, y, &taps).map(|channel| channel + offset);
			*pixel = uncovered(sum, alpha, convolution.surroundings);
		}
	});
	result
}

/// Each pixel's share of the full-strength content's coverage that the faded fill left, which a filter sharpening the coverage fades its result by. O(width × height).
pub fn fill_shares(full: &Plane<[f32; 4]>, filled: &Plane<[f32; 4]>) -> Plane<f32> {
	full.zip(filled, |full, filled| if full[3] > 0. { (filled[3] / full[3]).min(1.) } else { 0. })
}

/// The fill's filtered straight colors at the coverage of the full-strength content's filtered result, scaled by each pixel's share of it from [`fill_shares`],
/// so the fill fades the result as if after the filter. O(width × height).
pub fn faded_by_shares(filtered_filled: &Plane<[f32; 4]>, filtered_full: &Plane<[f32; 4]>, shares: &Plane<f32>) -> Plane<[f32; 4]> {
	let straight = |[red, green, blue, alpha]: [f32; 4]| [red / alpha, green / alpha, blue / alpha];
	let colors = filtered_filled.zip(filtered_full, |filled, full| {
		let [red, green, blue] = if filled[3] > 0. {
			straight(filled)
		} else if full[3] > 0. {
			straight(full)
		} else {
			[0.; 3]
		};
		[red, green, blue, full[3]]
	});
	colors.zip(shares, |[red, green, blue, full_alpha], share| {
		let alpha = full_alpha * share;
		[red * alpha, green * alpha, blue * alpha, alpha]
	})
}

/// For PSD interop, each straight color and the coverage sharpened against the `blur` around it, measured through `document_to_pixels`, by the `amount` and `threshold` fractions [`unsharp_change`] takes.
/// The colors are compared against the average of those visible in the `full`-strength content alone (or where too few are, the pixel's own color), so the content's surroundings take no part.
/// With no amount and threshold, it's a high pass instead: the colors' difference from their blur around middle gray, with the coverage kept. O(width × height) per blur pass.
pub fn unsharp_mask(plane: &Plane<[f32; 4]>, full: &Plane<[f32; 4]>, blur: UnsharpBlur, sharpening: Option<(f32, f32)>, document_to_pixels: DMat2) -> Plane<[f32; 4]> {
	// The visible colors made opaque among the others left out, so dividing their blur by its alpha averages the visible colors alone
	let visible = plane.zip(full, |[red, green, blue, alpha], [.., full_alpha]| match alpha > 0. && full_alpha >= VISIBLE_ALPHA {
		true => [red / alpha, green / alpha, blue / alpha, 1.],
		false => [0.; 4],
	});

	// A high pass keeps the coverage, so only unsharp masking blurs it
	let (blurred_colors, blurred_coverage) = match blur {
		UnsharpBlur::Gaussian(radius) => {
			let sigma = radius.max(0.) * spans_along_axes(document_to_pixels);
			let kernels = (BlurKernel::Gaussian { sigma: sigma.x }, BlurKernel::Gaussian { sigma: sigma.y });
			let blurred_coverage = sharpening.is_some().then(|| plane.map(|[.., alpha]| alpha).blurred(kernels.0, kernels.1));
			(visible.blurred(kernels.0, kernels.1), blurred_coverage)
		}
		UnsharpBlur::EdgeNeighbors => {
			let blurred_coverage = sharpening.is_some().then(|| neighbors_average(plane, document_to_pixels).map(|[.., alpha]| alpha));
			(neighbors_average(&visible, document_to_pixels), blurred_coverage)
		}
	};
	let gains = (blur == UnsharpBlur::EdgeNeighbors).then(|| edge_gains(plane, full, document_to_pixels));

	let mut result = Plane::new(plane.width, plane.height);
	fill_rows(&mut result.data, plane.width, |y, row| {
		for (x, pixel) in row.iter_mut().enumerate() {
			let index = y * plane.width + x;
			let [red, green, blue, alpha] = plane.data[index];
			if alpha <= 0. {
				continue;
			}

			// Where too little of the blur lands on visible colors to know their average, the pixel eases into being compared against its own color.
			// Edges Only moves each channel only its gain's share of the way toward the average.
			let own = [red / alpha, green / alpha, blue / alpha, alpha];
			let blurred = blurred_colors.data[index];
			let weight = average_weight(blurred[3]);
			let gain = gains.as_ref().map_or([1.; 4], |gains| gains.data[index]);
			let average: [f32; 3] = std::array::from_fn(|channel| {
				if weight > 0. {
					own[channel] + (blurred[channel] / blurred[3] - own[channel]) * weight * gain[channel]
				} else {
					own[channel]
				}
			});
			let coverage_average = blurred_coverage.as_ref().map_or(alpha, |blurred_coverage| alpha + (blurred_coverage.data[index] - alpha) * gain[3]);
			let difference: [f32; 4] = std::array::from_fn(|channel| own[channel] - if channel == 3 { coverage_average } else { average[channel] });

			let sharpened: [f32; 4] = match sharpening {
				Some((amount, threshold)) => std::array::from_fn(|channel| (own[channel] + unsharp_change(difference[channel], amount, threshold)).clamp(0., 1.)),
				None => std::array::from_fn(|channel| if channel == 3 { alpha } else { (difference[channel] + HIGH_PASS_GRAY).clamp(0., 1.) }),
			};
			let alpha = sharpened[3];
			*pixel = [sharpened[0] * alpha, sharpened[1] * alpha, sharpened[2] * alpha, alpha];
		}
	});
	result
}

/// The average of each pixel's four nearest neighbors ([`SHARPEN_NEIGHBORS`], measured through `document_to_pixels`) in every channel, read between pixels with none beyond the plane's edges,
/// as Edges Only compares against, matching the GPU's `neighbors_average`. O(width × height).
fn neighbors_average(plane: &Plane<[f32; 4]>, document_to_pixels: DMat2) -> Plane<[f32; 4]> {
	let taps = SHARPEN_NEIGHBORS.map(|shift| BilinearTap::new(document_to_pixels * shift, 0.25));
	let (columns, rows) = (plane.width as isize, plane.height as isize);
	let at = |x: isize, y: isize| match x >= 0 && y >= 0 && x < columns && y < rows {
		true => plane.data[(y * columns + x) as usize],
		false => [0.; 4],
	};

	let mut average = Plane::new(plane.width, plane.height);
	fill_rows(&mut average.data, plane.width, |y, row: &mut [[f32; 4]]| {
		for (x, pixel) in row.iter_mut().enumerate() {
			for tap in &taps {
				let [x, y] = [x as isize + tap.whole[0], y as isize + tap.whole[1]];
				let corners = [at(x, y), at(x + 1, y), at(x, y + 1), at(x + 1, y + 1)];
				for (corner, bilinear) in corners.into_iter().zip(tap.bilinear) {
					(0..4).for_each(|channel| pixel[channel] += corner[channel] * bilinear * tap.weight);
				}
			}
		}
	});
	average
}

/// For PSD interop, the share of its change Edges Only makes in each channel of each pixel, the [`edge_gain`] of the channel's Sobel gradient there, matching the GPU's `edge_gains`.
/// The colors are read continued past those visible in the `full`-strength content, and the coverage with none beyond it. O(width × height).
fn edge_gains(plane: &Plane<[f32; 4]>, full: &Plane<[f32; 4]>, document_to_pixels: DMat2) -> Plane<[f32; 4]> {
	// A faint pixel the gradient reads finds its nearest visible color within the square of a pixel around it
	let colors = SurroundedColors::new(plane, full, Surroundings::Continued, 1., document_to_pixels);
	let coverage = SurroundedColors::coverage(plane);
	let taps = |axis: usize| SOBEL_TAPS.map(|(shift, weights)| BilinearTap::new(document_to_pixels * shift, weights[axis] as f32));
	let (across, down) = (taps(0), taps(1));

	let mut gains = Plane::new(plane.width, plane.height);
	fill_rows(&mut gains.data, plane.width, |y, row: &mut [[f32; 4]]| {
		for (x, gain) in row.iter_mut().enumerate() {
			let (x, y) = (x as isize, y as isize);
			let [colors_across, colors_down] = [colors.sum(x, y, &across), colors.sum(x, y, &down)];
			let [coverage_across, coverage_down] = [coverage.sum(x, y, &across)[0], coverage.sum(x, y, &down)[0]];

			*gain = std::array::from_fn(|channel| match channel {
				3 => edge_gain(coverage_across.hypot(coverage_down)),
				_ => edge_gain(colors_across[channel].hypot(colors_down[channel])),
			});
		}
	});
	gains
}

/// For PSD interop, each channel darkened by how steeply it changes, the length of its Sobel gradient, so edges show dark over white.
/// The content is matted over white, so its silhouette shows as an edge too. O(width × height).
pub fn find_edges(plane: &Plane<[f32; 4]>, document_to_pixels: DMat2) -> Plane<[f32; 4]> {
	let surroundings = Surroundings::Matted(1.);
	let colors = SurroundedColors::new(plane, plane, surroundings, 1., document_to_pixels);
	let taps = |axis: usize| SOBEL_TAPS.map(|(shift, weights)| BilinearTap::new(document_to_pixels * shift, weights[axis] as f32));
	let (across, down) = (taps(0), taps(1));

	let mut result = Plane::new(plane.width, plane.height);
	fill_rows(&mut result.data, plane.width, |y, row| {
		for (x, pixel) in row.iter_mut().enumerate() {
			let alpha = plane.data[y * plane.width + x][3];
			if alpha <= 0. {
				continue;
			}

			let (x, y) = (x as isize, y as isize);
			let [across, down] = [colors.sum(x, y, &across), colors.sum(x, y, &down)];
			let edges = std::array::from_fn(|channel| 1. - across[channel].hypot(down[channel]));
			*pixel = uncovered(edges, alpha, surroundings);
		}
	});
	result
}

/// For PSD interop, each pixel's color is the lightest in the square of `radius` document pixels around it (or with `maximum` false, the darkest), and its coverage the opposite extreme.
/// The maximum spreads every covered pixel's straight color. The minimum spreads the darkest colors matted over white, lifted back off within the grown coverage, so faint pixels darken only as much as they cover.
/// A fractional radius mixes the whole radii around it. O(width × height).
pub fn neighborhood_extreme(plane: &Plane<[f32; 4]>, radius: f64, maximum: bool, document_to_pixels: DMat2) -> Plane<[f32; 4]> {
	let spans = spans_along_axes(document_to_pixels);
	let ([near_x, far_x], fraction) = neighborhood_radii(radius, spans.x);
	let ([near_y, far_y], _) = neighborhood_radii(radius, spans.y);
	if near_x.max(near_y) == 0 && (fraction <= 0. || far_x.max(far_y) == 0) {
		return plane.clone();
	}

	// Colors and coverage in the form the filter takes the extremes of, with uncovered pixels' colors (and those beyond the edges) at the identity of their extreme
	let (prepared, beyond) = match maximum {
		true => (
			plane.map(|[red, green, blue, alpha]| match alpha > 0. {
				true => [red / alpha, green / alpha, blue / alpha, alpha],
				false => [0.; 4],
			}),
			[0.; 4],
		),
		false => (
			plane.map(|[red, green, blue, alpha]| [red + 1. - alpha, green + 1. - alpha, blue + 1. - alpha, alpha]),
			[1., 1., 1., 0.],
		),
	};
	let extreme = move |first: [f32; 4], second: [f32; 4]| -> [f32; 4] {
		std::array::from_fn(|channel| match (channel == 3) != maximum {
			true => first[channel].max(second[channel]),
			false => first[channel].min(second[channel]),
		})
	};
	// Back to premultiplied colors, which for the minimum lifts white back off within the grown coverage
	let premultiplied = move |[red, green, blue, alpha]: [f32; 4]| {
		let channel = |channel: f32| match maximum {
			true => channel * alpha,
			false => (channel - (1. - alpha)).max(0.).min(alpha),
		};
		[channel(red), channel(green), channel(blue), alpha]
	};

	// The larger square's extremes grow from the smaller one's, as a square widened by another is the square of their radii summed
	let near = extremes_in_squares(prepared, [near_x, near_y], beyond, extreme);
	if fraction <= 0. {
		return near.map(premultiplied);
	}
	let far = extremes_in_squares(near.clone(), [far_x - near_x, far_y - near_y], beyond, extreme);
	near.zip(&far, |near, far| {
		let (near, far) = (premultiplied(near), premultiplied(far));
		std::array::from_fn(|channel| near[channel] + (far[channel] - near[channel]) * fraction as f32)
	})
}

/// The `extreme` of each rectangle reaching `across` and `down` pixels from each pixel, with `beyond` read past the edges, along the rows then the columns. O(width × height).
fn extremes_in_squares(plane: Plane<[f32; 4]>, [across, down]: [u32; 2], beyond: [f32; 4], extreme: impl Fn([f32; 4], [f32; 4]) -> [f32; 4] + Copy + Sync) -> Plane<[f32; 4]> {
	let along_rows = |plane: Plane<[f32; 4]>, radius: u32| {
		if radius == 0 {
			return plane;
		}
		let mut result = Plane::new(plane.width, plane.height);
		fill_rows(&mut result.data, plane.width, |y, row| {
			window_extremes(&plane.data[y * plane.width..(y + 1) * plane.width], row, radius as usize, beyond, extreme)
		});
		result
	};

	let plane = along_rows(plane, across);
	match down {
		0 => plane,
		_ => along_rows(plane.transposed(), down).transposed(),
	}
}

/// The `extreme` of each window of `2 × radius + 1` values along a row, with `beyond` read past its ends, by van Herk and Gil-Werman's blocks of extremes running from each block's start and to its end.
/// O(length) whatever the radius, since a window as wide as the row already holds all of it.
fn window_extremes(source: &[[f32; 4]], target: &mut [[f32; 4]], radius: usize, beyond: [f32; 4], extreme: impl Fn([f32; 4], [f32; 4]) -> [f32; 4]) {
	let radius = radius.min(source.len());
	let window = 2 * radius + 1;
	let padded: Vec<[f32; 4]> = std::iter::repeat_n(beyond, radius).chain(source.iter().copied()).chain(std::iter::repeat_n(beyond, radius)).collect();

	let mut from_start = padded.clone();
	let mut to_end = padded.clone();
	for index in 1..padded.len() {
		if index % window != 0 {
			from_start[index] = extreme(from_start[index - 1], padded[index]);
		}
	}
	for index in (0..padded.len() - 1).rev() {
		if (index + 1) % window != 0 {
			to_end[index] = extreme(to_end[index + 1], padded[index]);
		}
	}

	// Each window spans the end of one block and the start of the next
	for (index, value) in target.iter_mut().enumerate() {
		*value = extreme(to_end[index], from_start[index + window - 1]);
	}
}

/// A value strictly between 0 and 1 from the top 23 bits of a hash, which an f32 holds exactly with the half added, matching the GPU's.
fn unit_interval(hash: u32) -> f32 {
	((hash >> 9) as f32 + 0.5) / (1_u32 << 23) as f32
}

/// The three random values one field of the `seed` holds at a cell of the effect space.
fn field_values(cell: IVec2, seed: u32) -> [f32; 3] {
	pcg3d([cell.x as u32, cell.y as u32, seed]).map(unit_interval)
}

/// The noise each of a pixel's colors takes before the amount scales it, averaged over the pixel's samples of the field: uniform from -1 to 1, or Gaussian with a deviation of 1 cut off at [`GAUSSIAN_NOISE_LIMIT`].
fn noise_at(sampling: &FieldSampling, pixel: [usize; 2], distribution: NoiseDistribution, monochromatic: bool, seed: i64) -> [f32; 3] {
	let (offsets_seed, angles_seed) = (RandomField::NoiseOffsets.seed(seed), RandomField::NoiseAngles.seed(seed));

	let mut sum = [0.; 3];
	let mut count = 0.;
	for cell in sampling.sample_cells(pixel) {
		let offsets = field_values(cell, offsets_seed);
		let noise = match distribution {
			NoiseDistribution::Uniform => offsets.map(|value| 2. * value - 1.),
			// The Box-Muller transform of two uniform values into one with a normal distribution
			NoiseDistribution::Gaussian => {
				let angles = field_values(cell, angles_seed);
				std::array::from_fn(|channel| ((-2. * offsets[channel].ln()).sqrt() * (std::f32::consts::TAU * angles[channel]).cos()).clamp(-GAUSSIAN_NOISE_LIMIT, GAUSSIAN_NOISE_LIMIT))
			}
		};
		(0..3).for_each(|channel| sum[channel] += noise[channel]);
		count += 1.;
	}

	let average = sum.map(|channel| channel / count);
	if monochromatic { [average[0]; 3] } else { average }
}

/// For PSD interop, each straight color offset by noise of `amount` (a fraction of full brightness) with its `distribution`, the same for every color of a pixel if it's `monochromatic`, and clamped.
/// The coverage is kept. The noise is fixed to the cells of the effect space, which `pixels_to_effect` places the plane's pixels among. O(width × height × samples per pixel).
pub fn add_noise(plane: &Plane<[f32; 4]>, amount: f64, distribution: NoiseDistribution, monochromatic: bool, seed: i64, pixels_to_effect: DAffine2) -> Plane<[f32; 4]> {
	let sampling = FieldSampling::new(pixels_to_effect);
	let extent = noise_extent(amount, distribution) as f32;
	let scale = (amount * sampling.deviation_scale) as f32;

	let mut result = Plane::new(plane.width, plane.height);
	fill_rows(&mut result.data, plane.width, |y, row| {
		for (x, pixel) in row.iter_mut().enumerate() {
			let [red, green, blue, alpha] = plane.data[y * plane.width + x];
			if alpha <= 0. {
				continue;
			}

			let noise = noise_at(&sampling, [x, y], distribution, monochromatic, seed);
			let noisy = |channel: f32, noise: f32| (channel / alpha + (noise * scale).clamp(-extent, extent)).clamp(0., 1.) * alpha;
			*pixel = [noisy(red, noise[0]), noisy(green, noise[1]), noisy(blue, noise[2]), alpha];
		}
	});
	result
}

/// For PSD interop, how much of a pixel Dissolve shows, from 0 to 1: the share of its samples passing the `opacity` (a fraction), and the threshold level from 0 to 254 its coverage must pass.
/// For pixels spanning more cells than they sample, the share deviates from the opacity less, as it would over them all.
fn dissolve_field(sampling: &FieldSampling, pixel: [usize; 2], opacity: f32, seed: i64) -> (f32, f32) {
	let seed = RandomField::Dissolve.seed(seed);

	let (mut shown, mut count) = (0., 0.);
	for cell in sampling.sample_cells(pixel) {
		shown += if field_values(cell, seed)[0] < opacity { 1. } else { 0. };
		count += 1.;
	}
	let share = (opacity + (shown / count - opacity) * sampling.deviation_scale as f32).clamp(0., 1.);

	let threshold = (field_values(sampling.center_cell(pixel), seed)[1] * 255.).floor();
	(share, threshold)
}

/// For PSD interop, the content shown fully opaque where the `opacity` (a fraction) and the coverage each pass a threshold of their own, random for each cell of the effect space, and hidden elsewhere.
/// The thresholds are fixed to those cells, which `pixels_to_effect` places the plane's pixels among, and pixels spanning several show the share of them passing. O(width × height × samples per pixel).
pub fn dissolve(plane: &Plane<[f32; 4]>, opacity: f64, seed: i64, pixels_to_effect: DAffine2) -> Plane<[f32; 4]> {
	let sampling = FieldSampling::new(pixels_to_effect);
	let opacity = opacity as f32;

	let mut result = Plane::new(plane.width, plane.height);
	fill_rows(&mut result.data, plane.width, |y, row| {
		for (x, pixel) in row.iter_mut().enumerate() {
			let [red, green, blue, alpha] = plane.data[y * plane.width + x];
			if alpha <= 0. {
				continue;
			}

			// The coverage passes a threshold level when it's at least one 8-bit level above it, a half level counting as the level below as browsers truncate it
			let (share, threshold) = dissolve_field(&sampling, [x, y], opacity, seed);
			let shown = if alpha * 255. > threshold + 0.5 { share } else { 0. };
			*pixel = [red / alpha * shown, green / alpha * shown, blue / alpha * shown, shown];
		}
	});
	result
}

#[cfg(test)]
mod tests {
	use super::*;

	fn plane(width: usize, height: usize, data: Vec<[f32; 4]>) -> Plane<[f32; 4]> {
		Plane { width, height, data }
	}

	#[test]
	fn kernels_of_any_shape_read_around_their_center() {
		let (black, white) = ([0., 0., 0., 1.], [1.; 4]);

		// An even row centers on its second weight, so its first reads the pixel to the left, moving the colors right
		let line = plane(3, 1, vec![white, black, black]);
		let even = convolve(&line, &line, &Convolution::custom(&[1., 0.], 2, 1., 0.), DMat2::IDENTITY);
		assert_eq!(&even.data[1..], &[white, black]);

		// A column reads the pixels above and below
		let column = plane(1, 3, vec![white, black, black]);
		let down = convolve(&column, &column, &Convolution::custom(&[1., 0., 0.], 1, 1., 0.), DMat2::IDENTITY);
		assert_eq!(&down.data[1..], &[white, black]);
	}

	#[test]
	fn a_kernel_of_one_weight_shifts_the_colors_within_the_coverage() {
		let (black, white) = ([0., 0., 0., 1.], [1.; 4]);
		let line = plane(4, 1, vec![black, white, black, [0.; 4]]);

		// The weight right of the center reads the pixel to the right, moving the colors left
		let right = [0., 0., 0., 0., 0., 1., 0., 0., 0.];
		let shifted = convolve(&line, &line, &Convolution::custom(&right, 3, 1., 0.), DMat2::IDENTITY);
		assert_eq!(shifted.data, vec![white, black, black, [0.; 4]]);

		// A shift of half a pixel splits between two
		let halved = convolve(&line, &line, &Convolution::custom(&right, 3, 1., 0.), DMat2::from_diagonal(DVec2::splat(0.5)));
		assert_eq!(halved.data, vec![[0.5, 0.5, 0.5, 1.], [0.5, 0.5, 0.5, 1.], black, [0.; 4]]);

		// A shift past the edge reads between the colors continued past it and none further, which it leaves out rather than darkening toward
		let edge = plane(5, 1, vec![white, white, [0.; 4], [0.; 4], [0.; 4]]);
		let past_edge = convolve(&edge, &edge, &Convolution::custom(&right, 3, 1., 0.), DMat2::from_diagonal(DVec2::splat(1.25)));
		assert_eq!(past_edge.data[1], white);
	}

	#[test]
	fn sums_are_divided_raised_and_clamped() {
		let gray = [0.25, 0.25, 0.25, 0.5];
		let line = plane(3, 1, vec![gray; 3]);

		// Doubled to full brightness, then raised past it
		let raised = convolve(&line, &line, &Convolution::custom(&[2.], 1, 1., 255. / 4.), DMat2::IDENTITY);
		assert_eq!(raised.data[1], [0.5; 4]);

		// The difference across a flat color is nothing, which the offset lifts to middle gray
		let difference = convolve(&line, &line, &Convolution::custom(&[-1., 0., 1.], 3, 2., 255. / 2.), DMat2::IDENTITY);
		assert_eq!(difference.data[1], [0.25, 0.25, 0.25, 0.5]);
	}

	#[test]
	fn colors_continue_past_the_edges_from_the_nearest_visible_pixel() {
		let red = [1., 0., 0., 1.];
		let faint_blue = [0., 0., 0.01, 0.01];
		let line = plane(5, 1, vec![[0.; 4], [0.; 4], red, faint_blue, [0.; 4]]);

		// Averaging three neighbors at the edge reads the red beyond it rather than black, leaving the red as it is
		let averaged = convolve(&line, &line, &Convolution::custom(&[1., 1., 1.], 3, 3., 0.), DMat2::IDENTITY);
		assert!((averaged.data[2][0] - 1.).abs() < 1e-6, "{:?}", averaged.data[2]);
		// The faint pixel reads red rather than its own blue, keeping its own coverage
		let faint = averaged.data[3];
		assert!(faint[0] > 0. && faint[2] == 0. && faint[3] == 0.01, "{faint:?}");
	}

	/// A white square of `inner` pixels in the middle of a transparent square of `outer` pixels.
	fn white_square(outer: usize, inner: std::ops::Range<usize>) -> Plane<[f32; 4]> {
		let mut square = Plane::new(outer, outer);
		for y in inner.clone() {
			square.data[y * outer + inner.start..y * outer + inner.end].fill([1.; 4]);
		}
		square
	}

	#[test]
	fn emboss_grays_flat_colors_and_shows_the_silhouette_in_relief() {
		let square = white_square(16, 4..12);
		let embossed = convolve(&square, &square, &Convolution::emboss(0., 2., 100.), DMat2::IDENTITY);
		let at = |x: usize| embossed.data[8 * 16 + x];

		// The inside is flat, so it's middle gray
		assert_eq!(at(8), [0.5, 0.5, 0.5, 1.]);
		// Lit from the right, the right edge's white falls off to the gray beyond it, and the left edge rises from it
		assert!(at(11)[0] < 0.5 && at(4)[0] > 0.5, "{:?} {:?}", at(11), at(4));
		assert_eq!(at(2), [0.; 4], "nothing outside");
	}

	#[test]
	fn find_edges_whitens_flat_colors_and_darkens_the_silhouette() {
		let square = white_square(16, 4..12);
		let edges = find_edges(&square, DMat2::IDENTITY);
		let at = |x: usize, y: usize| edges.data[y * 16 + x];

		assert_eq!(at(8, 8), [1.; 4]);
		// Over white, the white square's edge has no contrast, so it stays white
		assert_eq!(at(4, 8), [1.; 4]);

		// A black square's edge against the white it's matted over is dark
		let black = white_square(16, 4..12).map(|[_, _, _, alpha]| [0., 0., 0., alpha]);
		let edges = find_edges(&black, DMat2::IDENTITY);
		assert_eq!(edges.data[8 * 16 + 4], [0., 0., 0., 1.]);
		assert_eq!(edges.data[8 * 16 + 8], [1.; 4]);
	}

	#[test]
	fn neighborhood_maximum_spreads_light_colors_as_coverage_shrinks() {
		let mut square = white_square(16, 2..14);
		square.data[8 * 16 + 8] = [0., 0., 0., 1.];

		let maximum = neighborhood_extreme(&square, 1., true, DMat2::IDENTITY);
		assert_eq!(maximum.data[8 * 16 + 8], [1.; 4], "the dark pixel takes its lighter neighbors' color");
		assert_eq!(maximum.data[8 * 16 + 2], [0.; 4], "the coverage shrinks by the radius");
		assert_eq!(maximum.data[8 * 16 + 3], [1.; 4]);

		let minimum = neighborhood_extreme(&square, 1., false, DMat2::IDENTITY);
		assert_eq!(minimum.data[9 * 16 + 9], [0., 0., 0., 1.], "the dark pixel spreads");
		assert_eq!(minimum.data[8 * 16 + 1], [1.; 4], "the coverage grows by the radius, with the colors beyond taking no part");

		// Half a radius more mixes halfway toward the next
		let between = neighborhood_extreme(&square, 1.5, false, DMat2::IDENTITY);
		assert_eq!(between.data[9 * 16 + 9], [0., 0., 0., 1.]);
		assert_eq!(between.data[10 * 16 + 10], [0.5, 0.5, 0.5, 1.]);
		assert_eq!(between.data[8 * 16], [0.5; 4]);
	}

	#[test]
	fn neighborhood_extremes_keep_faint_colors_and_a_zero_radius_changes_nothing() {
		// A faint red block keeps its color at its own coverage, rather than taking either extreme's identity
		let mut faint = plane(7, 7, vec![[0.; 4]; 49]);
		for y in 1..6 {
			faint.data[y * 7 + 1..y * 7 + 6].fill([0.04, 0., 0., 0.04]);
		}
		for maximum in [true, false] {
			let extreme = neighborhood_extreme(&faint, 1., maximum, DMat2::IDENTITY);
			let [red, green, blue, alpha] = extreme.data[3 * 7 + 3];
			assert!(
				(red - 0.04).abs() < 1e-6 && green.abs() < 1e-6 && blue.abs() < 1e-6 && (alpha - 0.04).abs() < 1e-6,
				"{:?}",
				extreme.data[3 * 7 + 3]
			);

			assert_eq!(neighborhood_extreme(&faint, 0., maximum, DMat2::IDENTITY).data, faint.data);
		}

		// At twice the zoom, a radius of one document pixel reaches two pixels
		let mut dot = plane(9, 9, vec![[1.; 4]; 81]);
		dot.data[4 * 9 + 4] = [0., 0., 0., 1.];
		let spread = neighborhood_extreme(&dot, 1., false, DMat2::from_diagonal(DVec2::splat(2.)));
		assert_eq!(spread.data[2 * 9 + 6], [0., 0., 0., 1.]);
		assert_eq!(spread.data[9 + 4], [1.; 4]);
	}

	#[test]
	fn a_fill_fades_sharpened_coverage_evenly() {
		// Sharpening the faded square alone would overshoot at its edge, where the full-strength square's clamp at full coverage hides it
		let square = white_square(16, 4..12);
		let faded = square.map(|pixel| pixel.map(|channel| channel * 0.5));
		let sharpen = |plane: &Plane<[f32; 4]>| unsharp_mask(plane, plane, UnsharpBlur::EdgeNeighbors, Some((1., 0.)), DMat2::IDENTITY);
		assert!(sharpen(&faded).data.iter().any(|pixel| pixel[3] > 0.5));

		let result = faded_by_shares(&sharpen(&faded), &sharpen(&square), &fill_shares(&square, &faded));
		assert!(result.data.iter().all(|pixel| pixel[3] == 0. || pixel[3] == 0.5), "{:?}", result.data);
	}

	#[test]
	fn window_extremes_match_a_direct_search() {
		let values: Vec<[f32; 4]> = (0..37).map(|index| [((index * 7919) % 31) as f32; 4]).collect();
		for radius in [1, 2, 5, 40] {
			let mut fast = vec![[0.; 4]; values.len()];
			window_extremes(&values, &mut fast, radius, [-1.; 4], |first, second| std::array::from_fn(|channel| first[channel].max(second[channel])));

			for (index, value) in fast.iter().enumerate() {
				let window = (index as isize - radius as isize)..=(index as isize + radius as isize);
				let direct = window.map(|at| values.get(at as usize).filter(|_| at >= 0).map_or(-1., |value| value[0])).fold(f32::MIN, f32::max);
				assert_eq!(value[0], direct, "radius {radius} at {index}");
			}
		}
	}

	#[test]
	fn unsharp_masking_exaggerates_differences_past_its_threshold() {
		// A gray line with a lighter middle, a step the blur softens
		let mut line = plane(9, 1, vec![[0.5, 0.5, 0.5, 1.]; 9]);
		line.data[4] = [0.6, 0.6, 0.6, 1.];

		let gaussian = UnsharpBlur::Gaussian(1.);
		let sharpened = unsharp_mask(&line, &line, gaussian, Some((1., 0.)), DMat2::IDENTITY);
		assert!(sharpened.data[4][0] > 0.6 && sharpened.data[3][0] < 0.5, "{:?}", sharpened.data);
		assert_eq!(sharpened.data[4][3], 1., "full coverage stays full");

		// A threshold past the exaggerated difference leaves every pixel as it is
		let untouched = unsharp_mask(&line, &line, gaussian, Some((1., 0.2)), DMat2::IDENTITY);
		assert_eq!(untouched.data, line.data);

		// A high pass of a flat color is middle gray
		let flat = plane(5, 1, vec![[0.3, 0.3, 0.3, 1.]; 5]);
		assert_eq!(
			unsharp_mask(&flat, &flat, UnsharpBlur::Gaussian(2.), None, DMat2::IDENTITY).data[2],
			[HIGH_PASS_GRAY, HIGH_PASS_GRAY, HIGH_PASS_GRAY, 1.]
		);

		// A fill faint enough to fall below the visible threshold still averages the colors the full-strength content shows
		let faint = line.map(|pixel| pixel.map(|channel| channel * 0.05));
		let faint_sharpened = unsharp_mask(&faint, &line, gaussian, Some((1., 0.)), DMat2::IDENTITY);
		let straight = |pixel: [f32; 4]| pixel[0] / pixel[3];
		assert!((straight(faint_sharpened.data[4]) - straight(sharpened.data[4])).abs() < 1e-4, "{:?}", faint_sharpened.data);
	}

	#[test]
	fn edges_only_sharpens_against_the_neighbors_as_far_as_an_edge_runs_through() {
		// A gray step up by `height` between columns, across which the Sobel gradient is four times as long, read along the middle row
		let step = |height: f32| {
			let mut columns = plane(6, 3, vec![[0.4, 0.4, 0.4, 1.]; 18]);
			(0..3).for_each(|y| (3..6).for_each(|x| columns.data[y * 6 + x] = [0.4 + height, 0.4 + height, 0.4 + height, 1.]));
			columns
		};
		let sharpen = |plane: &Plane<[f32; 4]>, amount: f32, threshold: f32| unsharp_mask(plane, plane, UnsharpBlur::EdgeNeighbors, Some((amount, threshold)), DMat2::IDENTITY).data;
		let close = |actual: f32, expected: f32| (actual - expected).abs() < 1e-5;

		// A strong step sharpens fully, each side moving by its difference from its four neighbors' average, a quarter of the step, while flat areas stay
		let strong = sharpen(&step(0.4), 1., 0.);
		assert!(close(strong[8][0], 0.3) && close(strong[9][0], 0.9) && close(strong[7][0], 0.4), "{strong:?}");

		// A step under a sixteenth of full contrast stays as it is, and one of an eighth sharpens a third as much
		assert!(close(sharpen(&step(0.05), 1., 0.)[8][0], 0.4));
		assert!(close(sharpen(&step(0.125), 1., 0.)[8][0], 0.4 - 0.125 / 4. / 3.));

		// The threshold softens the change, which half the amount halves
		assert!(close(sharpen(&step(0.4), 1., 0.05)[8][0], 0.35));
		assert!(close(sharpen(&step(0.4), 0.5, 0.)[8][0], 0.35));
	}

	#[test]
	fn a_kernel_applied_to_alpha_weighs_the_coverage_too() {
		let mut line = plane(5, 1, vec![[0.; 4]; 5]);
		line.data[2] = [0.6, 0.3, 0., 1.];
		let average = Convolution::custom(&[1., 1., 1.], 3, 3., 0.);

		// Averaged with its neighbors, the coverage of a lone pixel spreads a third of itself to each side
		let spread = convolve(
			&line,
			&line,
			&Convolution {
				weighs_coverage: true,
				..average.clone()
			},
			DMat2::IDENTITY,
		);
		let alphas: Vec<f32> = spread.data.iter().map(|pixel| pixel[3]).collect();
		assert!(alphas.iter().zip([0., 1., 1., 1., 0.]).all(|(alpha, thirds)| (alpha - thirds / 3.).abs() < 1e-6), "{alphas:?}");

		// Otherwise, the coverage stays as it is
		let kept = convolve(&line, &line, &average, DMat2::IDENTITY);
		assert!(kept.data.iter().zip(&line.data).all(|(kept, line)| kept[3] == line[3]));
	}

	/// The mean and standard deviation of each color's offset from middle gray, and the correlation between the first two colors' offsets.
	fn offset_statistics(noisy: &Plane<[f32; 4]>) -> ([f64; 3], [f64; 3], f64) {
		let count = noisy.data.len() as f64;
		let offsets: Vec<[f64; 3]> = noisy.data.iter().map(|pixel| std::array::from_fn(|channel| pixel[channel] as f64 / pixel[3] as f64 - 0.5)).collect();
		let mean: [f64; 3] = std::array::from_fn(|channel| offsets.iter().map(|offset| offset[channel]).sum::<f64>() / count);
		let deviation: [f64; 3] = std::array::from_fn(|channel| (offsets.iter().map(|offset| (offset[channel] - mean[channel]).powi(2)).sum::<f64>() / count).sqrt());
		let covariance = offsets.iter().map(|offset| (offset[0] - mean[0]) * (offset[1] - mean[1])).sum::<f64>() / count;
		(mean, deviation, covariance / (deviation[0] * deviation[1]))
	}

	#[test]
	fn noise_follows_its_distribution_independently_for_each_color() {
		let gray = plane(256, 256, vec![[0.5, 0.5, 0.5, 1.]; 256 * 256]);
		let amount = 0.2;

		// Uniform noise spreads evenly up to the amount, with a deviation of the amount over √3
		let uniform = add_noise(&gray, amount, NoiseDistribution::Uniform, false, 0, DAffine2::IDENTITY);
		let (mean, deviation, correlation) = offset_statistics(&uniform);
		for channel in 0..3 {
			assert!(
				mean[channel].abs() < 0.002 && (deviation[channel] / (amount / 3_f64.sqrt()) - 1.).abs() < 0.02,
				"{mean:?} {deviation:?}"
			);
		}
		assert!(correlation.abs() < 0.02, "{correlation}");
		assert!(uniform.data.iter().all(|pixel| (pixel[0] - 0.5).abs() <= amount as f32 && pixel[3] == 1.));

		// Gaussian noise deviates by the amount, cut off at its limit
		let gaussian = add_noise(&gray, amount, NoiseDistribution::Gaussian, false, 0, DAffine2::IDENTITY);
		let (_, deviation, _) = offset_statistics(&gaussian);
		assert!((deviation[0] / amount - 1.).abs() < 0.03, "{deviation:?}");

		// Monochromatic noise shifts every color of a pixel together, and another seed gives other noise
		let monochromatic = add_noise(&gray, amount, NoiseDistribution::Gaussian, true, 0, DAffine2::IDENTITY);
		assert!(monochromatic.data.iter().all(|pixel| pixel[0] == pixel[1] && pixel[1] == pixel[2]));
		let reseeded = add_noise(&gray, amount, NoiseDistribution::Uniform, false, 1, DAffine2::IDENTITY);
		assert_ne!(reseeded.data[..64], uniform.data[..64]);
	}

	#[test]
	fn noise_is_fixed_to_the_effect_spaces_cells_and_averages_where_pixels_span_several() {
		let gray = |size: usize| plane(size, size, vec![[0.5, 0.5, 0.5, 1.]; size * size]);
		let noise = |plane: &Plane<[f32; 4]>, pixels_to_effect| add_noise(plane, 0.3, NoiseDistribution::Uniform, false, 4, pixels_to_effect);

		// A plane placed three cells right and five down holds the same noise at the same cells
		let placed = noise(&gray(16), DAffine2::IDENTITY);
		let moved = noise(&gray(16), DAffine2::from_translation(DVec2::new(3., 5.)));
		assert_eq!(moved.data[0], placed.data[5 * 16 + 3]);
		assert_eq!(moved.data[16 + 2], placed.data[6 * 16 + 5]);

		// Pixels spanning eight cells each deviate as an average over all 64 would, an eighth as much
		let zoomed_out = noise(&gray(128), DAffine2::from_scale(DVec2::splat(8.)));
		let (_, deviation, _) = offset_statistics(&zoomed_out);
		assert!((deviation[0] / (0.3 / 3_f64.sqrt() / 8.) - 1.).abs() < 0.05, "{deviation:?}");

		// Pixels at exactly one cell each take one sample, like zoomed in
		assert_eq!(FieldSampling::new(DAffine2::IDENTITY).counts, glam::UVec2::ONE);
		assert_eq!(FieldSampling::new(DAffine2::from_scale(DVec2::splat(0.25))).counts, glam::UVec2::ONE);
	}

	#[test]
	fn dissolve_shows_opaque_pixels_by_the_opacity_and_the_coverage() {
		let shown_share = |plane: &Plane<[f32; 4]>| plane.data.iter().filter(|pixel| pixel[3] > 0.).count() as f64 / plane.data.len() as f64;
		let red = |alpha: f32| plane(256, 256, vec![[alpha, 0., 0., alpha]; 256 * 256]);

		// Where the content is opaque, the opacity's share shows, and a larger opacity shows those and more
		let some = dissolve(&red(1.), 0.3, 0, DAffine2::IDENTITY);
		let more = dissolve(&red(1.), 0.6, 0, DAffine2::IDENTITY);
		assert!((shown_share(&some) - 0.3).abs() < 0.01 && (shown_share(&more) - 0.6).abs() < 0.01);
		assert!(some.data.iter().zip(&more.data).all(|(some, more)| some[3] <= more[3]));

		// Half-covered content shows half as much, each shown pixel fully opaque in its straight color
		let half = dissolve(&red(0.5), 1., 0, DAffine2::IDENTITY);
		assert!((shown_share(&half) - 127. / 255.).abs() < 0.01, "{}", shown_share(&half));
		assert!(half.data.iter().all(|pixel| *pixel == [0.; 4] || *pixel == [1., 0., 0., 1.]));

		// Pixels spanning several cells show the share of them passing the opacity
		let zoomed_out = dissolve(&red(1.), 0.3, 0, DAffine2::from_scale(DVec2::splat(2.)));
		let average = zoomed_out.data.iter().map(|pixel| pixel[3] as f64).sum::<f64>() / zoomed_out.data.len() as f64;
		assert!((average - 0.3).abs() < 0.01 && zoomed_out.data.iter().any(|pixel| pixel[3] == 0.5), "{average}");

		// For PSD interop, the opacity and the coverage pass thresholds of their own, so equal products show different pixels, neither set holding the other
		let quarter = dissolve(&red(64. / 255.), 1., 0, DAffine2::IDENTITY);
		let half_at_half = dissolve(&red(128. / 255.), 0.5, 0, DAffine2::IDENTITY);
		let only_in = |first: &Plane<[f32; 4]>, second: &Plane<[f32; 4]>| first.data.iter().zip(&second.data).filter(|(first, second)| first[3] > 0. && second[3] <= 0.).count();
		assert!(only_in(&quarter, &half_at_half) > 1000 && only_in(&half_at_half, &quarter) > 1000);
	}

	#[test]
	fn random_values_lie_strictly_between_zero_and_one() {
		assert!(unit_interval(u32::MAX) < 1. && unit_interval(0) > 0.);
	}
}
