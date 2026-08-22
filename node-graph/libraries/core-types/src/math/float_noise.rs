/// Recovers the intended number from floating point imprecision noise when that can be done reliably, e.g. 0.30000000000000004 -> 0.3.
/// Rounding to each significant digit count from 1 to 12, the first candidate within a relative 1e-13 of the original is accepted.
/// Actual high-precision values (like 0.3333333333333333) never pass the tolerance and are returned unchanged.
pub fn round_away_float_noise(value: f64) -> f64 {
	snap_to_shortest_decimal(value, 12, 1e-13)
}

/// The [`round_away_float_noise`] counterpart for f32, whose roughly 7 significant digits of precision leave noise
/// that survives shortest round-trip formatting, e.g. 1.1 + 2.2 printing as 3.3000002.
pub fn round_away_float_noise_f32(value: f32) -> f32 {
	snap_to_shortest_decimal(value as f64, 6, 1e-6) as f32
}

/// Rounding to each significant digit count up to `max_significant_digits`, returns the first candidate within
/// `tolerance` of the original, relatively, or the original if none is close enough to be an honest simplification.
fn snap_to_shortest_decimal(value: f64, max_significant_digits: i32, tolerance: f64) -> f64 {
	if value == 0. || !value.is_finite() {
		return if value == 0. { 0. } else { value };
	}

	let exponent = value.abs().log10().floor() as i32;
	for significant_digits in 1..=max_significant_digits {
		let scale = 10_f64.powi(significant_digits - 1 - exponent);
		let rounded = (value * scale).round() / scale;
		if ((rounded - value) / value).abs() < tolerance {
			return rounded;
		}
	}

	value
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn round_away_float_noise_snaps_noisy_values() {
		assert_eq!(round_away_float_noise(0.1 + 0.2), 0.3);
		assert_eq!(round_away_float_noise(0.3000000000000012), 0.3);
		assert_eq!(round_away_float_noise(2.99999999999993), 3.);
		assert_eq!(round_away_float_noise(45.00000000000001), 45.);
	}

	#[test]
	fn round_away_float_noise_keeps_honest_values() {
		assert_eq!(round_away_float_noise(1. / 3.), 1. / 3.);
		assert_eq!(round_away_float_noise(0.2394023940209349), 0.2394023940209349);
		assert_eq!(round_away_float_noise(0.25), 0.25);
		assert_eq!(round_away_float_noise(-17.5), -17.5);
	}

	#[test]
	fn round_away_float_noise_keeps_deliberate_values_with_zero_runs() {
		assert_eq!(round_away_float_noise(0.30000005), 0.30000005);
		assert_eq!(round_away_float_noise(0.3000000000001), 0.3000000000001);
		assert_eq!(round_away_float_noise(1.00000001), 1.00000001);
		assert_eq!(round_away_float_noise(2.9999993), 2.9999993);
	}

	#[test]
	fn round_away_float_noise_normalizes_zero() {
		let result = round_away_float_noise(-0.);
		assert_eq!(result, 0.);
		assert!(result.is_sign_positive());
	}

	#[test]
	fn round_away_float_noise_f32_snaps_noisy_values() {
		assert_eq!(round_away_float_noise_f32(1.1 + 2.2), 3.3);
		assert_eq!(round_away_float_noise_f32((0..10).fold(0., |sum, _| sum + 0.1_f32)), 1.);
		assert_eq!(round_away_float_noise_f32(0.1 + 0.2), 0.3);
	}

	#[test]
	fn round_away_float_noise_f32_keeps_honest_values() {
		assert_eq!(round_away_float_noise_f32(0.1234567), 0.1234567);
		assert_eq!(round_away_float_noise_f32(1. / 3.), 1. / 3.);
		assert_eq!(round_away_float_noise_f32(-17.5), -17.5);
	}
}
