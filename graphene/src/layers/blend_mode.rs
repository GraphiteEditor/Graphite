use serde::{Deserialize, Serialize};

#[derive(PartialEq, Copy, Clone, Debug, Serialize, Deserialize)]
pub enum BlendMode {
	Normal,
	Multiply,
	Darken,
	ColorBurn,
	Screen,
	Lighten,
	ColorDodge,
	Overlay,
	SoftLight,
	HardLight,
	Difference,
	Exclusion,
	Hue,
	Saturation,
	Color,
	Luminosity,
}

impl BlendMode {
	pub fn to_svg_style_name(&self) -> &str {
		match self {
			BlendMode::Normal => "normal",
			BlendMode::Multiply => "multiply",
			BlendMode::Darken => "darken",
			BlendMode::ColorBurn => "color-burn",
			BlendMode::Screen => "screen",
			BlendMode::Lighten => "lighten",
			BlendMode::ColorDodge => "color-dodge",
			BlendMode::Overlay => "overlay",
			BlendMode::SoftLight => "soft-light",
			BlendMode::HardLight => "hard-light",
			BlendMode::Difference => "difference",
			BlendMode::Exclusion => "exclusion",
			BlendMode::Hue => "hue",
			BlendMode::Saturation => "saturation",
			BlendMode::Color => "color",
			BlendMode::Luminosity => "luminosity",
		}
	}
}
