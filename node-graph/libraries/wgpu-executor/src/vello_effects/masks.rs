//! The GPU passes that fade content by a mask or a feather, whose shaders are in `vello_masks.wgsl`.

use super::{COVERAGE_FORMAT, Parameters, PassRecorder, Seeds, WORKING_FORMAT};
use graphic_types::masks::{MaskMode, feather_parts, mask_density_line};
use raster_kernels::effect_kernels::{BlurKernel, NEGLIGIBLE_LENGTH};

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
