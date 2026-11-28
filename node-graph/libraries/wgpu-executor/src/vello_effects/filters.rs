//! The GPU passes of the filter effects, matching the CPU's `filter_kernels`.

use super::{PARAMETER_TAPS, Parameters, PassRecorder, SUMS_FORMAT, WORKING_FORMAT};
use glam::{DAffine2, DMat2, DVec2};
use graphic_types::effects::spans_along_axes;
use graphic_types::filters::{
	Convolution, FieldSampling, NoiseDistribution, RandomField, SHARPEN_NEIGHBORS, SOBEL_TAPS, Surroundings, UnsharpBlur, VISIBLE_ALPHA, neighborhood_radii, noise_extent, sharpens_coverage, unsharp,
};
use graphic_types::{EffectOperation, MinimumMaximumMode};
use raster_kernels::effect_kernels::BlurKernel;
use raster_kernels::filter_kernels::pixel_fill_shifts;

/// The most texels a neighborhood's first pass reads each way, beyond which each further pass at most doubles its reach by reading two texels.
const MAX_FIRST_NEIGHBORHOOD_PASS_RADIUS: u32 = 8;

/// The `convolution`'s shifts mapped onto texels by `lengths_to_pixels`, with their weights divided by its divisor.
fn convolution_taps(convolution: &Convolution, lengths_to_pixels: DMat2) -> Vec<[f32; 4]> {
	let tap = |&(shift, weight): &(DVec2, f64)| {
		let shift = lengths_to_pixels * shift;
		[shift.x as f32, shift.y as f32, (weight / convolution.divisor) as f32, 0.]
	};
	convolution.taps.iter().map(tap).collect()
}

/// The [`SOBEL_TAPS`] mapped onto texels by `lengths_to_pixels`, with their weights across and down.
fn sobel_taps(lengths_to_pixels: DMat2) -> [[f32; 4]; 8] {
	SOBEL_TAPS.map(|(shift, [across, down])| {
		let shift = lengths_to_pixels * shift;
		[shift.x as f32, shift.y as f32, across as f32, down as f32]
	})
}

/// The parameters placing a random field's samples in each texel as `sampling` describes, matching the shader's `field_values`, with its integers split into exact halves.
fn field_parameters(sampling: &FieldSampling, first_seed: u32, second_seed: u32) -> Parameters {
	let halves = |value: u32| [(value & 0xffff) as f32, (value >> 16) as f32];
	let axes = sampling.pixel_axes;

	let mut taps = [[0.; 4]; PARAMETER_TAPS];
	taps[0] = [axes.x_axis.x, axes.x_axis.y, axes.y_axis.x, axes.y_axis.y].map(|value| value as f32);
	taps[1] = [sampling.origin_fraction.x as f32, sampling.origin_fraction.y as f32, 0., 0.];
	taps[2] = [sampling.counts.x as f32, sampling.counts.y as f32, sampling.spreads.x as f32, sampling.spreads.y as f32];
	let ([first_low, first_high], [second_low, second_high]) = (halves(first_seed), halves(second_seed));
	taps[3] = [first_low, first_high, second_low, second_high];
	let ([x_low, x_high], [y_low, y_high]) = (halves(sampling.origin_cell.x as u32), halves(sampling.origin_cell.y as u32));
	taps[4] = [x_low, x_high, y_low, y_high];
	Parameters { taps, ..Default::default() }
}

/// The passes of a random field filter, rendering its map for an SVG and carrying it out, with their parameters, or `None` for any other operation.
fn field_passes(operation: &EffectOperation, effect_to_pixels: DAffine2) -> Option<(&'static str, &'static str, Parameters)> {
	let sampling = FieldSampling::new(effect_to_pixels.inverse());
	match *operation {
		EffectOperation::AddNoise {
			amount,
			distribution,
			monochromatic,
			seed,
		} => {
			let amount = amount / 100.;
			let gaussian = if distribution == NoiseDistribution::Gaussian { 1. } else { 0. };
			let parameters = Parameters {
				values: [
					(amount * sampling.deviation_scale) as f32,
					noise_extent(amount, distribution) as f32,
					gaussian,
					if monochromatic { 1. } else { 0. },
				],
				..field_parameters(&sampling, RandomField::NoiseOffsets.seed(seed), RandomField::NoiseAngles.seed(seed))
			};
			Some(("noise_map", "add_noise", parameters))
		}
		EffectOperation::Dissolve { opacity, seed } => {
			let parameters = Parameters {
				values: [(opacity / 100.) as f32, sampling.deviation_scale as f32, 0., 0.],
				..field_parameters(&sampling, RandomField::Dissolve.seed(seed), 0)
			};
			Some(("dissolve_map", "dissolve", parameters))
		}
		_ => None,
	}
}

