use crate::consts::{MAX_ABSOLUTE_DIFFERENCE, STRICT_MAX_ABSOLUTE_DIFFERENCE};
use crate::ManipulatorGroup;

use glam::{BVec2, DMat2, DVec2};

#[derive(Copy, Clone, PartialEq)]
/// A structure which can be used to reference a particular point along a `Bezier`.
/// Assuming a 2-dimensional Bezier is represented as a parametric curve defined by components `(x(f(t), y(f(t))))`, this structure defines variants for `f(t)`.
/// - The `Parametric` variant represents the point calculated using the parametric equation of the curve at argument `t`. That is, `f(t) = t`. Speed along the curve's parametric form is not constant. `t` must lie in the range `[0, 1]`.
/// - The `Euclidean` variant represents the point calculated at a distance ratio `t` along the arc length of the curve in the range `[0, 1]`. Speed is constant along the curve's arc length.
///   - E.g. If `d` is the distance from the start point of a `Bezier` to a certain point along the curve, and `l` is the total arc length of the curve, that certain point lies at a distance ratio `t = d / l`.
///   - All `Bezier` functions will implicitly convert a Euclidean [TValue] argument to a parametric `t`-value using binary search, computed within a particular error. That is, a point at distance ratio `t*`,
///     satisfying `|t* - t| <= error`. The default error is `0.001`. Given this requires a lengthier calculation, it is not recommended to use the `Euclidean` or `EuclideanWithinError` variants frequently in computationally intensive tasks.
/// - The `EuclideanWithinError` variant functions exactly as the `Euclidean` variant, but allows the `error` to be customized when computing `t` internally.
pub enum TValue {
	Parametric(f64),
	Euclidean(f64),
	EuclideanWithinError { t: f64, error: f64 },
}

#[derive(Copy, Clone, PartialEq)]
pub enum TValueType {
	Parametric,
	Euclidean,
}

#[derive(Copy, Clone, PartialEq)]
pub enum SubpathTValue {
	Parametric { segment_index: usize, t: f64 },
	GlobalParametric(f64),
	Euclidean { segment_index: usize, t: f64 },
	GlobalEuclidean(f64),
	EuclideanWithinError { segment_index: usize, t: f64, error: f64 },
	GlobalEuclideanWithinError { t: f64, error: f64 },
}

#[derive(Copy, Clone)]
/// Represents the shape of the join between two segments of a path which meet at an angle.
/// Bevel provides a flat connection, Miter provides a sharp connection, and Round provides a rounded connection.
/// As defined in SVG: <https://www.w3.org/TR/SVG2/painting.html#LineJoin>.
pub enum Join {
	/// The join is a straight line between the end points of the offset path sides from the two connecting segments.
	Bevel,
	/// Optional f64 is the miter limit, which defaults to 4 if `None` or a value less than 1 is provided.
	/// The miter limit is used to prevent highly sharp angles from resulting in excessively long miter joins.
	/// If the miter limit is exceeded, the join will be converted to a bevel join.
	/// The value is the ratio of the miter length to the stroke width.
	/// When that ratio is greater than the miter limit, a bevel join is used instead.
	Miter(Option<f64>),
	/// The join is a circular arc between the end points of the offset path sides from the two connecting segments.
	Round,
}

#[derive(Copy, Clone)]
/// Enum to represent the cap type at the ends of an outline
/// As defined in SVG: <https://www.w3.org/TR/SVG2/painting.html#LineCaps>.
pub enum Cap {
	Butt,
	Round,
	Square,
}

/// Helper to perform the computation of a and c, where b is the provided point on the curve.
/// Given the correct power of `t` and `(1-t)`, the computation is the same for quadratic and cubic cases.
/// Relevant derivation and the definitions of a, b, and c can be found in [the projection identity section](https://pomax.github.io/bezierinfo/#abc) of Pomax's bezier curve primer.
fn compute_abc_through_points(start_point: DVec2, point_on_curve: DVec2, end_point: DVec2, t_to_nth_power: f64, nth_power_of_one_minus_t: f64) -> [DVec2; 3] {
	let point_c_ratio = nth_power_of_one_minus_t / (t_to_nth_power + nth_power_of_one_minus_t);
	let c = point_c_ratio * start_point + (1. - point_c_ratio) * end_point;
	let ab_bc_ratio = (t_to_nth_power + nth_power_of_one_minus_t - 1.).abs() / (t_to_nth_power + nth_power_of_one_minus_t);
	let a = point_on_curve + (point_on_curve - c) / ab_bc_ratio;
	[a, point_on_curve, c]
}

