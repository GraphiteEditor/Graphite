//! The GPU passes of the layer styles' finer options: the outline's paints.

use super::{COVERAGE_FORMAT, GRADIENT_SAMPLES, Parameters, PassRecorder, Seeds, WORKING_FORMAT};
use glam::DVec2;
use graphic_types::effects::GlowFill;
use raster_kernels::effect_kernels::NEGLIGIBLE_LENGTH;

/// An outline's coverage outside the content's edge and inside it, where it reaches, and each pixel's place across it for a contour gradient.
pub(super) struct OutlineBands {
	pub outside: Option<wgpu::Texture>,
	pub inside: Option<wgpu::Texture>,
	pub positions: Option<wgpu::Texture>,
}

impl PassRecorder<'_> {
	/// The bands of an outline reaching `outside` texels past the edge of the content's silhouette and `inside` texels within it, matching the CPU's `outline_bands`.
	/// With a contour gradient's `start` and `span` in texels, it also places each texel across the outline.
	pub(super) fn outline_bands(&mut self, content: &wgpu::Texture, outside: f64, inside: f64, contour_gradient: Option<(f64, f64)>) -> OutlineBands {
		let distances = |recorder: &mut Self, inverse: bool, reach: f64| {
			let silhouette = recorder.silhouette(content, DVec2::ZERO, inverse);
			let distances = recorder.distances(&silhouette, reach + 1., Seeds::Covered);
			recorder.textures.release(silhouette);
			distances
		};
		let outward = (outside >= NEGLIGIBLE_LENGTH || contour_gradient.is_some()).then(|| distances(self, false, outside));
		let inward = (inside >= NEGLIGIBLE_LENGTH || contour_gradient.is_some()).then(|| distances(self, true, inside));

		let positions = match (&outward, &inward, contour_gradient) {
			(Some(outward), Some(inward), Some((start, span))) => {
				let parameters = Parameters {
					values: [start as f32, span as f32, 0., 0.],
					..Default::default()
				};
				Some(self.run("contour_gradient_position", COVERAGE_FORMAT, self.size, outward, Some(inward), None, parameters))
			}
			_ => None,
		};

		// The silhouette grown by the band's width, as `grown` grows it
		let band = |recorder: &mut Self, distances: Option<wgpu::Texture>, width: f64| {
			let distances = distances?;
			let band = (width >= NEGLIGIBLE_LENGTH).then(|| {
				let parameters = Parameters {
					values: [width as f32, 0., 0., 0.],
					..Default::default()
				};
				recorder.run("grow", COVERAGE_FORMAT, recorder.size, &distances, None, None, parameters)
			});
			recorder.textures.release(distances);
			band
		};

		OutlineBands {
			outside: band(self, outward, outside),
			inside: band(self, inward, inside),
			positions,
		}
	}

	/// The `band`'s coverage of the premultiplied `paint`, in place of the band.
	pub(super) fn paint_band(&mut self, band: wgpu::Texture, paint: &wgpu::Texture) -> wgpu::Texture {
		let painted = self.run("paint_band", WORKING_FORMAT, self.size, &band, Some(paint), None, Parameters::default());
		self.textures.release(band);
		painted
	}

	/// The `band`'s coverage of a contour `gradient` at each texel's place across the outline, in place of the band, matching the CPU's `contour_gradient_colors`.
	pub(super) fn contour_gradient_band(&mut self, band: wgpu::Texture, positions: &wgpu::Texture, gradient: &GlowFill) -> wgpu::Texture {
		let Some((samples, opacity)) = gradient.gradient_samples(GRADIENT_SAMPLES as usize) else {
			return band;
		};
		let samples: Vec<half::f16> = samples.into_iter().flatten().map(half::f16::from_f32).collect();
		let samples = self.row_texture("vello_effects_gradient", wgpu::TextureFormat::Rgba16Float, GRADIENT_SAMPLES, bytemuck::cast_slice(&samples));

		let parameters = Parameters {
			amount: opacity,
			..Default::default()
		};
		let painted = self.run("contour_gradient_band", WORKING_FORMAT, self.size, positions, Some(&samples), Some(&band), parameters);
		self.textures.release(band);
		painted
	}
}
