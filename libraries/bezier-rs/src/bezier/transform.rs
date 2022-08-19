use super::*;

use glam::DMat2;
use std::f64::consts::PI;

/// Functionality that transform Beziers, such as split, reduce, offset, etc.
impl Bezier {
	/// Returns the pair of Bezier curves that result from splitting the original curve at the point corresponding to `t`.
	pub fn split(&self, t: f64) -> [Bezier; 2] {
		let split_point = self.evaluate(t);

		match self.handles {
			BezierHandles::Linear => [Bezier::from_linear_dvec2(self.start, split_point), Bezier::from_linear_dvec2(split_point, self.end)],
			// TODO: Actually calculate the correct handle locations
			BezierHandles::Quadratic { handle } => {
				let t_minus_one = t - 1.;
				[
					Bezier::from_quadratic_dvec2(self.start, t * handle - t_minus_one * self.start, split_point),
					Bezier::from_quadratic_dvec2(split_point, t * self.end - t_minus_one * handle, self.end),
				]
			}
			BezierHandles::Cubic { handle_start, handle_end } => {
				let t_minus_one = t - 1.;
				[
					Bezier::from_cubic_dvec2(
						self.start,
						t * handle_start - t_minus_one * self.start,
						(t * t) * handle_end - 2. * t * t_minus_one * handle_start + (t_minus_one * t_minus_one) * self.start,
						split_point,
					),
					Bezier::from_cubic_dvec2(
						split_point,
						(t * t) * self.end - 2. * t * t_minus_one * handle_end + (t_minus_one * t_minus_one) * handle_start,
						t * self.end - t_minus_one * handle_end,
						self.end,
					),
				]
			}
		}
	}

	/// Returns the Bezier curve representing the sub-curve starting at the point corresponding to `t1` and ending at the point corresponding to `t2`.
	pub fn trim(&self, t1: f64, t2: f64) -> Bezier {
		// Depending on the order of `t1` and `t2`, determine which half of the split we need to keep
		let t1_split_side = if t1 <= t2 { 1 } else { 0 };
		let t2_split_side = if t1 <= t2 { 0 } else { 1 };
		let bezier_starting_at_t1 = self.split(t1)[t1_split_side];
		// Adjust the ratio `t2` to its corresponding value on the new curve that was split on `t1`
		let adjusted_t2 = if t1 < t2 || (t1 == t2 && t1 == 0.) {
			// Case where we took the split from t1 to the end
			// Also cover the `t1` == t2 case where there would otherwise be a divide by 0
			(t2 - t1) / (1. - t1)
		} else {
			// Case where we took the split from the beginning to `t1`
			t2 / t1
		};
		bezier_starting_at_t1.split(adjusted_t2)[t2_split_side]
	}

	/// Returns a Bezier curve that results from applying the transformation function to each point in the Bezier.
	pub fn apply_transformation(&self, transformation_function: &dyn Fn(DVec2) -> DVec2) -> Bezier {
		let transformed_start = transformation_function(self.start);
		let transformed_end = transformation_function(self.end);
		match self.handles {
			BezierHandles::Linear => Bezier::from_linear_dvec2(transformed_start, transformed_end),
			BezierHandles::Quadratic { handle } => {
				let transformed_handle = transformation_function(handle);
				Bezier::from_quadratic_dvec2(transformed_start, transformed_handle, transformed_end)
			}
			BezierHandles::Cubic { handle_start, handle_end } => {
				let transformed_handle_start = transformation_function(handle_start);
				let transformed_handle_end = transformation_function(handle_end);
				Bezier::from_cubic_dvec2(transformed_start, transformed_handle_start, transformed_handle_end, transformed_end)
			}
		}
	}

	/// Returns a Bezier curve that results from rotating the curve around the origin by the given angle (in radians).
	pub fn rotate(&self, angle: f64) -> Bezier {
		let rotation_matrix = DMat2::from_angle(angle);
		self.apply_transformation(&|point| rotation_matrix.mul_vec2(point))
	}

	/// Returns a Bezier curve that results from translating the curve by the given `DVec2`.
	pub fn translate(&self, translation: DVec2) -> Bezier {
		self.apply_transformation(&|point| point + translation)
	}

