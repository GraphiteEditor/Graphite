//! The image operations that effects are built from, on the CPU: blurring, growing shapes, and shifting.
//! They mirror the renderer's GPU passes, so a raster's effects read the same in a node that needs its pixels as on the canvas.

use raster_types::parallel_rows::fill_rows;

/// Blurs and spreads shorter than this, in pixels, leave no visible trace.
pub const NEGLIGIBLE_LENGTH: f64 = 0.01;

/// The largest Gaussian standard deviation, in pixels, convolved on the image itself rather than a halved copy.
pub const MAX_DIRECT_GAUSSIAN_SIGMA: f64 = 8.;

/// The largest box or tent half width, in pixels, convolved on the image itself rather than a halved copy.
pub const MAX_DIRECT_BOX_RADIUS: f64 = 24.;

/// The farthest texel from the center a kernel weighs, which fits every direct limit.
pub const MAX_KERNEL_REACH: usize = 24;

/// The most times an image is halved for a blur, by which point it is one pixel across.
const MAX_LEVELS: u32 = 16;

/// How many passes after jump flooding to `reach` look for nearer seeds through the neighbors one chamfer step away, which settles the near misses within about 20 pixels.
/// Since a chamfer length is the shortest path of such steps, a pixel whose neighbor along that path found the nearest seed finds it too.
pub fn chamfer_passes(reach: f64) -> usize {
	if reach > 16. { 4 } else { 2 }
}

/// The steps of a chamfer path (one pixel straight or diagonally, or a knight's move) by rows from the top, the order the GPU visits them in too.
const CHAMFER_STEPS: [[i32; 2]; 16] = [
	[-1, -2],
	[1, -2],
	[-2, -1],
	[-1, -1],
	[0, -1],
	[1, -1],
	[2, -1],
	[-1, 0],
	[1, 0],
	[-2, 1],
	[-1, 1],
	[0, 1],
	[1, 1],
	[2, 1],
	[-1, 2],
	[1, 2],
];

/// A blur's kernel along one axis, sized in pixels.
#[derive(Clone, Copy, Debug)]
pub enum BlurKernel {
	Gaussian {
		sigma: f64,
	},
	Box {
		radius: f64,
	},
	/// Weighs each texel by how far short of `radius` it lies, the shape of two box blurs of that width in a row.
	Tent {
		radius: f64,
	},
}

impl BlurKernel {
	/// How many times the image is halved before the kernel runs, so the kernel stays within [`MAX_KERNEL_REACH`].
	pub fn levels(self) -> u32 {
		let (length, direct_limit) = match self {
			Self::Gaussian { sigma } => (sigma, MAX_DIRECT_GAUSSIAN_SIGMA),
			Self::Box { radius } | Self::Tent { radius } => (radius, MAX_DIRECT_BOX_RADIUS),
		};

		(length / direct_limit).log2().ceil().clamp(0., MAX_LEVELS as f64) as u32
	}

	/// The weights of the texels from the center outward for an image halved `levels` times, normalized so the symmetric kernel sums to 1.
	pub fn weights(self, levels: u32) -> Vec<f64> {
		let level_scale = (1 << levels) as f64;

		let weights: Vec<f64> = match self {
			Self::Gaussian { sigma } => {
				// Halving the image and doubling it back blur it too, so the kernel makes up only the rest of the variance
				let pyramid_variance = (4_f64.powi(levels as i32) - 1.) / 3.;
				let sigma = (sigma * sigma - pyramid_variance).max(0.).sqrt() / level_scale;
				if sigma < NEGLIGIBLE_LENGTH {
					return vec![1.];
				}

				let reach = ((3. * sigma).ceil() as usize).min(MAX_KERNEL_REACH);
				(0..=reach).map(|i| (-((i * i) as f64) / (2. * sigma * sigma)).exp()).collect()
			}
			// The texels at the ends weigh only the fraction of them the box covers, so the blur grows smoothly with its radius
			Self::Box { radius } => {
				let radius = radius / level_scale;
				let reach = (radius.ceil() as usize).min(MAX_KERNEL_REACH);
				(0..=reach).map(|i| (radius + 1. - i as f64).clamp(0., 1.)).collect()
			}
			Self::Tent { radius } => {
				let radius = radius / level_scale;
				if radius <= 1. {
					return vec![1.];
				}

				let reach = (radius.ceil() as usize).min(MAX_KERNEL_REACH);
				(0..=reach).map(|i| (radius - i as f64).max(0.)).collect()
			}
		};

		let total = weights[0] + 2. * weights[1..].iter().sum::<f64>();
		weights.into_iter().map(|weight| weight / total).collect()
	}
}

