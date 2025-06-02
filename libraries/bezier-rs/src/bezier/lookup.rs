use super::*;
use crate::utils::{TValue, TValueType};

/// Functionality relating to looking up properties of the `Bezier` or points along the `Bezier`.
impl Bezier {
	/// Convert a euclidean distance ratio along the `Bezier` curve to a parametric `t`-value.
	pub fn euclidean_to_parametric(&self, ratio: f64, error: f64) -> f64 {
		let total_length = self.length(None);
		self.euclidean_to_parametric_with_total_length(ratio, error, total_length)
	}

	/// Convert a euclidean distance ratio along the `Bezier` curve to a parametric `t`-value.
	/// For performance reasons, this version of the [`euclidean_to_parametric`] function allows the caller to
	/// provide the total length of the curve so it doesn't have to be calculated every time the function is called.
	pub fn euclidean_to_parametric_with_total_length(&self, euclidean_t: f64, error: f64, total_length: f64) -> f64 {
		if euclidean_t < error {
			return 0.;
		}
		if 1. - euclidean_t < error {
			return 1.;
		}

		match self.handles {
			BezierHandles::Linear => euclidean_t,
			BezierHandles::Quadratic { handle } => {
				// Use Casteljau subdivision, noting that the length is more than the straight line distance from start to end but less than the straight line distance through the handles
				fn recurse(a0: DVec2, a1: DVec2, a2: DVec2, level: u8, desired_len: f64) -> (f64, f64) {
					let lower = a0.distance(a2);
					let upper = a0.distance(a1) + a1.distance(a2);
					if level >= 8 {
						let approx_len = (lower + upper) / 2.;
						return (approx_len, desired_len / approx_len);
					}

					let b1 = 0.5 * (a0 + a1);
					let c1 = 0.5 * (a1 + a2);
					let b2 = 0.5 * (b1 + c1);
					let (first_len, t) = recurse(a0, b1, b2, level + 1, desired_len);
					if first_len > desired_len {
						return (first_len, t * 0.5);
					}
					let (second_len, t) = recurse(b2, c1, a2, level + 1, desired_len - first_len);
					(first_len + second_len, t * 0.5 + 0.5)
				}
				recurse(self.start, handle, self.end, 0, total_length * euclidean_t).1
			}
			BezierHandles::Cubic { handle_start, handle_end } => {
				// Use Casteljau subdivision, noting that the length is more than the straight line distance from start to end but less than the straight line distance through the handles
				fn recurse(a0: DVec2, a1: DVec2, a2: DVec2, a3: DVec2, level: u8, desired_len: f64) -> (f64, f64) {
					let lower = a0.distance(a3);
					let upper = a0.distance(a1) + a1.distance(a2) + a2.distance(a3);
					if level >= 8 {
						let approx_len = (lower + upper) / 2.;
						return (approx_len, desired_len / approx_len);
					}

					let b1 = 0.5 * (a0 + a1);
					let t0 = 0.5 * (a1 + a2);
					let c1 = 0.5 * (a2 + a3);
					let b2 = 0.5 * (b1 + t0);
					let c2 = 0.5 * (t0 + c1);
					let b3 = 0.5 * (b2 + c2);
					let (first_len, t) = recurse(a0, b1, b2, b3, level + 1, desired_len);
					if first_len > desired_len {
						return (first_len, t * 0.5);
					}
					let (second_len, t) = recurse(b3, c2, c1, a3, level + 1, desired_len - first_len);
					(first_len + second_len, t * 0.5 + 0.5)
				}
				recurse(self.start, handle_start, handle_end, self.end, 0, total_length * euclidean_t).1
			}
		}
		.clamp(0., 1.)
	}

	/// Convert a [TValue] to a parametric `t`-value.
	pub(crate) fn t_value_to_parametric(&self, t: TValue) -> f64 {
		match t {
			TValue::Parametric(t) => {
				assert!((0.0..=1.).contains(&t));
				t
			}
			TValue::Euclidean(t) => {
				assert!((0.0..=1.).contains(&t));
				self.euclidean_to_parametric(t, DEFAULT_EUCLIDEAN_ERROR_BOUND)
			}
			TValue::EuclideanWithinError { t, error } => {
				assert!((0.0..=1.).contains(&t));
				self.euclidean_to_parametric(t, error)
			}
		}
	}