impl PassRecorder<'_> {
	/// Carries out a filter effect on both the full-strength content (`content` itself until a pass replaces it) and the faded fill, with `effect_to_pixels` placing the effect space on their pixels.
	/// A filter hands what an SVG takes as a map to `capture` as plane 0 for the content and 1 for the fill, and a random field's map, which an SVG asks for as `captured_plane` 0, serves both.
	#[expect(clippy::too_many_arguments)]
	pub(super) fn filter(
		&mut self,
		content: &wgpu::Texture,
		full: &mut Option<wgpu::Texture>,
		filled: &mut Option<wgpu::Texture>,
		operation: &EffectOperation,
		effect_to_pixels: DAffine2,
		captured_plane: Option<usize>,
		capture: &mut dyn FnMut(&mut Self, usize, &wgpu::Texture),
	) {
		// A random field doesn't depend on the content, so one map serves both planes, except where the fill forks them and an SVG takes each one's result
		if let Some((map_entry_point, filter_entry_point, parameters)) = field_passes(operation, effect_to_pixels) {
			let forked = filled.is_some();
			if captured_plane == Some(0) && !forked {
				let map = self.run(map_entry_point, WORKING_FORMAT, self.size, full.as_ref().unwrap_or(content), None, None, parameters);
				capture(self, 0, &map);
				self.textures.release(map);
			}

			let filtered = self.run(filter_entry_point, WORKING_FORMAT, self.size, full.as_ref().unwrap_or(content), None, None, parameters);
			if forked {
				capture(self, 0, &filtered);
			}
			self.replace_or_fill(full, filtered);

			if let Some(filled) = filled {
				let filtered = self.run(filter_entry_point, WORKING_FORMAT, self.size, filled, None, None, parameters);
				capture(self, 1, &filtered);
				self.replace(filled, filtered);
			}
			return;
		}

		// A filter sharpening the coverage fades the fill's result by the share of the full-strength coverage each texel's fill left, matching the CPU's `faded_by_shares`
		let unfiltered = full.as_ref().unwrap_or(content);
		let shares = filled
			.as_ref()
			.filter(|_| sharpens_coverage(operation))
			.map(|filled| self.run("fill_shares", WORKING_FORMAT, self.size, filled, None, Some(unfiltered), Parameters::default()));

		// The fill fades the content after the filter, so the full-strength content decides which of the fill's pixels are visible
		let filtered_full = self.filtered(unfiltered, unfiltered, operation, effect_to_pixels, 0, capture);
		let filtered_filled = match filled.as_ref() {
			Some(filled) if shares.is_some() => Some(self.filtered(filled, unfiltered, operation, effect_to_pixels, 1, &mut |_, _, _| {})),
			Some(filled) => Some(self.filtered(filled, unfiltered, operation, effect_to_pixels, 1, capture)),
			None => None,
		};
		self.replace_or_fill(full, filtered_full);
		let (Some(filled), Some(filtered)) = (filled.as_mut(), filtered_filled) else { return };

		let filtered = match shares {
			Some(shares) => {
				let full = full.as_ref().unwrap_or(content);
				let faded = self.run("faded_by_shares", WORKING_FORMAT, self.size, &filtered, Some(full), Some(&shares), Parameters::default());
				[filtered, shares].into_iter().for_each(|texture| self.textures.release(texture));
				capture(self, 1, &faded);
				faded
			}
			None => filtered,
		};
		self.replace(filled, filtered);
	}

	/// The image with a filter effect carried out on it, where the `full`-strength content covers it visibly, handing any map an SVG takes to `capture` as `plane`.
	fn filtered(
		&mut self,
		image: &wgpu::Texture,
		full: &wgpu::Texture,
		operation: &EffectOperation,
		effect_to_pixels: DAffine2,
		plane: usize,
		capture: &mut dyn FnMut(&mut Self, usize, &wgpu::Texture),
	) -> wgpu::Texture {
		let lengths_to_pixels = effect_to_pixels.matrix2;
		if let Some(convolution) = operation.convolution() {
			let convolved = self.convolve(image, full, &convolution, lengths_to_pixels);
			if !convolution.weighs_coverage {
				return self.uncover_capturing(convolved, image, convolution.surroundings, plane, capture);
			}

			// A kernel weighing the coverage too gives the colors the coverage it weighs, so an SVG off the pixel lattice takes the whole result as its map
			let coverage = self.run_in_chunks("sum_taps", SUMS_FORMAT, image, None, &convolution_taps(&convolution, lengths_to_pixels), [0., -1.]);
			let uncovered = self.uncover(convolved, &coverage, convolution.surroundings);
			self.textures.release(coverage);
			capture(self, plane, &uncovered);
			return uncovered;
		}

		// An SVG takes the whole result as its map where it can't sharpen natively, though a sharpened fill is captured once faded by shares
		if let Some((blur, sharpening)) = unsharp(operation) {
			let sharpening = sharpening.map(|[amount, threshold]| [amount as f32, threshold as f32]);
			let sharpened = self.unsharp_mask(image, full, blur, sharpening, lengths_to_pixels);
			capture(self, plane, &sharpened);
			return sharpened;
		}

		match *operation {
			EffectOperation::FindEdges => {
				let edges = self.find_edges(image, lengths_to_pixels);
				self.uncover_capturing(edges, image, Surroundings::Matted(1.), plane, capture)
			}
			// An SVG takes the whole result as its map where the fill forks the content
			EffectOperation::MinimumMaximum { radius, mode } => {
				let extreme = self.neighborhood_extreme(image, radius, mode == MinimumMaximumMode::Maximum, lengths_to_pixels);
				capture(self, plane, &extreme);
				extreme
			}
			_ => self.scale(image, 1.),
		}
	}

	/// Runs the shader's `entry_point` over the `taps` in passes of at most [`PARAMETER_TAPS`] each, so no pass runs long enough to stall the GPU, reading `source` and `secondary` with the shader's own `settings` in `values.x` and `y`.
	/// Each pass after the first takes on what those before it left (in `between_format`) as its `content`, with `values.z` 1, and the last gives the result in [`WORKING_FORMAT`], with `values.w` 1.
	fn run_in_chunks(
		&mut self,
		entry_point: &'static str,
		between_format: wgpu::TextureFormat,
		source: &wgpu::Texture,
		secondary: Option<&wgpu::Texture>,
		taps: &[[f32; 4]],
		settings: [f32; 2],
	) -> wgpu::Texture {
		let flag = |on: bool| if on { 1. } else { 0. };
		let passes = taps.len().div_ceil(PARAMETER_TAPS).max(1);

		let mut result: Option<wgpu::Texture> = None;
		for pass in 0..passes {
			let chunk = &taps[(pass * PARAMETER_TAPS).min(taps.len())..((pass + 1) * PARAMETER_TAPS).min(taps.len())];
			let mut tap_array = [[0.; 4]; PARAMETER_TAPS];
			tap_array[..chunk.len()].copy_from_slice(chunk);

			let last = pass + 1 == passes;
			let parameters = Parameters {
				tap_count: chunk.len() as u32,
				taps: tap_array,
				values: [settings[0], settings[1], flag(result.is_some()), flag(last)],
				..Default::default()
			};
			let format = if last { WORKING_FORMAT } else { between_format };
			let next = self.run(entry_point, format, self.size, source, secondary, result.as_ref(), parameters);
			if let Some(earlier) = result.replace(next) {
				self.textures.release(earlier);
			}
		}
		result.unwrap_or_else(|| self.scale(source, 1.))
	}

	/// The opaque weighted sum `convolution` describes of the image's colors, those visible judged by the `full`-strength content, matching the CPU's `convolve` before the image's coverage returns.
	fn convolve(&mut self, image: &wgpu::Texture, full: &wgpu::Texture, convolution: &Convolution, lengths_to_pixels: DMat2) -> wgpu::Texture {
		let colors = self.surrounded_colors(image, full, convolution.surroundings, convolution.reach(), lengths_to_pixels);
		let taps = convolution_taps(convolution, lengths_to_pixels);

		// Beyond the edges, continued colors read nothing, while matted ones read the gray they lie over
		let beyond = match convolution.surroundings {
			Surroundings::Continued => -1.,
			Surroundings::Matted(gray) => gray as f32,
		};
		// The sums of a kernel past one pass's taps carry between passes at full precision
		let convolved = self.run_in_chunks("convolve_taps", SUMS_FORMAT, &colors, None, &taps, [convolution.offset as f32, beyond]);

		self.textures.release(colors);
		convolved
	}

	/// The opaque colors a filter reading the image within `reach` document pixels sees, as its `surroundings` describe, those visible judged by the `full`-strength content, matching the CPU's `SurroundedColors`.
	fn surrounded_colors(&mut self, image: &wgpu::Texture, full: &wgpu::Texture, surroundings: Surroundings, reach: f64, lengths_to_pixels: DMat2) -> wgpu::Texture {
		match surroundings {
			Surroundings::Continued => self.visible_colors(image, full, reach, lengths_to_pixels),
			Surroundings::Matted(gray) => {
				let parameters = Parameters {
					values: [0., 0., gray as f32, 0.],
					..Default::default()
				};
				self.run("matte_colors", WORKING_FORMAT, self.size, image, None, None, parameters)
			}
		}
	}

	/// Filtered opaque `colors` given the image's coverage, as their `surroundings` describe, matching the CPU's `uncovered`, in place of the colors.
	fn uncover(&mut self, colors: wgpu::Texture, image: &wgpu::Texture, surroundings: Surroundings) -> wgpu::Texture {
		let gray = match surroundings {
			Surroundings::Continued => -1.,
			Surroundings::Matted(gray) => gray as f32,
		};
		let parameters = Parameters {
			values: [0., 0., gray, 0.],
			..Default::default()
		};
		let uncovered = self.run("uncover", WORKING_FORMAT, self.size, &colors, None, Some(image), parameters);
		self.textures.release(colors);
		uncovered
	}

	/// The filtered opaque `colors` given the image's coverage, as [`Self::uncover`] gives them, handing the map an SVG takes to `capture` as `plane`, matching the SVG's `uncovered_map`:
	/// the opaque colors for continued surroundings, which it places atop the image, and the whole result for matted ones.
	fn uncover_capturing(
		&mut self,
		colors: wgpu::Texture,
		image: &wgpu::Texture,
		surroundings: Surroundings,
		plane: usize,
		capture: &mut dyn FnMut(&mut Self, usize, &wgpu::Texture),
	) -> wgpu::Texture {
		if surroundings == Surroundings::Continued {
			capture(self, plane, &colors);
		}
		let uncovered = self.uncover(colors, image, surroundings);
		if surroundings != Surroundings::Continued {
			capture(self, plane, &uncovered);
		}
		uncovered
	}

	/// The image's straight colors as the filters read them, matching the CPU's `visible_colors`, with texels faint in the `full`-strength content taking the nearest visible color within `reach` document pixels.
	fn visible_colors(&mut self, image: &wgpu::Texture, full: &wgpu::Texture, reach: f64, lengths_to_pixels: DMat2) -> wgpu::Texture {
		let shifts: Vec<[f32; 4]> = pixel_fill_shifts(reach, lengths_to_pixels).into_iter().map(|[x, y]| [x as f32, y as f32, 0., 0.]).collect();
		self.run_in_chunks("visible_colors", WORKING_FORMAT, image, Some(full), &shifts, [VISIBLE_ALPHA, 0.])
	}

	/// The opaque result of Find Edges on the image matted over white, before its coverage returns, matching the CPU's `find_edges`.
	fn find_edges(&mut self, image: &wgpu::Texture, lengths_to_pixels: DMat2) -> wgpu::Texture {
		let colors = self.surrounded_colors(image, image, Surroundings::Matted(1.), 1., lengths_to_pixels);
		let edges = self.run_in_chunks("find_edges", WORKING_FORMAT, &colors, None, &sobel_taps(lengths_to_pixels), [0., 1.]);
		self.textures.release(colors);
		edges
	}

	/// For PSD interop, the share of its change Edges Only makes in each channel of each texel, the edge gain of the channel's Sobel gradient there, matching the CPU's `edge_gains`.
	/// The colors are read continued past those visible in the `full`-strength content, and the coverage with none beyond it.
	fn edge_gains(&mut self, image: &wgpu::Texture, full: &wgpu::Texture, lengths_to_pixels: DMat2) -> wgpu::Texture {
		// A faint texel the gradient reads finds its nearest visible color within the square of a pixel around it
		let colors = self.surrounded_colors(image, full, Surroundings::Continued, 1., lengths_to_pixels);
		let gains = self.run_in_chunks("edge_gains", WORKING_FORMAT, &colors, Some(image), &sobel_taps(lengths_to_pixels), [0., -1.]);
		self.textures.release(colors);
		gains
	}

	/// What the unsharp mask compares the image against for Edges Only: its straight colors (or for `coverage`, its coverage) moved toward their `neighbors`' average, consumed here, by the share their `gains` give.
	fn edges_only_average(&mut self, image: &wgpu::Texture, neighbors: wgpu::Texture, gains: &wgpu::Texture, coverage: bool) -> wgpu::Texture {
		let parameters = Parameters {
			values: [0., 0., if coverage { 1. } else { 0. }, 0.],
			..Default::default()
		};
		let average = self.run("edges_only_average", WORKING_FORMAT, self.size, image, Some(&neighbors), Some(gains), parameters);
		self.textures.release(neighbors);
		average
	}

	/// The image with each straight color and its coverage sharpened against the `blur` around it, by the amount and threshold fractions in `sharpening`,
	/// or with none, a high pass of its colors, those visible judged by the `full`-strength content, matching the CPU's `unsharp_mask`.
	fn unsharp_mask(&mut self, image: &wgpu::Texture, full: &wgpu::Texture, blur: UnsharpBlur, sharpening: Option<[f32; 2]>, lengths_to_pixels: DMat2) -> wgpu::Texture {
		let visibility = Parameters {
			values: [0., 0., VISIBLE_ALPHA, 0.],
			..Default::default()
		};
		let visible = self.run("visible_only", WORKING_FORMAT, self.size, image, Some(full), None, visibility);

		// A high pass keeps the coverage, so only unsharp masking blurs it
		let (blurred_colors, blurred_coverage) = match blur {
			UnsharpBlur::Gaussian(radius) => {
				let kernel = BlurKernel::Gaussian {
					sigma: radius.max(0.) * lengths_to_pixels.determinant().abs().sqrt(),
				};
				let blurred_colors = self.blur(visible, kernel, false);
				let blurred_coverage = sharpening.is_some().then(|| {
					let coverage = self.scale(image, 1.);
					self.blur(coverage, kernel, false)
				});
				(blurred_colors, blurred_coverage)
			}
			UnsharpBlur::EdgeNeighbors => {
				let gains = self.edge_gains(image, full, lengths_to_pixels);
				let neighbors = self.neighbors_average(&visible, lengths_to_pixels);
				self.textures.release(visible);
				let blurred_colors = self.edges_only_average(image, neighbors, &gains, false);
				let blurred_coverage = sharpening.is_some().then(|| {
					let neighbors = self.neighbors_average(image, lengths_to_pixels);
					self.edges_only_average(image, neighbors, &gains, true)
				});
				self.textures.release(gains);
				(blurred_colors, blurred_coverage)
			}
		};

		let [amount, threshold] = sharpening.unwrap_or_default();
		let parameters = Parameters {
			values: [amount, threshold, 0., if sharpening.is_none() { 1. } else { 0. }],
			..Default::default()
		};
		let sharpened = self.run("unsharp_mask", WORKING_FORMAT, self.size, image, Some(&blurred_colors), blurred_coverage.as_ref(), parameters);
		[Some(blurred_colors), blurred_coverage].into_iter().flatten().for_each(|texture| self.textures.release(texture));
		sharpened
	}

	/// The average in every channel of each texel's four nearest neighbors ([`SHARPEN_NEIGHBORS`]), with none beyond the edges, matching the CPU's `neighbors_average`.
	fn neighbors_average(&mut self, texture: &wgpu::Texture, lengths_to_pixels: DMat2) -> wgpu::Texture {
		let taps = SHARPEN_NEIGHBORS.map(|shift| {
			let shift = lengths_to_pixels * shift;
			[shift.x as f32, shift.y as f32, 0.25, 0.]
		});
		self.run_in_chunks("sum_taps", WORKING_FORMAT, texture, None, &taps, [0., -1.])
	}

	/// The image with each texel's color the lightest (or with `maximum` false, the darkest) in the square of `radius` document pixels around it,
	/// and its coverage the opposite extreme, matching the CPU's `neighborhood_extreme`. O(width × height × log radius), with the radius along each axis at most the texture's size along it.
	fn neighborhood_extreme(&mut self, image: &wgpu::Texture, radius: f64, maximum: bool, lengths_to_pixels: DMat2) -> wgpu::Texture {
		let spans = spans_along_axes(lengths_to_pixels);
		let ([near_x, far_x], fraction) = neighborhood_radii(radius, spans.x);
		let ([near_y, far_y], _) = neighborhood_radii(radius, spans.y);
		if near_x.max(near_y) == 0 && (fraction <= 0. || far_x.max(far_y) == 0) {
			return self.scale(image, 1.);
		}
		let maximum = if maximum { 1. } else { 0. };

		// A square reaching past the texture along an axis holds the same texels as one reaching just across it
		let [near_x, far_x] = [near_x, far_x].map(|radius| radius.min(self.size.x));
		let [near_y, far_y] = [near_y, far_y].map(|radius| radius.min(self.size.y));

		let prepared = self.run(
			"neighborhood_prepare",
			WORKING_FORMAT,
			self.size,
			image,
			None,
			None,
			Parameters {
				values: [0., 0., maximum, 0.],
				..Default::default()
			},
		);

		// The larger square's extremes grow from the smaller one's, which the finish reads in their place if the two squares are the same
		let near = match self.extremes_in_squares(&prepared, [0, 0], [near_x, near_y], maximum) {
			Some(near) => {
				self.textures.release(prepared);
				near
			}
			None => prepared,
		};
		let far = (fraction > 0.).then(|| self.extremes_in_squares(&near, [near_x, near_y], [far_x, far_y], maximum)).flatten();

		let parameters = Parameters {
			values: [fraction as f32, 0., maximum, 0.],
			..Default::default()
		};
		let finished = self.run("neighborhood_finish", WORKING_FORMAT, self.size, &near, far.as_ref(), None, parameters);
		[Some(near), far].into_iter().flatten().for_each(|texture| self.textures.release(texture));
		finished
	}

	/// The extremes of each rectangle reaching `to` texels across and down from each texel, from the `extremes` of those reaching `from`, along the rows then the columns,
	/// or `None` if they already reach as far. Each axis takes passes logarithmic in its reach, each reading two texels after a first one reading a few.
	fn extremes_in_squares(&mut self, extremes: &wgpu::Texture, from: [u32; 2], to: [u32; 2], maximum: f32) -> Option<wgpu::Texture> {
		let mut widened = None;
		for (axis, from, to) in [([1., 0.], from[0], to[0]), ([0., 1.], from[1], to[1])] {
			let mut reach = from;
			for radius in widened_radii(from, to) {
				// A window grows from nothing by reading each texel it reaches, then from the two windows either side spanning it
				let (entry_point, distance) = if reach == 0 { ("neighborhood_extreme", radius) } else { ("neighborhood_widen", radius - reach) };
				let parameters = Parameters {
					step: axis,
					values: [distance as f32, 0., maximum, 0.],
					..Default::default()
				};
				let next = self.run(entry_point, WORKING_FORMAT, self.size, widened.as_ref().unwrap_or(extremes), None, None, parameters);
				self.replace_or_fill(&mut widened, next);
				reach = radius;
			}
		}
		widened
	}
}

