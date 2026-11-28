//! The pixel filters on the CPU, which mirror the renderer's GPU passes on premultiplied gamma-encoded channels.

use crate::effect_kernels::Plane;
use glam::{DMat2, DVec2};
use graphic_types::EffectOperation;
use graphic_types::filters::{Convolution, VISIBLE_ALPHA, fill_shifts};
use raster_types::parallel_rows::fill_rows;

/// Carries out a filter effect on premultiplied gamma-encoded pixels, with its lengths measured through `document_to_pixels`, or returns them as they are for any other operation.
/// The pixels visible in the `full`-strength content (the plane itself for `None`) are those whose colors the filter reads.
pub fn filter(plane: Plane<[f32; 4]>, full: Option<&Plane<[f32; 4]>>, operation: &EffectOperation, document_to_pixels: DMat2) -> Plane<[f32; 4]> {
	match operation.convolution() {
		Some(convolution) => convolve(&plane, full.unwrap_or(&plane), &convolution, document_to_pixels),
		None => plane,
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

/// The premultiplied pixels with each one's color replaced by the weighted sum `convolution` describes, with its shifts measured through `document_to_pixels`.
/// It reads the [`visible_colors`] judged by the `full`-strength content (none where there are none, and beyond the edges), then divides, raises, and clamps the sum, keeping each pixel's coverage. O(width × height × taps).
pub fn convolve(plane: &Plane<[f32; 4]>, full: &Plane<[f32; 4]>, convolution: &Convolution, document_to_pixels: DMat2) -> Plane<[f32; 4]> {
	let taps: Vec<BilinearTap> = convolution
		.taps
		.iter()
		.map(|&(shift, weight)| BilinearTap::new(document_to_pixels * shift, (weight / convolution.divisor) as f32))
		.collect();
	let offset = convolution.offset as f32;

	let colors = visible_colors(plane, full, &pixel_fill_shifts(convolution.reach(), document_to_pixels));
	let (columns, rows) = (plane.width as isize, plane.height as isize);
	let at = |x: isize, y: isize| match x >= 0 && y >= 0 && x < columns && y < rows {
		true => colors.data[(y * columns + x) as usize],
		false => [0.; 4],
	};

	let mut result = Plane::new(plane.width, plane.height);
	fill_rows(&mut result.data, plane.width, |y, row| {
		for (x, pixel) in row.iter_mut().enumerate() {
			let index = y * plane.width + x;
			let alpha = plane.data[index][3];
			if alpha <= 0. {
				continue;
			}
			let (x, y) = (x as isize, y as isize);

			let mut sum = [offset; 3];
			for tap in &taps {
				let [shift_x, shift_y] = [x + tap.whole[0], y + tap.whole[1]];
				let weighted = tap.weighted([at(shift_x, shift_y), at(shift_x + 1, shift_y), at(shift_x, shift_y + 1), at(shift_x + 1, shift_y + 1)]);
				(0..3).for_each(|channel| sum[channel] += weighted[channel]);
			}

			let [red, green, blue] = sum.map(|channel| channel.clamp(0., 1.) * alpha);
			*pixel = [red, green, blue, alpha];
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
}
