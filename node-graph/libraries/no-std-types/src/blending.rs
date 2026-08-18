use crate::color::Color;
use core::fmt::Display;
use node_macro::BufferStruct;
use num_enum::{FromPrimitive, IntoPrimitive};

#[repr(i32)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "std", derive(dyn_any::DynAny, serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "std", derive(graphene_hash::CacheHash))]
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash, BufferStruct, FromPrimitive, IntoPrimitive)]
pub enum BlendMode {
	// Basic group
	#[default]
	Normal,

	// Darken group
	Darken,
	Multiply,
	ColorBurn,
	LinearBurn,
	DarkerColor,

	// Lighten group
	Lighten,
	Screen,
	ColorDodge,
	LinearDodge,
	LighterColor,

	// Contrast group
	Overlay,
	SoftLight,
	HardLight,
	VividLight,
	LinearLight,
	PinLight,
	HardMix,

	// Inversion group
	Difference,
	Exclusion,
	Subtract,
	Divide,

	// Component group
	Hue,
	Saturation,
	Color,
	Luminosity,

	// Other stuff
	Erase,
	Restore,
	MultiplyAlpha,
}

impl BlendMode {
	/// All standard blend modes ordered by group.
	pub fn list() -> [&'static [BlendMode]; 6] {
		use BlendMode::*;
		[
			// Normal group
			&[Normal],
			// Darken group
			&[Darken, Multiply, ColorBurn, LinearBurn, DarkerColor],
			// Lighten group
			&[Lighten, Screen, ColorDodge, LinearDodge, LighterColor],
			// Contrast group
			&[Overlay, SoftLight, HardLight, VividLight, LinearLight, PinLight, HardMix],
			// Inversion group
			&[Difference, Exclusion, Subtract, Divide],
			// Component group
			&[Hue, Saturation, Color, Luminosity],
		]
	}

	/// The subset of [`BlendMode::list()`] that is supported by SVG.
	pub fn list_svg_subset() -> [&'static [BlendMode]; 6] {
		use BlendMode::*;
		[
			// Normal group
			&[Normal],
			// Darken group
			&[Darken, Multiply, ColorBurn],
			// Lighten group
			&[Lighten, Screen, ColorDodge],
			// Contrast group
			&[Overlay, SoftLight, HardLight],
			// Inversion group
			&[Difference, Exclusion],
			// Component group
			&[Hue, Saturation, Color, Luminosity],
		]
	}

	pub fn index_in_list(&self) -> Option<usize> {
		Self::list().iter().flat_map(|x| x.iter()).position(|&blend_mode| blend_mode == *self)
	}

	pub fn index_in_list_svg_subset(&self) -> Option<usize> {
		Self::list_svg_subset().iter().flat_map(|x| x.iter()).position(|&blend_mode| blend_mode == *self)
	}

	/// Convert the enum to the CSS string for the blend mode.
	/// [Read more](https://developer.mozilla.org/en-US/docs/Web/CSS/blend-mode#values)
	pub fn to_svg_style_name(&self) -> Option<&'static str> {
		match self {
			// Normal group
			BlendMode::Normal => Some("normal"),
			// Darken group
			BlendMode::Darken => Some("darken"),
			BlendMode::Multiply => Some("multiply"),
			BlendMode::ColorBurn => Some("color-burn"),
			// Lighten group
			BlendMode::Lighten => Some("lighten"),
			BlendMode::Screen => Some("screen"),
			BlendMode::ColorDodge => Some("color-dodge"),
			// Contrast group
			BlendMode::Overlay => Some("overlay"),
			BlendMode::SoftLight => Some("soft-light"),
			BlendMode::HardLight => Some("hard-light"),
			// Inversion group
			BlendMode::Difference => Some("difference"),
			BlendMode::Exclusion => Some("exclusion"),
			// Component group
			BlendMode::Hue => Some("hue"),
			BlendMode::Saturation => Some("saturation"),
			BlendMode::Color => Some("color"),
			BlendMode::Luminosity => Some("luminosity"),
			_ => None,
		}
	}

	/// Renders the blend mode CSS style declaration.
	#[cfg(feature = "std")]
	pub fn render(&self) -> String {
		format!(
			r#" mix-blend-mode: {};"#,
			self.to_svg_style_name().unwrap_or_else(|| {
				log::warn!("Unsupported blend mode {self:?}");
				"normal"
			})
		)
	}
}