/// Compute `a`, `b`, and `c` for a quadratic curve that fits the start, end and point on curve at `t`.
/// The definition for the `a`, `b`, `c` points are defined in [the projection identity section](https://pomax.github.io/bezierinfo/#abc) of Pomax's bezier curve primer.
pub fn compute_abc_for_quadratic_through_points(start_point: DVec2, point_on_curve: DVec2, end_point: DVec2, t: f64) -> [DVec2; 3] {
	let t_squared = t * t;
	let one_minus_t = 1. - t;
	let squared_one_minus_t = one_minus_t * one_minus_t;
	compute_abc_through_points(start_point, point_on_curve, end_point, t_squared, squared_one_minus_t)
}

/// Compute `a`, `b`, and `c` for a cubic curve that fits the start, end and point on curve at `t`.
/// The definition for the `a`, `b`, `c` points are defined in [the projection identity section](https://pomax.github.io/bezierinfo/#abc) of Pomax's bezier curve primer.
pub fn compute_abc_for_cubic_through_points(start_point: DVec2, point_on_curve: DVec2, end_point: DVec2, t: f64) -> [DVec2; 3] {
	let t_cubed = t * t * t;
	let one_minus_t = 1. - t;
	let cubed_one_minus_t = one_minus_t * one_minus_t * one_minus_t;

	compute_abc_through_points(start_point, point_on_curve, end_point, t_cubed, cubed_one_minus_t)
}

/// Return the index and the value of the closest point in the LUT compared to the provided point.
pub fn get_closest_point_in_lut(lut: &[DVec2], point: DVec2) -> (usize, f64) {
	lut.iter().enumerate().map(|(i, p)| (i, point.distance_squared(*p))).min_by(|x, y| (x.1).total_cmp(&(y.1))).unwrap()
}

/// Find the roots of the linear equation `ax + b`.
pub fn solve_linear(a: f64, b: f64) -> [Option<f64>; 3] {
	// There exist roots when `a` is not 0
	if a.abs() > MAX_ABSOLUTE_DIFFERENCE {
		[Some(-b / a), None, None]
	} else {
		[None; 3]
	}
}

/// Find the roots of the linear equation `ax^2 + bx + c`.
/// Precompute the `discriminant` (`b^2 - 4ac`) and `two_times_a` arguments prior to calling this function for efficiency purposes.
pub fn solve_quadratic(discriminant: f64, two_times_a: f64, b: f64, c: f64) -> [Option<f64>; 3] {
	let mut roots = [None; 3];
	if two_times_a.abs() <= STRICT_MAX_ABSOLUTE_DIFFERENCE {
		roots = solve_linear(b, c);
	} else if discriminant.abs() <= STRICT_MAX_ABSOLUTE_DIFFERENCE {
		roots[0] = Some(-b / (two_times_a));
	} else if discriminant > 0. {
		let root_discriminant = discriminant.sqrt();
		roots[0] = Some((-b + root_discriminant) / (two_times_a));
		roots[1] = Some((-b - root_discriminant) / (two_times_a));
	}
	roots
}

