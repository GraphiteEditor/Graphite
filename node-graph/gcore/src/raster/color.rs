#[cfg(feature = "std")]
use dyn_any::{DynAny, StaticType};
use serde::{Deserialize, Serialize};

/// Structure that represents a color.
/// Internally alpha is stored as `f32` that ranges from `0.0` (transparent) to `1.0` (opaque).
/// The other components (RGB) are stored as `f32` that range from `0.0` up to `f32::MAX`,
/// the values encode the brightness of each channel proportional to the light intensity in cd/m² (nits) in HDR, and `0.0` (black) to `1.0` (white) in SDR color.
#[repr(C)]
#[cfg_attr(feature = "std", derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, DynAny))]
#[cfg_attr(not(feature = "std"), derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize))]
pub struct Color {
	red: f32,
	green: f32,
	blue: f32,
	alpha: f32,
}

impl Color {
	pub const BLACK: Color = Color::from_rgbf32_unchecked(0., 0., 0.);
	pub const WHITE: Color = Color::from_rgbf32_unchecked(1., 1., 1.);
	pub const RED: Color = Color::from_rgbf32_unchecked(1., 0., 0.);
	pub const GREEN: Color = Color::from_rgbf32_unchecked(0., 1., 0.);
	pub const BLUE: Color = Color::from_rgbf32_unchecked(0., 0., 1.);

	/// Returns `Some(Color)` if `red`, `green`, `blue` and `alpha` have a valid value. Negative numbers (including `-0.0`), NaN, and infinity are not valid values and return `None`.
	/// Alpha values greater than `1.0` are not valid.
	///
	/// # Examples
	/// ```
	/// use graphene_core::raster::color::Color;
	/// let color = Color::from_rgbaf32(0.3, 0.14, 0.15, 0.92).unwrap();
	/// assert!(color.components() == (0.3, 0.14, 0.15, 0.92));
	///
	/// let color = Color::from_rgbaf32(1.0, 1.0, 1.0, f32::NAN);
	/// assert!(color == None);
	/// ```
	pub fn from_rgbaf32(red: f32, green: f32, blue: f32, alpha: f32) -> Option<Color> {
		if alpha > 1. || [red, green, blue, alpha].iter().any(|c| c.is_sign_negative() || !c.is_finite()) {
			return None;
		}
		Some(Color { red, green, blue, alpha })
	}

	/// Return an opaque `Color` from given `f32` RGB channels.
	pub const fn from_rgbaf32_unchecked(red: f32, green: f32, blue: f32, alpha: f32) -> Color {
		Color { red, green, blue, alpha }
	}

	/// Return an opaque `Color` from given `f32` RGB channels.
	pub const fn from_rgbf32_unchecked(red: f32, green: f32, blue: f32) -> Color {
		Color { red, green, blue, alpha: 1. }
	}

	/// Return an opaque SDR `Color` given RGB channels from `0` to `255`.
	///
	/// # Examples
	/// ```
	/// use graphene_core::raster::color::Color;
	/// let color = Color::from_rgb8(0x72, 0x67, 0x62);
	/// let color2 = Color::from_rgba8(0x72, 0x67, 0x62, 0xFF);
	/// assert!(color == color2)
	/// ```
	pub fn from_rgb8(red: u8, green: u8, blue: u8) -> Color {
		Color::from_rgba8(red, green, blue, 255)
	}

	/// Return an SDR `Color` given RGBA channels from `0` to `255`.
	///
	/// # Examples
	/// ```
	/// use graphene_core::raster::color::Color;
	/// let color = Color::from_rgba8(0x72, 0x67, 0x62, 0x61);
	/// ```
	pub fn from_rgba8(red: u8, green: u8, blue: u8, alpha: u8) -> Color {
		let map_range = |int_color| int_color as f32 / 255.0;
		Color {
			red: map_range(red),
			green: map_range(green),
			blue: map_range(blue),
			alpha: map_range(alpha),
		}
	}

