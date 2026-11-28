//! The definitions the filter effects share across renderers: each pixel filter as weighted copies of the content shifted by document lengths, which the CPU, GPU, and SVG each carry out.

use crate::effects::{EffectOperation, Effects};
use glam::DVec2;

/// The most rows or columns a kernel takes, beyond which those around its center are kept, so its 65,025 weights at most keep each render within reach.
pub const MAX_KERNEL_SIDE: usize = 255;

/// The largest magnitude a weight divided by its divisor takes, so a kernel's sums stay finite.
const MAX_WEIGHT: f64 = 1e6;

/// The least alpha at which a pixel's own color is read, below which (for its faint, poorly known color) it reads the color of the nearest pixel at least this covered.
/// This is halfway between the sixteenth and fifteenth of 255 levels, so it divides 8-bit renders the same way everywhere.
pub const VISIBLE_ALPHA: f32 = 15.5 / 255.;

/// A weighted sum of copies of the content's straight colors, each shifted by a length in document pixels, divided by a divisor and raised by an offset, keeping the content's coverage.
/// For PSD interop, a pixel too faint to be visible reads the nearest visible pixel's color (by [`fill_shifts`]) so colors continue past the edges, or with none within reach its own.
#[derive(Clone, Debug, PartialEq)]
pub struct Convolution {
	/// Each copy's shift from the pixel it adds into toward the pixel it reads, in document pixels, with its weight.
	pub taps: Vec<(DVec2, f64)>,
	/// What the weighted sum is divided by, which isn't zero.
	pub divisor: f64,
	/// How much the result's colors are raised, as a fraction of full brightness.
	pub offset: f64,
}

impl Convolution {
	/// For PSD interop, the custom filter's kernel, given its weights row by row across `columns`, and kept to the [`MAX_KERNEL_SIDE`] rows and columns around its center.
	/// Each weight multiplies the pixel at its place in the kernel, whose center cell lies on the pixel being found: the middle row and column, or for an even count, the one after the middle.
	/// Weights that aren't finite count as zero, and a `scale` below 1 counts as 1.
	pub fn custom(weights: &[f64], columns: usize, scale: f64, offset: f64) -> Self {
		let scale = if scale.is_finite() { scale.max(1.) } else { 1. };

		let taps = kept_weights(weights, columns)
			.filter_map(|(shift, weight)| {
				let weight = if weight.is_finite() { (weight / scale).clamp(-MAX_WEIGHT, MAX_WEIGHT) } else { 0. };
				(weight != 0.).then_some((shift, weight))
			})
			.collect();

		Self {
			taps,
			divisor: 1.,
			offset: if offset.is_finite() { offset / 255. } else { 0. },
		}
	}

	/// How far from a pixel the farthest copy reads along either axis, in document pixels.
	pub fn reach(&self) -> f64 {
		self.taps.iter().map(|(shift, _)| shift.abs().max_element()).fold(0., f64::max)
	}

	/// How far from a pixel its result reads in any direction, which a rotation can turn along an axis, in document pixels.
	/// That's the farthest copy, then a faint pixel's nearest visible color within the square of [`Self::reach`], plus two pixels for rounding and copies between pixels.
	pub fn read_reach(&self) -> f64 {
		let farthest = self.taps.iter().map(|(shift, _)| shift.length()).fold(0., f64::max);
		farthest + std::f64::consts::SQRT_2 * self.reach().ceil() + 2.
	}

	/// Whether the result is the content as it is.
	pub fn is_identity(&self) -> bool {
		self.offset == 0. && matches!(self.taps.as_slice(), [(shift, weight)] if *shift == DVec2::ZERO && *weight == self.divisor)
	}
}

/// The weights of a kernel written as text, row by row with each row padded by zeros to the widest, and how many columns it has.
/// Each line is a row, with its numbers separated by spaces, commas, or both. Blank lines are skipped, and a number that can't be read counts as zero.
pub fn parse_kernel(text: &str) -> (Vec<f64>, usize) {
	let rows: Vec<Vec<f64>> = text
		.lines()
		.map(|line| {
			let tokens = line.split(|character: char| character.is_whitespace() || character == ',').filter(|token| !token.is_empty());
			tokens.map(|token| token.parse().unwrap_or(0.)).collect::<Vec<f64>>()
		})
		.filter(|row| !row.is_empty())
		.collect();

	let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
	let weights = rows.into_iter().flat_map(|row| {
		let padding = columns - row.len();
		row.into_iter().chain(std::iter::repeat_n(0., padding))
	});
	(weights.collect(), columns)
}

