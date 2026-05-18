//! Analytic per-channel sRGB transfer functions (gamma encoding/decoding).
//!
//! These work in `f32` at full precision. For round-trip-exact `u8` ⇄ `f32` conversion at the
//! display byte boundary, use the lookup tables in [`super::discrete_srgb`] instead.

#[cfg(not(feature = "std"))]
use num_traits::float::Float;

/// Decode an sRGB gamma-encoded channel value to linear-light.
#[inline(always)]
pub fn srgb_to_linear(channel: f32) -> f32 {
	if channel <= 0.04045 { channel / 12.92 } else { ((channel + 0.055) / 1.055).powf(2.4) }
}

/// Encode a linear-light channel value to sRGB gamma-encoded.
#[inline(always)]
pub fn linear_to_srgb(channel: f32) -> f32 {
	if channel <= 0.0031308 { channel * 12.92 } else { 1.055 * channel.powf(1. / 2.4) - 0.055 }
}