	/// Determine if it is possible to scale the given curve, using the following conditions:
	/// 1. All the handles are located on a single side of the curve.
	/// 2. The on-curve point for `t = 0.5` must occur roughly in the center of the polygon defined by the curve's endpoint normals.
	/// See [the offset section](https://pomax.github.io/bezierinfo/#offsetting) of Pomax's bezier curve primer for more details.
	fn is_scalable(&self) -> bool {
		if self.handles == BezierHandles::Linear {
			return true;
		}
		// Verify all the handles are located on a single side of the curve.
		if let BezierHandles::Cubic { handle_start, handle_end } = self.handles {
			let angle_1 = (self.end - self.start).angle_between(handle_start - self.start);
			let angle_2 = (self.end - self.start).angle_between(handle_end - self.start);
			if (angle_1 > 0. && angle_2 < 0.) || (angle_1 < 0. && angle_2 > 0.) {
				return false;
			}
		}
		// Verify the angle formed by the endpoint normals is sufficiently small, ensuring the on-curve point for `t = 0.5` occurs roughly in the center of the polygon.
		let normal_0 = self.normal(0.);
		let normal_1 = self.normal(1.);
		let endpoint_normal_angle = (normal_0.x * normal_1.x + normal_0.y * normal_1.y).acos();
		endpoint_normal_angle < SCALABLE_CURVE_MAX_ENDPOINT_NORMAL_ANGLE
	}

	/// Add the bezier endpoints if not already present, and combine and sort the dimensional extrema.
	fn get_extrema_t_list(&self) -> Vec<f64> {
		let mut extrema = self.local_extrema().into_iter().flatten().collect::<Vec<f64>>();
		extrema.append(&mut vec![0., 1.]);
		extrema.dedup();
		extrema.sort_by(|ex1, ex2| ex1.partial_cmp(ex2).unwrap());
		extrema
	}

	/// Returns a tuple of the scalable subcurves and the corresponding `t` values that were used to split the curve.
	/// This function may introduce gaps if subsections of the curve are not reducible.
	/// The function takes the following parameter:
	/// - `step_size` - Dictates the granularity at which the function searches for reducible subcurves. The default value is `0.01`.
	///   A small granularity may increase the chance the function does not introduce gaps, but will increase computation time.
	pub(crate) fn reduced_curves_and_t_values(&self, step_size: Option<f64>) -> (Vec<Bezier>, Vec<f64>) {
		// A linear segment is scalable, so return itself
		if let BezierHandles::Linear = self.handles {
			return (vec![*self], vec![0., 1.]);
		}

		let step_size = step_size.unwrap_or(DEFAULT_REDUCE_STEP_SIZE);

		let extrema = self.get_extrema_t_list();

		// Split each subcurve such that each resulting segment is scalable.
		let mut result_beziers: Vec<Bezier> = Vec::new();
		let mut result_t_values: Vec<f64> = vec![extrema[0]];

		extrema.windows(2).for_each(|t_pair| {
			let t_subcurve_start = t_pair[0];
			let t_subcurve_end = t_pair[1];
			let subcurve = self.trim(t_subcurve_start, t_subcurve_end);
			// Perform no processing on the subcurve if it's already scalable.
			if subcurve.is_scalable() {
				result_beziers.push(subcurve);
				result_t_values.push(t_subcurve_end);
				return;
			}
			// According to <https://pomax.github.io/bezierinfo/#offsetting>, it is generally sufficient to split subcurves with no local extrema at `t = 0.5` to generate two scalable segments.
			let [first_half, second_half] = subcurve.split(0.5);
			if first_half.is_scalable() && second_half.is_scalable() {
				result_beziers.push(first_half);
				result_beziers.push(second_half);
				result_t_values.push(t_subcurve_start + (t_subcurve_end - t_subcurve_start) / 2.);
				result_t_values.push(t_subcurve_end);
				return;
			}

			// Greedily iterate across the subcurve at intervals of size `step_size` to break up the curve into maximally large segments
			let mut segment: Bezier;
			let mut t1 = 0.;
			let mut t2 = step_size;
			while t2 <= 1. + step_size {
				segment = subcurve.trim(t1, f64::min(t2, 1.));
				if !segment.is_scalable() {
					t2 -= step_size;

					// If the previous step does not exist, the start of the subcurve is irreducible.
					// Otherwise, add the valid segment from the previous step to the result.
					if f64::abs(t1 - t2) >= step_size {
						segment = subcurve.trim(t1, t2);
						result_beziers.push(segment);
						result_t_values.push(t_subcurve_start + t2 * (t_subcurve_end - t_subcurve_start));
					} else {
						return;
					}
					t1 = t2;
				}
				t2 += step_size;
			}
			// Collect final remainder of the curve.
			if t1 < 1. {
				segment = subcurve.trim(t1, 1.);
				if segment.is_scalable() {
					result_beziers.push(segment);
					result_t_values.push(t_subcurve_end);
				}
			}
		});
		(result_beziers, result_t_values)
	}

