use super::color_traits::{Alpha, AlphaMut, AssociatedAlpha, Luminance, LuminanceMut, Pixel, RGB, RGBMut, Rec709Primaries, SRGB};
use super::discrete_srgb::{float_to_srgb_u8, srgb_u8_to_float};
use bytemuck::{Pod, Zeroable};
use core::fmt::Debug;
use glam::Vec4;
use half::f16;
use node_macro::BufferStruct;
#[cfg(not(feature = "std"))]
use num_traits::Euclid;
#[cfg(not(feature = "std"))]
use num_traits::float::Float;

#[repr(C)]
#[derive(Default, Clone, Copy, PartialEq, Pod, Zeroable)]
#[cfg_attr(not(target_arch = "spirv"), derive(Debug))]
#[cfg_attr(feature = "std", derive(dyn_any::DynAny, serde::Serialize, serde::Deserialize))]
pub struct RGBA16F {
	red: f16,
	green: f16,
	blue: f16,
	alpha: f16,
}

/// hack around half still masking out impl Debug for f16 on spirv
#[cfg(target_arch = "spirv")]
impl core::fmt::Debug for RGBA16F {
	fn fmt(&self, _f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		Ok(())
	}
}

impl From<Color> for RGBA16F {
	#[inline(always)]
	fn from(c: Color) -> Self {
		Self {
			red: f16::from_f32(c.r()),
			green: f16::from_f32(c.g()),
			blue: f16::from_f32(c.b()),
			alpha: f16::from_f32(c.a()),
		}
	}
}

impl Luminance for RGBA16F {
	type LuminanceChannel = f32;
	#[inline(always)]
	fn luminance(&self) -> f32 {
		// TODO: verify this is correct for sRGB
		0.2126 * self.red() + 0.7152 * self.green() + 0.0722 * self.blue()
	}
}

impl RGB for RGBA16F {
	type ColorChannel = f32;
	#[inline(always)]
	fn red(&self) -> f32 {
		self.red.to_f32()
	}
	#[inline(always)]
	fn green(&self) -> f32 {
		self.green.to_f32()
	}
	#[inline(always)]
	fn blue(&self) -> f32 {
		self.blue.to_f32()
	}
}

impl Rec709Primaries for RGBA16F {}

impl Alpha for RGBA16F {
	type AlphaChannel = f32;
	#[inline(always)]
	fn alpha(&self) -> f32 {
		self.alpha.to_f32() / 255.
	}

	const TRANSPARENT: Self = RGBA16F {
		red: f16::from_f32_const(0.),
		green: f16::from_f32_const(0.),
		blue: f16::from_f32_const(0.),
		alpha: f16::from_f32_const(0.),
	};

	fn multiplied_alpha(&self, alpha: Self::AlphaChannel) -> Self {
		let alpha = alpha * 255.;
		let mut result = *self;
		result.alpha = f16::from_f32(alpha * self.alpha());
		result
	}
}

impl Pixel for RGBA16F {}

/// An sRGB color with 8-bit unassociated-alpha channels. Used as the wire format at the DOM boundary:
/// bijective with hex codes, byte-identical to CSS/SVG/PNG/peniko conventions. Internal computations use
/// the linear-light [`Color`] type. Convert via [`From<SRGBA8> for Color`] and [`From<Color> for SRGBA8`].
#[repr(C)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify), tsify(from_wasm_abi))]
#[cfg_attr(feature = "std", derive(dyn_any::DynAny, serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "std", derive(graphene_hash::CacheHash))]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Pod, Zeroable)]
pub struct SRGBA8 {
	pub red: u8,
	pub green: u8,
	pub blue: u8,
	pub alpha: u8,
}

impl SRGBA8 {
	pub const TRANSPARENT: Self = Self::new(0, 0, 0, 0);
	pub const BLACK: Self = Self::new(0, 0, 0, 255);
	pub const WHITE: Self = Self::new(255, 255, 255, 255);

	/// Construct from raw 8-bit channels. Alpha is unassociated (not premultiplied), matching CSS/SVG/PNG convention.
	#[inline(always)]
	pub const fn new(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
		Self { red, green, blue, alpha }
	}

	/// Construct an opaque (alpha = 255) color from raw 8-bit RGB channels.
	#[inline(always)]
	pub const fn new_opaque(red: u8, green: u8, blue: u8) -> Self {
		Self::new(red, green, blue, 255)
	}

	/// Parse `RRGGBB` or `RRGGBBAA` (with or without a leading `#`). Returns `None` for any other format.
	/// For full CSS Color 4 parsing (named colors, shorthand hex, `rgb(...)`, `hsl(...)`), parse in the caller and construct via [`Self::new`].
	#[cfg(feature = "std")]
	pub fn from_hex_str(hex: &str) -> Option<Self> {
		let hex = hex.trim().trim_start_matches('#');
		if hex.len() != 6 && hex.len() != 8 {
			return None;
		}

		let red = u8::from_str_radix(&hex[0..2], 16).ok()?;
		let green = u8::from_str_radix(&hex[2..4], 16).ok()?;
		let blue = u8::from_str_radix(&hex[4..6], 16).ok()?;
		let alpha = if hex.len() == 8 { u8::from_str_radix(&hex[6..8], 16).ok()? } else { 255 };

		Some(Self::new(red, green, blue, alpha))
	}