/// A pixel value the operations can weigh, sum, and compare: one coverage channel, or premultiplied color channels.
pub trait Sample: Copy + Default + Send + Sync {
	fn scaled(self, factor: f32) -> Self;
	fn plus(self, other: Self) -> Self;
	fn max(self, other: Self) -> Self;
}

impl Sample for f32 {
	fn scaled(self, factor: f32) -> Self {
		self * factor
	}
	fn plus(self, other: Self) -> Self {
		self + other
	}
	fn max(self, other: Self) -> Self {
		f32::max(self, other)
	}
}

impl Sample for [f32; 4] {
	fn scaled(self, factor: f32) -> Self {
		self.map(|channel| channel * factor)
	}
	fn plus(self, other: Self) -> Self {
		std::array::from_fn(|channel| self[channel] + other[channel])
	}
	fn max(self, other: Self) -> Self {
		std::array::from_fn(|channel| self[channel].max(other[channel]))
	}
}

/// An image of [`Sample`]s in rows, which the operations read and write.
#[derive(Clone, Debug, PartialEq)]
pub struct Plane<T> {
	pub width: usize,
	pub height: usize,
	pub data: Vec<T>,
}

impl<T: Sample> Plane<T> {
	pub fn new(width: usize, height: usize) -> Self {
		Self {
			width,
			height,
			data: vec![T::default(); width * height],
		}
	}

	/// A plane of the same size whose pixels are `map` of this one's.
	pub fn map<U: Sample>(&self, map: impl Fn(T) -> U + Sync) -> Plane<U> {
		let mut mapped = Plane::new(self.width, self.height);
		fill_rows(&mut mapped.data, self.width, |y, row| {
			let source = &self.data[y * self.width..(y + 1) * self.width];
			row.iter_mut().zip(source).for_each(|(target, &value)| *target = map(value));
		});
		mapped
	}

	/// Combines this plane with `other`, of the same size, pixel by pixel.
	pub fn zip<U: Sample, V: Sample>(&self, other: &Plane<U>, combine: impl Fn(T, U) -> V + Sync) -> Plane<V> {
		let mut combined = Plane::new(self.width, self.height);
		fill_rows(&mut combined.data, self.width, |y, row| {
			let range = y * self.width..(y + 1) * self.width;
			let pairs = self.data[range.clone()].iter().zip(&other.data[range]);
			row.iter_mut().zip(pairs).for_each(|(target, (&a, &b))| *target = combine(a, b));
		});
		combined
	}

	/// The plane with its rows and columns swapped, so a horizontal operation can work vertically. O(width × height).
	fn transposed(&self) -> Self {
		let mut transposed = Self::new(self.height, self.width);
		fill_rows(&mut transposed.data, self.height, |x, row| {
			row.iter_mut().enumerate().for_each(|(y, target)| *target = self.data[y * self.width + x]);
		});
		transposed
	}

	/// Runs `operation` on each row, which it reads in full and writes as a new row of `width` pixels.
	fn map_rows(&self, width: usize, operation: impl Fn(&[T], &mut [T]) + Sync) -> Self {
		let mut result = Self::new(width, self.height);
		fill_rows(&mut result.data, width, |y, row| operation(&self.data[y * self.width..(y + 1) * self.width], row));
		result
	}

	/// Runs `along_rows` on the plane's rows, then on its columns by way of one transpose there and back.
	fn along_both_axes<P>(self, horizontal: P, vertical: P, along_rows: impl Fn(Self, P) -> Self) -> Self {
		let plane = along_rows(self, horizontal);
		along_rows(plane.transposed(), vertical).transposed()
	}

