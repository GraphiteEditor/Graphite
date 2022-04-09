use serde::{Deserialize, Serialize};

/// Describes how overlapping SVG elements should be blended together.
/// See the [MDN Docs](https://developer.mozilla.org/en-US/docs/Web/CSS/blend-mode#examples) for examples.
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
	/// Convert the enum to the CSS string for the blend mode.
	/// [Read more](https://developer.mozilla.org/en-US/docs/Web/CSS/blend-mode#values)
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