	/// Split the curve into a number of scalable subcurves. This function may introduce gaps if subsections of the curve are not reducible.
	/// The function takes the following parameter:
	/// - `step_size` - Dictates the granularity at which the function searches for reducible subcurves. The default value is `0.01`.
	///   A small granularity may increase the chance the function does not introduce gaps, but will increase computation time.
	pub fn reduce(&self, step_size: Option<f64>) -> Vec<Bezier> {
		self.reduced_curves_and_t_values(step_size).0
	}

	/// Scale will translate a bezier curve a fixed distance away from its original position, and stretch/compress the transformed curve to match the translation ratio.
	/// Note that not all bezier curves are possible to scale, so this function asserts that the provided curve is scalable.
	/// A proof for why this is true can be found in the [Curve offsetting section](https://pomax.github.io/bezierinfo/#offsetting) of Pomax's bezier curve primer.
	/// `scale` takes the parameter `distance`, which is the distance away from the curve that the new one will be scaled to. Positive values will scale the curve in the
	/// same direction as the endpoint normals, while negative values will scale in the opposite direction.
	fn scale(&self, distance: f64) -> Bezier {
		assert!(self.is_scalable(), "The curve provided to scale is not scalable. Reduce the curve first.");

		let normal_start = self.normal(0.);
		let normal_end = self.normal(1.);

		// If normal unit vectors are equal, then the lines are parallel
		if normal_start.abs_diff_eq(normal_end, MAX_ABSOLUTE_DIFFERENCE) {
			return self.translate(distance * normal_start);
		}

		// Find the intersection point of the endpoint normals
		let intersection = utils::line_intersection(self.start, normal_start, self.end, normal_end);

		let should_flip_direction = (self.start - intersection).normalize().abs_diff_eq(normal_start, MAX_ABSOLUTE_DIFFERENCE);
		self.apply_transformation(&|point| {
			let mut direction_unit_vector = (intersection - point).normalize();
			if should_flip_direction {
				direction_unit_vector *= -1.;
			}
			point + distance * direction_unit_vector
		})
	}

	/// Offset will get all the reduceable subcurves, and for each subcurve, it will scale the subcurve a set distance away from the original curve.
	/// Note that not all bezier curves are possible to offset, so this function first reduces the curve to scalable segments and then offsets those segments.
	/// A proof for why this is true can be found in the [Curve offsetting section](https://pomax.github.io/bezierinfo/#offsetting) of Pomax's bezier curve primer.
	/// Offset takes the following parameter:
	/// - `distance` - The distance away from the curve that the new one will be offset to. Positive values will offset the curve in the same direction as the endpoint normals,
	/// while negative values will offset in the opposite direction.
	pub fn offset(&self, distance: f64) -> Vec<Bezier> {
		let mut reduced = self.reduce(None);
		reduced.iter_mut().for_each(|bezier| *bezier = bezier.scale(distance));
		reduced
	}