	/// Calculate the point on the curve based on the `t`-value provided.
	pub(crate) fn unrestricted_parametric_evaluate(&self, t: f64) -> DVec2 {
		// Basis code based off of pseudocode found here: <https://pomax.github.io/bezierinfo/#explanation>.

		let t_squared = t * t;
		let one_minus_t = 1. - t;
		let squared_one_minus_t = one_minus_t * one_minus_t;

		match self.handles {
			BezierHandles::Linear => self.start.lerp(self.end, t),
			BezierHandles::Quadratic { handle } => squared_one_minus_t * self.start + 2. * one_minus_t * t * handle + t_squared * self.end,
			BezierHandles::Cubic { handle_start, handle_end } => {
				let t_cubed = t_squared * t;
				let cubed_one_minus_t = squared_one_minus_t * one_minus_t;
				cubed_one_minus_t * self.start + 3. * squared_one_minus_t * t * handle_start + 3. * one_minus_t * t_squared * handle_end + t_cubed * self.end
			}
		}
	}

	/// Calculate the coordinates of the point `t` along the curve.
	/// Expects `t` to be within the inclusive range `[0, 1]`.
	/// <iframe frameBorder="0" width="100%" height="350px" src="https://graphite.rs/libraries/bezier-rs#bezier/evaluate/solo" title="Evaluate Demo"></iframe>
	pub fn evaluate(&self, t: TValue) -> DVec2 {
		let t = self.t_value_to_parametric(t);
		self.unrestricted_parametric_evaluate(t)
	}

	/// Return a selection of equidistant points on the bezier curve.
	/// If no value is provided for `steps`, then the function will default `steps` to be 10.
	/// <iframe frameBorder="0" width="100%" height="350px" src="https://graphite.rs/libraries/bezier-rs#bezier/lookup-table/solo" title="Lookup-Table Demo"></iframe>
	pub fn compute_lookup_table(&self, steps: Option<usize>, tvalue_type: Option<TValueType>) -> impl Iterator<Item = DVec2> + '_ {
		let steps = steps.unwrap_or(DEFAULT_LUT_STEP_SIZE);
		let tvalue_type = tvalue_type.unwrap_or(TValueType::Parametric);