	/// `rrggbb` (lowercase, no `#` prefix, alpha discarded). Use where alpha is specified separately, e.g. SVG `fill="#..." fill-opacity="..."`.
	#[cfg(feature = "std")]
	pub fn to_rgb_hex(self) -> String {
		format!("{:02x}{:02x}{:02x}", self.red, self.green, self.blue)
	}

	/// `rrggbbaa` (lowercase, no `#` prefix).
	#[cfg(feature = "std")]
	pub fn to_rgba_hex(self) -> String {
		format!("{:02x}{:02x}{:02x}{:02x}", self.red, self.green, self.blue, self.alpha)
	}

	/// `#rrggbb` if fully opaque, `#rrggbbaa` otherwise. Suitable for direct insertion into a CSS property or SVG attribute.
	#[cfg(feature = "std")]
	pub fn to_css_hex(self) -> String {
		if self.alpha == 255 {
			format!("#{}", self.to_rgb_hex())
		} else {
			format!("#{}", self.to_rgba_hex())
		}
	}

	/// Returns [`Self::BLACK`] or [`Self::WHITE`], whichever gives more legible text against this color
	/// (alpha composited over white in gamma space, WCAG-style relative-luminance threshold).
	pub fn contrasting_text_color(self) -> Self {
		// Composite over white in gamma space, then convert to linear for the luminance test.
		let r = self.red as f32 / 255.;
		let g = self.green as f32 / 255.;
		let b = self.blue as f32 / 255.;
		let a = self.alpha as f32 / 255.;
		let composited = Color::from_gamma_srgb_channels(1. - a + r * a, 1. - a + g * a, 1. - a + b * a, 1.);
		let luminance = composited.luminance_rec_709();
		// WCAG-derived perceptual midpoint between black and white (~0.179)
		let threshold = (1.05_f32 * 0.05).sqrt() - 0.05;
		if luminance > threshold { Self::BLACK } else { Self::WHITE }
	}
}

impl From<[u8; 4]> for SRGBA8 {
	#[inline(always)]
	fn from(bytes: [u8; 4]) -> Self {
		let [red, green, blue, alpha] = bytes;
		Self::new(red, green, blue, alpha)
	}
}

impl From<SRGBA8> for [u8; 4] {
	#[inline(always)]
	fn from(c: SRGBA8) -> Self {
		let SRGBA8 { red, green, blue, alpha } = c;
		[red, green, blue, alpha]
	}
}

/// Lets `Image<SRGBA8>` cross the wasm boundary as gamma bytes, since `Color` (linear-light) isn't exposed with Tsify.
impl Pixel for SRGBA8 {}

impl From<Color> for SRGBA8 {
	#[inline(always)]
	fn from(c: Color) -> Self {
		Self {
			red: float_to_srgb_u8(c.r()),
			green: float_to_srgb_u8(c.g()),
			blue: float_to_srgb_u8(c.b()),
			alpha: (c.a() * 255.) as u8,
		}
	}
}

impl From<SRGBA8> for Color {
	#[inline(always)]
	fn from(color: SRGBA8) -> Self {
		Self {
			red: srgb_u8_to_float(color.red),
			green: srgb_u8_to_float(color.green),
			blue: srgb_u8_to_float(color.blue),
			alpha: color.alpha as f32 / 255.,
		}
	}
}

#[repr(C)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "std", derive(dyn_any::DynAny, serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct Luma(pub f32);

impl Luminance for Luma {
	type LuminanceChannel = f32;
	#[inline(always)]
	fn luminance(&self) -> f32 {
		self.0
	}
}

impl LuminanceMut for Luma {
	fn set_luminance(&mut self, luminance: Self::LuminanceChannel) {
		self.0 = luminance
	}
}

impl RGB for Luma {
	type ColorChannel = f32;
	#[inline(always)]
	fn red(&self) -> f32 {
		self.0
	}
	#[inline(always)]
	fn green(&self) -> f32 {
		self.0
	}
	#[inline(always)]
	fn blue(&self) -> f32 {
		self.0
	}
}

impl Pixel for Luma {}

/// Structure that represents a color.
/// Internally alpha is stored as `f32` that ranges from `0.0` (transparent) to `1.0` (opaque).
/// The other components (RGB) are stored as `f32` that range from `0.0` up to `f32::MAX`,
/// the values encode the brightness of each channel proportional to the light intensity in cd/m² (nits) in HDR, and `0.0` (black) to `1.0` (white) in SDR color.
/// Linear-light sRGB color with `f32` channels (alpha unassociated for swatch/UI colors, associated/premultiplied for pixel data inside [`Image<Color>`]).
///
/// Channels range from `0.0` to `f32::MAX`, encoding brightness proportional to light intensity (cd/m² nits in HDR, or `0..=1` mapped to white for SDR).
///
/// Anything crossing the Wasm/JS boundary must go through [`SRGBA8`] instead.
#[repr(C)]
#[cfg_attr(feature = "std", derive(dyn_any::DynAny))]
#[cfg_attr(feature = "std", derive(graphene_hash::CacheHash))]
#[derive(Debug, Default, Clone, Copy, PartialEq, Pod, Zeroable, BufferStruct)]
pub struct Color {
	red: f32,
	green: f32,
	blue: f32,
	alpha: f32,
}

// `f32` channels mean `Color` doesn't qualify for a derived `Eq`, but in practice we never store NaN here, and the renderer's `HashMap<CacheHashWrapper<Image<Color>>, _>` deduplication needs `Color: Eq` to propagate up through the wrapper.
impl Eq for Color {}

