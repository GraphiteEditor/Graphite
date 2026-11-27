//! The GPU passes that mask, feather, and blur content by a radius mask, with their shaders in `vello_masks.wgsl`.

use super::{COVERAGE_FORMAT, Parameters, PassRecorder, SUMS_FORMAT, Seeds, WORKING_FORMAT};
use glam::DVec2;
use graphic_types::masks::{MaskMode, feather_parts, mask_density_line, masked_radius_blur_interpolation};
use raster_kernels::effect_kernels::{BlurKernel, NEGLIGIBLE_LENGTH, variable_box_scale};

impl PassRecorder<'_> {
	/// How much the `mask` graphic's texture shows the content at each pixel, from 0 to 1, matching the CPU's `mask_strength`.
	/// Its coverage is softened by a Gaussian of standard deviation `sigma` in pixels, then mapped through its `density` percentage, flipped if `invert`.
	pub(super) fn mask_strength(&mut self, mask: &wgpu::Texture, mode: MaskMode, density: f64, sigma: f64, invert: bool) -> wgpu::Texture {
		let mode = Parameters {
			values: [if mode == MaskMode::Alpha { 1. } else { 0. }, 0., 0., 0.],
			..Default::default()
		};
		let mut coverage = self.run("mask_coverage", COVERAGE_FORMAT, self.size, mask, None, None, mode);

		if sigma >= NEGLIGIBLE_LENGTH {
			coverage = self.blur(coverage, BlurKernel::Gaussian { sigma }, false);
		}

		let (slope, intercept) = mask_density_line(density, invert);
		let line = Parameters {
			values: [slope, intercept, 0., 0.],
			..Default::default()
		};
		let strength = self.run("mask_density_line", COVERAGE_FORMAT, self.size, &coverage, None, None, line);
		self.textures.release(coverage);
		strength
	}

	/// The `strength` of one more trailing mask multiplied into the `fade` of those before it, if any, freeing both.
	pub(super) fn faded_together(&mut self, fade: Option<wgpu::Texture>, strength: wgpu::Texture) -> wgpu::Texture {
		let Some(fade) = fade else { return strength };

		let together = self.run("apply_mask", COVERAGE_FORMAT, self.size, &fade, Some(&strength), None, Parameters::default());
		self.textures.release(fade);
		self.textures.release(strength);
		together
	}

	/// The full-strength `content`'s alpha with the trailing masks' `fade` beside it, for the passes that fade the opacity they work from by it.
	pub(super) fn content_with_layer_fade(&mut self, content: &wgpu::Texture, fade: &wgpu::Texture) -> wgpu::Texture {
		self.run("content_with_layer_fade", WORKING_FORMAT, self.size, content, Some(fade), None, Parameters::default())
	}

	/// How much a feather of `distance` pixels keeps of the `full`-strength content at each pixel, from 0 to 1, as `feather_parts` defines it, matching the CPU's `feather_strength`.
	pub(super) fn feather_strength(&mut self, full: &wgpu::Texture, distance: f64) -> wgpu::Texture {
		let outside = self.run("feather_outside", COVERAGE_FORMAT, self.size, full, None, None, Parameters::default());
		let (erosion, sigma) = feather_parts(distance);
		let to_outside = self.distances(&outside, erosion + 2., Seeds::CoveredStraight);

		let parameters = Parameters {
			values: [erosion as f32, 0., 0., 0.],
			..Default::default()
		};
		let eroded = self.run("feather_erode", COVERAGE_FORMAT, self.size, &to_outside, Some(&outside), None, parameters);
		self.textures.release(to_outside);
		self.textures.release(outside);

		match sigma >= NEGLIGIBLE_LENGTH {
			true => self.blur(eroded, BlurKernel::Gaussian { sigma }, false),
			false => eroded,
		}
	}

	/// Blurs both the full-strength content (`content` itself until a pass replaces it) and the faded fill by the interpolation between Gaussian `levels` (from `masked_radius_blur_levels`, here in pixels)
	/// the map's `luminance` picks, matching the CPU's `masked_radius_blur`. The `luminance` holds 8-bit codes as fractions of 255.
	/// The levels blur in linear light if `linear`, and the full-strength content is left as it is unless `full_is_read`.
	#[allow(clippy::too_many_arguments)]
	pub(super) fn masked_radius_blur(
		&mut self,
		content: &wgpu::Texture,
		full: &mut Option<wgpu::Texture>,
		filled: &mut Option<wgpu::Texture>,
		luminance: &wgpu::Texture,
		levels: &[f64],
		linear: bool,
		full_is_read: bool,
	) {
		// The level below and the share of the level above for each 8-bit luminance code, which are the same at any scale
		let blends: Vec<half::f16> = (0..=255_u8)
			.flat_map(|code| {
				let (below, above_parts) = masked_radius_blur_interpolation(code, levels);
				[below as f32, above_parts as f32 / 255., 0., 0.]
			})
			.map(half::f16::from_f32)
			.collect();
		let table = self.row_texture("vello_effects_masked_radius_blur", wgpu::TextureFormat::Rgba16Float, 256, bytemuck::cast_slice(&blends));
		let interpolation = self.run("masked_radius_blur_interpolation", WORKING_FORMAT, self.size, luminance, Some(&table), None, Parameters::default());

		if full_is_read && let Some(blurred) = self.masked_radius_blur_image(full.as_ref().unwrap_or(content), &interpolation, levels, linear) {
			self.replace_or_fill(full, blurred);
		}
		if let Some(filled) = filled
			&& let Some(blurred) = self.masked_radius_blur_image(filled, &interpolation, levels, linear)
		{
			self.replace(filled, blurred);
		}
		self.textures.release(interpolation);
	}

	/// The `image` blurred by each texel's `interpolation` between the masked radius blur's Gaussian `levels`, in pixels.
	fn masked_radius_blur_image(&mut self, image: &wgpu::Texture, interpolation: &wgpu::Texture, levels: &[f64], linear: bool) -> Option<wgpu::Texture> {
		let mut sum: Option<wgpu::Texture> = None;
		for (index, &sigma) in levels.iter().enumerate() {
			let blurred = (sigma >= NEGLIGIBLE_LENGTH).then(|| {
				let copy = self.scale(image, 1.);
				self.blur(copy, BlurKernel::Gaussian { sigma }, linear)
			});

			let parameters = Parameters {
				values: [index as f32, if sum.is_some() { 1. } else { 0. }, 0., 0.],
				..Default::default()
			};
			// Summed in gamma on purpose (see `masked_radius_blur_interpolation`)
			let source = blurred.as_ref().unwrap_or(image);
			let accumulated = self.run("accumulate_masked_radius_blur_level", WORKING_FORMAT, self.size, source, Some(interpolation), sum.as_ref(), parameters);
			[sum.replace(accumulated), blurred].into_iter().flatten().for_each(|texture| self.textures.release(texture));
		}
		sum
	}

	/// Box blurs both the full-strength content (`content` itself until a pass replaces it) and the faded fill at each texel by its own half width,
	/// the fraction of the `radius` in pixels along each axis that the map's unrounded `luminance` gives, matching the CPU's `variable_box_blur`.
	/// It blurs in linear light if `linear`, and leaves the full-strength content as it is unless `full_is_read`.
	#[allow(clippy::too_many_arguments)]
	pub(super) fn masked_box_blur(
		&mut self,
		content: &wgpu::Texture,
		full: &mut Option<wgpu::Texture>,
		filled: &mut Option<wgpu::Texture>,
		luminance: &wgpu::Texture,
		radius: DVec2,
		linear: bool,
		full_is_read: bool,
	) {
		if full_is_read {
			let blurred = self.variable_box_blur(full.as_ref().unwrap_or(content), luminance, radius, linear);
			self.replace_or_fill(full, blurred);
		}
		if let Some(filled) = filled {
			let blurred = self.variable_box_blur(filled, luminance, radius, linear);
			self.replace(filled, blurred);
		}
	}

	/// The `image` box blurred at each texel by its own half width (see [`Self::masked_box_blur`]), summing each box from running sums in two passes of two channels each.
	/// O(width × height × log16(width × height)) for the sums, and O(1) per texel at any radius for the boxes.
	fn variable_box_blur(&mut self, image: &wgpu::Texture, luminance: &wgpu::Texture, radius: DVec2, linear: bool) -> wgpu::Texture {
		let decoded = linear.then(|| self.run("decode_to_linear", WORKING_FORMAT, self.size, image, None, None, Parameters::default()));
		let scale = variable_box_scale(self.size.x as usize * self.size.y as usize);

		let mut blurred: Option<wgpu::Texture> = None;
		for pair in 0..2 {
			let sums = self.running_sums(decoded.as_ref().unwrap_or(image), pair, scale);
			let parameters = Parameters {
				amount: if linear { 1. } else { 0. },
				values: [radius.x as f32, radius.y as f32, scale as f32, pair as f32],
				..Default::default()
			};
			let source = blurred.as_ref().unwrap_or(image);
			let next = self.run("variable_box_blur", WORKING_FORMAT, self.size, source, Some(luminance), Some(&sums), parameters);

			[Some(sums), blurred.replace(next)].into_iter().flatten().for_each(|texture| self.textures.release(texture));
		}
		decoded.into_iter().for_each(|decoded| self.textures.release(decoded));

		blurred.unwrap_or_else(|| self.scale(image, 1.))
	}

	/// The running sums (a summed-area table) of two channels of the `image` (red and green for `pair` 0, blue and alpha for 1), counted in `scale` steps from 0 to 1,
	/// with two exact digits for each channel (see `RUNNING_SUM_DIGIT` in `vello_masks.wgsl`). Each pass sums blocks 16 times as long along an axis.
	fn running_sums(&mut self, image: &wgpu::Texture, pair: usize, scale: f64) -> wgpu::Texture {
		let digits = Parameters {
			values: [pair as f32, scale as f32, 0., 0.],
			..Default::default()
		};
		let mut sums = self.run("variable_box_digits", SUMS_FORMAT, self.size, image, None, None, digits);

		// The sums are read as the `content` input, the only one taking 32-bit floats, so the `image` fills the others
		for (axis, length) in [([1., 0.], self.size.x), ([0., 1.], self.size.y)] {
			let mut block = 1;
			while block < length {
				let parameters = Parameters {
					values: [axis[0], axis[1], block as f32, 0.],
					..Default::default()
				};
				let summed = self.run("variable_box_running_sums", SUMS_FORMAT, self.size, image, None, Some(&sums), parameters);
				self.replace(&mut sums, summed);
				block *= 16;
			}
		}
		sums
	}

	/// Fades both the full-strength content (`content` itself until a pass replaces it) and the faded fill by a `strength` from 0 to 1.
	pub(super) fn fade_by(&mut self, content: &wgpu::Texture, full: &mut Option<wgpu::Texture>, filled: &mut Option<wgpu::Texture>, strength: &wgpu::Texture) {
		if let Some(filled) = filled {
			let faded = self.run("apply_mask", WORKING_FORMAT, self.size, filled, Some(strength), None, Parameters::default());
			self.replace(filled, faded);
		}
		let faded = self.run("apply_mask", WORKING_FORMAT, self.size, full.as_ref().unwrap_or(content), Some(strength), None, Parameters::default());
		self.replace_or_fill(full, faded);
	}
}
