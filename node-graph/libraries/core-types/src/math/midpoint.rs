/// The effective midpoint domain: NaN reads as the linear default, and extremes are bounded to `0.01..=0.99` so curves stay finite and cheap to subdivide.
pub fn sanitized_midpoint(midpoint: f64) -> f64 {
	if midpoint.is_nan() { 0.5 } else { midpoint.clamp(0.01, 0.99) }
}

/// Apply the midpoint curve to a normalized parameter `t` (0 to 1) given a `midpoint` (0 to 1, where 0.5 is linear).
pub fn apply_midpoint(t: f64, midpoint: f64) -> f64 {
	let midpoint = sanitized_midpoint(midpoint);
	if (midpoint - 0.5).abs() < 1e-6 {
		return t;
	}

	if midpoint < 0.5 {
		let q = -1. / (1. - midpoint).log2();
		1. - (1. - t).powf(q)
	} else {
		let p = -1. / midpoint.log2();
		t.powf(p)
	}
}

/// The inverse of [`apply_midpoint`], which lands a halfway parameter (0.5) at the midpoint.
/// Applied to uniformly distributed values, this makes the midpoint their median.
pub fn apply_midpoint_inverse(t: f64, midpoint: f64) -> f64 {
	let midpoint = sanitized_midpoint(midpoint);
	if (midpoint - 0.5).abs() < 1e-6 {
		return t;
	}

	if midpoint < 0.5 { 1. - (1. - t).powf(-(1. - midpoint).log2()) } else { t.powf(-midpoint.log2()) }
}

#[cfg(test)]
mod tests {
	use super::*;

	const MIDPOINTS: [f64; 7] = [0.01, 0.1, 0.3, 0.5, 0.7, 0.9, 0.99];

	#[test]
	fn inverse_undoes_the_curve() {
		for midpoint in MIDPOINTS {
			for step in 0..=20 {
				let t = step as f64 / 20.;
				let round_trip = apply_midpoint(apply_midpoint_inverse(t, midpoint), midpoint);
				assert!((round_trip - t).abs() < 1e-9, "midpoint {midpoint} took {t} to {round_trip}");
			}
		}
	}

	#[test]
	fn inverse_lands_halfway_at_the_midpoint() {
		for midpoint in MIDPOINTS {
			assert!((apply_midpoint_inverse(0.5, midpoint) - midpoint).abs() < 1e-9);
			assert!((apply_midpoint(midpoint, midpoint) - 0.5).abs() < 1e-9);
		}
	}

	#[test]
	fn opposite_midpoints_mirror_each_other() {
		for midpoint in MIDPOINTS {
			for step in 0..=20 {
				let t = step as f64 / 20.;
				let mirrored = 1. - apply_midpoint_inverse(1. - t, 1. - midpoint);
				assert!((apply_midpoint_inverse(t, midpoint) - mirrored).abs() < 1e-9);
			}
		}
	}

	#[test]
	fn curves_keep_their_endpoints() {
		for midpoint in [0., 0.25, 0.5, 0.75, 1., f64::NAN] {
			for curve in [apply_midpoint, apply_midpoint_inverse] {
				assert_eq!(curve(0., midpoint), 0.);
				assert_eq!(curve(1., midpoint), 1.);
			}
		}
	}
}