// TODO: Eventually remove this migration document upgrade code
#[cfg(feature = "std")]
impl serde::Serialize for Color {
	fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		use serde::ser::SerializeStruct;
		// Persist linear-light floats directly and tag with `"linear": true` so legacy gamma-encoded values (which lack this marker) can be detected and upgraded on load.
		let mut state = serializer.serialize_struct("Color", 5)?;
		state.serialize_field("red", &self.red)?;
		state.serialize_field("green", &self.green)?;
		state.serialize_field("blue", &self.blue)?;
		state.serialize_field("alpha", &self.alpha)?;
		// TODO: Remove the `linear` marker when switching to the new document format and Ctrl-C node serialization format
		state.serialize_field("linear", &true)?;
		state.end()
	}
}

// TODO: Eventually remove this migration document upgrade code
#[cfg(feature = "std")]
impl<'de> serde::Deserialize<'de> for Color {
	fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		// Documents from before the linear-storage migration lack the `linear` marker and stored gamma-encoded floats; convert them on load.
		#[derive(serde::Deserialize)]
		struct MigrationColor {
			red: f32,
			green: f32,
			blue: f32,
			alpha: f32,
			#[serde(default)]
			// TODO: Remove the `linear` marker when switching to the new document format and Ctrl-C node serialization format
			linear: bool,
		}
		let raw = MigrationColor::deserialize(deserializer)?;
		Ok(if raw.linear {
			Color {
				red: raw.red,
				green: raw.green,
				blue: raw.blue,
				alpha: raw.alpha,
			}
		} else {
			Color::from_gamma_srgb_channels(raw.red, raw.green, raw.blue, raw.alpha)
		})
	}
}

impl RGB for Color {
	type ColorChannel = f32;
	#[inline(always)]
	fn red(&self) -> f32 {
		self.red
	}
	#[inline(always)]
	fn green(&self) -> f32 {
		self.green
	}
	#[inline(always)]
	fn blue(&self) -> f32 {
		self.blue
	}
}
impl RGBMut for Color {
	fn set_red(&mut self, red: Self::ColorChannel) {
		self.red = red;
	}
	fn set_green(&mut self, green: Self::ColorChannel) {
		self.green = green;
	}
	fn set_blue(&mut self, blue: Self::ColorChannel) {
		self.blue = blue;
	}
}
impl AlphaMut for Color {
	fn set_alpha(&mut self, value: Self::AlphaChannel) {
		self.alpha = value;
	}
}

impl Pixel for Color {
	#[cfg(feature = "std")]
	fn to_bytes(&self) -> Vec<u8> {
		let SRGBA8 { red, green, blue, alpha } = (*self).into();
		[red, green, blue, alpha].to_vec()
	}

	fn from_bytes(bytes: &[u8]) -> Self {
		// `Image<Color>` pixel convention is linear-light with associated (premultiplied) alpha.
		let srgba = SRGBA8::new(bytes[0], bytes[1], bytes[2], bytes[3]);
		Color::from(srgba).apply_opacity(bytes[3] as f32 / 255.)
	}
	fn byte_size() -> usize {
		4
	}
}

impl Alpha for Color {
	type AlphaChannel = f32;
	const TRANSPARENT: Self = Self::TRANSPARENT;

	#[inline(always)]
	fn alpha(&self) -> f32 {
		self.alpha
	}
	#[inline(always)]
	fn multiplied_alpha(&self, alpha: Self::AlphaChannel) -> Self {
		Self {
			red: self.red * alpha,
			green: self.green * alpha,
			blue: self.blue * alpha,
			alpha: self.alpha * alpha,
		}
	}
}

impl AssociatedAlpha for Color {
	fn to_unassociated<Out: super::UnassociatedAlpha>(&self) -> Out {
		todo!()
	}
}

impl Luminance for Color {
	type LuminanceChannel = f32;
	#[inline(always)]
	fn luminance(&self) -> f32 {
		0.2126 * self.red + 0.7152 * self.green + 0.0722 * self.blue
	}
}

impl LuminanceMut for Color {
	fn set_luminance(&mut self, luminance: f32) {
		let current = self.luminance();
		// When we have a black-ish color, we just set the color to a grey-scale value. This prohibits a divide-by-0.
		if current < f32::EPSILON {
			self.red = 0.2126 * luminance;
			self.green = 0.7152 * luminance;
			self.blue = 0.0722 * luminance;
			return;
		}
		let fac = luminance / current;
		// TODO: when we have for example the rgb color (0, 0, 1) and want to
		// TODO: do `.set_luminance(1)`, then the actual luminance is not 1 at
		// TODO: the end. With no clamp, the resulting color would be
		// TODO: (0, 0, 12.8504). The excess should be spread to the other
		// TODO: channels, but is currently just clamped away.
		self.red = (self.red * fac).clamp(0., 1.);
		self.green = (self.green * fac).clamp(0., 1.);
		self.blue = (self.blue * fac).clamp(0., 1.);
	}
}

impl Rec709Primaries for Color {}
impl SRGB for Color {}

