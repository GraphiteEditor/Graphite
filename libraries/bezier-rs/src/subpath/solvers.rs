use super::*;

use glam::DVec2;

impl Subpath {
	/// Calculate the point on the subpath based on the parametric `t`-value provided.
	/// Expects `t` to be within the inclusive range `[0, 1]`.
	pub fn parametric_evaluate(&self, t: f64) -> DVec2 {
		assert!((0.0..=1.).contains(&t));

		let mut number_of_curves = self.len() as f64;
		if !self.closed {
			number_of_curves -= 1.;
		}

		let scaled_t = t * number_of_curves;

		let target_curve_index = scaled_t.floor() as i32;
		let target_curve_t = scaled_t % 1.;

		if let Some(curve) = self.iter().nth(target_curve_index as usize) {
			curve.evaluate(target_curve_t)
		} else {
			self.iter().last().unwrap().evaluate(1.)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::Bezier;
	use glam::DVec2;

	use crate::consts::MAX_ABSOLUTE_DIFFERENCE;
	use crate::utils;

	fn normalize_t(n: i64, t: f64) -> f64 {
		t * (n as f64) % 1.
	}

	#[test]
	fn evaluate_one_subpath_curve() {
		let start = DVec2::new(20., 30.);
		let end = DVec2::new(60., 45.);
		let handle = DVec2::new(75., 85.);

		let bezier = Bezier::from_quadratic_dvec2(start, handle, end);
		let subpath = Subpath::new(
			vec![
				ManipulatorGroup {
					anchor: start,
					in_handle: None,
					out_handle: Some(handle),
				},
				ManipulatorGroup {
					anchor: end,
					in_handle: None,
					out_handle: Some(handle),
				},
			],
			false,
		);

		let t0 = 0.;
		assert_eq!(subpath.parametric_evaluate(t0), bezier.evaluate(t0));

		let t1 = 0.25;
		assert_eq!(subpath.parametric_evaluate(t1), bezier.evaluate(t1));

		let t2 = 0.50;
		assert_eq!(subpath.parametric_evaluate(t2), bezier.evaluate(t2));

		let t3 = 1.;
		assert_eq!(subpath.parametric_evaluate(t3), bezier.evaluate(t3));
	}

	#[test]
	fn evaluate_multiple_subpath_curves() {
		let start = DVec2::new(20., 30.);
		let middle = DVec2::new(70., 70.);
		let end = DVec2::new(60., 45.);
		let handle1 = DVec2::new(75., 85.);
		let handle2 = DVec2::new(40., 30.);
		let handle3 = DVec2::new(10., 10.);

		let linear_bezier = Bezier::from_linear_dvec2(start, middle);
		let quadratic_bezier = Bezier::from_quadratic_dvec2(middle, handle1, end);
		let cubic_bezier = Bezier::from_cubic_dvec2(end, handle2, handle3, start);

		let mut subpath = Subpath::new(
			vec![
				ManipulatorGroup {
					anchor: start,
					in_handle: Some(handle3),
					out_handle: None,
				},
				ManipulatorGroup {
					anchor: middle,
					in_handle: None,
					out_handle: Some(handle1),
				},
				ManipulatorGroup {
					anchor: end,
					in_handle: None,
					out_handle: Some(handle2),
				},
			],
			false,
		);

		// Test open subpath

		let mut n = (subpath.len() as i64) - 1;

		let t0 = 0.;
		assert!(utils::dvec2_compare(subpath.parametric_evaluate(t0), linear_bezier.evaluate(normalize_t(n, t0)), MAX_ABSOLUTE_DIFFERENCE).all());

		let t1 = 0.25;
		assert!(utils::dvec2_compare(subpath.parametric_evaluate(t1), linear_bezier.evaluate(normalize_t(n, t1)), MAX_ABSOLUTE_DIFFERENCE).all());

		let t2 = 0.50;
		assert!(utils::dvec2_compare(subpath.parametric_evaluate(t2), quadratic_bezier.evaluate(normalize_t(n, t2)), MAX_ABSOLUTE_DIFFERENCE).all());

		let t3 = 0.75;
		assert!(utils::dvec2_compare(subpath.parametric_evaluate(t3), quadratic_bezier.evaluate(normalize_t(n, t3)), MAX_ABSOLUTE_DIFFERENCE).all());

		let t4 = 1.;
		assert!(utils::dvec2_compare(subpath.parametric_evaluate(t4), quadratic_bezier.evaluate(1.), MAX_ABSOLUTE_DIFFERENCE).all());

		// Test closed subpath

		subpath.closed = true;
		n = subpath.len() as i64;

		let t5 = 2. / 3.;
		assert!(utils::dvec2_compare(subpath.parametric_evaluate(t5), cubic_bezier.evaluate(normalize_t(n, t5)), MAX_ABSOLUTE_DIFFERENCE).all());

		let t6 = 1.;
		assert!(utils::dvec2_compare(subpath.parametric_evaluate(t6), cubic_bezier.evaluate(1.), MAX_ABSOLUTE_DIFFERENCE).all());
	}
}
