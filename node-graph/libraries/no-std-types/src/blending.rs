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

	background.alpha_blend(target_color.with_alpha(target_color.a() * opacity))
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

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn overlay_is_hard_light_with_swapped_operands() {
		let a = Color::from_rgbaf32_unchecked(0.8, 0.3, 0.6, 1.);
		let b = Color::from_rgbaf32_unchecked(0.2, 0.7, 0.4, 1.);

		let overlay = apply_blend_mode(a, b, BlendMode::Overlay);
		let swapped_hard_light = apply_blend_mode(b, a, BlendMode::HardLight);

		assert!((overlay.r() - swapped_hard_light.r()).abs() < 1e-5, "red was {} vs {}", overlay.r(), swapped_hard_light.r());
		assert!((overlay.g() - swapped_hard_light.g()).abs() < 1e-5, "green was {} vs {}", overlay.g(), swapped_hard_light.g());
		assert!((overlay.b() - swapped_hard_light.b()).abs() < 1e-5, "blue was {} vs {}", overlay.b(), swapped_hard_light.b());
	}

	#[test]
	fn blended_colors_keep_the_foreground_alpha() {
		let foreground = Color::from_rgbaf32_unchecked(0.8, 0.3, 0.6, 0.25);
		let background = Color::from_rgbaf32_unchecked(0.2, 0.7, 0.4, 1.);

		let modes = [
			BlendMode::Multiply,
			BlendMode::Overlay,
			BlendMode::DarkerColor,
			BlendMode::LighterColor,
			BlendMode::Hue,
			BlendMode::Saturation,
			BlendMode::Color,
			BlendMode::Luminosity,
		];
		for mode in modes {
			let blended = apply_blend_mode(foreground, background, mode);
			assert!((blended.a() - 0.25).abs() < 1e-5, "{mode} alpha was {}", blended.a());
		}
	}

	#[test]
	fn darker_color_ignores_backdrop_alpha() {
		// The backdrop's low alpha doesn't darken its color, so the 0.4 gray foreground is the darker color
		let foreground = Color::from_rgbaf32_unchecked(0.4, 0.4, 0.4, 1.);
		let background = Color::from_rgbaf32_unchecked(0.5, 0.5, 0.5, 0.2);

		let blended = apply_blend_mode(foreground, background, BlendMode::DarkerColor);

		assert!((blended.r() - 0.4).abs() < 1e-5, "red was {}", blended.r());
		assert!((blended.a() - 1.).abs() < 1e-5, "alpha was {}", blended.a());
	}

	#[test]
	fn source_over_weights_straight_colors_by_alpha() {
		let over = Color::from_rgbaf32_unchecked(1., 0., 0., 0.5);
		let under = Color::from_rgbaf32_unchecked(0., 0., 1., 1.);

		let blended = under.alpha_blend(over);

		assert!((blended.r() - 0.5).abs() < 1e-5, "red was {}", blended.r());
		assert!((blended.b() - 0.5).abs() < 1e-5, "blue was {}", blended.b());
		assert!((blended.a() - 1.).abs() < 1e-5, "alpha was {}", blended.a());
	}

	#[test]
	fn alpha_only_modes_fade_with_opacity() {
		let foreground = Color::from_rgbaf32_unchecked(0.9, 0.9, 0.9, 1.);
		let background = Color::from_rgbaf32_unchecked(0.3, 0.5, 0.7, 1.);

		// A full-opacity erase removes all coverage, and half opacity fades that effect halfway back toward the backdrop
		let full = blend_colors(foreground, background, BlendMode::Erase, 1.);
		let half = blend_colors(foreground, background, BlendMode::Erase, 0.5);

		assert!((full.a() - 0.).abs() < 1e-5, "alpha was {}", full.a());
		assert!((half.a() - 0.5).abs() < 1e-5, "alpha was {}", half.a());
		assert!((half.r() - 0.3).abs() < 1e-5, "red was {}", half.r());
	}

	// A transcription of the specification's pseudocode for the component blend modes, on gamma-encoded channels:
	// https://www.w3.org/TR/compositing-1/#blendingnonseparable
	mod specification {
		pub fn lum(c: [f32; 3]) -> f32 {
			0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]
		}

		fn clip_color(c: [f32; 3]) -> [f32; 3] {
			let l = lum(c);
			let n = c[0].min(c[1]).min(c[2]);
			let x = c[0].max(c[1]).max(c[2]);

			let mut c = c;
			if n < 0. {
				c = c.map(|channel| l + (channel - l) * l / (l - n));
			}
			if x > 1. {
				c = c.map(|channel| l + (channel - l) * (1. - l) / (x - l));
			}
			c
		}

		pub fn set_lum(c: [f32; 3], l: f32) -> [f32; 3] {
			let d = l - lum(c);
			clip_color(c.map(|channel| channel + d))
		}

		pub fn sat(c: [f32; 3]) -> f32 {
			c[0].max(c[1]).max(c[2]) - c[0].min(c[1]).min(c[2])
		}

		pub fn set_sat(c: [f32; 3], s: f32) -> [f32; 3] {
			let mut order = [0, 1, 2];
			order.sort_by(|&a, &b| c[a].total_cmp(&c[b]));
			let [min, mid, max] = order;

			let mut result = [0.; 3];
			if c[max] > c[min] {
				result[mid] = (c[mid] - c[min]) * s / (c[max] - c[min]);
				result[max] = s;
			}
			result
		}
	}

	#[test]
	fn component_modes_match_the_specification() {
		use specification::{lum, sat, set_lum, set_sat};

		let colors = [
			[0.8, 0.3, 0.6],
			[0.2, 0.7, 0.4],
			[1., 0., 0.],
			[0.05, 0.1, 0.95],
			[0.5, 0.5, 0.5],
			[0.95, 0.9, 0.1],
			[0., 0., 0.],
			[1., 1., 1.],
		];

		for backdrop in colors {
			for source in colors {
				let expectations = [
					(BlendMode::Hue, set_lum(set_sat(source, sat(backdrop)), lum(backdrop))),
					(BlendMode::Saturation, set_lum(set_sat(backdrop, sat(source)), lum(backdrop))),
					(BlendMode::Color, set_lum(source, lum(backdrop))),
					(BlendMode::Luminosity, set_lum(backdrop, lum(source))),
				];

				for (mode, expected) in expectations {
					let foreground = Color::from_gamma_srgb_channels(source[0], source[1], source[2], 1.);
					let background = Color::from_gamma_srgb_channels(backdrop[0], backdrop[1], backdrop[2], 1.);
					let [r, g, b, _] = apply_blend_mode(foreground, background, mode).to_gamma_srgb_channels();

					for (ours, expected) in [r, g, b].into_iter().zip(expected) {
						assert!((ours - expected).abs() < 1e-5, "{mode} of {source:?} over {backdrop:?} gave {:?}, not {expected:?}", [r, g, b]);
					}
				}
			}
		}
	}

	#[test]
	fn color_mode_pulls_an_overshooting_channel_back_without_shifting_its_luma() {
		let red = Color::from_gamma_srgb_channels(1., 0., 0., 1.);
		let gray = Color::from_gamma_srgb_channels(0.5, 0.5, 0.5, 1.);

		// Raising red's 0.3 luma to the gray's 0.5 puts red at 1.2, so every channel is pulled 5/7 of the way back toward 0.5
		let [r, g, b, _] = apply_blend_mode(red, gray, BlendMode::Color).to_gamma_srgb_channels();
		assert!((r - 1.).abs() < 1e-5 && (g - 2. / 7.).abs() < 1e-5 && (b - 2. / 7.).abs() < 1e-5, "got {r}, {g}, {b}");

		// A gray backdrop has only luma to keep, so it takes on the red's
		let [r, g, b, _] = apply_blend_mode(red, gray, BlendMode::Luminosity).to_gamma_srgb_channels();
		assert!((r - 0.3).abs() < 1e-5 && (g - 0.3).abs() < 1e-5 && (b - 0.3).abs() < 1e-5, "got {r}, {g}, {b}");
	}
}