impl Color {
	pub const BLACK: Color = Color::from_rgbf32_unchecked(0., 0., 0.);
	pub const WHITE: Color = Color::from_rgbf32_unchecked(1., 1., 1.);
	pub const RED: Color = Color::from_rgbf32_unchecked(1., 0., 0.);
	pub const GREEN: Color = Color::from_rgbf32_unchecked(0., 1., 0.);
	pub const BLUE: Color = Color::from_rgbf32_unchecked(0., 0., 1.);
	pub const YELLOW: Color = Color::from_rgbf32_unchecked(1., 1., 0.);
	pub const CYAN: Color = Color::from_rgbf32_unchecked(0., 1., 1.);
	pub const MAGENTA: Color = Color::from_rgbf32_unchecked(1., 0., 1.);
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
	/// use core_types::color::Color;
	/// let color = Color::from_rgbaf32(0.3, 0.14, 0.15, 0.92).unwrap();
	/// assert!(color.components() == (0.3, 0.14, 0.15, 0.92));
	///
	/// let color = Color::from_rgbaf32(1., 1., 1., f32::NAN);
	/// assert!(color == None);
	/// ```
	#[inline(always)]
	pub fn from_rgbaf32(red: f32, green: f32, blue: f32, alpha: f32) -> Option<Color> {
		if alpha > 1. || [red, green, blue, alpha].iter().any(|c| c.is_sign_negative() || !c.is_finite()) {
			return None;
		}
		let color = Color { red, green, blue, alpha };
		Some(color)
	}

	/// Construct an opaque `Color` from `f32` RGB channels, with no value validation (use [`Self::from_rgbaf32`] for validation).
	#[inline(always)]
	pub const fn from_rgbf32_unchecked(red: f32, green: f32, blue: f32) -> Color {
		Color { red, green, blue, alpha: 1. }
	}

	/// Construct a `Color` from `f32` RGBA channels, with no value validation (use [`Self::from_rgbaf32`] for validation).
	#[inline(always)]
	pub const fn from_rgbaf32_unchecked(red: f32, green: f32, blue: f32, alpha: f32) -> Color {
		Color { red, green, blue, alpha }
	}

	/// Construct a `Color` from unassociated (straight) RGBA channels, premultiplying the RGB channels by alpha.
	#[inline(always)]
	pub fn new_from_unassociated_rgba(red: f32, green: f32, blue: f32, alpha: f32) -> Color {
		Color::from_rgbaf32_unchecked(red * alpha, green * alpha, blue * alpha, alpha)
	}

	/// Create a linear-light `Color` from HSL coordinates (all between 0 and 1).
	/// HSL is defined on sRGB display values, so the RGB produced by the HSL math is gamma-encoded and decoded to linear before being wrapped in `Color`.
	///
	/// # Examples
	/// ```
	/// use core_types::color::Color;
	/// let color = Color::from_hsla(0.5, 0.2, 0.3, 1.);
	/// ```
	pub fn from_hsla(hue: f32, saturation: f32, lightness: f32, alpha: f32) -> Color {
		let temp1 = if lightness < 0.5 {
			lightness * (saturation + 1.)
		} else {
			lightness + saturation - lightness * saturation
		};
		let temp2 = 2. * lightness - temp1;
		#[cfg(feature = "std")]
		let rem = |x: f32| x.rem_euclid(1.);
		#[cfg(not(feature = "std"))]
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

		Color::from_gamma_srgb_channels(red, green, blue, alpha)
	}

	/// Create a linear-light `Color` from HSV coordinates (all between 0 and 1).
	/// HSV is defined on sRGB display values, so the RGB produced by the HSV math is gamma-encoded and decoded to linear before being wrapped in `Color`.
	pub fn from_hsva(hue: f32, saturation: f32, value: f32, alpha: f32) -> Color {
		let h_prime = (hue * 6.) % 6.;
		let i = h_prime as i32;
		let f = h_prime - i as f32;
		let p = value * (1. - saturation);
		let q = value * (1. - f * saturation);
		let t = value * (1. - (1. - f) * saturation);
		let (red, green, blue) = match i % 6 {
			0 => (value, t, p),
			1 => (q, value, p),
			2 => (p, value, t),
			3 => (p, q, value),
			4 => (t, p, value),
			_ => (value, p, q),
		};
		Color::from_gamma_srgb_channels(red, green, blue, alpha)
	}

	/// Return the `red` component.
	///
	/// # Examples
	/// ```
	/// use core_types::color::Color;
	/// let color = Color::from_rgbaf32(0.114, 0.103, 0.98, 0.97).unwrap();
	/// assert!(color.r() == 0.114);
	/// ```
	#[inline(always)]
	pub fn r(&self) -> f32 {
		self.red
	}

	/// Return the `green` component.
	///
	/// # Examples
	/// ```
	/// use core_types::color::Color;
	/// let color = Color::from_rgbaf32(0.114, 0.103, 0.98, 0.97).unwrap();
	/// assert!(color.g() == 0.103);
	/// ```
	#[inline(always)]
	pub fn g(&self) -> f32 {
		self.green
	}

	/// Return the `blue` component.
	///
	/// # Examples
	/// ```
	/// use core_types::color::Color;
	/// let color = Color::from_rgbaf32(0.114, 0.103, 0.98, 0.97).unwrap();
	/// assert!(color.b() == 0.98);
	/// ```
	#[inline(always)]
	pub fn b(&self) -> f32 {
		self.blue
	}

