use core::hash::Hash;

use dyn_any::{DynAny, StaticType};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::float::Float;

#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::Euclid;

#[cfg(feature = "gpu")]
use bytemuck::{Pod, Zeroable};

/// Structure that represents a color.
/// Internally alpha is stored as `f32` that ranges from `0.0` (transparent) to `1.0` (opaque).
/// The other components (RGB) are stored as `f32` that range from `0.0` up to `f32::MAX`,
/// the values encode the brightness of each channel proportional to the light intensity in cd/m² (nits) in HDR, and `0.0` (black) to `1.0` (white) in SDR color.
#[repr(C)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "gpu", derive(Pod, Zeroable))]
#[cfg_attr(feature = "std", derive(specta::Type))]
#[derive(Debug, Default, Clone, Copy, PartialEq, DynAny)]
pub struct Color {
	red: f32,
	green: f32,
	blue: f32,
	alpha: f32,
}

#[allow(clippy::derived_hash_with_manual_eq)]
impl Hash for Color {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.red.to_bits().hash(state);
		self.green.to_bits().hash(state);
		self.blue.to_bits().hash(state);
		self.alpha.to_bits().hash(state);
	}
}

impl Color {
	pub const BLACK: Color = Color::from_rgbf32_unchecked(0., 0., 0.);
	pub const WHITE: Color = Color::from_rgbf32_unchecked(1., 1., 1.);
	pub const RED: Color = Color::from_rgbf32_unchecked(1., 0., 0.);
	pub const GREEN: Color = Color::from_rgbf32_unchecked(0., 1., 0.);
	pub const BLUE: Color = Color::from_rgbf32_unchecked(0., 0., 1.);
	pub const TRANSPARENT: Color = Self {
		red: 0.,
		green: 0.,
		blue: 0.,
		alpha: 0.,
	};

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
	#[cfg(not(target_arch = "spirv"))]
	pub fn from_rgbaf32(red: f32, green: f32, blue: f32, alpha: f32) -> Option<Color> {
		if alpha > 1. || [red, green, blue, alpha].iter().any(|c| c.is_sign_negative() || !c.is_finite()) {
			return None;
		}
		let color = Color { red, green, blue, alpha };
		Some(color)
	}

	/// Return an opaque `Color` from given `f32` RGB channels.
	pub const fn from_rgbf32_unchecked(red: f32, green: f32, blue: f32) -> Color {
		Color { red, green, blue, alpha: 1. }
	}