	/// Create a [Color] from a hue, saturation, lightness and alpha (all between 0 and 1)
	///
	/// # Examples
	/// ```
	/// use graphene_core::raster::color::Color;
	/// let color = Color::from_hsla(0.5, 0.2, 0.3, 1.);
	/// ```
	pub fn from_hsla(hue: f32, saturation: f32, lightness: f32, alpha: f32) -> Color {
		let temp1 = if lightness < 0.5 {
			lightness * (saturation + 1.)
		} else {
			lightness + saturation - lightness * saturation
		};
		let temp2 = 2. * lightness - temp1;

		let mut red = (hue + 1. / 3.).rem_euclid(1.);
		let mut green = hue.rem_euclid(1.);
		let mut blue = (hue - 1. / 3.).rem_euclid(1.);

		for channel in [&mut red, &mut green, &mut blue] {
			*channel = if *channel * 6. < 1. {
				temp2 + (temp1 - temp2) * 6. * *channel
			} else if *channel * 2. < 1. {
				temp1
			} else if *channel * 3. < 2. {
				temp2 + (temp1 - temp2) * (2. / 3. - *channel) * 6.
			} else {
				temp2
			}
			.clamp(0., 1.);
		}

		Color { red, green, blue, alpha }
	}

	/// Return the `red` component.
	///
	/// # Examples
	/// ```
	/// use graphene_core::raster::color::Color;
	/// let color = Color::from_rgbaf32(0.114, 0.103, 0.98, 0.97).unwrap();
	/// assert!(color.r() == 0.114);
	/// ```
	pub fn r(&self) -> f32 {
		self.red
	}

	/// Return the `green` component.
	///
	/// # Examples
	/// ```
	/// use graphene_core::raster::color::Color;
	/// let color = Color::from_rgbaf32(0.114, 0.103, 0.98, 0.97).unwrap();
	/// assert!(color.g() == 0.103);
	/// ```
	pub fn g(&self) -> f32 {
		self.green
	}

	/// Return the `blue` component.
	///
	/// # Examples
	/// ```
	/// use graphene_core::raster::color::Color;
	/// let color = Color::from_rgbaf32(0.114, 0.103, 0.98, 0.97).unwrap();
	/// assert!(color.b() == 0.98);
	/// ```
	pub fn b(&self) -> f32 {
		self.blue
	}

	/// Return the `alpha` component without checking its expected `0.0` to `1.0` range.
	///
	/// # Examples
	/// ```
	/// use graphene_core::raster::color::Color;
	/// let color = Color::from_rgbaf32(0.114, 0.103, 0.98, 0.97).unwrap();
	/// assert!(color.a() == 0.97);
	/// ```
	pub fn a(&self) -> f32 {
		self.alpha
	}

	/// Return the all components as a tuple, first component is red, followed by green, followed by blue, followed by alpha.
	///
	/// # Examples
	/// ```
	/// use graphene_core::raster::color::Color;
	/// let color = Color::from_rgbaf32(0.114, 0.103, 0.98, 0.97).unwrap();
	/// assert!(color.components() == (0.114, 0.103, 0.98, 0.97));
	/// ```
	pub fn components(&self) -> (f32, f32, f32, f32) {
		(self.red, self.green, self.blue, self.alpha)
	}

	/// Return the all components as a u8 slice, first component is red, followed by green, followed by blue, followed by alpha.
	///
	/// # Examples
	/// ```
	/// use graphene_core::raster::color::Color;
	/// let color = Color::from_rgbaf32(0.114, 0.103, 0.98, 0.97).unwrap();
	/// //TODO: Add test
	/// ```
	pub fn to_rgba8(&self) -> [u8; 4] {
		[(self.red * 255.) as u8, (self.green * 255.) as u8, (self.blue * 255.) as u8, (self.alpha * 255.) as u8]
	}