	/// Approximate a bezier curve with circular arcs.
	/// The algorithm can be customized using the [ArcsOptions] structure.
	pub fn arcs(&self, arcs_options: ArcsOptions) -> Vec<CircleArc> {
		let ArcsOptions {
			strategy: maximize_arcs,
			error,
			max_iterations,
		} = arcs_options;

		match maximize_arcs {
			ArcStrategy::Automatic => {
				let (auto_arcs, final_low_t) = self.approximate_curve_with_arcs(0., 1., error, max_iterations, true);
				let arc_approximations = self.split(final_low_t)[1].arcs(ArcsOptions {
					strategy: ArcStrategy::FavorCorrectness,
					error,
					max_iterations,
				});
				if final_low_t != 1. {
					[auto_arcs, arc_approximations].concat()
				} else {
					auto_arcs
				}
			}
			ArcStrategy::FavorLargerArcs => self.approximate_curve_with_arcs(0., 1., error, max_iterations, false).0,
			ArcStrategy::FavorCorrectness => self
				.get_extrema_t_list()
				.windows(2)
				.flat_map(|t_pair| self.approximate_curve_with_arcs(t_pair[0], t_pair[1], error, max_iterations, false).0)
				.collect::<Vec<CircleArc>>(),
		}
	}

	/// Implements an algorithm that approximates a bezier curve with circular arcs.
	/// This algorithm uses a method akin to binary search to find an arc that approximates a maximal segment of the curve.
	/// Once a maximal arc has been found for a sub-segment of the curve, the algorithm continues by starting again at the end of the previous approximation.
	/// More details can be found in the [Approximating a Bezier curve with circular arcs](https://pomax.github.io/bezierinfo/#arcapproximation) section of Pomax's bezier curve primer.
	/// A caveat with this algorithm is that it is possible to find erroneous approximations in cases such as in a very narrow `U`.
	/// - `stop_when_invalid`: Used to determine whether the algorithm should terminate early if erroneous approximations are encountered.
	///
	/// Returns a tuple where the first element is the list of circular arcs and the second is the `t` value where the next segment should start from.
	/// The second value will be `1.` except for when `stop_when_invalid` is true and an invalid approximation is encountered.
	fn approximate_curve_with_arcs(&self, local_low: f64, local_high: f64, error: f64, max_iterations: usize, stop_when_invalid: bool) -> (Vec<CircleArc>, f64) {
		let mut low = local_low;
		let mut middle = (local_low + local_high) / 2.;
		let mut high = local_high;
		let mut previous_high = local_high;

		let mut iterations = 0;
		let mut previous_arc = CircleArc::default();
		let mut was_previous_good = false;
		let mut arcs = Vec::new();

		// Outer loop to iterate over the curve
		while low < local_high {
			// Inner loop to find the next maximal segment of the curve that can be approximated with a circular arc
			while iterations <= max_iterations {
				iterations += 1;
				let p1 = self.evaluate(low);
				let p2 = self.evaluate(middle);
				let p3 = self.evaluate(high);

				let wrapped_center = utils::compute_circle_center_from_points(p1, p2, p3);
				// If the segment is linear, move on to next segment
				if wrapped_center.is_none() {
					previous_high = high;
					low = high;
					high = 1.;
					middle = (low + high) / 2.;
					was_previous_good = false;
					break;
				}

				let center = wrapped_center.unwrap();
				let radius = center.distance(p1);

				let angle_p1 = DVec2::new(1., 0.).angle_between(p1 - center);
				let angle_p2 = DVec2::new(1., 0.).angle_between(p2 - center);
				let angle_p3 = DVec2::new(1., 0.).angle_between(p3 - center);

				let mut start_angle = angle_p1;
				let mut end_angle = angle_p3;

				// Adjust start and end angles of the arc to ensure that it travels in the counter-clockwise direction
				if angle_p1 < angle_p3 {
					if angle_p2 < angle_p1 || angle_p3 < angle_p2 {
						std::mem::swap(&mut start_angle, &mut end_angle);
					}
				} else if angle_p2 < angle_p1 && angle_p3 < angle_p2 {
					std::mem::swap(&mut start_angle, &mut end_angle);
				}

				let new_arc = CircleArc {
					center,
					radius,
					start_angle,
					end_angle,
				};

				// Use points in between low, middle, and high to evaluate how well the arc approximates the curve
				let e1 = self.evaluate((low + middle) / 2.);
				let e2 = self.evaluate((middle + high) / 2.);

				// Iterate until we find the largest good approximation such that the next iteration is not a good approximation with an arc
				if utils::f64_compare(radius, e1.distance(center), error) && utils::f64_compare(radius, e2.distance(center), error) {
					// Check if the good approximation is actually valid: the sector angle cannot be larger than 180 degrees (PI radians)
					let mut sector_angle = end_angle - start_angle;
					if sector_angle < 0. {
						sector_angle += 2. * PI;
					}
					if stop_when_invalid && sector_angle > PI {
						return (arcs, low);
					}
					if high == local_high {
						// Found the final arc approximation
						arcs.push(new_arc);
						low = high;
						break;
					}
					// If the approximation is good, expand the segment by half to try finding a larger good approximation
					previous_high = high;
					high = (high + (high - low) / 2.).min(local_high);
					middle = (low + high) / 2.;
					previous_arc = new_arc;
					was_previous_good = true;
				} else if was_previous_good {
					// If the previous approximation was good and the current one is bad, then we use the previous good approximation
					arcs.push(previous_arc);

					// Continue searching for approximations for the rest of the curve
					low = previous_high;
					high = local_high;
					middle = low + (high - low) / 2.;
					was_previous_good = false;
					break;
				} else {
					// If no good approximation has been seen yet, try again with half the segment
					previous_high = high;
					high = middle;
					middle = low + (high - low) / 2.;
					previous_arc = new_arc;
				}
			}
		}

		(arcs, low)
	}
}

