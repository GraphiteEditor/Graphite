//! The definitions the filter effects share across renderers: each pixel filter as weighted copies of the content shifted by document lengths, which the CPU, GPU, and SVG each carry out.

use crate::effects::{EffectOperation, Effects};
use core_types::graphene_hash::CacheHash;
use dyn_any::DynAny;
use glam::{DAffine2, DMat2, DVec2, IVec2, UVec2};

/// The most rows or columns a kernel takes, beyond which those around its center are kept, so its 65,025 weights at most keep each render within reach.
pub const MAX_KERNEL_SIDE: usize = 255;

/// The largest magnitude a weight divided by its divisor takes, so a kernel's sums stay finite.
const MAX_WEIGHT: f64 = 1e6;

/// The least alpha at which a pixel's own color is read, below which (for its faint, poorly known color) it reads the color of the nearest pixel at least this covered.
/// This is halfway between the sixteenth and fifteenth of 255 levels, so it divides 8-bit renders the same way everywhere.
pub const VISIBLE_ALPHA: f32 = 15.5 / 255.;

/// What a filter reads beyond the content's visible colors, and how its result returns to the content's coverage.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Surroundings {
	/// For PSD interop, a pixel too faint to be visible reads the nearest visible pixel's color (by [`fill_shifts`]) so colors continue past the edges, or with none within reach its own.
	/// The result keeps the content's coverage, unless the convolution weighs that too.
	Continued,
	/// The content is composited over an opaque gray of this level and filtered, then lifted back off the gray within its coverage, so its silhouette shows as an edge.
	Matted(f64),
}