	/// Blurs the plane by a kernel along each axis. Kernels too wide to convolve directly run on a copy halved along their axis enough times
	/// for them to fit, then doubled back. O(width × height) per pass, with a pass for each halving and doubling.
	pub fn blurred(self, horizontal: BlurKernel, vertical: BlurKernel) -> Self {
		self.along_both_axes(horizontal, vertical, |mut plane, kernel| {
			let levels = kernel.levels();
			let length = plane.width;
			let level_length = |level: u32| length.div_ceil(1 << level);

			for level in 1..=levels {
				plane = plane.map_rows(level_length(level), halve_row);
			}

			let weights = kernel.weights(levels);
			plane = plane.map_rows(level_length(levels), |source, target| convolve_row(source, target, &weights));

			for level in (0..levels).rev() {
				plane = plane.map_rows(level_length(level), double_row);
			}
			plane
		})
	}

	/// Moves the image by a fractional offset in pixels, interpolating between pixels and leaving transparency where it uncovers the edges.
	pub fn shifted(self, offset_x: f64, offset_y: f64) -> Self {
		self.along_both_axes(offset_x, offset_y, |plane, offset| plane.map_rows(plane.width, |source, target| shift_row(source, target, offset)))
	}
}

/// The length of a whole-pixel offset along the shortest path of steps one pixel straight, one diagonally, or a knight's move away.
/// For PSD interop, layer styles measure distances this way, which rounds the corners they grow around into 16 sides rather than a circle.
pub fn chamfer_length([x, y]: [i32; 2]) -> f32 {
	let (long, short) = (x.abs().max(y.abs()) as f32, x.abs().min(y.abs()) as f32);
	match 2. * short <= long {
		// Knight's moves and straight steps
		true => (long - 2. * short) + 5_f32.sqrt() * short,
		// Knight's moves and diagonal steps
		false => 5_f32.sqrt() * (long - short) + std::f32::consts::SQRT_2 * (2. * short - long),
	}
}

impl Plane<f32> {
	/// Each pixel's distance to the silhouette's edge plus half a pixel, so a pixel beside a fully covered one lies one away and covered ones none, or infinity past `reach`.
	/// Distances run by [`chamfer_length`] to the nearest covered pixel, then by its uncovered share within it, placing an antialiased edge between pixels.
	pub fn distances_to_silhouette(&self, reach: f64) -> Self {
		self.jump_flood(reach, |coverage| coverage > 0., |offset, coverage| chamfer_length(offset) + (0.5 - coverage) + 0.5)
	}

	/// Each pixel's least [`chamfer_length`] to a seed plus the seed's own value, where the seeds are the pixels with finite values, or infinity past `reach`.
	pub fn distances_through_seeds(&self, reach: f64) -> Self {
		self.jump_flood(reach, f32::is_finite, |offset, value| chamfer_length(offset) + value)
	}

	/// Each pixel's least `distance` to a seed, given its offset from the seed and the seed's value, or infinity past `reach`.
	/// Jump flooding finds them in a pass for each halving of a step that starts past `reach`, so farther pixels may find none.
	/// Seeds of differing values leave near misses, mostly settled by two more halving passes and then those of [`chamfer_passes`] along the chamfer steps. O(width × height × log(reach)).
	pub(crate) fn jump_flood(&self, reach: f64, is_seed: impl Fn(f32) -> bool, distance: impl Fn([i32; 2], f32) -> f32 + Sync) -> Self {
		let mut nearest: Vec<Option<[i32; 2]>> = self.data.iter().map(|&value| is_seed(value).then_some([0, 0])).collect();

		let (columns, rows) = (self.width as i32, self.height as i32);
		// The offset runs from the seed to the pixel
		let distance = |x: i32, y: i32, offset: [i32; 2]| distance(offset, self.data[((y - offset[1]) * columns + x - offset[0]) as usize]);

		// The neighbors each pass looks at in the GPU's order: the eight a step away, then those one chamfer step away
		let first_step = (reach.clamp(0., (columns.max(rows)) as f64).ceil() as u32 + 1).next_power_of_two() as i32;
		let halvings = std::iter::successors(Some(first_step), |&step| (step > 1).then_some(step / 2)).chain([2, 1]);
		let adjacent = CHAMFER_STEPS.iter().filter(|[x, y]| x.abs().max(y.abs()) == 1);
		let around = |step: i32| adjacent.clone().map(|&[x, y]| [x * step, y * step]).collect::<Vec<_>>();
		let passes = halvings.map(around).chain(std::iter::repeat_n(CHAMFER_STEPS.to_vec(), chamfer_passes(reach)));

		for neighbors in passes {
			let previous = nearest.clone();

			// Each pixel keeps the nearest of the seeds that it and its neighbors have found so far
			fill_rows(&mut nearest, self.width, |y, row| {
				let y = y as i32;
				for (x, best) in row.iter_mut().enumerate() {
					let x = x as i32;
					let mut best_distance = best.map_or(f32::INFINITY, |offset| distance(x, y, offset));

					for &[neighbor_x, neighbor_y] in &neighbors {
						let (from_x, from_y) = (x + neighbor_x, y + neighbor_y);
						if from_x < 0 || from_y < 0 || from_x >= columns || from_y >= rows {
							continue;
						}
						let Some([offset_x, offset_y]) = previous[(from_y * columns + from_x) as usize] else { continue };

						let candidate = [offset_x - neighbor_x, offset_y - neighbor_y];
						let candidate_distance = distance(x, y, candidate);
						if candidate_distance < best_distance {
							*best = Some(candidate);
							best_distance = candidate_distance;
						}
					}
				}
			});
		}

		let data = nearest
			.into_iter()
			.enumerate()
			.map(|(index, offset)| offset.map_or(f32::INFINITY, |offset| distance(index as i32 % columns, index as i32 / columns, offset).max(0.)))
			.collect();
		Self {
			width: self.width,
			height: self.height,
			data,
		}
	}