#[cfg(test)]
mod tests {
	use super::compare::{compare_arcs, compare_vector_of_beziers};
	use super::*;

	#[test]
	fn test_split() {
		let line = Bezier::from_linear_coordinates(25., 25., 75., 75.);
		let [part1, part2] = line.split(0.5);

		assert_eq!(part1.start(), line.start());
		assert_eq!(part1.end(), line.evaluate(0.5));
		assert_eq!(part1.evaluate(0.5), line.evaluate(0.25));

		assert_eq!(part2.start(), line.evaluate(0.5));
		assert_eq!(part2.end(), line.end());
		assert_eq!(part2.evaluate(0.5), line.evaluate(0.75));

		let quad_bezier = Bezier::from_quadratic_coordinates(10., 10., 50., 50., 90., 10.);
		let [part3, part4] = quad_bezier.split(0.5);

		assert_eq!(part3.start(), quad_bezier.start());
		assert_eq!(part3.end(), quad_bezier.evaluate(0.5));
		assert_eq!(part3.evaluate(0.5), quad_bezier.evaluate(0.25));

		assert_eq!(part4.start(), quad_bezier.evaluate(0.5));
		assert_eq!(part4.end(), quad_bezier.end());
		assert_eq!(part4.evaluate(0.5), quad_bezier.evaluate(0.75));

		let cubic_bezier = Bezier::from_cubic_coordinates(10., 10., 50., 50., 90., 10., 40., 50.);
		let [part5, part6] = cubic_bezier.split(0.5);

		assert_eq!(part5.start(), cubic_bezier.start());
		assert_eq!(part5.end(), cubic_bezier.evaluate(0.5));
		assert_eq!(part5.evaluate(0.5), cubic_bezier.evaluate(0.25));

		assert_eq!(part6.start(), cubic_bezier.evaluate(0.5));
		assert_eq!(part6.end(), cubic_bezier.end());
		assert_eq!(part6.evaluate(0.5), cubic_bezier.evaluate(0.75));
	}