		(0..=steps).map(move |t| {
			let tvalue = match tvalue_type {
				TValueType::Parametric => TValue::Parametric(t as f64 / steps as f64),
				TValueType::Euclidean => TValue::Euclidean(t as f64 / steps as f64),
			};
			self.evaluate(tvalue)
		})
	}

	/// Return an approximation of the length of the bezier curve.
	/// - `tolerance` - Tolerance used to approximate the curve.
	/// <iframe frameBorder="0" width="100%" height="300px" src="https://graphite.rs/libraries/bezier-rs#bezier/length/solo" title="Length Demo"></iframe>
	pub fn length(&self, tolerance: Option<f64>) -> f64 {
		match self.handles {
			BezierHandles::Linear => (self.start - self.end).length(),
			BezierHandles::Quadratic { handle } => {
				// Use Casteljau subdivision, noting that the length is more than the straight line distance from start to end but less than the straight line distance through the handles
				fn recurse(a0: DVec2, a1: DVec2, a2: DVec2, tolerance: f64, level: u8) -> f64 {
					let lower = a0.distance(a2);
					let upper = a0.distance(a1) + a1.distance(a2);
					if upper - lower <= 2. * tolerance || level >= 8 {
						return (lower + upper) / 2.;
					}

					let b1 = 0.5 * (a0 + a1);
					let c1 = 0.5 * (a1 + a2);
					let b2 = 0.5 * (b1 + c1);
					recurse(a0, b1, b2, 0.5 * tolerance, level + 1) + recurse(b2, c1, a2, 0.5 * tolerance, level + 1)
				}
				recurse(self.start, handle, self.end, tolerance.unwrap_or_default(), 0)
			}
			BezierHandles::Cubic { handle_start, handle_end } => {
				// Use Casteljau subdivision, noting that the length is more than the straight line distance from start to end but less than the straight line distance through the handles
				fn recurse(a0: DVec2, a1: DVec2, a2: DVec2, a3: DVec2, tolerance: f64, level: u8) -> f64 {
					let lower = a0.distance(a3);
					let upper = a0.distance(a1) + a1.distance(a2) + a2.distance(a3);
					if upper - lower <= 2. * tolerance || level >= 8 {
						return (lower + upper) / 2.;
					}

					let b1 = 0.5 * (a0 + a1);
					let t0 = 0.5 * (a1 + a2);
					let c1 = 0.5 * (a2 + a3);
					let b2 = 0.5 * (b1 + t0);
					let c2 = 0.5 * (t0 + c1);
					let b3 = 0.5 * (b2 + c2);
					recurse(a0, b1, b2, b3, 0.5 * tolerance, level + 1) + recurse(b3, c2, c1, a3, 0.5 * tolerance, level + 1)
				}
				recurse(self.start, handle_start, handle_end, self.end, tolerance.unwrap_or_default(), 0)
			}
		}
	}

	/// Return an approximation of the length centroid, together with the length, of the bezier curve.
	///
	/// The length centroid is the center of mass for the arc length of the Bezier segment.
	/// An infinitely thin wire forming the Bezier segment's shape would balance at this point.
	///
	/// - `tolerance` - Tolerance used to approximate the curve.
	pub fn length_centroid_and_length(&self, tolerance: Option<f64>) -> (DVec2, f64) {
		match self.handles {
			BezierHandles::Linear => ((self.start + self.end()) / 2., (self.start - self.end).length()),
			BezierHandles::Quadratic { handle } => {
				// Use Casteljau subdivision, noting that the length is more than the straight line distance from start to end but less than the straight line distance through the handles
				fn recurse(a0: DVec2, a1: DVec2, a2: DVec2, tolerance: f64, level: u8) -> (f64, DVec2) {
					let lower = a0.distance(a2);
					let upper = a0.distance(a1) + a1.distance(a2);
					if upper - lower <= 2. * tolerance || level >= 8 {
						let length = (lower + upper) / 2.;
						return (length, length * (a0 + a1 + a2) / 3.);
					}

					let b1 = 0.5 * (a0 + a1);
					let c1 = 0.5 * (a1 + a2);
					let b2 = 0.5 * (b1 + c1);

					let (length1, centroid_part1) = recurse(a0, b1, b2, 0.5 * tolerance, level + 1);
					let (length2, centroid_part2) = recurse(b2, c1, a2, 0.5 * tolerance, level + 1);
					(length1 + length2, centroid_part1 + centroid_part2)
				}

				let (length, centroid_parts) = recurse(self.start, handle, self.end, tolerance.unwrap_or_default(), 0);
				(centroid_parts / length, length)
			}
			BezierHandles::Cubic { handle_start, handle_end } => {
				// Use Casteljau subdivision, noting that the length is more than the straight line distance from start to end but less than the straight line distance through the handles
				fn recurse(a0: DVec2, a1: DVec2, a2: DVec2, a3: DVec2, tolerance: f64, level: u8) -> (f64, DVec2) {
					let lower = a0.distance(a3);
					let upper = a0.distance(a1) + a1.distance(a2) + a2.distance(a3);
					if upper - lower <= 2. * tolerance || level >= 8 {
						let length = (lower + upper) / 2.;
						return (length, length * (a0 + a1 + a2 + a3) / 4.);
					}

					let b1 = 0.5 * (a0 + a1);
					let t0 = 0.5 * (a1 + a2);
					let c1 = 0.5 * (a2 + a3);
					let b2 = 0.5 * (b1 + t0);
					let c2 = 0.5 * (t0 + c1);
					let b3 = 0.5 * (b2 + c2);

					let (length1, centroid_part1) = recurse(a0, b1, b2, b3, 0.5 * tolerance, level + 1);
					let (length2, centroid_part2) = recurse(b3, c2, c1, a3, 0.5 * tolerance, level + 1);
					(length1 + length2, centroid_part1 + centroid_part2)
				}
				let (length, centroid_parts) = recurse(self.start, handle_start, handle_end, self.end, tolerance.unwrap_or_default(), 0);
				(centroid_parts / length, length)
			}
		}
	}

	/// Return an approximation of the length centroid of the Bezier curve.
	///
	/// The length centroid is the center of mass for the arc length of the Bezier segment.
	/// An infinitely thin wire with the Bezier segment's shape would balance at this point.
	///
	/// - `tolerance` - Tolerance used to approximate the curve.
	/// <iframe frameBorder="0" width="100%" height="300px" src="https://graphite.rs/libraries/bezier-rs#bezier/length-centroid/solo" title="Length Centroid Demo"></iframe>
	pub fn length_centroid(&self, tolerance: Option<f64>) -> DVec2 {
		self.length_centroid_and_length(tolerance).0
	}

	/// Returns the parametric `t`-value that corresponds to the closest point on the curve to the provided point.
	/// <iframe frameBorder="0" width="100%" height="300px" src="https://graphite.rs/libraries/bezier-rs#bezier/project/solo" title="Project Demo"></iframe>
	pub fn project(&self, point: DVec2) -> f64 {
		let sbasis = crate::symmetrical_basis::to_symmetrical_basis_pair(*self);
		let derivative = sbasis.derivative();
		let dd = (sbasis - point).dot(&derivative);
		let roots = dd.roots();

		let mut closest = 0.;
		let mut min_dist_squared = self.evaluate(TValue::Parametric(0.)).distance_squared(point);

		for time in roots {
			let distance = self.evaluate(TValue::Parametric(time)).distance_squared(point);
			if distance < min_dist_squared {
				closest = time;
				min_dist_squared = distance;
			}
		}

		if self.evaluate(TValue::Parametric(1.)).distance_squared(point) < min_dist_squared {
			closest = 1.;
		}
		closest
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_evaluate() {
		let p1 = DVec2::new(3., 5.);
		let p2 = DVec2::new(14., 3.);
		let p3 = DVec2::new(19., 14.);
		let p4 = DVec2::new(30., 21.);

		let bezier1 = Bezier::from_quadratic_dvec2(p1, p2, p3);
		assert_eq!(bezier1.evaluate(TValue::Parametric(0.5)), DVec2::new(12.5, 6.25));

		let bezier2 = Bezier::from_cubic_dvec2(p1, p2, p3, p4);
		assert_eq!(bezier2.evaluate(TValue::Parametric(0.5)), DVec2::new(16.5, 9.625));
	}

	#[test]
	fn test_compute_lookup_table() {
		let bezier1 = Bezier::from_quadratic_coordinates(10., 10., 30., 30., 50., 10.);
		let lookup_table1 = bezier1.compute_lookup_table(Some(2), Some(TValueType::Parametric)).collect::<Vec<_>>();
		assert_eq!(lookup_table1, vec![bezier1.start(), bezier1.evaluate(TValue::Parametric(0.5)), bezier1.end()]);

		let bezier2 = Bezier::from_cubic_coordinates(10., 10., 30., 30., 70., 70., 90., 10.);
		let lookup_table2 = bezier2.compute_lookup_table(Some(4), Some(TValueType::Parametric)).collect::<Vec<_>>();
		assert_eq!(
			lookup_table2,
			vec![
				bezier2.start(),
				bezier2.evaluate(TValue::Parametric(0.25)),
				bezier2.evaluate(TValue::Parametric(0.50)),
				bezier2.evaluate(TValue::Parametric(0.75)),
				bezier2.end()
			]
		);
	}

	#[test]
	fn test_length() {
		let p1 = DVec2::new(30., 50.);
		let p2 = DVec2::new(140., 30.);
		let p3 = DVec2::new(160., 170.);
		let p4 = DVec2::new(77., 129.);

		let bezier_linear = Bezier::from_linear_dvec2(p1, p2);
		assert!(utils::f64_compare(bezier_linear.length(None), p1.distance(p2), MAX_ABSOLUTE_DIFFERENCE));

		let bezier_quadratic = Bezier::from_quadratic_dvec2(p1, p2, p3);
		assert!(utils::f64_compare(bezier_quadratic.length(None), 204., 1e-2));

		let bezier_cubic = Bezier::from_cubic_dvec2(p1, p2, p3, p4);
		assert!(utils::f64_compare(bezier_cubic.length(None), 199., 1e-2));
	}

	#[test]
	fn test_length_centroid() {
		let p1 = DVec2::new(30., 50.);
		let p2 = DVec2::new(140., 30.);
		let p3 = DVec2::new(160., 170.);
		let p4 = DVec2::new(77., 129.);

		let bezier_linear = Bezier::from_linear_dvec2(p1, p2);
		assert!(bezier_linear.length_centroid_and_length(None).0.abs_diff_eq((p1 + p2) / 2., MAX_ABSOLUTE_DIFFERENCE));

		let bezier_quadratic = Bezier::from_quadratic_dvec2(p1, p2, p3);
		let expected = DVec2::new(112.81017736920136, 87.98713052477228);
		assert!(bezier_quadratic.length_centroid_and_length(None).0.abs_diff_eq(expected, MAX_ABSOLUTE_DIFFERENCE));

		let bezier_cubic = Bezier::from_cubic_dvec2(p1, p2, p3, p4);
		let expected = DVec2::new(95.23597072432115, 88.0645175770206);
		assert!(bezier_cubic.length_centroid_and_length(None).0.abs_diff_eq(expected, MAX_ABSOLUTE_DIFFERENCE));
	}

	#[test]
	fn test_project() {
		let bezier1 = Bezier::from_cubic_coordinates(4., 4., 23., 45., 10., 30., 56., 90.);
		assert_eq!(bezier1.project(DVec2::ZERO), 0.);
		assert_eq!(bezier1.project(DVec2::new(100., 100.)), 1.);

		let bezier2 = Bezier::from_quadratic_coordinates(0., 0., 0., 100., 100., 100.);
		assert_eq!(bezier2.project(DVec2::new(100., 0.)), 0.);

		let bezier3 = Bezier::from_cubic_coordinates(-50., -50., -50., -50., 50., -50., 50., -50.);
		assert_eq!(DVec2::new(0., -50.), bezier3.evaluate(TValue::Parametric(bezier3.project(DVec2::new(0., -50.)))));
	}
}
