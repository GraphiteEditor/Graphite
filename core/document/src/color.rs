use serde::{Deserialize, Serialize};

/// Structure that represent a color.
/// Internally alpha is stored as `f32` that range from `0.0` (transparent) to 1.0 (opaque).
/// The other components (RGB) are stored as `f32` that range from `0.0` up to `f32::MAX`,
/// the values encode the brightness of each channel proportional to the light intensity in cd/m² (nits) in HDR, and 0.0 (black) to 1.0 (white) in SDR color.
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

	/// Return Some(Color) if `red`, `green`, `blue` and `alpha` have a valid value. Negative number (including `-0.0`), `f32::NAN` and infinity are not valid value and return `None`.
	/// Values greater than `1.0` for alpha are not valid.
	/// # Examples
	/// ```
	/// use graphite_document_core::color::Color;
	/// let color = Color::from_rgbaf32(0.3, 0.14, 0.15, 0.92).unwrap();
	/// assert!(color.components() == (0.3, 0.14, 0.15, 0.92));
	///
	/// let color = Color::from_rgbaf32(1.0, 1.0, 1.0, f32::NAN);
	/// assert!(color == None);
	/// ```
	pub fn from_rgbaf32(red: f32, green: f32, blue: f32, alpha: f32) -> Option<Color> {
		let color = Color { red, green, blue, alpha };

		if alpha > 1. || [red, green, blue, alpha].iter().any(|c| c.is_sign_negative() || !c.is_finite()) {
			return None;
		}
		Some(color)
	}

	// Return Color without checking `red` `green` `blue` and without transparency (alpha = 1.0)
	const fn from_unsafe(red: f32, green: f32, blue: f32) -> Color {
		Color { red, green, blue, alpha: 1. }
	}

	/// Return a Color without transparency (alpha = 0xFF).
	/// # Examples
	/// ```
	/// use graphite_document_core::color::Color;
	/// let color = Color::from_rgb8(0x72, 0x67, 0x62);
	/// let color2 = Color::from_rgba8(0x72, 0x67, 0x62, 0xFF);
	/// assert!(color == color2)
	/// ```
	pub fn from_rgb8(red: u8, green: u8, blue: u8) -> Color {
		Color::from_rgba8(red, green, blue, 255)
	}

	/// Return a color initialized by it's 8bit component.
	///
	/// # Examples
	/// ```
	/// use graphite_document_core::color::Color;
	/// let color = Color::from_rgba8(0x72, 0x67, 0x62, 0x61);
	/// assert!("72676261" == color.rgba_hex())
	/// ```
	pub fn from_rgba8(red: u8, green: u8, blue: u8, alpha: u8) -> Color {
		let map = |int_color| int_color as f32 / 255.0;
		Color {
			red: map(red),
			green: map(green),
			blue: map(blue),
			alpha: map(alpha),
		}
	}

	/// Return the red component.
	///
	/// # Examples
	/// ```
	/// use graphite_document_core::color::Color;
	/// let color = Color::from_rgbaf32(0.114, 0.103, 0.98, 0.97).unwrap();
	/// assert!(color.r() == 0.114);
	/// ```
	pub fn r(&self) -> f32 {
		self.red
	}

	/// Return the green component.
	///
	/// # Examples
	/// ```
	/// use graphite_document_core::color::Color;
	/// let color = Color::from_rgbaf32(0.114, 0.103, 0.98, 0.97).unwrap();
	/// assert!(color.g() == 0.103);
	/// ```
	pub fn g(&self) -> f32 {
		self.green
	}

	/// Return the blue component.
	///
	/// # Examples
	/// ```
	/// use graphite_document_core::color::Color;
	/// let color = Color::from_rgbaf32(0.114, 0.103, 0.98, 0.97).unwrap();
	/// assert!(color.b() == 0.98);
	/// ```
	pub fn b(&self) -> f32 {
		self.blue
	}

	/// Return the alpha component.
	///
	/// # Examples
	/// ```
	/// use graphite_document_core::color::Color;
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
	/// use graphite_document_core::color::Color;
	/// let color = Color::from_rgbaf32(0.114, 0.103, 0.98, 0.97).unwrap();
	/// assert!(color.components() == (0.114, 0.103, 0.98, 0.97));
	/// ```
	pub fn components(&self) -> (f32, f32, f32, f32) {
		(self.red, self.green, self.blue, self.alpha)
	}

	/// Return a String of hexadecimal value with two digit per components ("RRGGBBAA").
	/// ```
	/// use graphite_document_core::color::Color;
	/// let color = Color::from_rgba8(0x72, 0x67, 0x62, 0x61);
	/// assert!("72676261" == color.rgba_hex())
	/// ```
	pub fn rgba_hex(&self) -> String {
		format!(
			"{:02X?}{:02X?}{:02X?}{:02X?}",
			(self.r() * 255.) as u8,
			(self.g() * 255.) as u8,
			(self.b() * 255.) as u8,
			(self.a() * 255.) as u8,
		)
	}

	/// Return a String of hexadecimal value with two digit per components ("RRGGBB").
	/// ```
	/// use graphite_document_core::color::Color;
	/// let color = Color::from_rgba8(0x72, 0x67, 0x62, 0x61);
	/// assert!("726762" == color.rgb_hex())
	/// ```
	pub fn rgb_hex(&self) -> String {
		format!("{:02X?}{:02X?}{:02X?}", (self.r() * 255.) as u8, (self.g() * 255.) as u8, (self.b() * 255.) as u8,)
	}
}