	/// Return the `alpha` component without checking its expected `0.0` to `1.0` range.
	///
	/// # Examples
	/// ```
	/// use core_types::color::Color;
	/// let color = Color::from_rgbaf32(0.114, 0.103, 0.98, 0.97).unwrap();
	/// assert!(color.a() == 0.97);
	/// ```
	#[inline(always)]
	pub fn a(&self) -> f32 {
		self.alpha
	}

	/// Whether the alpha channel is at (or within an epsilon of) fully opaque.
	#[inline(always)]
	pub fn is_opaque(&self) -> bool {
		self.alpha > 1. - f32::EPSILON
	}

	/// Mean of the three RGB channels.
	#[inline(always)]
	pub fn average_rgb_channels(&self) -> f32 {
		(self.red + self.green + self.blue) / 3.
	}

	/// Minimum of the three RGB channels.
	#[inline(always)]
	pub fn minimum_rgb_channels(&self) -> f32 {
		self.red.min(self.green).min(self.blue)
	}

	/// Maximum of the three RGB channels.
	#[inline(always)]
	pub fn maximum_rgb_channels(&self) -> f32 {
		self.red.max(self.green).max(self.blue)
	}

	/// Relative luminance using Rec.709 / sRGB-primary weights, computed on linear-light RGB.
	// From https://stackoverflow.com/a/56678483/775283
	#[inline(always)]
	pub fn luminance_rec_709(&self) -> f32 {
		0.2126 * self.red + 0.7152 * self.green + 0.0722 * self.blue
	}

	/// Luma using Rec.601 SDTV coefficients.
	// From https://en.wikipedia.org/wiki/Luma_(video)#Rec._601_luma_versus_Rec._709_luma_coefficients
	#[inline(always)]
	pub fn luminance_rec_601(&self) -> f32 {
		0.299 * self.red + 0.587 * self.green + 0.114 * self.blue
	}

	/// Luma using rounded Rec.601 coefficients (`0.3 / 0.59 / 0.11`), as used by some legacy image processing.
	// From https://en.wikipedia.org/wiki/Luma_(video)#Rec._601_luma_versus_Rec._709_luma_coefficients
	#[inline(always)]
	pub fn luminance_rec_601_rounded(&self) -> f32 {
		0.3 * self.red + 0.59 * self.green + 0.11 * self.blue
	}

	/// Perceptual lightness (CIE L*) of the Rec.709 luminance, normalized to 0..1.
	// From https://stackoverflow.com/a/56678483/775283
	#[inline(always)]
	pub fn luminance_perceptual(&self) -> f32 {
		let luminance = self.luminance_rec_709();

		if luminance <= 0.008856 {
			(luminance * 903.3) / 100.
		} else {
			(luminance.cbrt() * 116. - 16.) / 100.
		}
	}

	/// Construct an opaque grayscale color where R = G = B = `luminance`.
	#[inline(always)]
	pub fn from_luminance(luminance: f32) -> Color {
		Color {
			red: luminance,
			green: luminance,
			blue: luminance,
			alpha: 1.,
		}
	}

	/// Shift all RGB channels by the offset that moves Rec.601-rounded luma to `luminance`, clamping channels to 0..1. Approximate; channels above 1 are lost.
	#[inline(always)]
	pub fn with_luminance(&self, luminance: f32) -> Color {
		let delta = luminance - self.luminance_rec_601_rounded();
		self.map_rgb(|c| (c + delta).clamp(0., 1.))
	}

	/// The RGB chroma range, `max - min` across the three channels. Not the HSL/HSV saturation (use [`Self::to_hsla`] or [`Self::to_hsva`] for those).
	#[inline(always)]
	pub fn chroma_range(&self) -> f32 {
		let max = (self.red).max(self.green).max(self.blue);
		let min = (self.red).min(self.green).min(self.blue);

		max - min
	}

	/// Replace HSL saturation with the given value, preserving hue, lightness, and alpha.
	#[inline(always)]
	pub fn with_saturation(&self, saturation: f32) -> Color {
		let [hue, _, lightness, alpha] = self.to_hsla();
		Color::from_hsla(hue, saturation, lightness, alpha)
	}

	/// Replace the alpha channel, leaving RGB unchanged.
	pub fn with_alpha(&self, alpha: f32) -> Color {
		Color {
			red: self.red,
			green: self.green,
			blue: self.blue,
			alpha,
		}
	}

	/// Replace the red channel, leaving the others unchanged.
	pub fn with_red(&self, red: f32) -> Color {
		Color {
			red,
			green: self.green,
			blue: self.blue,
			alpha: self.alpha,
		}
	}

	/// Replace the green channel, leaving the others unchanged.
	pub fn with_green(&self, green: f32) -> Color {
		Color {
			red: self.red,
			green,
			blue: self.blue,
			alpha: self.alpha,
		}
	}

	/// Replace the blue channel, leaving the others unchanged.
	pub fn with_blue(&self, blue: f32) -> Color {
		Color {
			red: self.red,
			green: self.green,
			blue,
			alpha: self.alpha,
		}
	}

	/// Per-channel "Normal" blend: returns the source channel unchanged.
	#[inline(always)]
	pub fn blend_normal(_c_b: f32, c_s: f32) -> f32 {
		c_s
	}

	/// Per-channel "Multiply" blend.
	#[inline(always)]
	pub fn blend_multiply(c_b: f32, c_s: f32) -> f32 {
		c_s * c_b
	}

	/// Per-channel "Darken" blend: the smaller of the two.
	#[inline(always)]
	pub fn blend_darken(c_b: f32, c_s: f32) -> f32 {
		c_s.min(c_b)
	}

