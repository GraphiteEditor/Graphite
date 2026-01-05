use core_types::BlendMode;
use vello::peniko;

pub trait BlendModeExt {
	fn to_peniko(&self) -> peniko::Mix;
}

impl BlendModeExt for BlendMode {
	fn to_peniko(&self) -> peniko::Mix {
		match self {
			// Normal group
			Self::Normal => peniko::Mix::Normal,
			// Darken group
			Self::Darken => peniko::Mix::Darken,
			Self::Multiply => peniko::Mix::Multiply,
			Self::ColorBurn => peniko::Mix::ColorBurn,
			// Lighten group
			Self::Lighten => peniko::Mix::Lighten,
			Self::Screen => peniko::Mix::Screen,
			Self::ColorDodge => peniko::Mix::ColorDodge,
			// Contrast group
			Self::Overlay => peniko::Mix::Overlay,
			Self::SoftLight => peniko::Mix::SoftLight,
			Self::HardLight => peniko::Mix::HardLight,
			// Inversion group
			Self::Difference => peniko::Mix::Difference,
			Self::Exclusion => peniko::Mix::Exclusion,
			// Component group
			Self::Hue => peniko::Mix::Hue,
			Self::Saturation => peniko::Mix::Saturation,
			Self::Color => peniko::Mix::Color,
			Self::Luminosity => peniko::Mix::Luminosity,
			_ => todo!(),
		}
	}
}