// TODO: Use an `impl Iterator` return type instead of a `Vec`
/// Solve a cubic of the form `ax^3 + bx^2 + ct + d`.
pub fn solve_cubic(a: f64, b: f64, c: f64, d: f64) -> [Option<f64>; 3] {
	if a.abs() <= STRICT_MAX_ABSOLUTE_DIFFERENCE {
		if b.abs() <= STRICT_MAX_ABSOLUTE_DIFFERENCE {
			// If both a and b are approximately 0, treat as a linear problem
			solve_linear(c, d)
		} else {
			// If a is approximately 0, treat as a quadratic problem
			let discriminant = c * c - 4. * b * d;
			solve_quadratic(discriminant, 2. * b, c, d)
		}
	} else {
		// https://momentsingraphics.de/CubicRoots.html
		let d_recip = a.recip();
		const ONETHIRD: f64 = 1. / 3.;
		let scaled_c2 = b * (ONETHIRD * d_recip);
		let scaled_c1 = c * (ONETHIRD * d_recip);
		let scaled_c0 = d * d_recip;
		if !(scaled_c0.is_finite() && scaled_c1.is_finite() && scaled_c2.is_finite()) {
			// cubic coefficient is zero or nearly so.
			return solve_quadratic(c * c - 4. * b * d, 2. * b, c, d);
		}
		let (c0, c1, c2) = (scaled_c0, scaled_c1, scaled_c2);
		// (d0, d1, d2) is called "Delta" in article
		let d0 = (-c2).mul_add(c2, c1);
		let d1 = (-c1).mul_add(c2, c0);
		let d2 = c2 * c0 - c1 * c1;
		// d is called "Discriminant"
		let d = 4. * d0 * d2 - d1 * d1;
		// de is called "Depressed.x", Depressed.y = d0
		let de = (-2. * c2).mul_add(d0, d1);
		if d < 0. {
			let sq = (-0.25 * d).sqrt();
			let r = -0.5 * de;
			let t1 = (r + sq).cbrt() + (r - sq).cbrt();
			[Some(t1 - c2), None, None]
		} else if d == 0. {
			let t1 = (-d0).sqrt().copysign(de);
			[Some(t1 - c2), Some(-2. * t1 - c2).filter(|&a| a != t1 - c2), None]
		} else {
			let th = d.sqrt().atan2(-de) * ONETHIRD;
			// (th_cos, th_sin) is called "CubicRoot"
			let (th_sin, th_cos) = th.sin_cos();
			// (r0, r1, r2) is called "Root"
			let r0 = th_cos;
			let ss3 = th_sin * 3_f64.sqrt();
			let r1 = 0.5 * (-th_cos + ss3);
			let r2 = 0.5 * (-th_cos - ss3);
			let t = 2. * (-d0).sqrt();
			[Some(t.mul_add(r0, -c2)), Some(t.mul_add(r1, -c2)), Some(t.mul_add(r2, -c2))]
		}
	}
}

/// Determine if two rectangles have any overlap. The rectangles are represented by a pair of coordinates that designate the top left and bottom right corners (in a graphical coordinate system).
pub fn do_rectangles_overlap(rectangle1: [DVec2; 2], rectangle2: [DVec2; 2]) -> bool {
	let [bottom_left1, top_right1] = rectangle1;
	let [bottom_left2, top_right2] = rectangle2;

	top_right1.x >= bottom_left2.x && top_right2.x >= bottom_left1.x && top_right2.y >= bottom_left1.y && top_right1.y >= bottom_left2.y
}

/// Returns the intersection of two lines. The lines are given by a point on the line and its slope (represented by a vector).
pub fn line_intersection(point1: DVec2, point1_slope_vector: DVec2, point2: DVec2, point2_slope_vector: DVec2) -> DVec2 {
	assert!(point1_slope_vector.normalize() != point2_slope_vector.normalize());

	// Find the intersection when the first line is vertical
	if f64_compare(point1_slope_vector.x, 0., MAX_ABSOLUTE_DIFFERENCE) {
		let m2 = point2_slope_vector.y / point2_slope_vector.x;
		let b2 = point2.y - m2 * point2.x;
		DVec2::new(point1.x, point1.x * m2 + b2)
	}
	// Find the intersection when the second line is vertical
	else if f64_compare(point2_slope_vector.x, 0., MAX_ABSOLUTE_DIFFERENCE) {
		let m1 = point1_slope_vector.y / point1_slope_vector.x;
		let b1 = point1.y - m1 * point1.x;
		DVec2::new(point2.x, point2.x * m1 + b1)
	}
	// Find the intersection where neither line is vertical
	else {
		let m1 = point1_slope_vector.y / point1_slope_vector.x;
		let b1 = point1.y - m1 * point1.x;
		let m2 = point2_slope_vector.y / point2_slope_vector.x;
		let b2 = point2.y - m2 * point2.x;
		let intersection_x = (b2 - b1) / (m1 - m2);
		DVec2::new(intersection_x, intersection_x * m1 + b1)
	}
}

/// Check if 3 points are collinear.
pub fn are_points_collinear(p1: DVec2, p2: DVec2, p3: DVec2) -> bool {
	let matrix = DMat2::from_cols(p1 - p2, p2 - p3);
	f64_compare(matrix.determinant() / 2., 0., MAX_ABSOLUTE_DIFFERENCE)
}

