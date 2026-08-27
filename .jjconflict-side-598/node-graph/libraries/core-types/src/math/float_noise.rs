use std::fmt::Write;

/// Recovers the intended number from floating point imprecision noise when that can be done reliably, e.g. 0.30000000000000004 -> 0.3.
/// Rounding to each significant digit count from 1 to 12, the first candidate within a relative 1e-13 of the original is accepted.
/// Actual high-precision values (like 0.3333333333333333) never pass the tolerance and are returned unchanged.
/// f64 only, as f32 lacks precision to reliably distinguish between intentional digits and noise.
pub fn round_away_float_noise(value: f64) -> f64 {
	if value == 0. || !value.is_finite() {
		return if value == 0. { 0. } else { value };
	}

	// Candidates come from decimal formatting rather than scaling by a power of ten, which is inexact enough to invent
	// noise of its own: it turns 1e300 into 9.999999999999999e299 and 999999.9999999 into 999999.9999999999.
	// One buffer serves every candidate, since the digit counts are tried in turn.
	let mut buffer = String::with_capacity(32);
	for significant_digits in 1..=12 {
		buffer.clear();
		let _ = write!(buffer, "{value:.*e}", significant_digits - 1);

		let Ok(rounded) = buffer.parse::<f64>() else { continue };
		if ((rounded - value) / value).abs() < 1e-13 {
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
	fn round_away_float_noise_keeps_extreme_magnitudes_exact() {
		assert_eq!(round_away_float_noise(1e300), 1e300);
		assert_eq!(round_away_float_noise(1.5e300), 1.5e300);
		assert_eq!(round_away_float_noise(1e-300), 1e-300);
		assert_eq!(round_away_float_noise(f64::MIN_POSITIVE), f64::MIN_POSITIVE);
	}
}
