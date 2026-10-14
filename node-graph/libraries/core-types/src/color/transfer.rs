//! Analytic per-channel sRGB transfer functions (gamma encoding/decoding).
//!
//! These work in `f32` at full precision. For round-trip-exact `u8` ⇄ `f32` conversion at the
//! display byte boundary, use the lookup tables in [`super::discrete_srgb`] instead.

/// Decode an sRGB gamma-encoded channel value to linear-light.
#[inline(always)]
pub fn srgb_to_linear(channel: f32) -> f32 {
	if channel <= 0.04045 { channel / 12.92 } else { ((channel + 0.055) / 1.055).powf(2.4) }
}

/// Encode a linear-light channel value to sRGB gamma-encoded.
#[inline(always)]
pub fn linear_to_srgb(channel: f32) -> f32 {
	// White is kept exact, where the curve's rounding would land a hair under 1
	if channel <= 0.0031308 {
		channel * 12.92
	} else if channel == 1. {
		1.
	} else {
		1.055 * channel.powf(1. / 2.4) - 0.055
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn black_and_white_keep_their_values_both_ways() {
		assert_eq!((linear_to_srgb(0.), linear_to_srgb(1.)), (0., 1.));
		assert_eq!((srgb_to_linear(0.), srgb_to_linear(1.)), (0., 1.));
	}

	#[test]
	fn encoding_follows_the_srgb_curve() {
		for (linear, encoded) in [(0.001, 0.01292), (0.0031308, 0.040449936), (0.18, 0.461356), (0.5, 0.735357), (2., 1.353256)] {
			assert!((linear_to_srgb(linear) - encoded).abs() < 1e-6, "{linear} encodes to {}", linear_to_srgb(linear));
			assert!((srgb_to_linear(encoded) - linear).abs() < 1e-6, "{encoded} decodes to {}", srgb_to_linear(encoded));
		}
	}
}