/// A weighted sum of copies of the content's straight colors, each shifted by a length in document pixels, divided by a divisor and raised by an offset, keeping the content's coverage unless it weighs that too.
#[derive(Clone, Debug, PartialEq)]
pub struct Convolution {
	/// Each copy's shift from the pixel it adds into toward the pixel it reads, in document pixels, with its weight.
	pub taps: Vec<(DVec2, f64)>,
	/// What the weighted sum is divided by, which isn't zero.
	pub divisor: f64,
	/// How much the result's colors are raised, as a fraction of full brightness.
	pub offset: f64,
	/// What the sum reads beyond the content's visible colors, and how the result returns to the content's coverage.
	pub surroundings: Surroundings,
	/// Whether the coverage is weighed by the same copies, with none beyond the content and without the offset, then clamped, rather than kept.
	pub weighs_coverage: bool,
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
			surroundings: Surroundings::Continued,
			weighs_coverage: false,
		}
	}

	/// For PSD interop, emboss's difference between two samples `height` pixels apart along the direction the light comes from at `angle` degrees, scaled by the `amount` percentage and lifted to middle gray.
	/// Each sample spreads over the pixels less than a pixel from it, weighted by how far short of a pixel they lie.
	pub fn emboss(angle: f64, height: f64, amount: f64) -> Self {
		// The direction without the rounding left by sines and cosines of right angles, so samples on whole pixels land exactly on them
		let angle = angle.to_radians();
		let without_rounding = |value: f64| if value.abs() < 1e-12 { 0. } else { value };
		let half = DVec2::new(without_rounding(angle.cos()), without_rounding(-angle.sin())) * height.max(0.) / 2.;
		let spread = |shift: DVec2| (1. - shift.length()).max(0.);

		// Only the four pixels around each sample are less than a pixel from it, whatever the height. O(1).
		let mut shifts: Vec<DVec2> = [half, -half]
			.into_iter()
			.flat_map(|sample| [DVec2::ZERO, DVec2::X, DVec2::Y, DVec2::ONE].map(|corner| sample.floor() + corner))
			.collect();
		shifts.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
		shifts.dedup();
		let taps = shifts
			.into_iter()
			.map(|shift| (shift, (spread(shift - half) - spread(shift + half)) * amount / 100.))
			.filter(|(_, weight)| weight.abs() > 1e-12)
			.collect();

		Self {
			taps,
			divisor: 1.,
			offset: 0.5,
			surroundings: Surroundings::Matted(0.5),
			weighs_coverage: false,
		}
	}

	/// How far from a pixel the farthest copy reads along either axis, in document pixels.
	pub fn reach(&self) -> f64 {
		self.taps.iter().map(|(shift, _)| shift.abs().max_element()).fold(0., f64::max)
	}

	/// How far from a pixel its result reads in any direction, which a rotation can turn along an axis, in document pixels.
	/// That's the farthest copy, then for continued colors a faint pixel's nearest visible one within the square of [`Self::reach`], plus two pixels for rounding and copies between pixels.
	pub fn read_reach(&self) -> f64 {
		let farthest = self.taps.iter().map(|(shift, _)| shift.length()).fold(0., f64::max);
		let faint = match self.surroundings {
			Surroundings::Continued => std::f64::consts::SQRT_2 * self.reach().ceil(),
			Surroundings::Matted(_) => 0.,
		};
		farthest + faint + 2.
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

/// Whether the operation is a pixel filter, which processes the pixels of the content and everything drawn with it so far.
pub fn is_filter(operation: &EffectOperation) -> bool {
	matches!(
		operation,
		EffectOperation::ConvolutionKernel { .. }
			| EffectOperation::Emboss { .. }
			| EffectOperation::FindEdges
			| EffectOperation::MinimumMaximum { .. }
			| EffectOperation::Sharpen { .. }
			| EffectOperation::HighPass { .. }
			| EffectOperation::AddNoise { .. }
			| EffectOperation::Dissolve { .. }
	)
}

/// Whether the operation is a pixel filter reading the pixels around each one, its details and edges, rather than only the pixel itself and a random field.
pub fn reads_neighbors(operation: &EffectOperation) -> bool {
	is_filter(operation) && !is_random_field_filter(operation)
}

/// Whether the operation is a pixel filter combining each pixel with a random field fixed to the content, which changes endless content as much as any other.
pub fn is_random_field_filter(operation: &EffectOperation) -> bool {
	matches!(operation, EffectOperation::AddNoise { .. } | EffectOperation::Dissolve { .. })
}

/// Whether any of the effects is a pixel filter reading the pixels around each one that changes something, which reads endless content over the area in view.
/// The random field filters change endless content without its neighbors (see [`is_random_field_filter`]).
pub fn has_filters(effects: &Effects) -> bool {
	effects.operations().any(|operation| reads_neighbors(&operation) && !operation.changes_nothing())
}

/// For PSD interop, the shifts Find Edges reads its Sobel gradient from, in document pixels, with their weights across and down.
pub const SOBEL_TAPS: [(DVec2, [f64; 2]); 8] = [
	(DVec2::new(-1., -1.), [-1., -1.]),
	(DVec2::new(0., -1.), [0., -2.]),
	(DVec2::new(1., -1.), [1., -1.]),
	(DVec2::new(-1., 0.), [-2., 0.]),
	(DVec2::new(1., 0.), [2., 0.]),
	(DVec2::new(-1., 1.), [-1., 1.]),
	(DVec2::new(0., 1.), [0., 2.]),
	(DVec2::new(1., 1.), [1., 1.]),
];

/// The blurred visibility around a pixel below which unsharp masking compares the pixel against its own color, and above which against the average of the visible colors, easing between the two.
/// Below it, too little of the blur lands on visible pixels for 8-bit renders to know their average.
pub const AVERAGE_VISIBILITY_RAMP: [f32; 2] = [8. / 255., 24. / 255.];

/// For PSD interop, the middle gray a high pass centers its differences on, as an 8-bit level.
pub const HIGH_PASS_GRAY: f32 = 128. / 255.;

/// How much of the visible colors' average a pixel is compared against, the rest being its own color, given how much of the blur around it lands on visible pixels.
pub fn average_weight(visibility: f32) -> f32 {
	let [start, end] = AVERAGE_VISIBILITY_RAMP;
	((visibility - start) / (end - start)).clamp(0., 1.)
}

/// Whether the filter sharpens or otherwise weighs the content's coverage, so a fill must fade its result rather than its input.
pub fn sharpens_coverage(operation: &EffectOperation) -> bool {
	matches!(operation, EffectOperation::Sharpen { .. } | EffectOperation::ConvolutionKernel { apply_to_alpha: true, .. })
}

/// For PSD interop, the shifts of the four nearest neighbors in document pixels, whose average Edges Only compares each pixel against.
pub const SHARPEN_NEIGHBORS: [DVec2; 4] = [DVec2::new(0., -1.), DVec2::new(-1., 0.), DVec2::new(1., 0.), DVec2::new(0., 1.)];

/// For PSD interop, the Sobel gradient lengths (as Find Edges measures them, on values from 0 to 1) between which Edges Only ramps from no change to the whole of it.
/// A step between flat areas has a quarter of that length at a sixteenth of full contrast, and its whole length at a quarter.
pub const EDGE_GAIN_RAMP: [f32; 2] = [0.25, 1.];

/// For PSD interop, the share of its change that Edges Only makes in a channel whose Sobel gradient has the length `gradient`.
pub fn edge_gain(gradient: f32) -> f32 {
	let [start, end] = EDGE_GAIN_RAMP;
	((gradient - start) / (end - start)).clamp(0., 1.)
}

/// What unsharp masking or a high pass compares each pixel against.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UnsharpBlur {
	/// A Gaussian blur with this standard deviation in document pixels.
	Gaussian(f64),
	/// For Edges Only, the average of the four nearest neighbors, [`SHARPEN_NEIGHBORS`], read between pixels where they don't land on one.
	/// Each channel is moved only part of the way toward it, by its [`edge_gain`], so it changes only along edges.
	EdgeNeighbors,
}