/// The radii a window of extremes widens through from `from` texels each way to `to`, each at most double the one before it, or from none, at most [`MAX_FIRST_NEIGHBORHOOD_PASS_RADIUS`].
/// Each is the next one halved and rounded up, so their count is logarithmic in `to`.
fn widened_radii(from: u32, to: u32) -> Vec<u32> {
	let mut radii = Vec::new();
	let mut radius = to;
	while radius > from {
		radii.push(radius);

		let reached_from = if from == 0 { MAX_FIRST_NEIGHBORHOOD_PASS_RADIUS } else { from.saturating_mul(2) };
		if radius <= reached_from {
			break;
		}
		radius = radius.div_ceil(2);
	}
	radii.reverse();
	radii
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn each_neighborhood_pass_at_most_doubles_the_reach_before_it() {
		for from in [0, 1, 2, 5, 8, 100] {
			for to in from..from + 1000 {
				let radii = widened_radii(from, to);
				assert_eq!(radii.last().copied().unwrap_or(from), to, "from {from} to {to}");

				let mut reach = from;
				for radius in radii {
					let most = if reach == 0 { MAX_FIRST_NEIGHBORHOOD_PASS_RADIUS } else { 2 * reach };
					assert!(reach < radius && radius <= most, "from {from} to {to}, {reach} widens to {radius}");
					reach = radius;
				}
			}
		}

		// A first pass of 7 texels each way, then six doublings
		assert_eq!(widened_radii(0, 400), [7, 13, 25, 50, 100, 200, 400]);
		assert_eq!(widened_radii(400, 404), [404]);
	}
}
