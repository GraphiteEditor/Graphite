use serde::{Deserialize, Serialize};

/// Describes how overlapping SVG Elements should be blended together.
/// See the [MDN Docs](https://developer.mozilla.org/en-US/docs/Web/CSS/blend-mode#examples) for examples.
#[derive(PartialEq, Copy, Clone, Debug, Serialize, Deserialize)]
pub enum BlendMode {
	/// The final color is the top color, regardless of what the bottom color is.
	/// The effect is like two opaque pieces of paper overlapping.
	Normal,
	/// The final color is the result of multiplying the top and bottom colors.
	/// A black layer leads to a black final layer, and a white layer leads to no change.
	/// The effect is like two images printed on transparent film overlapping.
	Multiply,
	/// The final color is composed of the darkest values of each color channel.
	Darken,
	/// The final color is the result of inverting the bottom color, dividing the value by the top color, and inverting that value.
	/// A white foreground leads to no change. A foreground with the inverse color of the backdrop leads to a black final image.
	/// This blend mode is similar to [Multiply](BlendMode::Multiply), but the foreground need only be as dark as the inverse of the backdrop to make the final image black.
	ColorBurn,
	/// The final color is the result of inverting the colors, multiplying them, and inverting that value.
	/// A black layer leads to no change, and a white layer leads to a white final layer.
	/// The effect is like two images shone onto a projection screen.
	Screen,
	/// The final color is composed of the lightest values of each color channel.
	Lighten,
	/// The final color is the result of dividing the bottom color by the inverse of the top color.
	/// A black foreground leads to no change. A foreground with the inverse color of the backdrop leads to a fully lit color.
	/// This blend mode is similar to [Screen](BlendMode::Screen), but the foreground need only be as light as the inverse of the backdrop to create a fully lit color.
	ColorDodge,
	/// The final color is the result of [Multiply](BlendMode::Multiply) if the bottom color is darker, or [Screen](BlendMode::Screen) if the bottom color is lighter.
	/// This blend mode is equivalent to [HardLight](BlendMode::HardLight) but with the layers swapped.
	Overlay,
	/// The final color is similar to [HardLight](BlendMode::HardLight), but softer.
	/// This blend mode behaves similar to [HardLight](BlendMode::HardLight).
	/// The effect is similar to shining a *diffused* spotlight on the backdrop.
	SoftLight,
	/// The final color is the result of [Multiply](BlendMode::Multiply) if the top color is darker, or [Screen](BlendMode::Screen) if the top color is lighter.
	/// This blend mode is equivalent to [Overlay](BlendMode::Overlay) but with the layers swapped.
	/// The effect is similar to shining a harsh spotlight on the backdrop.
	HardLight,
	/// The final color is the result of subtracting the darker of the two colors from the lighter one.
	/// A black layer has no effect, while a white layer inverts the other layer's color.
	Difference,
	/// The final color is similar to [Difference](BlendMode::Difference), but with less contrast.
	/// As with [Difference](BlendMode::Difference), a black layer has no effect, while a white layer inverts the other layer's color.
	Exclusion,
	/// The final color has the *hue* of the top color, while using the *saturation* and *luminosity* of the bottom color.
	Hue,
	/// The final color has the *saturation* of the top color, while using the *hue* and *luminosity* of the bottom color.
	/// A pure gray backdrop, having no saturation, will have no effect.
	Saturation,
	/// The final color has the *hue* and *saturation* of the top color, while using the *luminosity* of the bottom color.
	/// The effect preserves gray levels and can be used to colorize the foreground.
	Color,
	/// The final color has the *luminosity* of the top color, while using the *hue* and *saturation* of the bottom color.
	/// This blend mode is equivalent to [Color](BlendMode::Color), but with the layers swapped.
	Luminosity,
}

impl BlendMode {
	/// Convert the enum to the css string representation.
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
