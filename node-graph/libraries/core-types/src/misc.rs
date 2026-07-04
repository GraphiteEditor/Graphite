// TODO(TrueDoctor): Replace this with the more idiomatic approach instead of using `trait Clampable`.

/// A trait for types that can be clamped within a min/max range defined by f64.
pub trait Clampable: Sized {
	/// Clamps the value to be no less than `min`.
	fn clamp_hard_min(self, min: f64) -> Self;
	/// Clamps the value to be no more than `max`.
	fn clamp_hard_max(self, max: f64) -> Self;
}

// Implement for common numeric types
macro_rules! impl_clampable_float {
	($($ty:ty),*) => {
		$(
			impl Clampable for $ty {
				#[inline(always)]
				fn clamp_hard_min(self, min: f64) -> Self {
					self.max(min as $ty)
				}
				#[inline(always)]
				fn clamp_hard_max(self, max: f64) -> Self {
					self.min(max as $ty)
				}
			}
		)*
	};
}
impl_clampable_float!(f32, f64);

macro_rules! impl_clampable_int {
	($($ty:ty),*) => {
		$(
			impl Clampable for $ty {
				#[inline(always)]
				fn clamp_hard_min(self, min: f64) -> Self {
					// Using try_from to handle potential range issues safely, though min should ideally be valid.
					// Consider using a different approach if f64 precision vs integer range is a concern.
					<$ty>::try_from(min.ceil() as i64).ok().map_or(self, |min_val| self.max(min_val))
				}
				#[inline(always)]
				fn clamp_hard_max(self, max: f64) -> Self {
					<$ty>::try_from(max.floor() as i64).ok().map_or(self, |max_val| self.min(max_val))
				}
			}
		)*
	};
}
// Add relevant integer types (adjust as needed)
impl_clampable_int!(u32, u64, i32, i64);

// Implement for DVec2 (component-wise clamping)
use glam::DVec2;
impl Clampable for DVec2 {
	#[inline(always)]
	fn clamp_hard_min(self, min: f64) -> Self {
		self.max(DVec2::splat(min))
	}
	#[inline(always)]
	fn clamp_hard_max(self, max: f64) -> Self {
		self.min(DVec2::splat(max))
	}
}

// Implement for ranked wires (element-wise clamping across the frame)
use crate::list::{Item, List};
impl<T: Clampable> Clampable for Item<T> {
	fn clamp_hard_min(self, min: f64) -> Self {
		let (element, attributes) = self.into_parts();
		Item::from_parts(element.clamp_hard_min(min), attributes)
	}
	fn clamp_hard_max(self, max: f64) -> Self {
		let (element, attributes) = self.into_parts();
		Item::from_parts(element.clamp_hard_max(max), attributes)
	}
}
impl<T: Clampable> Clampable for List<T> {
	fn clamp_hard_min(self, min: f64) -> Self {
		self.into_iter().map(|item| item.clamp_hard_min(min)).collect()
	}
	fn clamp_hard_max(self, max: f64) -> Self {
		self.into_iter().map(|item| item.clamp_hard_max(max)).collect()
	}
}

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct LegacyTable<T> {
	#[serde(alias = "instances", alias = "instance")]
	element: Vec<T>,
}

// TODO: Eventually remove this migration document upgrade code
pub fn migrate_to_optional_color<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<no_std_types::color::Color>, D::Error> {
	use no_std_types::color::Color;
	use serde::Deserialize;

	#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
	#[cfg_attr(feature = "serde", serde(untagged))]
	enum ColorFormat {
		OptionalColor(Option<Color>),
		List(LegacyTable<Color>),
	}

	Ok(match ColorFormat::deserialize(deserializer)? {
		ColorFormat::OptionalColor(color) => color,
		ColorFormat::List(list) => list.element.into_iter().next(),
	})
}

// TODO: Eventually remove this migration document upgrade code
pub fn migrate_to_f64_array<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<f64>, D::Error> {
	use serde::Deserialize;

	#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
	#[cfg_attr(feature = "serde", serde(untagged))]
	enum F64ArrayFormat {
		Array(Vec<f64>),
		List(LegacyTable<f64>),
	}

	Ok(match F64ArrayFormat::deserialize(deserializer)? {
		F64ArrayFormat::Array(values) => values,
		F64ArrayFormat::List(list) => list.element,
	})
}

/// Parse a CSS color string (named color, hex, `rgb(...)`, `hsl(...)`, etc.) into a linear-light [`Color`] using the `color` crate's CSS Color 4 parser.
/// Tries the input as-is first (catches CSS named colors like `red`, `rgb(...)`, and well-formed hex like `#abcdef`), then falls back to treating the input as bare hex with length-based expansion to a CSS-parseable form:
/// - 1 char `f` → `#fff` (CSS 3-char shorthand)
/// - 2 char `ab` → `#ababab` (repeated to 6 chars)
/// - 4 char `abcd` → `#00abcd` (left-padded with `00`)
/// - 5 char `abcde` → `#0abcde` (left-padded with `0`)
/// - 3, 6, 8 char inputs are passed through with a `#` prefix.
pub fn parse_css_color(input: &str) -> Option<crate::Color> {
	let trimmed = input.trim();

	let parsed = color::parse_color(trimmed).ok().or_else(|| {
		let bare = trimmed.strip_prefix('#').unwrap_or(trimmed);
		if bare.is_empty() || !bare.chars().all(|c| c.is_ascii_hexdigit()) {
			return None;
		}
		let expanded = match bare.len() {
			1 => bare.repeat(3),
			2 => bare.repeat(3),
			4 => format!("00{bare}"),
			5 => format!("0{bare}"),
			_ => bare.to_string(),
		};
		let candidate = format!("#{expanded}");
		// Avoid retrying the exact same string we just failed to parse.
		(candidate != trimmed).then(|| color::parse_color(&candidate).ok()).flatten()
	})?;

	let srgb: color::AlphaColor<color::Srgb> = parsed.to_alpha_color();
	let [red, green, blue, alpha] = srgb.components;
	// Reject out-of-gamut values that `color::parse_color` accepts for newer CSS syntax (e.g., `rgb(300 -50 200)`).
	let in_gamut = alpha <= 1. && ![red, green, blue, alpha].iter().any(|c| c.is_sign_negative() || !c.is_finite());
	in_gamut.then(|| crate::Color::from_gamma_srgb_channels(red, green, blue, alpha))
}