	#[test]
	fn test_split_at_anchors() {
		let start = DVec2::new(30., 50.);
		let end = DVec2::new(160., 170.);

		let bezier_quadratic = Bezier::from_quadratic_dvec2(start, DVec2::new(140., 30.), end);

		// Test splitting a quadratic bezier at the startpoint
		let [point_bezier1, remainder1] = bezier_quadratic.split(0.);
		assert_eq!(point_bezier1, Bezier::from_quadratic_dvec2(start, start, start));
		assert!(remainder1.abs_diff_eq(&bezier_quadratic, MAX_ABSOLUTE_DIFFERENCE));

		// Test splitting a quadratic bezier at the endpoint
		let [remainder2, point_bezier2] = bezier_quadratic.split(1.);
		assert_eq!(point_bezier2, Bezier::from_quadratic_dvec2(end, end, end));
		assert!(remainder2.abs_diff_eq(&bezier_quadratic, MAX_ABSOLUTE_DIFFERENCE));

		let bezier_cubic = Bezier::from_cubic_dvec2(start, DVec2::new(60., 140.), DVec2::new(150., 30.), end);

		// Test splitting a cubic bezier at the startpoint
		let [point_bezier3, remainder3] = bezier_cubic.split(0.);
		assert_eq!(point_bezier3, Bezier::from_cubic_dvec2(start, start, start, start));
		assert!(remainder3.abs_diff_eq(&bezier_cubic, MAX_ABSOLUTE_DIFFERENCE));

		// Test splitting a cubic bezier at the endpoint
		let [remainder4, point_bezier4] = bezier_cubic.split(1.);
		assert_eq!(point_bezier4, Bezier::from_cubic_dvec2(end, end, end, end));
		assert!(remainder4.abs_diff_eq(&bezier_cubic, MAX_ABSOLUTE_DIFFERENCE));
	}

	#[test]
	fn test_trim() {
		let line = Bezier::from_linear_coordinates(80., 80., 40., 40.);
		let trimmed1 = line.trim(0.25, 0.75);

		assert_eq!(trimmed1.start(), line.evaluate(0.25));
		assert_eq!(trimmed1.end(), line.evaluate(0.75));
		assert_eq!(trimmed1.evaluate(0.5), line.evaluate(0.5));

		let quadratic_bezier = Bezier::from_quadratic_coordinates(80., 80., 40., 40., 70., 70.);
		let trimmed2 = quadratic_bezier.trim(0.25, 0.75);

		assert_eq!(trimmed2.start(), quadratic_bezier.evaluate(0.25));
		assert_eq!(trimmed2.end(), quadratic_bezier.evaluate(0.75));
		assert_eq!(trimmed2.evaluate(0.5), quadratic_bezier.evaluate(0.5));

		let cubic_bezier = Bezier::from_cubic_coordinates(80., 80., 40., 40., 70., 70., 150., 150.);
		let trimmed3 = cubic_bezier.trim(0.25, 0.75);

		assert_eq!(trimmed3.start(), cubic_bezier.evaluate(0.25));
		assert_eq!(trimmed3.end(), cubic_bezier.evaluate(0.75));
		assert_eq!(trimmed3.evaluate(0.5), cubic_bezier.evaluate(0.5));
	}

	#[test]
	fn test_trim_t2_greater_than_t1() {
		// Test trimming quadratic curve when t2 > t1
		let bezier_quadratic = Bezier::from_quadratic_coordinates(30., 50., 140., 30., 160., 170.);
		let trim1 = bezier_quadratic.trim(0.25, 0.75);
		let trim2 = bezier_quadratic.trim(0.75, 0.25);
		assert!(trim1.abs_diff_eq(&trim2, MAX_ABSOLUTE_DIFFERENCE));

		// Test trimming cubic curve when t2 > t1
		let bezier_cubic = Bezier::from_cubic_coordinates(30., 30., 60., 140., 150., 30., 160., 160.);
		let trim3 = bezier_cubic.trim(0.25, 0.75);
		let trim4 = bezier_cubic.trim(0.75, 0.25);
		assert!(trim3.abs_diff_eq(&trim4, MAX_ABSOLUTE_DIFFERENCE));
	}

	#[test]
	fn test_rotate() {
		let bezier_linear = Bezier::from_linear_coordinates(30., 60., 140., 120.);
		let rotated_bezier_linear = bezier_linear.rotate(-PI / 2.);
		let expected_bezier_linear = Bezier::from_linear_coordinates(60., -30., 120., -140.);
		assert!(rotated_bezier_linear.abs_diff_eq(&expected_bezier_linear, MAX_ABSOLUTE_DIFFERENCE));

		let bezier_quadratic = Bezier::from_quadratic_coordinates(30., 50., 140., 30., 160., 170.);
		let rotated_bezier_quadratic = bezier_quadratic.rotate(PI);
		let expected_bezier_quadratic = Bezier::from_quadratic_coordinates(-30., -50., -140., -30., -160., -170.);
		assert!(rotated_bezier_quadratic.abs_diff_eq(&expected_bezier_quadratic, MAX_ABSOLUTE_DIFFERENCE));

		let bezier = Bezier::from_cubic_coordinates(30., 30., 60., 140., 150., 30., 160., 160.);
		let rotated_bezier = bezier.rotate(PI / 2.);
		let expected_bezier = Bezier::from_cubic_coordinates(-30., 30., -140., 60., -30., 150., -160., 160.);
		assert!(rotated_bezier.abs_diff_eq(&expected_bezier, MAX_ABSOLUTE_DIFFERENCE));
	}

