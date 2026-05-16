use core_types::BlendMode;
use core_types::color::SRGBA8;
use vello::peniko;

pub trait BlendModeExt {
	fn to_peniko(&self) -> peniko::Mix;
}

pub trait ToPenikoColor {
	fn to_peniko_color(&self) -> peniko::Color;
}

impl ToPenikoColor for SRGBA8 {
	#[inline(always)]
	fn to_peniko_color(&self) -> peniko::Color {
		peniko::Color::from_rgba8(self.red, self.green, self.blue, self.alpha)
	}
}

impl BlendModeExt for BlendMode {
	fn to_peniko(&self) -> peniko::Mix {
		match self {
			// Normal group
			BlendMode::Normal => peniko::Mix::Normal,
			// Darken group
			BlendMode::Darken => peniko::Mix::Darken,
			BlendMode::Multiply => peniko::Mix::Multiply,
			BlendMode::ColorBurn => peniko::Mix::ColorBurn,
			// Lighten group
			BlendMode::Lighten => peniko::Mix::Lighten,
			BlendMode::Screen => peniko::Mix::Screen,
			BlendMode::ColorDodge => peniko::Mix::ColorDodge,
			// Contrast group
			BlendMode::Overlay => peniko::Mix::Overlay,
			BlendMode::SoftLight => peniko::Mix::SoftLight,
			BlendMode::HardLight => peniko::Mix::HardLight,
			// Inversion group
			BlendMode::Difference => peniko::Mix::Difference,
			BlendMode::Exclusion => peniko::Mix::Exclusion,
			// Component group
			BlendMode::Hue => peniko::Mix::Hue,
			BlendMode::Saturation => peniko::Mix::Saturation,
			BlendMode::Color => peniko::Mix::Color,
			BlendMode::Luminosity => peniko::Mix::Luminosity,
			_ => todo!(),
		}
	}
}
