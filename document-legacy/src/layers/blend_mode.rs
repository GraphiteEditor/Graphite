use serde::{Deserialize, Serialize};
use std::fmt;

/// Describes how overlapping SVG elements should be blended together.
/// See the [MDN Docs](https://developer.mozilla.org/en-US/docs/Web/CSS/blend-mode#examples) for examples.
#[derive(PartialEq, Eq, Copy, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub enum BlendMode {
	// Normal group
	Normal,
	// Not supported by SVG, but we should someday support: Dissolve

	// Darken group
	Darken,
	Multiply,
	ColorBurn,
	// Not supported by SVG, but we should someday support: Linear Burn, Darker Color

	// Lighten group
	Lighten,
	Screen,
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
			// Normal group
			BlendMode::Normal => write!(f, "Normal"),

			// Darken group
			BlendMode::Darken => write!(f, "Darken"),
			BlendMode::Multiply => write!(f, "Multiply"),
			BlendMode::ColorBurn => write!(f, "Color Burn"),

			// Lighten group
			BlendMode::Lighten => write!(f, "Lighten"),
			BlendMode::Screen => write!(f, "Screen"),
			BlendMode::ColorDodge => write!(f, "Color Dodge"),

			// Contrast group
			BlendMode::Overlay => write!(f, "Overlay"),
			BlendMode::SoftLight => write!(f, "Soft Light"),
			BlendMode::HardLight => write!(f, "Hard Light"),

			// Inversion group
			BlendMode::Difference => write!(f, "Difference"),
			BlendMode::Exclusion => write!(f, "Exclusion"),

			// Component group
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
			// Normal group
			BlendMode::Normal => "normal",
			// Darken group
			BlendMode::Darken => "darken",
			BlendMode::Multiply => "multiply",
			BlendMode::ColorBurn => "color-burn",
			// Lighten group
			BlendMode::Lighten => "lighten",
			BlendMode::Screen => "screen",
			BlendMode::ColorDodge => "color-dodge",
			// Contrast group
			BlendMode::Overlay => "overlay",
			BlendMode::SoftLight => "soft-light",
			BlendMode::HardLight => "hard-light",
			// Inversion group
			BlendMode::Difference => "difference",
			BlendMode::Exclusion => "exclusion",
			// Component group
			BlendMode::Hue => "hue",
			BlendMode::Saturation => "saturation",
			BlendMode::Color => "color",
			BlendMode::Luminosity => "luminosity",
		}
	}

	/// List of all the blend modes in their conventional ordering and grouping.
	pub fn list_modes_in_groups() -> [&'static [BlendMode]; 6] {
		[
			// Normal group
			&[BlendMode::Normal],
			// Darken group
			&[BlendMode::Darken, BlendMode::Multiply, BlendMode::ColorBurn],
			// Lighten group
			&[BlendMode::Lighten, BlendMode::Screen, BlendMode::ColorDodge],
			// Contrast group
			&[BlendMode::Overlay, BlendMode::SoftLight, BlendMode::HardLight],
			// Inversion group
			&[BlendMode::Difference, BlendMode::Exclusion],
			// Component group
			&[BlendMode::Hue, BlendMode::Saturation, BlendMode::Color, BlendMode::Luminosity],
		]
	}
}