/// Compute the center of the circle that passes through all three provided points. The provided points cannot be collinear.
pub fn compute_circle_center_from_points(p1: DVec2, p2: DVec2, p3: DVec2) -> Option<DVec2> {
	if are_points_collinear(p1, p2, p3) {
		return None;
	}

	let midpoint_a = p1.lerp(p2, 0.5);
	let midpoint_b = p2.lerp(p3, 0.5);
	let midpoint_c = p3.lerp(p1, 0.5);

	let tangent_a = (p1 - p2).perp();
	let tangent_b = (p2 - p3).perp();
	let tangent_c = (p3 - p1).perp();

	let intersect_a_b = line_intersection(midpoint_a, tangent_a, midpoint_b, tangent_b);
	let intersect_b_c = line_intersection(midpoint_b, tangent_b, midpoint_c, tangent_c);
	let intersect_c_a = line_intersection(midpoint_c, tangent_c, midpoint_a, tangent_a);

	Some((intersect_a_b + intersect_b_c + intersect_c_a) / 3.)
}

/// Compare two `f64` numbers with a provided max absolute value difference.
pub fn f64_compare(a: f64, b: f64, max_abs_diff: f64) -> bool {
	(a - b).abs() < max_abs_diff
}

/// Determine if an `f64` number is within a given range by using a max absolute value difference comparison.
pub fn f64_approximately_in_range(value: f64, min: f64, max: f64, max_abs_diff: f64) -> bool {
	(min..=max).contains(&value) || f64_compare(value, min, max_abs_diff) || f64_compare(value, max, max_abs_diff)
}

/// Compare the two values in a `DVec2` independently with a provided max absolute value difference.
pub fn dvec2_compare(a: DVec2, b: DVec2, max_abs_diff: f64) -> BVec2 {
	BVec2::new((a.x - b.x).abs() < max_abs_diff, (a.y - b.y).abs() < max_abs_diff)
}

/// Determine if the values in a `DVec2` are within a given range independently by using a max absolute value difference comparison.
pub fn dvec2_approximately_in_range(point: DVec2, min_corner: DVec2, max_corner: DVec2, max_abs_diff: f64) -> BVec2 {
	(point.cmpge(min_corner) & point.cmple(max_corner)) | dvec2_compare(point, min_corner, max_abs_diff) | dvec2_compare(point, max_corner, max_abs_diff)
}

/// Calculate a new position for a point given its original position, a unit vector in the desired direction, and a distance to move it by.
pub fn scale_point_from_direction_vector(point: DVec2, direction_unit_vector: DVec2, should_flip_direction: bool, distance: f64) -> DVec2 {
	let should_reverse_factor = if should_flip_direction { -1. } else { 1. };
	point + distance * direction_unit_vector * should_reverse_factor
}

/// Scale a point by a given distance with respect to the provided origin.
pub fn scale_point_from_origin(point: DVec2, origin: DVec2, should_flip_direction: bool, distance: f64) -> DVec2 {
	scale_point_from_direction_vector(point, (origin - point).normalize(), should_flip_direction, distance)
}

