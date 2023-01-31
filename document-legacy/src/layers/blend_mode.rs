use serde::{Deserialize, Serialize};
use std::fmt;

/// Describes how overlapping SVG elements should be blended together.
/// See the [MDN Docs](https://developer.mozilla.org/en-US/docs/Web/CSS/blend-mode#examples) for examples.
#[derive(PartialEq, Eq, Copy, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub enum BlendMode {
	// Basic group
	Normal,
	// Not supported by SVG, but we should someday support: Dissolve

	// Darken group
	Multiply,
	Darken,
	ColorBurn,
	// Not supported by SVG, but we should someday support: Linear Burn, Darker Color

	// Lighten group
	Screen,
	Lighten,
	ColorDodge,
	// Not supported by SVG, but we should someday support: Linear Dodge (Add), Lighter Color

	// Contrast group
	Overlay,
	SoftLight,
	HardLight,
	// Not supported by SVG, but we should someday support: Vivid Light, Linear Light, Pin Light, Hard Mix

	// Inversion group
	Difference,
	Exclusion,
	// Not supported by SVG, but we should someday support: Subtract, Divide

	// Component group
	Hue,
	Saturation,
	Color,
	Luminosity,
}

impl fmt::Display for BlendMode {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			BlendMode::Normal => write!(f, "Normal"),

			BlendMode::Multiply => write!(f, "Multiply"),
			BlendMode::Darken => write!(f, "Darken"),
			BlendMode::ColorBurn => write!(f, "Color Burn"),

			BlendMode::Screen => write!(f, "Screen"),
			BlendMode::Lighten => write!(f, "Lighten"),
			BlendMode::ColorDodge => write!(f, "Color Dodge"),

			BlendMode::Overlay => write!(f, "Overlay"),
			BlendMode::SoftLight => write!(f, "Soft Light"),
			BlendMode::HardLight => write!(f, "Hard Light"),

			BlendMode::Difference => write!(f, "Difference"),
			BlendMode::Exclusion => write!(f, "Exclusion"),

			BlendMode::Hue => write!(f, "Hue"),
			BlendMode::Saturation => write!(f, "Saturation"),
			BlendMode::Color => write!(f, "Color"),
			BlendMode::Luminosity => write!(f, "Luminosity"),
		}
	}
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

	/// List of all the blend modes in their conventional ordering and grouping.
	pub fn list_modes_in_groups() -> [&'static [BlendMode]; 6] {
		[
			&[BlendMode::Normal],
			&[BlendMode::Multiply, BlendMode::Darken, BlendMode::ColorBurn],
			&[BlendMode::Screen, BlendMode::Lighten, BlendMode::ColorDodge],
			&[BlendMode::Overlay, BlendMode::SoftLight, BlendMode::HardLight],
			&[BlendMode::Difference, BlendMode::Exclusion],
			&[BlendMode::Hue, BlendMode::Saturation, BlendMode::Color, BlendMode::Luminosity],
		]
	}
}
