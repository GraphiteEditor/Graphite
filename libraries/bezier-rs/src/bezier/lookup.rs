use crate::utils::{f64_compare, TValue, TValueType};

use super::*;

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

		let mut low = 0.;
		let mut mid = 0.5;
		let mut high = 1.;

		// The euclidean t-value input generally correlates with the parametric t-value result.
		// So we can assume a low t-value has a short length from the start of the curve, and a high t-value has a short length from the end of the curve.
		// We'll use a strategy where we measure from either end of the curve depending on which side is closer than thus more likely to be proximate to the sought parametric t-value.
		// This allows us to use fewer segments to approximate the curve, which usually won't go much beyond half the curve.
		let result_likely_closer_to_start = euclidean_t < 0.5;
		// If the curve is near either end, we need even fewer segments to approximate the curve with reasonable accuracy.
		// A point that's likely near the center is the worst case where we need to use up to half the predefined number of max subdivisions.
		let subdivisions_proportional_to_likely_length = ((euclidean_t - 0.5).abs() * DEFAULT_LENGTH_SUBDIVISIONS as f64).round().max(1.) as usize;

		// Binary search for the parametric t-value that corresponds to the euclidean distance ratio by trimming the curve between the start and the tested parametric t-value during each iteration of the search.
		while low < high {
			mid = (low + high) / 2.;

			// We can search from the curve start to the sought point, or from the sought point to the curve end, depending on which side is likely closer to the result.
			let current_length = if result_likely_closer_to_start {
				let trimmed = self.trim(TValue::Parametric(0.), TValue::Parametric(mid));
				trimmed.length(Some(subdivisions_proportional_to_likely_length))
			} else {
				let trimmed = self.trim(TValue::Parametric(mid), TValue::Parametric(1.));
				let trimmed_length = trimmed.length(Some(subdivisions_proportional_to_likely_length));
				total_length - trimmed_length
			};
			let current_euclidean_t = current_length / total_length;

			if f64_compare(current_euclidean_t, euclidean_t, error) {
				break;
			} else if current_euclidean_t < euclidean_t {
				low = mid;
			} else {
				high = mid;
			}
		}

		mid
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
	pub fn compute_lookup_table(&self, steps: Option<usize>, tvalue_type: Option<TValueType>) -> Vec<DVec2> {
		let steps = steps.unwrap_or(DEFAULT_LUT_STEP_SIZE);
		let tvalue_type = tvalue_type.unwrap_or(TValueType::Parametric);

		(0..=steps)
			.map(|t| {
				let tvalue = match tvalue_type {
					TValueType::Parametric => TValue::Parametric(t as f64 / steps as f64),
					TValueType::Euclidean => TValue::Euclidean(t as f64 / steps as f64),
				};
				self.evaluate(tvalue)
			})
			.collect()
	}

	/// Return an approximation of the length of the bezier curve.
	/// - `num_subdivisions` - Number of subdivisions used to approximate the curve. The default value is 1000.
	/// <iframe frameBorder="0" width="100%" height="300px" src="https://graphite.rs/libraries/bezier-rs#bezier/length/solo" title="Length Demo"></iframe>
	pub fn length(&self, num_subdivisions: Option<usize>) -> f64 {
		match self.handles {
			BezierHandles::Linear => (self.start - self.end).length(),
			_ => {
				// Code example from <https://gamedev.stackexchange.com/questions/5373/moving-ships-between-two-planets-along-a-bezier-missing-some-equations-for-acce/5427#5427>.

				// We will use an approximate approach where we split the curve into many subdivisions
				// and calculate the euclidean distance between the two endpoints of the subdivision
				let lookup_table = self.compute_lookup_table(Some(num_subdivisions.unwrap_or(DEFAULT_LENGTH_SUBDIVISIONS)), Some(TValueType::Parametric));
				let approx_curve_length: f64 = lookup_table.windows(2).map(|points| (points[1] - points[0]).length()).sum();

				approx_curve_length
			}
		}
	}

	/// Returns the parametric `t`-value that corresponds to the closest point on the curve to the provided point.
	/// Uses a searching algorithm akin to binary search that can be customized using the optional [ProjectionOptions] struct.
	/// <iframe frameBorder="0" width="100%" height="300px" src="https://graphite.rs/libraries/bezier-rs#bezier/project/solo" title="Project Demo"></iframe>
	pub fn project(&self, point: DVec2, options: Option<ProjectionOptions>) -> f64 {
		let options = options.unwrap_or_default();
		let ProjectionOptions {
			lut_size,
			convergence_epsilon,
			convergence_limit,
			iteration_limit,
		} = options;

		// TODO: Consider optimizations from precomputing useful values, or using the GPU
		// First find the closest point from the results of a lookup table
		let lut = self.compute_lookup_table(Some(lut_size), Some(TValueType::Parametric));
		let (minimum_position, minimum_distance) = utils::get_closest_point_in_lut(&lut, point);

		// Get the t values to the left and right of the closest result in the lookup table
		let lut_size_f64 = lut_size as f64;
		let minimum_position_f64 = minimum_position as f64;
		let mut left_t = (minimum_position_f64 - 1.).max(0.) / lut_size_f64;
		let mut right_t = (minimum_position_f64 + 1.).min(lut_size_f64) / lut_size_f64;

		// Perform a finer search by finding closest t from 5 points between [left_t, right_t] inclusive
		// Choose new left_t and right_t for a smaller range around the closest t and repeat the process
		let mut final_t = left_t;
		let mut distance;

		// Increment minimum_distance to ensure that the distance < minimum_distance comparison will be true for at least one iteration
		let mut new_minimum_distance = minimum_distance + 1.;
		// Maintain the previous distance to identify convergence
		let mut previous_distance;
		// Counter to limit the number of iterations
		let mut iteration_count = 0;
		// Counter to identify how many iterations have had a similar result. Used for convergence test
		let mut convergence_count = 0;

		// Store calculated distances to minimize unnecessary recomputations
		let mut distances: [f64; NUM_DISTANCES] = [
			point.distance(lut[(minimum_position as i64 - 1).max(0) as usize]),
			0.,
			0.,
			0.,
			point.distance(lut[lut_size.min(minimum_position + 1)]),
		];

		while left_t <= right_t && convergence_count < convergence_limit && iteration_count < iteration_limit {
			previous_distance = new_minimum_distance;
			let step = (right_t - left_t) / (NUM_DISTANCES as f64 - 1.);
			let mut iterator_t = left_t;
			let mut target_index = 0;
			// Iterate through first 4 points and will handle the right most point later
			for (step_index, table_distance) in distances.iter_mut().enumerate().take(4) {
				// Use previously computed distance for the left most point, and compute new values for the others
				if step_index == 0 {
					distance = *table_distance;
				} else {
					distance = point.distance(self.evaluate(TValue::Parametric(iterator_t)));
					*table_distance = distance;
				}
				if distance < new_minimum_distance {
					new_minimum_distance = distance;
					target_index = step_index;
					final_t = iterator_t
				}
				iterator_t += step;
			}
			// Check right most edge separately since step may not perfectly add up to it (floating point errors)
			if distances[NUM_DISTANCES - 1] < new_minimum_distance {
				new_minimum_distance = distances[NUM_DISTANCES - 1];
				final_t = right_t;
			}

			// Update left_t and right_t to be the t values (final_t +/- step), while handling the edges (i.e. if final_t is 0, left_t will be 0 instead of -step)
			// Ensure that the t values never exceed the [0, 1] range
			left_t = (final_t - step).max(0.);
			right_t = (final_t + step).min(1.);

			// Re-use the corresponding computed distances (target_index is the index corresponding to final_t)
			// Since target_index is a u_size, can't subtract one if it is zero
			distances[0] = distances[if target_index == 0 { 0 } else { target_index - 1 }];
			distances[NUM_DISTANCES - 1] = distances[(target_index + 1).min(NUM_DISTANCES - 1)];

			iteration_count += 1;
			// update count for consecutive iterations of similar minimum distances
			if previous_distance - new_minimum_distance < convergence_epsilon {
				convergence_count += 1;
			} else {
				convergence_count = 0;
			}
		}

		final_t
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
		let lookup_table1 = bezier1.compute_lookup_table(Some(2), Some(TValueType::Parametric));
		assert_eq!(lookup_table1, vec![bezier1.start(), bezier1.evaluate(TValue::Parametric(0.5)), bezier1.end()]);

		let bezier2 = Bezier::from_cubic_coordinates(10., 10., 30., 30., 70., 70., 90., 10.);
		let lookup_table2 = bezier2.compute_lookup_table(Some(4), Some(TValueType::Parametric));
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
	fn test_project() {
		let bezier1 = Bezier::from_cubic_coordinates(4., 4., 23., 45., 10., 30., 56., 90.);
		assert_eq!(bezier1.project(DVec2::ZERO, None), 0.);
		assert_eq!(bezier1.project(DVec2::new(100., 100.), None), 1.);

		let bezier2 = Bezier::from_quadratic_coordinates(0., 0., 0., 100., 100., 100.);
		assert_eq!(bezier2.project(DVec2::new(100., 0.), None), 0.);
	}
}
