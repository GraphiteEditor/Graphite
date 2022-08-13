use serde::{Deserialize, Serialize};

/// Structure that represents a color.
/// Internally alpha is stored as `f32` that ranges from `0.0` (transparent) to `1.0` (opaque).
/// The other components (RGB) are stored as `f32` that range from `0.0` up to `f32::MAX`,
/// the values encode the brightness of each channel proportional to the light intensity in cd/m² (nits) in HDR, and `0.0` (black) to `1.0` (white) in SDR color.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Color {
	red: f32,
	green: f32,
	blue: f32,
	alpha: f32,
}

impl Color {
	pub const BLACK: Color = Color::from_unsafe(0., 0., 0.);
	pub const WHITE: Color = Color::from_unsafe(1., 1., 1.);
	pub const RED: Color = Color::from_unsafe(1., 0., 0.);
	pub const GREEN: Color = Color::from_unsafe(0., 1., 0.);
	pub const BLUE: Color = Color::from_unsafe(0., 0., 1.);

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
	pub const fn from_unsafe(red: f32, green: f32, blue: f32) -> Color {
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