impl Display for BlendMode {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			// Normal group
			BlendMode::Normal => write!(f, "Normal"),
			// Darken group
			BlendMode::Darken => write!(f, "Darken"),
			BlendMode::Multiply => write!(f, "Multiply"),
			BlendMode::ColorBurn => write!(f, "Color Burn"),
			BlendMode::LinearBurn => write!(f, "Linear Burn"),
			BlendMode::DarkerColor => write!(f, "Darker Color"),
			// Lighten group
			BlendMode::Lighten => write!(f, "Lighten"),
			BlendMode::Screen => write!(f, "Screen"),
			BlendMode::ColorDodge => write!(f, "Color Dodge"),
			BlendMode::LinearDodge => write!(f, "Linear Dodge"),
			BlendMode::LighterColor => write!(f, "Lighter Color"),
			// Contrast group
			BlendMode::Overlay => write!(f, "Overlay"),
			BlendMode::SoftLight => write!(f, "Soft Light"),
			BlendMode::HardLight => write!(f, "Hard Light"),
			BlendMode::VividLight => write!(f, "Vivid Light"),
			BlendMode::LinearLight => write!(f, "Linear Light"),
			BlendMode::PinLight => write!(f, "Pin Light"),
			BlendMode::HardMix => write!(f, "Hard Mix"),
			// Inversion group
			BlendMode::Difference => write!(f, "Difference"),
			BlendMode::Exclusion => write!(f, "Exclusion"),
			BlendMode::Subtract => write!(f, "Subtract"),
			BlendMode::Divide => write!(f, "Divide"),
			// Component group
			BlendMode::Hue => write!(f, "Hue"),
			BlendMode::Saturation => write!(f, "Saturation"),
			BlendMode::Color => write!(f, "Color"),
			BlendMode::Luminosity => write!(f, "Luminosity"),
			// Other utility blend modes (hidden from the normal list)
			BlendMode::Erase => write!(f, "Erase"),
			BlendMode::Restore => write!(f, "Restore"),
			BlendMode::MultiplyAlpha => write!(f, "Multiply Alpha"),
		}
	}
}

/// Composites `foreground` over `background` with the given blend mode, fading the result by `opacity`.
#[inline(always)]
pub fn blend_colors(foreground: Color, background: Color, blend_mode: BlendMode, opacity: f32) -> Color {
	// The alpha-only utility modes composite no color, so opacity interpolates their alpha toward the backdrop's instead
	let faded_alpha = |applied: Color| background.with_alpha(background.a() + (applied.a() - background.a()) * opacity);

	let target_color = match blend_mode {
		// Other utility blend modes (hidden from the normal list) - do not have alpha blend
		BlendMode::Erase => return faded_alpha(background.alpha_subtract(foreground)),
		BlendMode::Restore => return faded_alpha(background.alpha_add(foreground)),
		BlendMode::MultiplyAlpha => return faded_alpha(background.alpha_multiply(foreground)),
		blend_mode => apply_blend_mode(foreground, background, blend_mode),
	};

	background.alpha_blend(target_color.apply_opacity(opacity))
}

/// Mixes the two colors by the blend mode's own formula, leaving the alpha compositing to the caller.
pub fn apply_blend_mode(foreground: Color, background: Color, blend_mode: BlendMode) -> Color {
	match blend_mode {
		// Normal group
		BlendMode::Normal => background.blend_rgb(foreground, Color::blend_normal),
		// Darken group
		BlendMode::Darken => background.blend_rgb(foreground, Color::blend_darken),
		BlendMode::Multiply => background.blend_rgb(foreground, Color::blend_multiply),
		BlendMode::ColorBurn => background.blend_rgb(foreground, Color::blend_color_burn),
		BlendMode::LinearBurn => background.blend_rgb(foreground, Color::blend_linear_burn),
		BlendMode::DarkerColor => background.blend_darker_color(foreground),
		// Lighten group
		BlendMode::Lighten => background.blend_rgb(foreground, Color::blend_lighten),
		BlendMode::Screen => background.blend_rgb(foreground, Color::blend_screen),
		BlendMode::ColorDodge => background.blend_rgb(foreground, Color::blend_color_dodge),
		BlendMode::LinearDodge => background.blend_rgb(foreground, Color::blend_linear_dodge),
		BlendMode::LighterColor => background.blend_lighter_color(foreground),
		// Contrast group
		BlendMode::Overlay => background.blend_rgb(foreground, Color::blend_overlay),
		BlendMode::SoftLight => background.blend_rgb(foreground, Color::blend_softlight),
		BlendMode::HardLight => background.blend_rgb(foreground, Color::blend_hardlight),
		BlendMode::VividLight => background.blend_rgb(foreground, Color::blend_vivid_light),
		BlendMode::LinearLight => background.blend_rgb(foreground, Color::blend_linear_light),
		BlendMode::PinLight => background.blend_rgb(foreground, Color::blend_pin_light),
		BlendMode::HardMix => background.blend_rgb(foreground, Color::blend_hard_mix),
		// Inversion group
		BlendMode::Difference => background.blend_rgb(foreground, Color::blend_difference),
		BlendMode::Exclusion => background.blend_rgb(foreground, Color::blend_exclusion),
		BlendMode::Subtract => background.blend_rgb(foreground, Color::blend_subtract),
		BlendMode::Divide => background.blend_rgb(foreground, Color::blend_divide),
		// Component group
		BlendMode::Hue => background.blend_hue(foreground),
		BlendMode::Saturation => background.blend_saturation(foreground),
		BlendMode::Color => background.blend_color(foreground),
		BlendMode::Luminosity => background.blend_luminosity(foreground),
		// The alpha-only utility modes mix no color, so the foreground passes through for the caller to composite
		BlendMode::Erase | BlendMode::Restore | BlendMode::MultiplyAlpha => foreground,
	}
}
