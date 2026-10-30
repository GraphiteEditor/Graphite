//! The kernels blending one color with another.

use crate::{MapColor, Wgsl, gpu_param_enum, gpu_params};
use core_types::blending::{BlendMode, apply_blend_mode};
use core_types::color::Color;

gpu_param_enum!(i32: BlendMode);

gpu_params! {
	#[derive(Clone)]
	pub(crate) struct ColorOverlay {
		pub(crate) color: Color,
		pub(crate) blend_mode: BlendMode,
		pub(crate) opacity: f32,
	}
}

impl MapColor for ColorOverlay {
	fn map_cpu(&self, pixel: Color) -> Color {
		let overlay = apply_blend_mode(self.color, pixel, self.blend_mode);
		let mix = |image: f32, overlay: f32| image + (overlay - image) * self.opacity;

		Color::from_rgbaf32_unchecked(mix(pixel.r(), overlay.r()), mix(pixel.g(), overlay.g()), mix(pixel.b(), overlay.b()), pixel.a())
	}

	fn wgsl() -> Wgsl {
		Wgsl::function(format!("{}\n{}", include_str!("wgsl/blending.wgsl"), include_str!("wgsl/color_overlay.wgsl")))
	}
}