	/// Return an opaque `Color` from given `f32` RGB channels.
	pub const fn from_rgbaf32_unchecked(red: f32, green: f32, blue: f32, alpha: f32) -> Color {
		Color { red, green, blue, alpha }
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
		#[cfg(not(target_arch = "spirv"))]
		let rem = |x: f32| x.rem_euclid(1.);
		#[cfg(target_arch = "spirv")]
		let rem = |x: f32| x.rem_euclid(&1.);

		let mut red = rem(hue + 1. / 3.);
		let mut green = rem(hue);
		let mut blue = rem(hue - 1. / 3.);

		fn map_channel(channel: &mut f32, temp2: f32, temp1: f32) {
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
		map_channel(&mut red, temp2, temp1);
		map_channel(&mut green, temp2, temp1);
		map_channel(&mut blue, temp2, temp1);

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

	pub fn average_rgb_channels(&self) -> f32 {
		(self.red + self.green + self.blue) / 3.
	}

	pub fn minimum_rgb_channels(&self) -> f32 {
		self.red.min(self.green).min(self.blue)
	}

	pub fn maximum_rgb_channels(&self) -> f32 {
		self.red.max(self.green).max(self.blue)
	}

	// From https://stackoverflow.com/a/56678483/775283
	pub fn luminance_srgb(&self) -> f32 {
		0.2126 * self.red + 0.7152 * self.green + 0.0722 * self.blue
	}

	// From https://en.wikipedia.org/wiki/Luma_(video)#Rec._601_luma_versus_Rec._709_luma_coefficients
	pub fn luminance_rec_601(&self) -> f32 {
		0.299 * self.red + 0.587 * self.green + 0.114 * self.blue
	}

	// From https://en.wikipedia.org/wiki/Luma_(video)#Rec._601_luma_versus_Rec._709_luma_coefficients
	pub fn luminance_rec_601_rounded(&self) -> f32 {
		0.3 * self.red + 0.59 * self.green + 0.11 * self.blue
	}

	// From https://stackoverflow.com/a/56678483/775283
	pub fn luminance_perceptual(&self) -> f32 {
		let luminance = self.luminance_srgb();

		if luminance <= 0.008856 {
			(luminance * 903.3) / 100.
		} else {
			(luminance.powf(1. / 3.) * 116. - 16.) / 100.
		}
	}

	pub fn with_luminance(&self, luminance: f32) -> Color {
		let d = luminance - self.luminance_rec_601_rounded();
		self.map_rgb(|c| (c + d).clamp(0., 1.))
	}

	pub fn saturation(&self) -> f32 {
		let max = (self.red).max(self.green).max(self.blue);
		let min = (self.red).min(self.green).min(self.blue);

		max - min
	}

	pub fn with_saturation(&self, saturation: f32) -> Color {
		let [hue, _, lightness, alpha] = self.to_hsla();
		Color::from_hsla(hue, saturation, lightness, alpha)
	}

	pub fn blend_normal(_c_b: f32, c_s: f32) -> f32 {
		c_s
	}

	pub fn blend_multiply(c_b: f32, c_s: f32) -> f32 {
		c_s * c_b
	}

	pub fn blend_darken(c_b: f32, c_s: f32) -> f32 {
		c_s.min(c_b)
	}

	pub fn blend_color_burn(c_b: f32, c_s: f32) -> f32 {
		if c_b == 1. {
			1.
		} else if c_s == 0. {
			0.
		} else {
			1. - ((1. - c_b) / c_s).min(1.)
		}
	}

	pub fn blend_linear_burn(c_b: f32, c_s: f32) -> f32 {
		c_b + c_s - 1.
	}

	pub fn blend_darker_color(&self, other: Color) -> Color {
		if self.average_rgb_channels() <= other.average_rgb_channels() {
			*self
		} else {
			other
		}
	}

	pub fn blend_screen(c_b: f32, c_s: f32) -> f32 {
		1. - (1. - c_s) * (1. - c_b)
	}

	pub fn blend_lighten(c_b: f32, c_s: f32) -> f32 {
		c_s.max(c_b)
	}

	pub fn blend_color_dodge(c_b: f32, c_s: f32) -> f32 {
		if c_s == 1. {
			1.
		} else {
			(c_b / (1. - c_s)).min(1.)
		}
	}

	pub fn blend_linear_dodge(c_b: f32, c_s: f32) -> f32 {
		c_b + c_s
	}

	pub fn blend_lighter_color(&self, other: Color) -> Color {
		if self.average_rgb_channels() >= other.average_rgb_channels() {
			*self
		} else {
			other
		}
	}

	pub fn blend_softlight(c_b: f32, c_s: f32) -> f32 {
		if c_s <= 0.5 {
			c_b - (1. - 2. * c_s) * c_b * (1. - c_b)
		} else {
			let d: fn(f32) -> f32 = |x| if x <= 0.25 { ((16. * x - 12.) * x + 4.) * x } else { x.sqrt() };
			c_b + (2. * c_s - 1.) * (d(c_b) - c_b)
		}
	}

	pub fn blend_hardlight(c_b: f32, c_s: f32) -> f32 {
		if c_s <= 0.5 {
			Color::blend_multiply(2. * c_s, c_b)
		} else {
			Color::blend_screen(2. * c_s - 1., c_b)
		}
	}

	pub fn blend_vivid_light(c_b: f32, c_s: f32) -> f32 {
		if c_s <= 0.5 {
			Color::blend_color_burn(2. * c_s, c_b)
		} else {
			Color::blend_color_dodge(2. * c_s - 1., c_b)
		}
	}

	pub fn blend_linear_light(c_b: f32, c_s: f32) -> f32 {
		if c_s <= 0.5 {
			Color::blend_linear_burn(2. * c_s, c_b)
		} else {
			Color::blend_linear_dodge(2. * c_s - 1., c_b)
		}
	}

	pub fn blend_pin_light(c_b: f32, c_s: f32) -> f32 {
		if c_s <= 0.5 {
			Color::blend_darken(2. * c_s, c_b)
		} else {
			Color::blend_lighten(2. * c_s - 1., c_b)
		}
	}

	pub fn blend_hard_mix(c_b: f32, c_s: f32) -> f32 {
		if Color::blend_linear_light(c_b, c_s) < 0.5 {
			0.
		} else {
			1.
		}
	}

	pub fn blend_difference(c_b: f32, c_s: f32) -> f32 {
		(c_b - c_s).abs()
	}

	pub fn blend_exclusion(c_b: f32, c_s: f32) -> f32 {
		c_b + c_s - 2. * c_b * c_s
	}

	pub fn blend_subtract(c_b: f32, c_s: f32) -> f32 {
		c_b - c_s
	}

	pub fn blend_divide(c_b: f32, c_s: f32) -> f32 {
		if c_b == 0. {
			1.
		} else {
			c_b / c_s
		}
	}

	pub fn blend_hue(&self, c_s: Color) -> Color {
		let sat_b = self.saturation();
		let lum_b = self.luminance_rec_601();
		c_s.with_saturation(sat_b).with_luminance(lum_b)
	}

	pub fn blend_saturation(&self, c_s: Color) -> Color {
		let sat_s = c_s.saturation();
		let lum_b = self.luminance_rec_601();

		self.with_saturation(sat_s).with_luminance(lum_b)
	}

	pub fn blend_color(&self, c_s: Color) -> Color {
		let lum_b = self.luminance_rec_601();

		c_s.with_luminance(lum_b)
	}

	pub fn blend_luminosity(&self, c_s: Color) -> Color {
		let lum_s = c_s.luminance_rec_601();

		self.with_luminance(lum_s)
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

	/// Return an 8-character RGBA hex string (without a # prefix).
	///
	/// # Examples
	/// ```
	/// use graphene_core::raster::color::Color;
	/// let color = Color::from_rgba8(0x7C, 0x67, 0xFA, 0x61);
	/// assert!("7C67FA61" == color.rgba_hex())
	/// ```
	#[cfg(feature = "std")]
	pub fn rgba_hex(&self) -> String {
		format!(
			"{:02X?}{:02X?}{:02X?}{:02X?}",
			(self.r() * 255.) as u8,
			(self.g() * 255.) as u8,
			(self.b() * 255.) as u8,
			(self.a() * 255.) as u8,
		)
	}

	/// Return a 6-character RGB hex string (without a # prefix).
	/// ```
	/// use graphene_core::raster::color::Color;
	/// let color = Color::from_rgba8(0x7C, 0x67, 0xFA, 0x61);
	/// assert!("7C67FA" == color.rgb_hex())
	/// ```
	#[cfg(feature = "std")]
	pub fn rgb_hex(&self) -> String {
		format!("{:02X?}{:02X?}{:02X?}", (self.r() * 255.) as u8, (self.g() * 255.) as u8, (self.b() * 255.) as u8,)
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
		#[cfg(not(target_arch = "spirv"))]
		let hue = hue.rem_euclid(1.);
		#[cfg(target_arch = "spirv")]
		let hue = hue.rem_euclid(&1.);

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
	#[cfg(not(target_arch = "spirv"))]
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
	#[cfg(not(target_arch = "spirv"))]
	pub fn from_rgb_str(color_str: &str) -> Option<Color> {
		if color_str.len() != 6 {
			return None;
		}
		let r = u8::from_str_radix(&color_str[0..2], 16).ok()?;
		let g = u8::from_str_radix(&color_str[2..4], 16).ok()?;
		let b = u8::from_str_radix(&color_str[4..6], 16).ok()?;

		Some(Color::from_rgb8(r, g, b))
	}

	/// Linearly interpolates between two colors based on t.
	///
	/// T must be between 0 and 1.
	pub fn lerp(self, other: Color, t: f32) -> Self {
		assert!((0. ..=1.).contains(&t));
		Color::from_rgbaf32_unchecked(
			self.red + ((other.red - self.red) * t),
			self.green + ((other.green - self.green) * t),
			self.blue + ((other.blue - self.blue) * t),
			self.alpha + ((other.alpha - self.alpha) * t),
		)
	}

	pub fn gamma(&self, gamma: f32) -> Color {
		// From https://www.dfstudios.co.uk/articles/programming/image-programming-algorithms/image-processing-algorithms-part-6-gamma-correction/
		let inverse_gamma = 1. / gamma;
		let per_channel = |channel: f32| channel.powf(inverse_gamma);
		self.map_rgb(per_channel)
	}

	pub fn to_linear_srgb(&self) -> Self {
		Self {
			red: Self::srgb_to_linear(self.red),
			green: Self::srgb_to_linear(self.green),
			blue: Self::srgb_to_linear(self.blue),
			alpha: self.alpha,
		}
	}

	pub fn to_gamma_srgb(&self) -> Self {
		Self {
			red: Self::linear_to_srgb(self.red),
			green: Self::linear_to_srgb(self.green),
			blue: Self::linear_to_srgb(self.blue),
			alpha: self.alpha,
		}
	}

	pub fn srgb_to_linear(channel: f32) -> f32 {
		if channel <= 0.04045 {
			channel / 12.92
		} else {
			((channel + 0.055) / 1.055).powf(2.4)
		}
	}

	pub fn linear_to_srgb(channel: f32) -> f32 {
		if channel <= 0.0031308 {
			channel * 12.92
		} else {
			1.055 * channel.powf(1. / 2.4) - 0.055
		}
	}

	pub fn map_rgba<F: Fn(f32) -> f32>(&self, f: F) -> Self {
		Self::from_rgbaf32_unchecked(f(self.r()), f(self.g()), f(self.b()), f(self.a()))
	}
	pub fn map_rgb<F: Fn(f32) -> f32>(&self, f: F) -> Self {
		Self::from_rgbaf32_unchecked(f(self.r()), f(self.g()), f(self.b()), self.a())
	}

	pub fn to_unassociated_alpha(&self) -> Self {
		let factor = 1. / self.alpha;
		Self {
			red: self.red * factor,
			green: self.green * factor,
			blue: self.blue * factor,
			alpha: self.alpha,
		}
	}

	pub fn blend_rgb<F: Fn(f32, f32) -> f32>(&self, other: Color, f: F) -> Self {
		let background = self.to_unassociated_alpha();
		Color {
			red: f(background.red, other.red).clamp(0., 1.),
			green: f(background.green, other.green).clamp(0., 1.),
			blue: f(background.blue, other.blue).clamp(0., 1.),
			alpha: other.alpha,
		}
	}

	pub fn multiply_alpha(&self, alpha: f32) -> Self {
		Self {
			red: self.red * alpha,
			green: self.green * alpha,
			blue: self.blue * alpha,
			alpha: self.alpha * alpha,
		}
	}

	pub fn alpha_blend(&self, other: Color) -> Self {
		let inv_alpha = 1. - other.alpha;
		Self {
			red: self.red * inv_alpha + other.red,
			green: self.green * inv_alpha + other.green,
			blue: self.blue * inv_alpha + other.blue,
			alpha: self.alpha * inv_alpha + other.alpha,
		}
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