/// The scale that keeps a kernel's brightness the same: the sum of the finite weights [`Convolution::custom`] keeps, but at least 1.
pub fn automatic_scale(weights: &[f64], columns: usize) -> f64 {
	let sum: f64 = kept_weights(weights, columns).map(|(_, weight)| weight).filter(|weight| weight.is_finite()).sum();
	sum.clamp(1., f64::MAX)
}

/// The weights given row by row across `columns`, kept to the [`MAX_KERNEL_SIDE`] rows and columns around the center, each with its shift from the center cell.
fn kept_weights(weights: &[f64], columns: usize) -> impl Iterator<Item = (DVec2, f64)> + '_ {
	let full = [columns.max(1), weights.len().div_ceil(columns.max(1))];
	let kept = full.map(|count| count.min(MAX_KERNEL_SIDE));
	let start = [0, 1].map(|axis| full[axis] / 2 - kept[axis] / 2);
	let center = kept.map(|count| (count / 2) as f64);

	(0..kept[0] * kept[1]).filter_map(move |index| {
		let (column, row) = (index % kept[0], index / kept[0]);
		let weight = *weights.get((row + start[1]) * full[0] + column + start[0])?;
		Some((DVec2::new(column as f64 - center[0], row as f64 - center[1]), weight))
	})
}

/// Whether the operation is a pixel filter, which reads the content's details and edges.
pub fn is_filter(operation: &EffectOperation) -> bool {
	matches!(operation, EffectOperation::ConvolutionKernel { .. })
}

/// Whether any of the effects is a pixel filter that changes something, which reads endless content with details, like a gradient, over the area in view.
pub fn has_filters(effects: &Effects) -> bool {
	effects.operations().any(|operation| is_filter(&operation) && !operation.changes_nothing())
}