	/// Per-channel "Color Burn" blend.
	#[inline(always)]
	pub fn blend_color_burn(c_b: f32, c_s: f32) -> f32 {
		if c_b == 1. {
			1.
		} else if c_s == 0. {
			0.
		} else {
			1. - ((1. - c_b) / c_s).min(1.)
		}
	}

	/// Per-channel "Linear Burn" blend.
	#[inline(always)]
	pub fn blend_linear_burn(c_b: f32, c_s: f32) -> f32 {
		c_b + c_s - 1.
	}

	/// Whole-color "Darker Color" blend: keeps whichever color has the lower mean RGB.
	#[inline(always)]
	pub fn blend_darker_color(&self, other: Color) -> Color {
		if self.average_rgb_channels() <= other.average_rgb_channels() { *self } else { other }
	}

	/// Per-channel "Screen" blend.
	#[inline(always)]
	pub fn blend_screen(c_b: f32, c_s: f32) -> f32 {
		1. - (1. - c_s) * (1. - c_b)
	}

	/// Per-channel "Lighten" blend: the larger of the two.
	#[inline(always)]
	pub fn blend_lighten(c_b: f32, c_s: f32) -> f32 {
		c_s.max(c_b)
	}

	/// Per-channel "Color Dodge" blend.
	#[inline(always)]
	pub fn blend_color_dodge(c_b: f32, c_s: f32) -> f32 {
		if c_s == 1. { 1. } else { (c_b / (1. - c_s)).min(1.) }
	}

	/// Per-channel "Linear Dodge" (Add) blend.
	#[inline(always)]
	pub fn blend_linear_dodge(c_b: f32, c_s: f32) -> f32 {
		c_b + c_s
	}

	/// Whole-color "Lighter Color" blend: keeps whichever color has the higher mean RGB.
	#[inline(always)]
	pub fn blend_lighter_color(&self, other: Color) -> Color {
		if self.average_rgb_channels() >= other.average_rgb_channels() { *self } else { other }
	}

	/// Per-channel "Soft Light" blend.
	pub fn blend_softlight(c_b: f32, c_s: f32) -> f32 {
		if c_s <= 0.5 {
			c_b - (1. - 2. * c_s) * c_b * (1. - c_b)
		} else {
			let d = |x: f32| if x <= 0.25 { ((16. * x - 12.) * x + 4.) * x } else { x.sqrt() };
			c_b + (2. * c_s - 1.) * (d(c_b) - c_b)
		}
	}

	/// Per-channel "Hard Light" blend.
	pub fn blend_hardlight(c_b: f32, c_s: f32) -> f32 {
		if c_s <= 0.5 {
			Color::blend_multiply(2. * c_s, c_b)
		} else {
			Color::blend_screen(2. * c_s - 1., c_b)
		}
	}

	/// Per-channel "Vivid Light" blend.
	pub fn blend_vivid_light(c_b: f32, c_s: f32) -> f32 {
		if c_s <= 0.5 {
			Color::blend_color_burn(2. * c_s, c_b)
		} else {
			Color::blend_color_dodge(2. * c_s - 1., c_b)
		}
	}

	/// Per-channel "Linear Light" blend.
	pub fn blend_linear_light(c_b: f32, c_s: f32) -> f32 {
		if c_s <= 0.5 {
			Color::blend_linear_burn(2. * c_s, c_b)
		} else {
			Color::blend_linear_dodge(2. * c_s - 1., c_b)
		}
	}

	/// Per-channel "Pin Light" blend.
	pub fn blend_pin_light(c_b: f32, c_s: f32) -> f32 {
		if c_s <= 0.5 {
			Color::blend_darken(2. * c_s, c_b)
		} else {
			Color::blend_lighten(2. * c_s - 1., c_b)
		}
	}

	/// Per-channel "Hard Mix" blend: thresholds Linear Light at 0.5.
	pub fn blend_hard_mix(c_b: f32, c_s: f32) -> f32 {
		if Color::blend_linear_light(c_b, c_s) < 0.5 { 0. } else { 1. }
	}

	/// Per-channel "Difference" blend.
	pub fn blend_difference(c_b: f32, c_s: f32) -> f32 {
		(c_b - c_s).abs()
	}

	/// Per-channel "Exclusion" blend.
	pub fn blend_exclusion(c_b: f32, c_s: f32) -> f32 {
		c_b + c_s - 2. * c_b * c_s
	}

	/// Per-channel "Subtract" blend.
	pub fn blend_subtract(c_b: f32, c_s: f32) -> f32 {
		c_b - c_s
	}

	/// Per-channel "Divide" blend.
	pub fn blend_divide(c_b: f32, c_s: f32) -> f32 {
		if c_b == 0. { 1. } else { c_b / c_s }
	}

	/// Whole-color "Hue" blend: source hue with this color's saturation and Rec.601 luma.
	pub fn blend_hue(&self, c_s: Color) -> Color {
		let sat_b = self.chroma_range();
		let lum_b = self.luminance_rec_601();
		c_s.with_saturation(sat_b).with_luminance(lum_b)
	}

	/// Whole-color "Saturation" blend: this color's hue/luma with source saturation.
	pub fn blend_saturation(&self, c_s: Color) -> Color {
		let sat_s = c_s.chroma_range();
		let lum_b = self.luminance_rec_601();

		self.with_saturation(sat_s).with_luminance(lum_b)
	}