	#[test]
	fn test_translate() {
		let bezier_linear = Bezier::from_linear_coordinates(30., 60., 140., 120.);
		let rotated_bezier_linear = bezier_linear.translate(DVec2::new(10., 10.));
		let expected_bezier_linear = Bezier::from_linear_coordinates(40., 70., 150., 130.);
		assert!(rotated_bezier_linear.abs_diff_eq(&expected_bezier_linear, MAX_ABSOLUTE_DIFFERENCE));

		let bezier_quadratic = Bezier::from_quadratic_coordinates(30., 50., 140., 30., 160., 170.);
		let rotated_bezier_quadratic = bezier_quadratic.translate(DVec2::new(-10., 10.));
		let expected_bezier_quadratic = Bezier::from_quadratic_coordinates(20., 60., 130., 40., 150., 180.);
		assert!(rotated_bezier_quadratic.abs_diff_eq(&expected_bezier_quadratic, MAX_ABSOLUTE_DIFFERENCE));

		let bezier = Bezier::from_cubic_coordinates(30., 30., 60., 140., 150., 30., 160., 160.);
		let translated_bezier = bezier.translate(DVec2::new(10., -10.));
		let expected_bezier = Bezier::from_cubic_coordinates(40., 20., 70., 130., 160., 20., 170., 150.);
		assert!(translated_bezier.abs_diff_eq(&expected_bezier, MAX_ABSOLUTE_DIFFERENCE));
	}

	#[test]
	fn test_reduce() {
		let p1 = DVec2::new(0., 0.);
		let p2 = DVec2::new(50., 50.);
		let p3 = DVec2::new(0., 0.);
		let bezier = Bezier::from_quadratic_dvec2(p1, p2, p3);

		let expected_bezier_points = vec![
			vec![DVec2::new(0., 0.), DVec2::new(0.5, 0.5), DVec2::new(0.989, 0.989)],
			vec![DVec2::new(0.989, 0.989), DVec2::new(2.705, 2.705), DVec2::new(4.2975, 4.2975)],
			vec![DVec2::new(4.2975, 4.2975), DVec2::new(5.6625, 5.6625), DVec2::new(6.9375, 6.9375)],
		];
		let reduced_curves = bezier.reduce(None);
		assert!(compare_vector_of_beziers(&reduced_curves, expected_bezier_points));

		// Check that the reduce helper is correct
		let (helper_curves, helper_t_values) = bezier.reduced_curves_and_t_values(None);
		assert_eq!(&reduced_curves, &helper_curves);
		assert!(reduced_curves
			.iter()
			.zip(helper_t_values.windows(2))
			.all(|(curve, t_pair)| curve.abs_diff_eq(&bezier.trim(t_pair[0], t_pair[1]), MAX_ABSOLUTE_DIFFERENCE)))
	}

