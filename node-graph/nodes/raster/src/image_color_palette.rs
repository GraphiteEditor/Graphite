use color::{ColorSpace, Oklab};
use core_types::color::Color;
use core_types::context::Ctx;
use core_types::list::{Item, List};
use raster_types::{CPU, Raster};
use std::collections::BinaryHeap;
use std::ops::{AddAssign, SubAssign};

/// Histogram resolution along each OkLab axis.
const CELLS_PER_AXIS: usize = 32;
/// Cumulative table length along each axis, with a leading slice of zeros so boxes at the lower edge need no special case.
const TABLE_SIDE: usize = CELLS_PER_AXIS + 1;
/// The sRGB gamut's extent along each OkLab axis, which the histogram spans. Colors outside it land in the edge cells.
const OKLAB_GAMUT_RANGES: [(f32, f32); 3] = [(0., 1.), (-0.24, 0.28), (-0.32, 0.2)];

/// Finds the colors that best represent the distinct color ranges in an image, ignoring transparent areas.
#[node_macro::node(category("Color"), icon("NodeImageColorPalette"))]
async fn image_color_palette(
	_: impl Ctx,
	/// The image to find colors in.
	image: Item<Raster<CPU>>,
	/// How many colors to find. Raising it adds a color at the end and may adjust one existing color, leaving the rest as they were.
	#[default(4)]
	#[hard(1..)]
	count: Item<i64>,
) -> List<Color> {
	let count = *count.element() as usize;

	// O(n) binning of the image's n pixels into an OkLab histogram, weighted by alpha so transparent pixels are ignored
	let mut cumulative = vec![Moments::default(); TABLE_SIDE.pow(3)];
	for pixel in image.element().data.iter() {
		let weight = (pixel.a().clamp(0., 1.) * f32::from(u16::MAX)).round() as i64;
		if weight == 0 {
			continue;
		}

		let color = Oklab::from_linear_srgb([pixel.r(), pixel.g(), pixel.b()]);
		if !color.iter().all(|channel| channel.is_finite()) {
			continue;
		}

		let position = std::array::from_fn(|axis| {
			let (minimum, maximum) = OKLAB_GAMUT_RANGES[axis];
			let cell = ((color[axis] - minimum) / (maximum - minimum) * CELLS_PER_AXIS as f32) as usize;
			cell.min(CELLS_PER_AXIS - 1) + 1
		});
		cumulative[table_index(position)] += Moments::new(weight, color.map(f64::from));
	}

	// O(1) conversion of the histogram into 3D cumulative sums by accumulating along each axis in turn
	for stride in [TABLE_SIDE * TABLE_SIDE, TABLE_SIDE, 1] {
		for index in 0..cumulative.len() {
			if (index / stride) % TABLE_SIDE != 0 {
				let previous = cumulative[index - stride];
				cumulative[index] += previous;
			}
		}
	}

	// An empty or fully transparent image has no colors
	let whole = ColorBox {
		lower: [0; 3],
		upper: [CELLS_PER_AXIS; 3],
	};
	if whole.moments(&cumulative).weight == 0 {
		return List::new();
	}

	// O(k log k) for k colors, as Wu's quantizer repeatedly splits whichever box has the largest squared error
	let mut boxes = vec![whole];
	let mut splittable = BinaryHeap::from([(whole.split_priority(&cumulative), 0)]);
	while boxes.len() < count
		&& let Some((_, index)) = splittable.pop()
	{
		let Some([larger, smaller]) = boxes[index].split(&cumulative) else { continue };

		// The larger half keeps its parent's place, so raising the count alters only one existing color and appends another
		boxes[index] = larger;
		boxes.push(smaller);

		splittable.push((larger.split_priority(&cumulative), index));
		splittable.push((smaller.split_priority(&cumulative), boxes.len() - 1));
	}

	boxes
		.iter()
		.map(|color_box| {
			// The sRGB gamut isn't convex in OkLab, so a mean of in-gamut colors can land slightly outside it
			let mean = color_box.moments(&cumulative).mean().map(|channel| channel as f32);
			let [red, green, blue] = Oklab::to_linear_srgb(mean).map(|channel| channel.clamp(0., 1.));

			Item::new_from_element(Color::from_rgbf32_unchecked(red, green, blue))
		})
		.collect()
}

/// Alpha-weighted statistics of a set of OkLab colors, from which their mean and squared error follow.
#[derive(Clone, Copy, Default)]
struct Moments {
	/// An integer, so empty boxes get exactly zero weight despite being computed by differencing large cumulative sums.
	weight: i64,
	sum: [f64; 3],
	sum_of_squares: f64,
}