	/// Whole-color "Color" blend: source hue/saturation with this color's luma.
	pub fn blend_color(&self, c_s: Color) -> Color {
		let lum_b = self.luminance_rec_601();

		c_s.with_luminance(lum_b)
	}

	/// Whole-color "Luminosity" blend: this color's hue/saturation with source luma.
	pub fn blend_luminosity(&self, c_s: Color) -> Color {
		let lum_s = c_s.luminance_rec_601();

		self.with_luminance(lum_s)
	}

	/// Return the all components as a tuple, first component is red, followed by green, followed by blue, followed by alpha.
	///
	/// # Examples
	/// ```
	/// use core_types::color::Color;
	/// let color = Color::from_rgbaf32(0.114, 0.103, 0.98, 0.97).unwrap();
	/// assert_eq!(color.components(),  (0.114, 0.103, 0.98, 0.97));
	/// ```
	#[inline(always)]
	pub fn components(&self) -> (f32, f32, f32, f32) {
		(self.red, self.green, self.blue, self.alpha)
	}

	/// Convert this color to HSV coordinates (all between 0 and 1).
	/// HSV is defined on sRGB display values, so this color's linear RGB is gamma-encoded before the HSV math.
	pub fn to_hsva(&self) -> [f32; 4] {
		#[cfg(feature = "std")]
		let rem = |x: f32, m: f32| x.rem_euclid(m);
		#[cfg(not(feature = "std"))]
		let rem = |x: f32, m: f32| x.rem_euclid(&m);

		let [red, green, blue, alpha] = self.to_gamma_srgb_channels();
		let max = red.max(green).max(blue);
		let min = red.min(green).min(blue);
		let delta = max - min;

		let mut hue = if delta == 0. {
			0.
		} else if max == red {
			rem((green - blue) / delta, 6.)
		} else if max == green {
			(blue - red) / delta + 2.
		} else {
			(red - green) / delta + 4.
		};
		hue = rem(hue * 60. + 360., 360.) / 360.;

		let saturation = if max == 0. { 0. } else { delta / max };
		let value = max;

		[hue, saturation, value, alpha]
	}

	// https://www.niwa.nu/2013/05/math-behind-colorspace-conversions-rgb-hsl/
	/// Convert this color to HSL coordinates (all between 0 and 1).
	/// HSL is defined on sRGB display values, so this color's linear RGB is gamma-encoded before the HSL math.
	pub fn to_hsla(&self) -> [f32; 4] {
		let [red, green, blue, alpha] = self.to_gamma_srgb_channels();
		let min_channel = red.min(green).min(blue);
		let max_channel = red.max(green).max(blue);

		let lightness = (min_channel + max_channel) / 2.;
		let saturation = if min_channel == max_channel {
			0.
		} else if lightness <= 0.5 {
			(max_channel - min_channel) / (max_channel + min_channel)
		} else {
			(max_channel - min_channel) / (2. - max_channel - min_channel)
		};
		let hue = if red >= green && red >= blue {
			(green - blue) / (max_channel - min_channel)
		} else if green >= red && green >= blue {
			2. + (blue - red) / (max_channel - min_channel)
		} else {
			4. + (red - green) / (max_channel - min_channel)
		} / 6.;
		#[cfg(feature = "std")]
		let hue = hue.rem_euclid(1.);
		#[cfg(not(feature = "std"))]
		let hue = hue.rem_euclid(&1.);

		[hue, saturation, lightness, alpha]
	}

	/// Linearly interpolate each RGBA channel between `self` (`t = 0`) and `other` (`t = 1`); `t` must be in 0..=1.
	#[inline(always)]
	pub fn lerp(&self, other: &Color, t: f32) -> Self {
		assert!((0. ..=1.).contains(&t));
		Color::from_rgbaf32_unchecked(
			self.red + ((other.red - self.red) * t),
			self.green + ((other.green - self.green) * t),
			self.blue + ((other.blue - self.blue) * t),
			self.alpha + ((other.alpha - self.alpha) * t),
		)
	}

	/// Generic power curve `c.powf(1 / exponent)` applied per RGB channel. Distinct from the sRGB transfer curve (see [`Self::to_gamma_srgb_channels`]).
	/// The expected output must still be treated as linear-light.
	#[inline(always)]
	pub fn apply_gamma_exponent(&self, exponent: f32) -> Color {
		let exponent = exponent.max(0.0001);

		// From https://www.dfstudios.co.uk/articles/programming/image-programming-algorithms/image-processing-algorithms-part-6-gamma-correction/
		let inverse = 1. / exponent;
		self.map_rgb(|c: f32| c.powf(inverse))
	}

	/// Decompose into the four channel components after sRGB gamma encoding (linear → gamma). Alpha is unchanged.
	/// Use [`Self::from_gamma_srgb_channels`] to wrap these gamma-encoded channels back into a linear-light `Color`.
	#[inline(always)]
	pub fn to_gamma_srgb_channels(&self) -> [f32; 4] {
		[super::linear_to_srgb(self.red), super::linear_to_srgb(self.green), super::linear_to_srgb(self.blue), self.alpha]
	}