	#[test]
	fn test_offset() {
		let p1 = DVec2::new(30., 50.);
		let p2 = DVec2::new(140., 30.);
		let p3 = DVec2::new(160., 170.);
		let bezier1 = Bezier::from_quadratic_dvec2(p1, p2, p3);
		let expected_bezier_points1 = vec![
			vec![DVec2::new(31.7888, 59.8387), DVec2::new(44.5924, 57.46446), DVec2::new(56.09375, 57.5)],
			vec![DVec2::new(56.09375, 57.5), DVec2::new(94.94197, 56.5019), DVec2::new(117.6473, 84.5936)],
			vec![DVec2::new(117.6473, 84.5936), DVec2::new(142.3985, 113.403), DVec2::new(150.1005, 171.4142)],
		];
		assert!(compare_vector_of_beziers(&bezier1.offset(10.), expected_bezier_points1));

		let p4 = DVec2::new(32., 77.);
		let p5 = DVec2::new(169., 25.);
		let p6 = DVec2::new(164., 157.);
		let bezier2 = Bezier::from_quadratic_dvec2(p4, p5, p6);
		let expected_bezier_points2 = vec![
			vec![DVec2::new(42.6458, 105.04758), DVec2::new(75.0218, 91.9939), DVec2::new(98.09357, 92.3043)],
			vec![DVec2::new(98.09357, 92.3043), DVec2::new(116.5995, 88.5479), DVec2::new(123.9055, 102.0401)],
			vec![DVec2::new(123.9055, 102.0401), DVec2::new(136.6087, 116.9522), DVec2::new(134.1761, 147.9324)],
			vec![DVec2::new(134.1761, 147.9324), DVec2::new(134.1812, 151.7987), DVec2::new(134.0215, 155.86445)],
		];
		assert!(compare_vector_of_beziers(&bezier2.offset(30.), expected_bezier_points2));
	}

	#[test]
	fn test_arcs_linear() {
		let bezier = Bezier::from_linear_coordinates(30., 60., 140., 120.);
		let linear_arcs = bezier.arcs(ArcsOptions::default());
		assert!(linear_arcs.is_empty());
	}

	#[test]
	fn test_arcs_quadratic() {
		let bezier1 = Bezier::from_quadratic_coordinates(30., 30., 50., 50., 100., 100.);
		assert!(bezier1.arcs(ArcsOptions::default()).is_empty());

		let bezier2 = Bezier::from_quadratic_coordinates(50., 50., 85., 65., 100., 100.);
		let actual_arcs = bezier2.arcs(ArcsOptions::default());
		let expected_arc = CircleArc {
			center: DVec2::new(15., 135.),
			radius: 91.92388,
			start_angle: -1.18019,
			end_angle: -0.39061,
		};
		assert_eq!(actual_arcs.len(), 1);
		assert!(compare_arcs(actual_arcs[0], expected_arc));
	}

	#[test]
	fn test_arcs_cubic() {
		let bezier = Bezier::from_cubic_coordinates(30., 30., 30., 80., 60., 80., 60., 140.);
		let actual_arcs = bezier.arcs(ArcsOptions::default());
		let expected_arcs = vec![
			CircleArc {
				center: DVec2::new(122.394877, 30.7777189),
				radius: 92.39815,
				start_angle: 2.5637146,
				end_angle: -3.1331755,
			},
			CircleArc {
				center: DVec2::new(-47.54881, 136.169378),
				radius: 107.61701,
				start_angle: -0.53556,
				end_angle: 0.0356025,
			},
		];

		assert_eq!(actual_arcs.len(), 2);
		assert!(compare_arcs(actual_arcs[0], expected_arcs[0]));
		assert!(compare_arcs(actual_arcs[1], expected_arcs[1]));

		// Bezier that contains the erroneous case when maximizing arcs
		let bezier2 = Bezier::from_cubic_coordinates(48., 176., 170., 10., 30., 90., 180., 160.);
		let auto_arcs = bezier2.arcs(ArcsOptions::default());

		let extrema_arcs = bezier2.arcs(ArcsOptions {
			strategy: ArcStrategy::FavorCorrectness,
			..ArcsOptions::default()
		});

		let maximal_arcs = bezier2.arcs(ArcsOptions {
			strategy: ArcStrategy::FavorLargerArcs,
			..ArcsOptions::default()
		});

		// Resulting automatic arcs match the maximal results until the bad arc (in this case, only index 0 should match)
		assert_eq!(auto_arcs[0], maximal_arcs[0]);
		// Check that the first result from MaximizeArcs::Automatic should not equal the first results from MaximizeArcs::Off
		assert_ne!(auto_arcs[0], extrema_arcs[0]);
		// The remaining results (index 2 onwards) should match the results where MaximizeArcs::Off from the next extrema point onwards (after index 2).
		assert!(auto_arcs.iter().skip(2).zip(extrema_arcs.iter().skip(2)).all(|(arc1, arc2)| compare_arcs(*arc1, *arc2)));
	}
}