/// Computes the necessary details to form a circular join from `left` to `right`, along a circle around `center`.
/// By default, the angle is assumed to be 180 degrees.
pub fn compute_circular_subpath_details<ManipulatorGroupId: crate::Identifier>(
	left: DVec2,
	arc_point: DVec2,
	right: DVec2,
	center: DVec2,
	angle: Option<f64>,
) -> (DVec2, ManipulatorGroup<ManipulatorGroupId>, DVec2) {
	let center_to_arc_point = arc_point - center;

	// Based on https://pomax.github.io/bezierinfo/#circles_cubic
	let handle_offset_factor = if let Some(angle) = angle { 4. / 3. * (angle / 4.).tan() } else { 0.551784777779014 };

	(
		left - (left - center).perp() * handle_offset_factor,
		ManipulatorGroup::new(
			arc_point,
			Some(arc_point + center_to_arc_point.perp() * handle_offset_factor),
			Some(arc_point - center_to_arc_point.perp() * handle_offset_factor),
		),
		right + (right - center).perp() * handle_offset_factor,
	)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::consts::MAX_ABSOLUTE_DIFFERENCE;

	/// Compare vectors of `f64`s with a provided max absolute value difference.
	fn f64_compare_vector(a: Vec<f64>, b: Vec<f64>, max_abs_diff: f64) -> bool {
		a.len() == b.len() && a.into_iter().zip(b).all(|(a, b)| f64_compare(a, b, max_abs_diff))
	}

	fn collect_roots(mut roots: [Option<f64>; 3]) -> Vec<f64> {
		roots.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
		roots.into_iter().flatten().collect()
	}

	#[test]
	fn test_solve_linear() {
		// Line that is on the x-axis
		assert!(collect_roots(solve_linear(0., 0.)).is_empty());
		// Line that is parallel to but not on the x-axis
		assert!(collect_roots(solve_linear(0., 1.)).is_empty());
		// Line with a non-zero slope
		assert!(collect_roots(solve_linear(2., -8.)) == vec![4.]);
	}

	#[test]
	fn test_solve_cubic() {
		// discriminant == 0
		let roots1 = collect_roots(solve_cubic(1., 0., 0., 0.));
		assert!(roots1 == vec![0.]);

		let roots2 = collect_roots(solve_cubic(1., 3., 0., -4.));
		assert!(roots2 == vec![-2., 1.]);

		// p == 0
		let roots3 = collect_roots(solve_cubic(1., 0., 0., -1.));
		assert!(roots3 == vec![1.]);

		// discriminant > 0
		let roots4 = collect_roots(solve_cubic(1., 3., 0., 2.));
		assert!(f64_compare_vector(roots4, vec![-3.196], MAX_ABSOLUTE_DIFFERENCE));

		// discriminant < 0
		let roots5 = collect_roots(solve_cubic(1., 3., 0., -1.));
		assert!(f64_compare_vector(roots5, vec![-2.879, -0.653, 0.532], MAX_ABSOLUTE_DIFFERENCE));

		// quadratic
		let roots6 = collect_roots(solve_cubic(0., 3., 0., -3.));
		assert!(roots6 == vec![-1., 1.]);

		// linear
		let roots7 = collect_roots(solve_cubic(0., 0., 1., -1.));
		assert!(roots7 == vec![1.]);
	}

	#[test]
	fn test_do_rectangles_overlap() {
		// Rectangles overlap
		assert!(do_rectangles_overlap([DVec2::new(0., 0.), DVec2::new(20., 20.)], [DVec2::new(10., 10.), DVec2::new(30., 20.)]));
		// Rectangles share a side
		assert!(do_rectangles_overlap([DVec2::new(0., 0.), DVec2::new(10., 10.)], [DVec2::new(10., 10.), DVec2::new(30., 30.)]));
		// Rectangle inside the other
		assert!(do_rectangles_overlap([DVec2::new(0., 0.), DVec2::new(10., 10.)], [DVec2::new(2., 2.), DVec2::new(6., 4.)]));
		// No overlap, rectangles are beside each other
		assert!(!do_rectangles_overlap([DVec2::new(0., 0.), DVec2::new(10., 10.)], [DVec2::new(20., 0.), DVec2::new(30., 10.)]));
		// No overlap, rectangles are above and below each other
		assert!(!do_rectangles_overlap([DVec2::new(0., 0.), DVec2::new(10., 10.)], [DVec2::new(0., 20.), DVec2::new(20., 30.)]));
	}

	#[test]
	fn test_find_intersection() {
		// y = 2x + 10
		// y = 5x + 4
		// intersect at (2, 14)

		let start1 = DVec2::new(0., 10.);
		let end1 = DVec2::new(0., 4.);
		let start_direction1 = DVec2::new(1., 2.);
		let end_direction1 = DVec2::new(1., 5.);
		assert!(line_intersection(start1, start_direction1, end1, end_direction1) == DVec2::new(2., 14.));

		// y = x
		// y = -x + 8
		// intersect at (4, 4)

		let start2 = DVec2::new(0., 0.);
		let end2 = DVec2::new(8., 0.);
		let start_direction2 = DVec2::new(1., 1.);
		let end_direction2 = DVec2::new(1., -1.);
		assert!(line_intersection(start2, start_direction2, end2, end_direction2) == DVec2::new(4., 4.));
	}

	#[test]
	fn test_are_points_collinear() {
		assert!(are_points_collinear(DVec2::new(2., 4.), DVec2::new(6., 8.), DVec2::new(4., 6.)));
		assert!(!are_points_collinear(DVec2::new(1., 4.), DVec2::new(6., 8.), DVec2::new(4., 6.)));
	}

	#[test]
	fn test_compute_circle_center_from_points() {
		// 3/4 of unit circle
		let center1 = compute_circle_center_from_points(DVec2::new(0., 1.), DVec2::new(-1., 0.), DVec2::new(1., 0.));
		assert_eq!(center1.unwrap(), DVec2::new(0., 0.));
		// 1/4 of unit circle
		let center2 = compute_circle_center_from_points(DVec2::new(-1., 0.), DVec2::new(0., 1.), DVec2::new(1., 0.));
		assert_eq!(center2.unwrap(), DVec2::new(0., 0.));
	}
}
