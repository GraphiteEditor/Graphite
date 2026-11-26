//! The kernels blending one color with another.

use crate::MapColor;
use crate::adjustment_steps::{AdjustmentSteps, StepKind, texel};
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

	fn encode_step(&self, steps: &mut AdjustmentSteps) {
		let color = [self.color.r(), self.color.g(), self.color.b(), self.color.a()];
		steps.push(StepKind::ColorOverlay, [texel(&[blend_mode_code(self.blend_mode) as f32, self.opacity]), color]);
	}
}

/// The color with `paint` mixed over it by the blend mode in gamma, as much as the paint's alpha times `opacity` (from 0 to 1), keeping the color's own alpha.
pub fn overlay_paint(color: Color, paint: Color, blend_mode: BlendMode, opacity: f32) -> Color {
	let gamma = |color: Color| {
		let [red, green, blue, _] = color.to_gamma_srgb_channels();
		[red, green, blue]
	};
	let (under, over) = (gamma(color), gamma(paint));
	let blended = blend_gamma(under, over, blend_mode);

	let amount = opacity * paint.a();
	let [red, green, blue] = std::array::from_fn(|channel| under[channel] + (blended[channel] - under[channel]) * amount);
	Color::from_gamma_srgb_channels(red, green, blue, color.a())
}

/// The blend mode's mix of straight gamma-encoded `backdrop` and `paint` colors.
pub fn blend_gamma(backdrop: [f32; 3], paint: [f32; 3], blend_mode: BlendMode) -> [f32; 3] {
	match blend_mode {
		// The component modes encode the colors they're given themselves
		BlendMode::Hue | BlendMode::Saturation | BlendMode::Color | BlendMode::Luminosity => {
			let color = |[red, green, blue]: [f32; 3]| Color::from_gamma_srgb_channels(red, green, blue, 1.);
			let [red, green, blue, _] = apply_blend_mode(color(paint), color(backdrop), blend_mode).to_gamma_srgb_channels();
			[red, green, blue]
		}
		_ => {
			let color = |[red, green, blue]: [f32; 3]| Color::from_rgbaf32_unchecked(red, green, blue, 1.);
			let blended = apply_blend_mode(color(paint), color(backdrop), blend_mode);
			[blended.r(), blended.g(), blended.b()]
		}
	}
}

/// Every blend mode with the name the shader gives its code.
const BLEND_MODES: [(BlendMode, &str); 29] = [
	(BlendMode::Normal, "BLEND_NORMAL"),
	(BlendMode::Darken, "BLEND_DARKEN"),
	(BlendMode::Multiply, "BLEND_MULTIPLY"),
	(BlendMode::ColorBurn, "BLEND_COLOR_BURN"),
	(BlendMode::LinearBurn, "BLEND_LINEAR_BURN"),
	(BlendMode::DarkerColor, "BLEND_DARKER_COLOR"),
	(BlendMode::Lighten, "BLEND_LIGHTEN"),
	(BlendMode::Screen, "BLEND_SCREEN"),
	(BlendMode::ColorDodge, "BLEND_COLOR_DODGE"),
	(BlendMode::LinearDodge, "BLEND_LINEAR_DODGE"),
	(BlendMode::LighterColor, "BLEND_LIGHTER_COLOR"),
	(BlendMode::Overlay, "BLEND_OVERLAY"),
	(BlendMode::SoftLight, "BLEND_SOFT_LIGHT"),
	(BlendMode::HardLight, "BLEND_HARD_LIGHT"),
	(BlendMode::VividLight, "BLEND_VIVID_LIGHT"),
	(BlendMode::LinearLight, "BLEND_LINEAR_LIGHT"),
	(BlendMode::PinLight, "BLEND_PIN_LIGHT"),
	(BlendMode::HardMix, "BLEND_HARD_MIX"),
	(BlendMode::Difference, "BLEND_DIFFERENCE"),
	(BlendMode::Exclusion, "BLEND_EXCLUSION"),
	(BlendMode::Subtract, "BLEND_SUBTRACT"),
	(BlendMode::Divide, "BLEND_DIVIDE"),
	(BlendMode::Hue, "BLEND_HUE"),
	(BlendMode::Saturation, "BLEND_SATURATION"),
	(BlendMode::Color, "BLEND_COLOR"),
	(BlendMode::Luminosity, "BLEND_LUMINOSITY"),
	(BlendMode::Erase, "BLEND_ERASE"),
	(BlendMode::Restore, "BLEND_RESTORE"),
	(BlendMode::MultiplyAlpha, "BLEND_MULTIPLY_ALPHA"),
];

/// The code the shader knows a blend mode by.
fn blend_mode_code(blend_mode: BlendMode) -> u32 {
	match blend_mode {
		BlendMode::Normal => 0,
		BlendMode::Darken => 1,
		BlendMode::Multiply => 2,
		BlendMode::ColorBurn => 3,
		BlendMode::LinearBurn => 4,
		BlendMode::DarkerColor => 5,
		BlendMode::Lighten => 6,
		BlendMode::Screen => 7,
		BlendMode::ColorDodge => 8,
		BlendMode::LinearDodge => 9,
		BlendMode::LighterColor => 10,
		BlendMode::Overlay => 11,
		BlendMode::SoftLight => 12,
		BlendMode::HardLight => 13,
		BlendMode::VividLight => 14,
		BlendMode::LinearLight => 15,
		BlendMode::PinLight => 16,
		BlendMode::HardMix => 17,
		BlendMode::Difference => 18,
		BlendMode::Exclusion => 19,
		BlendMode::Subtract => 20,
		BlendMode::Divide => 21,
		BlendMode::Hue => 22,
		BlendMode::Saturation => 23,
		BlendMode::Color => 24,
		BlendMode::Luminosity => 25,
		BlendMode::Erase => 26,
		BlendMode::Restore => 27,
		BlendMode::MultiplyAlpha => 28,
	}
}

/// The names the shader gives the codes of the blend modes.
pub(crate) fn shader_constants() -> Vec<(&'static str, u32)> {
	BLEND_MODES.into_iter().map(|(blend_mode, name)| (name, blend_mode_code(blend_mode))).collect()
}