impl Moments {
	fn new(weight: i64, color: [f64; 3]) -> Self {
		let scale = weight as f64;

		Self {
			weight,
			sum: color.map(|channel| scale * channel),
			sum_of_squares: scale * color.iter().map(|channel| channel * channel).sum::<f64>(),
		}
	}

	fn mean(&self) -> [f64; 3] {
		self.sum.map(|sum| sum / self.weight as f64)
	}

	/// The weight times the squared magnitude of the mean.
	fn weighted_squared_mean(&self) -> f64 {
		self.sum.iter().map(|sum| sum * sum).sum::<f64>() / self.weight as f64
	}

	/// The total squared OkLab distance of the colors from their mean.
	fn squared_error(&self) -> f64 {
		self.sum_of_squares - self.weighted_squared_mean()
	}
}

impl AddAssign for Moments {
	fn add_assign(&mut self, other: Self) {
		self.weight += other.weight;
		for (sum, other_sum) in self.sum.iter_mut().zip(other.sum) {
			*sum += other_sum;
		}
		self.sum_of_squares += other.sum_of_squares;
	}
}

impl SubAssign for Moments {
	fn sub_assign(&mut self, other: Self) {
		self.weight -= other.weight;
		for (sum, other_sum) in self.sum.iter_mut().zip(other.sum) {
			*sum -= other_sum;
		}
		self.sum_of_squares -= other.sum_of_squares;
	}
}

/// A block of histogram cells spanning from `lower` (exclusive) to `upper` (inclusive) along each axis of the cumulative table.
#[derive(Clone, Copy)]
struct ColorBox {
	lower: [usize; 3],
	upper: [usize; 3],
}

impl ColorBox {
	/// The total moments of the box's cells, found in O(1) by inclusion-exclusion over its eight corners in the cumulative table.
	fn moments(&self, cumulative: &[Moments]) -> Moments {
		let mut moments = Moments::default();

		for corner in 0..8_u32 {
			let position = std::array::from_fn(|axis| if corner & (1 << axis) == 0 { self.upper[axis] } else { self.lower[axis] });
			let corner_moments = cumulative[table_index(position)];

			// Corners taking an odd number of lower bounds are subtracted
			if corner.count_ones() % 2 == 0 {
				moments += corner_moments;
			} else {
				moments -= corner_moments;
			}
		}

		moments
	}

	/// A heap key ordering boxes by their squared error.
	fn split_priority(&self, cumulative: &[Moments]) -> u64 {
		// Nonnegative floats order the same as their bit patterns
		self.moments(cumulative).squared_error().max(0.).to_bits()
	}

	/// The halves of the cut minimizing their squared error, larger first, or `None` if no cut leaves both occupied.
	fn split(&self, cumulative: &[Moments]) -> Option<[ColorBox; 2]> {
		let total = self.moments(cumulative);
		let mut best: Option<(f64, [ColorBox; 2])> = None;

		for axis in 0..3 {
			for cut in self.lower[axis] + 1..self.upper[axis] {
				let mut lower_half = *self;
				lower_half.upper[axis] = cut;
				let mut upper_half = *self;
				upper_half.lower[axis] = cut;

				let lower_moments = lower_half.moments(cumulative);
				let mut upper_moments = total;
				upper_moments -= lower_moments;
				if lower_moments.weight == 0 || upper_moments.weight == 0 {
					continue;
				}

				// The halves' total `sum_of_squares` is fixed, so maximizing this minimizes their squared error
				let score = lower_moments.weighted_squared_mean() + upper_moments.weighted_squared_mean();
				if best.is_none_or(|(best_score, _)| score > best_score) {
					let halves = if lower_moments.weight >= upper_moments.weight {
						[lower_half, upper_half]
					} else {
						[upper_half, lower_half]
					};
					best = Some((score, halves));
				}
			}
		}

		best.map(|(_, halves)| halves)
	}
}

fn table_index(position: [usize; 3]) -> usize {
	(position[0] * TABLE_SIDE + position[1]) * TABLE_SIDE + position[2]
}

#[cfg(test)]
mod test {
	use super::*;
	use raster_types::Image;

