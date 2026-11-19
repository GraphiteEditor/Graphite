//! The pixel filters on the CPU, which mirror the renderer's GPU passes on premultiplied gamma-encoded channels.

use crate::effect_kernels::Plane;
use glam::{DMat2, DVec2};
use graphic_types::effects::spans_along_axes;
use graphic_types::filters::{Convolution, SOBEL_TAPS, Surroundings, VISIBLE_ALPHA, fill_shifts, neighborhood_radii};
use graphic_types::{EffectOperation, MinimumMaximumMode};
use raster_types::parallel_rows::fill_rows;

/// Carries out a filter effect on premultiplied gamma-encoded pixels, with its lengths measured through `document_to_pixels`, or returns them as they are for any other operation.
/// The pixels visible in the `full`-strength content (the plane itself for `None`) are those whose colors the filter reads.
pub fn filter(plane: Plane<[f32; 4]>, full: Option<&Plane<[f32; 4]>>, operation: &EffectOperation, document_to_pixels: DMat2) -> Plane<[f32; 4]> {
	if let Some(convolution) = operation.convolution() {
		return convolve(&plane, full.unwrap_or(&plane), &convolution, document_to_pixels);
	}

	match *operation {
		EffectOperation::FindEdges => find_edges(&plane, document_to_pixels),
		EffectOperation::MinimumMaximum { radius, mode } => neighborhood_extreme(&plane, radius, mode == MinimumMaximumMode::Maximum, document_to_pixels),
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
/// It reads the colors as the convolution's surroundings describe, those visible judged by the `full`-strength content, then divides, raises, and clamps the sum, keeping each pixel's coverage. O(width × height × taps).
pub fn convolve(plane: &Plane<[f32; 4]>, full: &Plane<[f32; 4]>, convolution: &Convolution, document_to_pixels: DMat2) -> Plane<[f32; 4]> {
	let taps: Vec<BilinearTap> = convolution
		.taps
		.iter()
		.map(|&(shift, weight)| BilinearTap::new(document_to_pixels * shift, (weight / convolution.divisor) as f32))
		.collect();
	let offset = convolution.offset as f32;
	let colors = SurroundedColors::new(plane, full, convolution.surroundings, convolution.reach(), document_to_pixels);

	let mut result = Plane::new(plane.width, plane.height);
	fill_rows(&mut result.data, plane.width, |y, row| {
		for (x, pixel) in row.iter_mut().enumerate() {
			let alpha = plane.data[y * plane.width + x][3];
			if alpha <= 0. {
				continue;
			}

			let sum = colors.sum(x as isize, y as isize, &taps).map(|channel| channel + offset);
			*pixel = uncovered(sum, alpha, convolution.surroundings);
		}
	});
	result
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
}