	/// Construct a `Color` from sRGB gamma-encoded channel components, decoding RGB to linear-light. Alpha is unchanged.
	#[inline(always)]
	pub fn from_gamma_srgb_channels(red: f32, green: f32, blue: f32, alpha: f32) -> Color {
		Color {
			red: super::srgb_to_linear(red),
			green: super::srgb_to_linear(green),
			blue: super::srgb_to_linear(blue),
			alpha,
		}
	}

	/// Apply `f` to each RGB channel after sRGB gamma encoding, returning a linear-light `Color`. Alpha is unchanged.
	/// Equivalent to unpacking via [`Self::to_gamma_srgb_channels`], mapping per channel, and rewrapping via [`Self::from_gamma_srgb_channels`].
	#[inline(always)]
	pub fn map_gamma_rgb<F: Fn(f32) -> f32>(&self, f: F) -> Color {
		let [r, g, b, a] = self.to_gamma_srgb_channels();
		Color::from_gamma_srgb_channels(f(r), f(g), f(b), a)
	}

	/// Apply `f` to each of the four RGBA channels independently.
	#[inline(always)]
	pub fn map_rgba<F: Fn(f32) -> f32>(&self, f: F) -> Self {
		Self::from_rgbaf32_unchecked(f(self.r()), f(self.g()), f(self.b()), f(self.a()))
	}

	/// Apply `f` to each of the three RGB channels; alpha is unchanged.
	#[inline(always)]
	pub fn map_rgb<F: Fn(f32) -> f32>(&self, f: F) -> Self {
		Self::from_rgbaf32_unchecked(f(self.r()), f(self.g()), f(self.b()), self.a())
	}

	/// Multiply all four channels (including alpha) by `opacity`, applying an additional premultiplication factor to this Color.
	#[inline(always)]
	pub fn apply_opacity(&self, opacity: f32) -> Self {
		Self::from_rgbaf32_unchecked(self.r() * opacity, self.g() * opacity, self.b() * opacity, self.a() * opacity)
	}

	/// Divide RGB by alpha to recover unassociated (straight-alpha) channels; no-op if alpha is zero.
	#[inline(always)]
	pub fn to_unassociated_alpha(&self) -> Self {
		if self.alpha == 0. {
			return *self;
		}
		let unmultiply = 1. / self.alpha;
		Self {
			red: self.red * unmultiply,
			green: self.green * unmultiply,
			blue: self.blue * unmultiply,
			alpha: self.alpha,
		}
	}

	/// Apply a per-channel blend function to this color (unmultiplied) and `other`, returning a color with `other`'s alpha; channels are clamped to 0..1.
	#[inline(always)]
	pub fn blend_rgb<F: Fn(f32, f32) -> f32>(&self, other: Color, f: F) -> Self {
		let background = self.to_unassociated_alpha();
		Color {
			red: f(background.red, other.red).clamp(0., 1.),
			green: f(background.green, other.green).clamp(0., 1.),
			blue: f(background.blue, other.blue).clamp(0., 1.),
			alpha: other.alpha,
		}
	}

	/// Porter-Duff "source over" composite of `other` over `self`. Both colors must use associated (premultiplied) alpha.
	#[inline(always)]
	pub fn alpha_blend(&self, other: Color) -> Self {
		let inv_alpha = 1. - other.alpha;
		Self {
			red: self.red * inv_alpha + other.red,
			green: self.green * inv_alpha + other.green,
			blue: self.blue * inv_alpha + other.blue,
			alpha: self.alpha * inv_alpha + other.alpha,
		}
	}

	/// Replace alpha with `self.alpha + other.alpha`, clamped to 0..1; RGB is unchanged.
	#[inline(always)]
	pub fn alpha_add(&self, other: Color) -> Self {
		Self {
			alpha: (self.alpha + other.alpha).clamp(0., 1.),
			..*self
		}
	}

	/// Replace alpha with `self.alpha - other.alpha`, clamped to 0..1; RGB is unchanged.
	#[inline(always)]
	pub fn alpha_subtract(&self, other: Color) -> Self {
		Self {
			alpha: (self.alpha - other.alpha).clamp(0., 1.),
			..*self
		}
	}

	/// Replace alpha with `self.alpha * other.alpha`, clamped to 0..1; RGB is unchanged.
	#[inline(always)]
	pub fn alpha_multiply(&self, other: Color) -> Self {
		Self {
			alpha: (self.alpha * other.alpha).clamp(0., 1.),
			..*self
		}
	}

	/// Construct from a `glam::Vec4` where `(x, y, z, w)` map to `(red, green, blue, alpha)`.
	#[inline(always)]
	pub const fn from_vec4(vec: Vec4) -> Self {
		Self {
			red: vec.x,
			green: vec.y,
			blue: vec.z,
			alpha: vec.w,
		}
	}

	/// Pack into a `glam::Vec4` as `(red, green, blue, alpha)`.
	#[inline(always)]
	pub fn to_vec4(&self) -> Vec4 {
		Vec4::new(self.red, self.green, self.blue, self.alpha)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
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
			let col: Color = SRGBA8::new(red, green, blue, 255).into();
			let [hue, saturation, lightness, alpha] = col.to_hsla();
			let result = Color::from_hsla(hue, saturation, lightness, alpha);
			assert!((col.r() - result.r()) < f32::EPSILON * 100.);
			assert!((col.g() - result.g()) < f32::EPSILON * 100.);
			assert!((col.b() - result.b()) < f32::EPSILON * 100.);
			assert!((col.a() - result.a()) < f32::EPSILON * 100.);
		}
	}
}