	/// The silhouette grown outward by `distance` pixels, measured by [`chamfer_length`] so it rounds the corners it grows around.
	/// The grown edge lies half a pixel short of the farthest pixel center it reaches past, and fades out over one pixel.
	pub fn grown(&self, distance: f64) -> Self {
		let reach = distance as f32 + 1.;
		self.distances_to_silhouette(distance + 1.).map(|nearest| (reach - nearest).clamp(0., 1.))
	}
}

/// The row's value at a position measured in pixels from the first pixel's center, interpolating linearly and holding the end pixels beyond the ends.
fn sample_row<T: Sample>(row: &[T], position: f64) -> T {
	let last = row.len() as f64 - 1.;
	let position = position.clamp(0., last.max(0.));
	let index = position.floor() as usize;
	let fraction = (position - index as f64) as f32;

	match row.get(index + 1) {
		Some(&next) if fraction > 0. => row[index].scaled(1. - fraction).plus(next.scaled(fraction)),
		_ => row[index],
	}
}

/// Halves the row by averaging each pair of pixels, the last pixel of an odd row pairing with itself.
fn halve_row<T: Sample>(source: &[T], target: &mut [T]) {
	for (index, value) in target.iter_mut().enumerate() {
		let first = source[2 * index];
		let second = source.get(2 * index + 1).copied().unwrap_or(first);
		*value = first.plus(second).scaled(0.5);
	}
}

/// Doubles a halved row back to its length, interpolating at each pixel's center within the pair it was averaged into.
fn double_row<T: Sample>(source: &[T], target: &mut [T]) {
	for (index, value) in target.iter_mut().enumerate() {
		*value = sample_row(source, index as f64 / 2. - 0.25);
	}
}

/// Weighs each pixel's neighbors by the symmetric kernel `weights`, from the center outward, holding the end pixels beyond the ends.
fn convolve_row<T: Sample>(source: &[T], target: &mut [T], weights: &[f64]) {
	let last = source.len() as isize - 1;
	let at = |index: isize| source[index.clamp(0, last) as usize];

	for (index, value) in target.iter_mut().enumerate() {
		let index = index as isize;
		let mut sum = at(index).scaled(weights[0] as f32);
		for (distance, &weight) in weights.iter().enumerate().skip(1) {
			let pair = at(index - distance as isize).plus(at(index + distance as isize));
			sum = sum.plus(pair.scaled(weight as f32));
		}
		*value = sum;
	}
}