/// What a Sharpen or High Pass compares each pixel against, with a Sharpen's amount and threshold as fractions, or `None` for any other operation.
pub fn unsharp(operation: &EffectOperation) -> Option<(UnsharpBlur, Option<[f64; 2]>)> {
	match *operation {
		EffectOperation::Sharpen {
			amount,
			radius,
			threshold,
			edges_only,
		} => {
			let blur = if edges_only { UnsharpBlur::EdgeNeighbors } else { UnsharpBlur::Gaussian(radius) };
			Some((blur, Some([amount / 100., threshold.max(0.) / 255.])))
		}
		EffectOperation::HighPass { radius } => Some((UnsharpBlur::Gaussian(radius), None)),
		_ => None,
	}
}

/// For PSD interop, unsharp masking's change to a value lying `difference` above the blur around it: the difference scaled by `amount`, softened toward none by `threshold`, both fractions.
pub fn unsharp_change(difference: f32, amount: f32, threshold: f32) -> f32 {
	difference.signum() * (amount * difference.abs() - threshold).max(0.)
}

/// The whole radii a neighborhood minimum or maximum of `radius` document pixels mixes between, measured in pixels by `pixels_per_length`, with how far it lies toward the larger.
pub fn neighborhood_radii(radius: f64, pixels_per_length: f64) -> ([u32; 2], f64) {
	let radius = radius.max(0.);
	let whole = |radius: f64| (radius * pixels_per_length).round() as u32;
	([whole(radius.floor()), whole(radius.ceil())], radius.fract())
}

/// The whole-pixel shifts out to `reach` pixels along each axis, nearest first, which a faint pixel looks along for the visible pixel to read the color of.
/// For PSD interop, ties go below before right, then left, then above. O(reach² log reach).
pub fn fill_shifts(reach: i32) -> Vec<[i32; 2]> {
	let mut shifts: Vec<[i32; 2]> = (-reach..=reach).flat_map(|y| (-reach..=reach).map(move |x| [x, y])).collect();
	shifts.sort_by_key(|&[x, y]| (x * x + y * y, -y, -x));
	shifts
}

/// How the random offsets Add Noise gives the colors are spread.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash, CacheHash, DynAny, node_macro::ChoiceType)]
#[widget(Radio)]
pub enum NoiseDistribution {
	/// Every offset up to the amount is equally likely.
	#[default]
	Uniform,
	/// Offsets cluster around none, with the amount as their standard deviation, so most are subtle and a few reach well past it.
	Gaussian,
}

/// For PSD interop, how many standard deviations Gaussian noise reaches, where its tails are cut off.
pub const GAUSSIAN_NOISE_LIMIT: f32 = 3.5;

/// The random fields a seed gives, each hashed apart so the fields of one seed are unrelated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RandomField {
	/// Add Noise's offset for each color, or for Gaussian noise its first uniform value.
	NoiseOffsets,
	/// Gaussian noise's second uniform value for each color, its angle.
	NoiseAngles,
	/// Dissolve's two thresholds, for the opacity and for the coverage.
	Dissolve,
}

impl RandomField {
	/// The value hashed with each cell for this field of the `seed`.
	pub fn seed(self, seed: i64) -> u32 {
		(seed as u32).wrapping_mul(4).wrapping_add(self as u32)
	}
}

/// The largest offset Add Noise of `amount` (a fraction) gives a color, as a fraction of full brightness, past which every color would be pushed to black or white anyway.
pub fn noise_extent(amount: f64, distribution: NoiseDistribution) -> f64 {
	let deviations = match distribution {
		NoiseDistribution::Uniform => 1.,
		NoiseDistribution::Gaussian => GAUSSIAN_NOISE_LIMIT as f64,
	};
	(amount.max(0.) * deviations).min(1.)
}

/// The most samples a pixel takes of a random field along each of its axes, which it takes when it spans that many cells or more.
pub const MAX_FIELD_SAMPLES_PER_AXIS: u32 = 4;

