//! Records the passes that resample an item's content from elsewhere than nearby, whose shaders are in `vello_resample.wgsl`.

use super::{PARAMETER_TAPS, Parameters, PassRecorder};
use graphic_types::resampling::{EffectFrame, RadialBlur, RadialBlurMethod};
use raster_kernels::resample_kernels::MotionBlurShear;

/// The tap array holding the frame's maps between the texture and its local space, and half its size, where the resampling shaders read them.
fn frame_taps(frame: &EffectFrame) -> [[f32; 4]; PARAMETER_TAPS] {
	let to_local = frame.pixels_to_local().to_cols_array().map(|value| value as f32);
	let to_pixels = frame.local_to_pixels().to_cols_array().map(|value| value as f32);
	let half = (frame.size / 2.).as_vec2();

	let mut taps = [[0.; 4]; PARAMETER_TAPS];
	taps[0] = [to_local[0], to_local[1], to_local[2], to_local[3]];
	taps[1] = [to_local[4], to_local[5], to_pixels[0], to_pixels[1]];
	taps[2] = [to_pixels[2], to_pixels[3], to_pixels[4], to_pixels[5]];
	taps[3] = [half.x, half.y, 0., 0.];
	taps
}

impl PassRecorder<'_> {
	/// `texture` smeared along the motion blur's line by its two shears around a box, in linear light if `linear`, matching the CPU's `motion_blurred`.
	pub(super) fn motion_blur(&mut self, texture: wgpu::Texture, shear: &MotionBlurShear, linear: bool) -> wgpu::Texture {
		let [start, end] = shear.box_span();
		let mut taps = [[0.; 4]; PARAMETER_TAPS];
		taps[0] = [start as f32, end as f32, shear.stride() as f32, 0.];
		let parameters = Parameters {
			values: [shear.slope as f32, shear.phase as f32, if shear.vertical { 1. } else { 0. }, 0.],
			taps,
			..Default::default()
		};

		self.in_light(texture, linear, |recorder, mut texture| {
			for entry_point in ["motion_shear", "motion_box", "motion_unshear"] {
				let next = recorder.run(entry_point, texture.format(), recorder.size, &texture, None, None, parameters);
				recorder.replace(&mut texture, next);
			}
			texture
		})
	}

	/// `texture` blurred along the radial blur's arcs or lines, placed in the `frame`, in linear light if `linear`, matching the CPU's `radial_blurred`.
	pub(super) fn radial_blur(&mut self, texture: wgpu::Texture, blur: &RadialBlur, frame: &EffectFrame, pixels_per_length: f64, linear: bool) -> wgpu::Texture {
		let mut taps = frame_taps(frame);
		let center = blur.center.as_vec2();
		taps[3][2..].copy_from_slice(&[center.x, center.y]);
		let parameters = Parameters {
			values: [blur.extent as f32, if blur.method == RadialBlurMethod::Zoom { 1. } else { 0. }, pixels_per_length as f32, 0.],
			taps,
			..Default::default()
		};

		self.in_light(texture, linear, |recorder, mut texture| {
			let blurred = recorder.run("radial_blur", texture.format(), recorder.size, &texture, None, None, parameters);
			recorder.replace(&mut texture, blurred);
			texture
		})
	}
}