/// Moves the row by `offset` pixels, with nothing beyond its ends.
fn shift_row<T: Sample>(source: &[T], target: &mut [T], offset: f64) {
	let length = source.len() as f64;
	for (index, value) in target.iter_mut().enumerate() {
		let position = index as f64 - offset;
		let floor = position.floor();
		let fraction = (position - floor) as f32;

		let at = |index: f64| if index >= 0. && index < length { source[index as usize] } else { T::default() };
		*value = at(floor).scaled(1. - fraction).plus(at(floor + 1.).scaled(fraction));
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn kernel_weights_sum_to_one() {
		for kernel in [
			BlurKernel::Gaussian { sigma: 3. },
			BlurKernel::Gaussian { sigma: 40. },
			BlurKernel::Box { radius: 5.5 },
			BlurKernel::Box { radius: 100. },
		] {
			let levels = kernel.levels();
			let weights = kernel.weights(levels);
			let total = weights[0] + 2. * weights[1..].iter().sum::<f64>();

			assert!((total - 1.).abs() < 1e-9, "{kernel:?} weights sum to {total}");
			assert!(weights.len() <= MAX_KERNEL_REACH + 1, "{kernel:?} reaches {} texels", weights.len());
		}
	}

	#[test]
	fn small_kernels_run_without_halving() {
		assert_eq!(BlurKernel::Gaussian { sigma: MAX_DIRECT_GAUSSIAN_SIGMA }.levels(), 0);
		assert_eq!(
			BlurKernel::Gaussian {
				sigma: MAX_DIRECT_GAUSSIAN_SIGMA * 1.01
			}
			.levels(),
			1
		);
		assert_eq!(BlurKernel::Box { radius: 1. }.levels(), 0);
	}

	#[test]
	fn gaussian_blur_spreads_a_point_with_the_requested_deviation() {
		// A single bright pixel blurs into the Gaussian itself, whose variance the pyramid must preserve at every level count
		for sigma in [2., 20.] {
			let size = 401;
			let mut plane = Plane::<f32>::new(size, 1);
			plane.data[size / 2] = 1.;

			let blurred = plane.blurred(BlurKernel::Gaussian { sigma }, BlurKernel::Gaussian { sigma: 0. });
			let total: f32 = blurred.data.iter().sum();
			let variance: f64 = blurred.data.iter().enumerate().map(|(x, &value)| value as f64 * (x as f64 - (size / 2) as f64).powi(2)).sum::<f64>() / total as f64;

			assert!((total - 1.).abs() < 1e-3, "sigma {sigma} keeps the total at {total}");
			assert!((variance.sqrt() - sigma).abs() / sigma < 0.05, "sigma {sigma} spreads to a deviation of {}", variance.sqrt());
		}
	}

	#[test]
	fn tents_weigh_texels_by_how_far_short_of_the_radius_they_lie() {
		// For PSD interop, a size of 16 weighs the texels 16, 15, ..., 1 from the center outward
		let weights = BlurKernel::Tent { radius: 16. }.weights(0);
		assert_eq!(weights.len(), 17);
		for (distance, &weight) in weights.iter().enumerate() {
			assert!((weight - (16 - distance) as f64 / 256.).abs() < 1e-12, "texel {distance} weighs {weight}");
		}

		// A straight edge blurred by it reaches (S - i)(S + 1 - i) / 2S² at the i-th pixel past the edge
		let mut plane = Plane::<f32>::new(64, 1);
		plane.data[..32].fill(1.);
		let blurred = plane.blurred(BlurKernel::Tent { radius: 16. }, BlurKernel::Tent { radius: 0. });
		for i in 1..17 {
			let expected = ((16 - i) * (17 - i)) as f32 / 512.;
			assert!((blurred.data[31 + i] - expected).abs() < 1e-6, "pixel {i} past the edge is {}", blurred.data[31 + i]);
		}
	}

	#[test]
	fn growing_rounds_the_corners_it_grows_around() {
		// A single pixel grows into a disk the distance across, rather than a square
		let mut point = Plane::<f32>::new(21, 21);
		point.data[10 * 21 + 10] = 1.;
		let grown = point.grown(5.);
		let at = |x: usize, y: usize| grown.data[y * 21 + x];

		assert_eq!(at(15, 10), 1., "the full distance along an axis");
		assert_eq!(at(16, 10), 0., "past the distance");
		assert_eq!(at(15, 14), 0., "past the distance diagonally, where a square would still reach");
	}

	#[test]
	fn chamfer_lengths_are_exact_along_their_steps() {
		assert_eq!(chamfer_length([7, 0]), 7.);
		assert!((chamfer_length([-4, 4]) - 4. * std::f32::consts::SQRT_2).abs() < 1e-5);
		assert!((chamfer_length([2, -1]) - 5_f32.sqrt()).abs() < 1e-6);
		// Between the steps, the path runs a little longer than a straight line
		assert!((chamfer_length([3, 1]) - (1. + 5_f32.sqrt())).abs() < 1e-6);
		assert!((chamfer_length([3, 2]) - (5_f32.sqrt() + std::f32::consts::SQRT_2)).abs() < 1e-6);
	}

	#[test]
	fn distances_reach_no_farther_than_asked() {
		let mut plane = Plane::<f32>::new(64, 1);
		plane.data[0] = 1.;
		let distances = plane.distances_to_silhouette(8.);

		assert_eq!(distances.data[0], 0.);
		assert_eq!(distances.data[5], 5.);
		assert!(
			distances.data[63].is_infinite() || distances.data[63] == 63.,
			"a pixel far past the reach finds no seed or the true one"
		);
	}

	#[test]
	fn distances_from_seeds_of_differing_values_are_exact_within_their_reach() {
		// An antialiased disk whose edge pixels seed with head starts set by their coverage away from half, as a Soft Chisel bevel's do
		let side = 40;
		let disk = Plane {
			width: side,
			height: side,
			data: (0..side * side)
				.map(|index| {
					let (x, y) = ((index % side) as f64 + 0.5 - 20.3, (index / side) as f64 + 0.5 - 19.6);
					(12.5 - (x * x + y * y).sqrt()).clamp(0., 1.) as f32
				})
				.collect(),
		};
		let head_starts = disk.map(|alpha| ((alpha - 0.5) * 22.).max(0.));
		let reach = 11.;

		// O(n²) over the pixel pairs, small enough here
		let exactly = |values: &Plane<f32>, distance: &dyn Fn([i32; 2], f32) -> Option<f32>| {
			(0..side * side)
				.map(|index| {
					let (x, y) = ((index % side) as i32, (index / side) as i32);
					let candidates = (0..side * side).filter_map(|from| {
						let offset = [x - (from % side) as i32, y - (from / side) as i32];
						distance(offset, values.data[from])
					});
					candidates.fold(f32::INFINITY, f32::min)
				})
				.collect::<Vec<_>>()
		};

		let found = head_starts.distances_through_seeds(reach);
		let exact = exactly(&head_starts, &|offset, value| Some(chamfer_length(offset) + value));
		let found_edges = disk.distances_to_silhouette(reach);
		let exact_edges = exactly(&disk, &|offset, coverage| (coverage > 0.).then(|| chamfer_length(offset) + 1. - coverage));

		for index in 0..side * side {
			if exact[index] <= reach as f32 {
				assert!(
					(found.data[index] - exact[index]).abs() < 1e-4,
					"pixel {index} lies {} from the seeds, not {}",
					found.data[index],
					exact[index]
				);
			}
			if exact_edges[index] <= reach as f32 {
				assert!(
					(found_edges.data[index] - exact_edges[index]).abs() < 1e-4,
					"pixel {index} lies {} from the edge, not {}",
					found_edges.data[index],
					exact_edges[index]
				);
			}
		}
	}

	#[test]
	fn growing_the_outside_inward_covers_only_the_band_along_the_edges() {
		let mut square = Plane::<f32>::new(21, 21);
		for y in 2..19 {
			square.data[y * 21 + 2..y * 21 + 19].fill(1.);
		}
		let inward = square.map(|coverage| 1. - coverage).grown(3.);
		let at = |x: usize, y: usize| inward.data[y * 21 + x];

		assert_eq!(at(2, 10), 1., "at the edge");
		assert_eq!(at(4, 10), 1., "the band's full width in");
		assert_eq!(at(5, 10), 0., "past the band's width");
		assert_eq!(at(10, 10), 0., "deep inside");
	}

	#[test]
	fn shifting_moves_by_fractions_of_a_pixel() {
		let mut plane = Plane::<f32>::new(4, 1);
		plane.data[1] = 1.;

		let shifted = plane.shifted(0.25, 0.);

		assert_eq!(shifted.data, vec![0., 0.75, 0.25, 0.]);
	}
}