/// Where each pixel of a render reads a random field of one value per cell, the unit squares of the effect space, which fixes it to the content.
/// A pixel no larger than a cell reads the one under its center. A larger one averages samples spread across it, as a downsample of the field at full size would.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FieldSampling {
	/// The cell at the render's origin, kept apart from the rest of the way to it so 32-bit math on a GPU stays precise far from the effect space's origin.
	pub origin_cell: IVec2,
	/// The rest of the way from the origin's cell to the origin.
	pub origin_fraction: DVec2,
	/// A step of one pixel along each axis, in cells.
	pub pixel_axes: DMat2,
	/// How many samples are taken along each of a pixel's axes.
	pub counts: UVec2,
	/// How far apart the outermost samples are along each axis, in pixels, so each lands near the middle of a cell of its own.
	pub spreads: DVec2,
	/// For pixels spanning more cells than they sample, how much less an average of them all would deviate from the field's mean than the samples' average does.
	pub deviation_scale: f64,
}

impl FieldSampling {
	/// The sampling of a render with its pixels placed in the effect space by `pixels_to_effect`.
	pub fn new(pixels_to_effect: DAffine2) -> Self {
		let origin = pixels_to_effect.translation;
		let origin_cell = origin.floor();
		let pixel_axes = pixels_to_effect.matrix2;

		// A pixel spanning a cell give or take rounding takes one sample
		let spans = DVec2::new(pixel_axes.x_axis.length(), pixel_axes.y_axis.length());
		let counts = spans.map(|span| (span - 1e-3).ceil().clamp(1., MAX_FIELD_SAMPLES_PER_AXIS as f64));
		let spread = |span: f64, count: f64| if count > 1. { (span - 1.) / span * count / (count - 1.) } else { 0. };

		Self {
			origin_cell: origin_cell.as_ivec2(),
			origin_fraction: origin - origin_cell,
			pixel_axes,
			counts: counts.as_uvec2(),
			spreads: DVec2::new(spread(spans.x, counts.x), spread(spans.y, counts.y)),
			deviation_scale: (counts / spans).min(DVec2::ONE).element_product().sqrt(),
		}
	}

	/// The cell under the point `offset` pixels from the middle of the pixel at `pixel` (its column and row).
	fn cell_at(&self, pixel: [usize; 2], offset: DVec2) -> IVec2 {
		let position = DVec2::new(pixel[0] as f64, pixel[1] as f64) + 0.5 + offset;
		self.origin_cell.wrapping_add((self.origin_fraction + self.pixel_axes * position).floor().as_ivec2())
	}

	/// The cell under the middle of the pixel at `pixel`.
	pub fn center_cell(&self, pixel: [usize; 2]) -> IVec2 {
		self.cell_at(pixel, DVec2::ZERO)
	}

	/// The cells the pixel at `pixel` samples, row by row.
	pub fn sample_cells(&self, pixel: [usize; 2]) -> impl Iterator<Item = IVec2> + '_ {
		let place = |index: u32, count: u32, spread: f64| ((index as f64 + 0.5) / count as f64 - 0.5) * spread;
		(0..self.counts.y)
			.flat_map(move |row| (0..self.counts.x).map(move |column| self.cell_at(pixel, DVec2::new(place(column, self.counts.x, self.spreads.x), place(row, self.counts.y, self.spreads.y)))))
	}
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
	fn emboss_differences_samples_spread_around_the_light_direction() {
		// Light from the right at a height of 2 samples a pixel to each side, each exactly on one pixel
		let emboss = Convolution::emboss(0., 2., 100.);
		assert_eq!(emboss.taps, vec![(DVec2::new(-1., 0.), -1.), (DVec2::new(1., 0.), 1.)]);
		assert_eq!(emboss.offset, 0.5);

		// Halfway between pixels, each sample spreads over the two beside it
		let between = Convolution::emboss(90., 1., 200.);
		let weight = |shift: DVec2| between.taps.iter().find(|(tap, _)| *tap == shift).map_or(0., |(_, weight)| *weight);
		assert!((weight(DVec2::new(0., -1.)) - 1.).abs() < 1e-12);
		assert!((weight(DVec2::ZERO)).abs() < 1e-12);
		assert!((weight(DVec2::new(0., 1.)) + 1.).abs() < 1e-12);
	}

	#[test]
	fn faint_pixels_look_nearest_first_and_below_before_right() {
		let shifts = fill_shifts(1);
		assert_eq!(shifts.len(), 9);
		assert_eq!(&shifts[..5], &[[0, 0], [0, 1], [1, 0], [-1, 0], [0, -1]]);
		assert_eq!(shifts[5], [1, 1]);
	}
}
