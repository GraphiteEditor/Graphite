//! The GPU passes of the layer styles' finer options: the outline's paints, and the bevel's contour and texture.

use super::{COVERAGE_FORMAT, GRADIENT_SAMPLES, PARAMETER_TAPS, Parameters, PassRecorder, Seeds, WORKING_FORMAT};
use core_types::transfer_curve::TransferCurve;
use glam::{DVec2, DVec3};
use graphic_types::BevelStyle;
use graphic_types::effects::{BEVEL_FOOT_FADE, GlowFill, contour_end_slopes, slope_contour_reshapes};
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

/// What shapes a bevel's surface besides its height: a contour reshaping its slopes across a percentage of them, and a texture's gray sinking it by a percentage of its span.
pub(super) struct BevelSurface<'a> {
	pub style: BevelStyle,
	pub slope_contour: &'a TransferCurve,
	pub slope_range: f64,
	/// Whether the slope contour is sampled at nine points across each texel.
	pub slope_antialiased: bool,
	pub texture: Option<&'a wgpu::Texture>,
	pub texture_depth: f64,
}

impl PassRecorder<'_> {
	/// How much brighter than flat ground the `light` lights each texel of a bevel's surface, negative where darker, with its `height` running from -0.5 at its slopes' foot to 0.5 at their top.
	/// The slopes rise `rise` texels as the `surface` options shape them. Matches the CPU's `bevel_shading` before its gloss contour.
	/// With `direct` as `Some(shadow)`, it's instead the highlight, or if `shadow` the shadow, that makes without a gloss contour or a fade at the foot.
	pub(super) fn bevel_lit(&mut self, height: &wgpu::Texture, surface: &BevelSurface, rise: f64, light: DVec3, direct: Option<bool>) -> wgpu::Texture {
		let reshaped = slope_contour_reshapes(surface.slope_contour, surface.style, surface.slope_range);
		let (start_slope, end_slope) = contour_end_slopes(&surface.slope_contour.evaluator());
		let samples = reshaped.then(|| self.contour_samples(surface.slope_contour));
		let style_number = match surface.style {
			BevelStyle::InnerBevel => 0.,
			BevelStyle::OuterBevel => 1.,
			BevelStyle::Emboss | BevelStyle::PillowEmboss => 2.,
		};

		let mut taps = [[0.; 4]; PARAMETER_TAPS];
		taps[0] = [style_number, (surface.slope_range.clamp(1., 100.) / 100.) as f32, start_slope as f32, end_slope as f32];
		taps[1] = [
			if reshaped { 1. } else { 0. },
			if reshaped && surface.slope_antialiased { 1. } else { 0. },
			if surface.texture.is_some() { (surface.texture_depth / 100.) as f32 } else { 0. },
			match direct {
				None => 0.,
				Some(false) => 1.,
				Some(true) => 2.,
			},
		];
		let light = light.as_vec3();
		let parameters = Parameters {
			values: [light.x, light.y, light.z, rise as f32],
			taps,
			..Default::default()
		};
		self.run("bevel_lit", COVERAGE_FORMAT, self.size, height, samples.as_ref(), surface.texture, parameters)
	}

	/// The highlight, or if `shadow` the shadow, of each texel of a bevel's surface lit `brighter` than flat ground, of brightness `light_z`.
	/// The `gloss` contour's samples, if any, reshape how lit it is first, antialiased if paired with true. If `fades_at_foot`, it fades out across the slopes' foot by its `height`.
	/// Matches the CPU's `bevel_shading` and `foot_faded`.
	pub(super) fn bevel_amount(&mut self, brighter: &wgpu::Texture, height: &wgpu::Texture, gloss: Option<&(wgpu::Texture, bool)>, light_z: f64, shadow: bool, fades_at_foot: bool) -> wgpu::Texture {
		let flag = |on: bool| if on { 1. } else { 0. };
		let mut taps = [[0.; 4]; PARAMETER_TAPS];
		taps[0] = [flag(fades_at_foot), 0., 0., 0.];
		let parameters = Parameters {
			amount: BEVEL_FOOT_FADE as f32,
			values: [light_z as f32, flag(gloss.is_some()), flag(gloss.is_some_and(|(_, antialiased)| *antialiased)), flag(shadow)],
			taps,
			..Default::default()
		};
		self.run("bevel_amount", COVERAGE_FORMAT, self.size, brighter, gloss.map(|(samples, _)| samples), Some(height), parameters)
	}
}