	// https://www.niwa.nu/2013/05/math-behind-colorspace-conversions-rgb-hsl/
	/// Convert a [Color] to a hue, saturation, lightness and alpha (all between 0 and 1)
	///
	/// # Examples
	/// ```
	/// use graphene_core::raster::color::Color;
	/// let color = Color::from_hsla(0.5, 0.2, 0.3, 1.).to_hsla();
	/// ```
	pub fn to_hsla(&self) -> [f32; 4] {
		let min_channel = self.red.min(self.green).min(self.blue);
		let max_channel = self.red.max(self.green).max(self.blue);

		let lightness = (min_channel + max_channel) / 2.;
		let saturation = if min_channel == max_channel {
			0.
		} else if lightness <= 0.5 {
			(max_channel - min_channel) / (max_channel + min_channel)
		} else {
			(max_channel - min_channel) / (2. - max_channel - min_channel)
		};
		let hue = if self.red >= self.green && self.red >= self.blue {
			(self.green - self.blue) / (max_channel - min_channel)
		} else if self.green >= self.red && self.green >= self.blue {
			2. + (self.blue - self.red) / (max_channel - min_channel)
		} else {
			4. + (self.red - self.green) / (max_channel - min_channel)
		} / 6.;
		let hue = hue.rem_euclid(1.);

		[hue, saturation, lightness, self.alpha]
	}

	// TODO: Readd formatting

	/// Creates a color from a 8-character RGBA hex string (without a # prefix).
	///
	/// # Examples
	/// ```
	/// use graphene_core::raster::color::Color;
	/// let color = Color::from_rgba_str("7C67FA61").unwrap();
	/// ```
	pub fn from_rgba_str(color_str: &str) -> Option<Color> {
		if color_str.len() != 8 {
			return None;
		}
		let r = u8::from_str_radix(&color_str[0..2], 16).ok()?;
		let g = u8::from_str_radix(&color_str[2..4], 16).ok()?;
		let b = u8::from_str_radix(&color_str[4..6], 16).ok()?;
		let a = u8::from_str_radix(&color_str[6..8], 16).ok()?;

		Some(Color::from_rgba8(r, g, b, a))
	}

	/// Creates a color from a 6-character RGB hex string (without a # prefix).
	/// ```
	/// use graphene_core::raster::color::Color;
	/// let color = Color::from_rgb_str("7C67FA").unwrap();
	/// ```
	pub fn from_rgb_str(color_str: &str) -> Option<Color> {
		if color_str.len() != 6 {
			return None;
		}
		let r = u8::from_str_radix(&color_str[0..2], 16).ok()?;
		let g = u8::from_str_radix(&color_str[2..4], 16).ok()?;
		let b = u8::from_str_radix(&color_str[4..6], 16).ok()?;

		Some(Color::from_rgb8(r, g, b))
	}
}

#[test]
fn hsl_roundtrip() {
	for (red, green, blue) in [
		(24, 98, 118),
		(69, 11, 89),
		(54, 82, 38),
		(47, 76, 50),
		(25, 15, 73),
		(62, 57, 33),
		(55, 2, 18),
		(12, 3, 82),
		(91, 16, 98),
		(91, 39, 82),
		(97, 53, 32),
		(76, 8, 91),
		(54, 87, 19),
		(56, 24, 88),
		(14, 82, 34),
		(61, 86, 31),
		(73, 60, 75),
		(95, 79, 88),
		(13, 34, 4),
		(82, 84, 84),
		(255, 255, 178),
	] {
		let col = Color::from_rgb8(red, green, blue);
		let [hue, saturation, lightness, alpha] = col.to_hsla();
		let result = Color::from_hsla(hue, saturation, lightness, alpha);
		assert!((col.r() - result.r()) < f32::EPSILON * 100.);
		assert!((col.g() - result.g()) < f32::EPSILON * 100.);
		assert!((col.b() - result.b()) < f32::EPSILON * 100.);
		assert!((col.a() - result.a()) < f32::EPSILON * 100.);
	}
}