	fn palette(runs: &[(Color, usize)], count: i64) -> Vec<Color> {
		let data = runs.iter().flat_map(|&(color, length)| std::iter::repeat_n(color, length)).collect::<Vec<_>>();
		let image = Raster::new_cpu(Image {
			width: data.len() as u32,
			height: 1,
			data,
			base64_string: None,
		});

		let palette = futures::executor::block_on(image_color_palette((), Item::new_from_element(image), Item::new_from_element(count)));
		palette.iter_element_values().copied().collect()
	}

	fn assert_colors_near(actual: &[Color], expected: &[Color]) {
		let near = actual.len() == expected.len()
			&& actual
				.iter()
				.zip(expected)
				.all(|(actual_color, expected_color)| actual_color.to_vec4().abs_diff_eq(expected_color.to_vec4(), 1e-4));
		assert!(near, "{actual:?} is not near {expected:?}");
	}

	#[test]
	fn uniform_image_yields_its_color() {
		assert_eq!(palette(&[(Color::BLACK, 10_000)], 1), [Color::BLACK]);
	}

	#[test]
	fn distinct_hues_separated() {
		let colors = palette(&[(Color::BLUE, 20), (Color::RED, 50), (Color::GREEN, 30)], 3);
		assert_colors_near(&colors, &[Color::RED, Color::GREEN, Color::BLUE]);
	}

	#[test]
	fn count_beyond_distinct_colors_returns_each_once() {
		let colors = palette(&[(Color::BLUE, 20), (Color::RED, 50), (Color::GREEN, 30)], i64::MAX);
		assert_colors_near(&colors, &[Color::RED, Color::GREEN, Color::BLUE]);
	}

	#[test]
	fn raising_count_changes_one_color_and_appends_another() {
		let runs = [
			(Color::from_rgbf32_unchecked(0.2, 0.4, 0.9), 15),
			(Color::from_rgbf32_unchecked(0.3, 0.5, 0.95), 15),
			(Color::from_rgbf32_unchecked(0.5, 0.7, 1.), 15),
			(Color::from_rgbf32_unchecked(0.1, 0.5, 0.1), 35),
			(Color::RED, 10),
			(Color::YELLOW, 10),
		];

		for count in 1..5 {
			let previous = palette(&runs, count);
			let next = palette(&runs, count + 1);

			assert_eq!(next.len(), previous.len() + 1);
			let changed = previous.iter().zip(&next).filter(|(previous_color, next_color)| previous_color != next_color).count();
			assert!(changed <= 1, "{previous:?} became {next:?}");
		}
	}

	#[test]
	fn transparent_pixels_ignored() {
		let colors = palette(&[(Color::TRANSPARENT, 900), (Color::RED, 100)], 2);
		assert_colors_near(&colors, &[Color::RED]);
	}

	#[test]
	fn semi_transparent_pixels_weigh_less() {
		// The red pixels outnumber the blue ones but each weighs a quarter as much, so blue is the larger half and comes first
		let colors = palette(&[(Color::RED.with_alpha(0.25), 60), (Color::BLUE, 40)], 2);
		assert_colors_near(&colors, &[Color::BLUE, Color::RED]);
	}

	#[test]
	fn images_without_visible_pixels_have_no_colors() {
		assert!(palette(&[], 4).is_empty());
		assert!(palette(&[(Color::TRANSPARENT, 100)], 4).is_empty());
	}

	#[test]
	fn non_finite_pixels_ignored() {
		let nan_channel = Color::from_rgbaf32_unchecked(f32::NAN, 0., 0., 1.);
		let nan_alpha = Color::from_rgbaf32_unchecked(0., 1., 0., f32::NAN);
		let colors = palette(&[(nan_channel, 10), (nan_alpha, 10), (Color::RED, 10)], 3);
		assert_colors_near(&colors, &[Color::RED]);
	}

	#[test]
	fn out_of_range_colors_clamped() {
		assert_colors_near(&palette(&[(Color::from_rgbf32_unchecked(5., 5., 5.), 10)], 1), &[Color::WHITE]);
		assert_colors_near(&palette(&[(Color::from_rgbf32_unchecked(-0.5, 0.2, 0.2), 10)], 1), &[Color::from_rgbf32_unchecked(0., 0.2, 0.2)]);
	}

	#[test]
	fn single_color_averages_whole_image() {
		// Halfway in OkLab between black and white is lightness 0.5, which cubes to 1/8 linear light
		let colors = palette(&[(Color::BLACK, 50), (Color::WHITE, 50)], 1);
		assert_colors_near(&colors, &[Color::from_rgbf32_unchecked(0.125, 0.125, 0.125)]);
	}
}
