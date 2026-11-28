//! The GPU passes of the filter effects, matching the CPU's `filter_kernels`.

use super::{PARAMETER_TAPS, Parameters, PassRecorder, SUMS_FORMAT, WORKING_FORMAT};
use glam::{DMat2, DVec2};
use graphic_types::EffectOperation;
use graphic_types::filters::{Convolution, VISIBLE_ALPHA};
use raster_kernels::filter_kernels::pixel_fill_shifts;

impl PassRecorder<'_> {
	/// Carries out a filter effect on both the full-strength content (`content` itself until a pass replaces it) and the faded fill, with its lengths mapped onto pixels by `lengths_to_pixels`.
	/// A filter whose result an SVG takes as a map hands it to `capture` as plane 0 for the content and 1 for the fill.
	pub(super) fn filter(
		&mut self,
		content: &wgpu::Texture,
		full: &mut Option<wgpu::Texture>,
		filled: &mut Option<wgpu::Texture>,
		operation: &EffectOperation,
		lengths_to_pixels: DMat2,
		capture: &mut dyn FnMut(&mut Self, usize, &wgpu::Texture),
	) {
		// The fill fades the content after the filter, so the full-strength content decides which of the fill's pixels are visible
		let unfiltered = full.as_ref().unwrap_or(content);
		let filtered_full = self.filtered(unfiltered, unfiltered, operation, lengths_to_pixels, 0, capture);
		let filtered_filled = filled.as_ref().map(|filled| self.filtered(filled, unfiltered, operation, lengths_to_pixels, 1, capture));

		self.replace_or_fill(full, filtered_full);
		if let (Some(filled), Some(filtered)) = (filled.as_mut(), filtered_filled) {
			self.replace(filled, filtered);
		}
	}

	/// The image with a filter effect carried out on it, where the `full`-strength content covers it visibly, handing any map an SVG takes to `capture` as `plane`.
	fn filtered(
		&mut self,
		image: &wgpu::Texture,
		full: &wgpu::Texture,
		operation: &EffectOperation,
		lengths_to_pixels: DMat2,
		plane: usize,
		capture: &mut dyn FnMut(&mut Self, usize, &wgpu::Texture),
	) -> wgpu::Texture {
		let Some(convolution) = operation.convolution() else { return self.scale(image, 1.) };

		let convolved = self.convolve(image, full, &convolution, lengths_to_pixels);
		capture(self, plane, &convolved);
		self.uncover(convolved, image)
	}

	/// Runs the shader's `entry_point` over the `taps` in passes of at most [`PARAMETER_TAPS`] each, so no pass runs long enough to stall the GPU, reading `source` and `secondary` with the shader's own `setting`.
	/// Each pass after the first takes on what those before it left (in `between_format`) as its `content`, with `values.y` 1, and the last gives the result in [`WORKING_FORMAT`], with `values.z` 1.
	fn run_in_chunks(
		&mut self,
		entry_point: &'static str,
		between_format: wgpu::TextureFormat,
		source: &wgpu::Texture,
		secondary: Option<&wgpu::Texture>,
		taps: &[[f32; 4]],
		setting: f32,
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
				values: [setting, flag(result.is_some()), flag(last), 0.],
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

	/// The opaque weighted sum `convolution` describes of the image's colors where the `full`-strength content is visible, matching the CPU's `convolve` before the image's coverage returns.
	fn convolve(&mut self, image: &wgpu::Texture, full: &wgpu::Texture, convolution: &Convolution, lengths_to_pixels: DMat2) -> wgpu::Texture {
		let colors = self.visible_colors(image, full, convolution.reach(), lengths_to_pixels);

		let taps: Vec<[f32; 4]> = convolution
			.taps
			.iter()
			.map(|&(shift, weight)| {
				let shift: DVec2 = lengths_to_pixels * shift;
				[shift.x as f32, shift.y as f32, (weight / convolution.divisor) as f32, 0.]
			})
			.collect();
		// The sums of a kernel past one pass's taps carry between passes at full precision
		let convolved = self.run_in_chunks("convolve_taps", SUMS_FORMAT, &colors, None, &taps, convolution.offset as f32);

		self.textures.release(colors);
		convolved
	}

	/// Filtered opaque straight `colors` given the image's coverage, in place of the colors.
	fn uncover(&mut self, colors: wgpu::Texture, image: &wgpu::Texture) -> wgpu::Texture {
		let uncovered = self.run("uncover", WORKING_FORMAT, self.size, &colors, None, Some(image), Parameters::default());
		self.textures.release(colors);
		uncovered
	}

	/// The image's straight colors as the filters read them, matching the CPU's `visible_colors`, with texels faint in the `full`-strength content taking the nearest visible color within `reach` document pixels.
	fn visible_colors(&mut self, image: &wgpu::Texture, full: &wgpu::Texture, reach: f64, lengths_to_pixels: DMat2) -> wgpu::Texture {
		let shifts: Vec<[f32; 4]> = pixel_fill_shifts(reach, lengths_to_pixels).into_iter().map(|[x, y]| [x as f32, y as f32, 0., 0.]).collect();
		self.run_in_chunks("visible_colors", WORKING_FORMAT, image, Some(full), &shifts, VISIBLE_ALPHA)
	}
}
