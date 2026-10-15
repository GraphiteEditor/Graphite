//! The kernels blending one color with another.

use crate::MapColor;
use core_types::Color;
use core_types::blending::{BlendMode, apply_blend_mode};

pub(crate) struct ColorOverlay {
	pub(crate) color: Color,
	pub(crate) blend_mode: BlendMode,
	pub(crate) opacity: f32,
}

impl MapColor for ColorOverlay {
	fn map_cpu(&self, pixel: Color) -> Color {
		overlay_paint(pixel, self.color, self.blend_mode, self.opacity)
	}
}

/// The color with `paint` mixed over it by the blend mode, as much as the paint's alpha times `opacity` (from 0 to 1), keeping the color's own alpha.
pub fn overlay_paint(color: Color, paint: Color, blend_mode: BlendMode, opacity: f32) -> Color {
	let overlay = apply_blend_mode(paint, color, blend_mode);
	let amount = opacity * paint.a();
	let mix = |under: f32, over: f32| under + (over - under) * amount;

	Color::from_rgbaf32_unchecked(mix(color.r(), overlay.r()), mix(color.g(), overlay.g()), mix(color.b(), overlay.b()), color.a())
}