/// The whole-pixel shifts out to `reach` pixels along each axis, nearest first, which a faint pixel looks along for the visible pixel to read the color of.
/// For PSD interop, ties go below before right, then left, then above. O(reach² log reach).
pub fn fill_shifts(reach: i32) -> Vec<[i32; 2]> {
	let mut shifts: Vec<[i32; 2]> = (-reach..=reach).flat_map(|y| (-reach..=reach).map(move |x| [x, y])).collect();
	shifts.sort_by_key(|&[x, y]| (x * x + y * y, -y, -x));
	shifts
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn kernels_parse_a_row_from_each_line() {
		// Spaces, commas, and tabs separate the numbers, blank lines are skipped, and a number that can't be read counts as zero
		let (weights, columns) = parse_kernel("1, 2 ,3\r\n\n  4\t-5e-1  x \n\n 6,,7\n");
		assert_eq!(columns, 3);
		assert_eq!(weights, vec![1., 2., 3., 4., -0.5, 0., 6., 7., 0.]);

		// Rows shorter than the widest end in zeros
		assert_eq!(parse_kernel("1\n2 3 4"), (vec![1., 0., 0., 2., 3., 4.], 3));
		assert_eq!(parse_kernel(" \n\t\n"), (Vec::new(), 0));
	}

	#[test]
	fn weights_read_row_by_row_around_the_center() {
		// The weight right of the center reads the pixel to the right, divided by the scale
		let convolution = Convolution::custom(&[0., 0., 0., 0., 0., 2., 0., 0., 0.], 3, 4., 128.);
		assert_eq!(convolution.taps, vec![(DVec2::new(1., 0.), 0.5)]);
		assert!((convolution.offset - 128. / 255.).abs() < 1e-12);
		assert_eq!(convolution.reach(), 1.);

		assert!(Convolution::custom(&[3.], 1, 3., 0.).is_identity());
		assert!(!Convolution::custom(&[3.], 1, 1., 0.).is_identity());
		// A scale below 1 divides by 1, and a weight past the largest stays finite
		assert_eq!(Convolution::custom(&[2.], 1, 0., 0.).taps, vec![(DVec2::ZERO, 2.)]);
		assert_eq!(Convolution::custom(&[2.], 1, 0.5, 0.).taps, vec![(DVec2::ZERO, 2.)]);
		assert_eq!(Convolution::custom(&[1e300], 1, 1., 0.).taps, vec![(DVec2::ZERO, MAX_WEIGHT)]);
		// Weights that aren't finite count as zero
		assert!(Convolution::custom(&[f64::INFINITY, f64::NAN, 0., 0., 1., 0.], 3, 1., 0.).is_identity());
	}

	#[test]
	fn kernels_center_on_their_middle_row_and_column() {
		let shift = |x: f64, y: f64| DVec2::new(x, y);
		let shifts = |convolution: Convolution| convolution.taps.into_iter().map(|(shift, _)| shift).collect::<Vec<_>>();

		// A row reads across and a column reads down, each around its middle
		assert_eq!(shifts(Convolution::custom(&[1., 2., 3.], 3, 1., 0.)), vec![shift(-1., 0.), shift(0., 0.), shift(1., 0.)]);
		assert_eq!(shifts(Convolution::custom(&[1., 2., 3.], 1, 1., 0.)), vec![shift(0., -1.), shift(0., 0.), shift(0., 1.)]);

		// An even count centers on the cell after its middle, so a 2×2 kernel reaches only up and left
		let square = Convolution::custom(&[1., 2., 3., 4.], 2, 1., 0.);
		assert_eq!(square.taps, vec![(shift(-1., -1.), 1.), (shift(0., -1.), 2.), (shift(-1., 0.), 3.), (shift(0., 0.), 4.)]);
		let wide = Convolution::custom(&[1.; 8], 4, 1., 0.);
		assert_eq!((wide.taps[0].0, wide.taps[7].0), (shift(-2., -1.), shift(1., 0.)));
	}

	#[test]
	fn kernels_past_the_largest_keep_the_rows_and_columns_around_their_center() {
		// A kernel larger than one pass of the GPU's taps keeps every weight
		assert_eq!(Convolution::custom(&[1.; 121], 11, 1., 0.).taps.len(), 121);

		let side = MAX_KERNEL_SIDE + 2;
		let center = side / 2 * side + side / 2;
		let mut wide = vec![0.; side * side];
		wide[center] = 3.;
		wide[center + 1] = 2.;
		wide[0] = 1.;
		assert_eq!(Convolution::custom(&wide, side, 1., 0.).taps, vec![(DVec2::ZERO, 3.), (DVec2::X, 2.)]);
		assert_eq!(Convolution::custom(&vec![1.; side * side], side, 1., 0.).reach(), (MAX_KERNEL_SIDE / 2) as f64);
	}

	#[test]
	fn the_automatic_scale_is_the_sum_of_the_kept_weights_but_at_least_1() {
		assert_eq!(automatic_scale(&[1.; 9], 3), 9.);
		assert_eq!(automatic_scale(&[2., f64::NAN, 3., f64::INFINITY], 4), 5.);
		// A sum past the largest finite number stays finite, rather than counting as 1
		assert_eq!(automatic_scale(&[f64::MAX, f64::MAX], 2), f64::MAX);

		// Kernels summing below 1, like those finding edges, divide by 1
		assert_eq!(automatic_scale(&[0., -1., 0., -1., 5., -1., 0., -1., 0.], 3), 1.);
		assert_eq!(automatic_scale(&[-1., 0., 1.], 3), 1.);
		assert_eq!(automatic_scale(&[0.25, 0.25], 2), 1.);
		assert_eq!(automatic_scale(&[], 0), 1.);

		let side = MAX_KERNEL_SIDE + 2;
		assert_eq!(automatic_scale(&vec![1.; side * side], side), (MAX_KERNEL_SIDE * MAX_KERNEL_SIDE) as f64);
	}

	#[test]
	fn faint_pixels_look_nearest_first_and_below_before_right() {
		let shifts = fill_shifts(1);
		assert_eq!(shifts.len(), 9);
		assert_eq!(&shifts[..5], &[[0, 0], [0, 1], [1, 0], [-1, 0], [0, -1]]);
		assert_eq!(shifts[5], [1, 1]);
	}
}
