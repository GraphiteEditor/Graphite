//! The image operations that effects are built from, on the CPU: blurring, growing shapes, and shifting.
//! They mirror the renderer's GPU passes, so a raster's effects read the same in a node that needs its pixels as on the canvas.

use raster_types::parallel_rows::fill_rows;

/// Blurs and spreads shorter than this, in pixels, leave no visible trace.
pub const NEGLIGIBLE_LENGTH: f64 = 0.01;

/// The largest Gaussian standard deviation, in pixels, convolved on the image itself rather than a halved copy.
pub const MAX_DIRECT_GAUSSIAN_SIGMA: f64 = 8.;

/// The largest box half width, in pixels, convolved on the image itself rather than a halved copy.
pub const MAX_DIRECT_BOX_RADIUS: f64 = 24.;

/// The farthest texel from the center a kernel weighs, which fits both direct limits.
pub const MAX_KERNEL_REACH: usize = 24;

/// The most times an image is halved for a blur, by which point it is one pixel across.
const MAX_LEVELS: u32 = 16;

/// A blur's kernel along one axis, sized in pixels.
#[derive(Clone, Copy, Debug)]
pub enum BlurKernel {
	Gaussian { sigma: f64 },
	Box { radius: f64 },
}

impl BlurKernel {
	/// How many times the image is halved before the kernel runs, so the kernel stays within [`MAX_KERNEL_REACH`].
	pub fn levels(self) -> u32 {
		let (length, direct_limit) = match self {
			Self::Gaussian { sigma } => (sigma, MAX_DIRECT_GAUSSIAN_SIGMA),
			Self::Box { radius } => (radius, MAX_DIRECT_BOX_RADIUS),
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

	/// Grows the shapes by a distance in pixels along each axis, in a square like SVG's `feMorphology`.
	pub fn dilated(self, horizontal: f64, vertical: f64) -> Self {
		self.along_both_axes(horizontal, vertical, |mut plane, distance| {
			let length = plane.width;

			// Steps that double, then the remainder, so each step grows the shapes by up to as much as all the steps before it
			let (mut covered, mut step) = (0., 1.);
			while covered + step <= distance {
				plane = plane.map_rows(length, |source, target| dilate_row(source, target, step));
				covered += step;
				step *= 2.;
			}
			if distance - covered >= NEGLIGIBLE_LENGTH {
				plane = plane.map_rows(length, |source, target| dilate_row(source, target, distance - covered));
			}
			plane
		})
	}

	/// Moves the image by a fractional offset in pixels, interpolating between pixels and leaving transparency where it uncovers the edges.
	pub fn shifted(self, offset_x: f64, offset_y: f64) -> Self {
		self.along_both_axes(offset_x, offset_y, |plane, offset| plane.map_rows(plane.width, |source, target| shift_row(source, target, offset)))
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

/// Takes each pixel's maximum with the row's values `step` pixels to either side.
fn dilate_row<T: Sample>(source: &[T], target: &mut [T], step: f64) {
	for (index, value) in target.iter_mut().enumerate() {
		let position = index as f64;
		*value = source[index].max(sample_row(source, position - step)).max(sample_row(source, position + step));
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
	fn dilation_grows_shapes_by_the_distance() {
		let mut plane = Plane::<f32>::new(21, 21);
		plane.data[10 * 21 + 10] = 1.;

		let dilated = plane.dilated(3., 3.);
		let covered = dilated.data.iter().filter(|&&value| value >= 0.999).count();

		assert_eq!(covered, 7 * 7, "a point grows into a 7 by 7 square");
	}

	#[test]
	fn shifting_moves_by_fractions_of_a_pixel() {
		let mut plane = Plane::<f32>::new(4, 1);
		plane.data[1] = 1.;

		let shifted = plane.shifted(0.25, 0.);

		assert_eq!(shifted.data, vec![0., 0.75, 0.25, 0.]);
	}
}
