use super::consts::MAX_ABSOLUTE_DIFFERENCE;
use super::*;
use crate::math::polynomial::pathseg_to_parametric_polynomial;
use crate::vector::algorithms::bezpath_algorithms::pathseg_length_centroid_and_length;
use crate::vector::algorithms::intersection::{filtered_all_segment_intersections, pathseg_self_intersections};
use glam::DVec2;

impl<PointId: Identifier> Subpath<PointId> {
	/// Returns a list of `t` values that correspond to all the self intersection points of the subpath always considering it as a closed subpath. The index and `t` value of both will be returned that corresponds to a point.
	/// The points will be sorted based on their index and `t` repsectively.
	/// - `error` - For intersections with non-linear beziers, `error` defines the threshold for bounding boxes to be considered an intersection point.
	/// - `minimum_separation`: the minimum difference two adjacent `t`-values must have when comparing adjacent `t`-values in sorted order.
	///
	/// If the comparison condition is not satisfied, the function takes the larger `t`-value of the two
	///
	/// **NOTE**: if an intersection were to occur within an `error` distance away from an anchor point, the algorithm will filter that intersection out.
	pub fn all_self_intersections(&self, accuracy: Option<f64>, minimum_separation: Option<f64>) -> Vec<(usize, f64)> {
		let mut intersections_vec = Vec::new();
		let err = accuracy.unwrap_or(MAX_ABSOLUTE_DIFFERENCE);
		let num_curves = self.len();
		// TODO: optimization opportunity - this for-loop currently compares all intersections with all curve-segments in the subpath collection
		self.iter_closed().enumerate().for_each(|(i, other)| {
			intersections_vec.extend(pathseg_self_intersections(other, accuracy, minimum_separation).iter().flat_map(|value| [(i, value.0), (i, value.1)]));
			self.iter_closed().enumerate().skip(i + 1).for_each(|(j, curve)| {
				intersections_vec.extend(
					filtered_all_segment_intersections(curve, other, accuracy, minimum_separation)
						.iter()
						.filter(|&value| (j != i + 1 || value.0 > err || (1. - value.1) > err) && (j != num_curves - 1 || i != 0 || value.1 > err || (1. - value.0) > err))
						.flat_map(|value| [(j, value.0), (i, value.1)]),
				);
			});
		});

		intersections_vec.sort_by(|a, b| a.partial_cmp(b).unwrap());

		intersections_vec
	}

	/// Return the area centroid, together with the area, of the `Subpath` always considering it as a closed subpath. The area will always be a positive value.
	///
	/// The area centroid is the center of mass for the area of a solid shape's interior.
	/// An infinitely flat material forming the subpath's closed shape would balance at this point.
	///
	/// It will return `None` if no manipulator is present. If the area is less than `error`, it will return `Some((DVec2::NAN, 0.))`.
	///
	/// Because the calculation of area and centroid for self-intersecting path requires finding the intersections, the following parameters are used:
	/// - `error` - For intersections with non-linear beziers, `error` defines the threshold for bounding boxes to be considered an intersection point.
	/// - `minimum_separation` - the minimum difference two adjacent `t`-values must have when comparing adjacent `t`-values in sorted order.
	///
	/// If the comparison condition is not satisfied, the function takes the larger `t`-value of the two.
	///
	/// **NOTE**: if an intersection were to occur within an `error` distance away from an anchor point, the algorithm will filter that intersection out.
	pub fn area_centroid_and_area(&self, error: Option<f64>, minimum_separation: Option<f64>) -> Option<(DVec2, f64)> {
		let all_intersections = self.all_self_intersections(error, minimum_separation);
		let mut current_sign: f64 = 1.;

		let (x_sum, y_sum, area) = self
			.iter_closed()
			.enumerate()
			.map(|(index, bezier)| {
				let (f_x, f_y) = pathseg_to_parametric_polynomial(bezier);
				let (f_x, f_y) = (f_x.as_size::<10>().unwrap(), f_y.as_size::<10>().unwrap());
				let f_y_prime = f_y.derivative();
				let f_x_prime = f_x.derivative();
				let f_xy = &f_x * &f_y;

				let mut x_part = &f_xy * &f_x_prime;
				let mut y_part = &f_xy * &f_y_prime;
				let mut area_part = &f_x * &f_y_prime;
				x_part.antiderivative_mut();
				y_part.antiderivative_mut();
				area_part.antiderivative_mut();

				let mut curve_sum_x = -current_sign * x_part.eval(0.);
				let mut curve_sum_y = -current_sign * y_part.eval(0.);
				let mut curve_sum_area = -current_sign * area_part.eval(0.);
				for (_, t) in all_intersections.iter().filter(|(i, _)| *i == index) {
					curve_sum_x += 2. * current_sign * x_part.eval(*t);
					curve_sum_y += 2. * current_sign * y_part.eval(*t);
					curve_sum_area += 2. * current_sign * area_part.eval(*t);
					current_sign *= -1.;
				}
				curve_sum_x += current_sign * x_part.eval(1.);
				curve_sum_y += current_sign * y_part.eval(1.);
				curve_sum_area += current_sign * area_part.eval(1.);

				(-curve_sum_x, curve_sum_y, curve_sum_area)
			})
			.reduce(|(x1, y1, area1), (x2, y2, area2)| (x1 + x2, y1 + y2, area1 + area2))?;

		if area.abs() < error.unwrap_or(MAX_ABSOLUTE_DIFFERENCE) {
			return Some((DVec2::NAN, 0.));
		}

		Some((DVec2::new(x_sum / area, y_sum / area), area.abs()))
	}

	/// Return the approximation of the length centroid, together with the length, of the `Subpath`.
	///
	/// The length centroid is the center of mass for the arc length of the solid shape's perimeter.
	/// An infinitely thin wire forming the subpath's closed shape would balance at this point.
	///
	/// It will return `None` if no manipulator is present.
	/// - `accuracy` is used to approximate the curve.
	/// - `always_closed` is to consider the subpath as closed always.
	pub fn length_centroid_and_length(&self, accuracy: Option<f64>, always_closed: bool) -> Option<(DVec2, f64)> {
		if always_closed { self.iter_closed() } else { self.iter() }
			.map(|bezier| pathseg_length_centroid_and_length(bezier, accuracy))
			.map(|(centroid, length)| (centroid * length, length))
			.reduce(|(centroid_part1, length1), (centroid_part2, length2)| (centroid_part1 + centroid_part2, length1 + length2))
			.map(|(centroid_part, length)| (centroid_part / length, length))
			.map(|(centroid_part, length)| (DVec2::new(centroid_part.x, centroid_part.y), length))
	}
}
