use kurbo::{BezPath, CubicBez, ParamCurve, PathEl, PathSeg, QuadBez, Vec2};
use std::f64;

use crate::math::polynomial::pathseg_to_parametric_polynomial;

use super::{contants::MAX_ABSOLUTE_DIFFERENCE, intersection::bezpath_all_self_intersections};

/// Return the approximation of the length centroid, together with the length, of the `BezPath`.
///
/// The length centroid is the center of mass for the arc length of the solid shape's perimeter.
/// An infinitely thin wire forming the subpath's closed shape would balance at this point.
///
/// It will return `None` if Bezpath has no segments.
/// - `accuracy` is used to approximate the curve.
/// - `always_closed` to consider the BezPath as closed always.
pub fn bezpath_length_centroid_and_length(mut bezpath: BezPath, accuracy: Option<f64>, always_closed: bool) -> Option<(Vec2, f64)> {
	// TODO: Take the Bezpath as a reference instead of value to avoid allocation. Presently we do it so we can close the path.

	if !bezpath.elements().last().is_some_and(|element| *element == PathEl::ClosePath) && always_closed {
		bezpath.close_path();
	}

	bezpath
		.segments()
		.map(|segment| pathseg_length_centroid_and_length(segment, accuracy))
		.map(|(centroid, length)| (centroid * length, length))
		.reduce(|(centroid_part1, length1), (centroid_part2, length2)| (centroid_part1 + centroid_part2, length1 + length2))
		.map(|(centroid_part, length)| (centroid_part / length, length))
}

/// Return an approximation of the length centroid, together with the length, of the bezier curve.
///
/// The length centroid is the center of mass for the arc length of the Bezier segment.
/// An infinitely thin wire forming the Bezier segment's shape would balance at this point.
///
/// - `accuracy` is used to approximate the curve.
fn pathseg_length_centroid_and_length(segment: PathSeg, accuracy: Option<f64>) -> (Vec2, f64) {
	match segment {
		PathSeg::Line(line) => ((line.start().to_vec2() + line.end().to_vec2()) / 2., (line.start().to_vec2() - line.end().to_vec2()).length()),
		PathSeg::Quad(quad_bez) => {
			let QuadBez { p0, p1, p2 } = quad_bez;
			// Use Casteljau subdivision, noting that the length is more than the straight line distance from start to end but less than the straight line distance through the handles
			fn recurse(a0: Vec2, a1: Vec2, a2: Vec2, accuracy: f64, level: u8) -> (f64, Vec2) {
				let lower = (a2 - a1).length();
				let upper = (a1 - a0).length() + (a2 - a1).length();
				if upper - lower <= 2. * accuracy || level >= 8 {
					let length = (lower + upper) / 2.;
					return (length, length * (a0 + a1 + a2) / 3.);
				}

				let b1 = 0.5 * (a0 + a1);
				let c1 = 0.5 * (a1 + a2);
				let b2 = 0.5 * (b1 + c1);

				let (length1, centroid_part1) = recurse(a0, b1, b2, 0.5 * accuracy, level + 1);
				let (length2, centroid_part2) = recurse(b2, c1, a2, 0.5 * accuracy, level + 1);
				(length1 + length2, centroid_part1 + centroid_part2)
			}

			let (length, centroid_parts) = recurse(p0.to_vec2(), p1.to_vec2(), p2.to_vec2(), accuracy.unwrap_or_default(), 0);
			(centroid_parts / length, length)
		}
		PathSeg::Cubic(cubic_bez) => {
			let CubicBez { p0, p1, p2, p3 } = cubic_bez;

			// Use Casteljau subdivision, noting that the length is more than the straight line distance from start to end but less than the straight line distance through the handles
			fn recurse(a0: Vec2, a1: Vec2, a2: Vec2, a3: Vec2, accuracy: f64, level: u8) -> (f64, Vec2) {
				let lower = (a3 - a0).length();
				let upper = (a1 - a0).length() + (a2 - a1).length() + (a3 - a2).length();
				if upper - lower <= 2. * accuracy || level >= 8 {
					let length = (lower + upper) / 2.;
					return (length, length * (a0 + a1 + a2 + a3) / 4.);
				}

				let b1 = 0.5 * (a0 + a1);
				let t0 = 0.5 * (a1 + a2);
				let c1 = 0.5 * (a2 + a3);
				let b2 = 0.5 * (b1 + t0);
				let c2 = 0.5 * (t0 + c1);
				let b3 = 0.5 * (b2 + c2);

				let (length1, centroid_part1) = recurse(a0, b1, b2, b3, 0.5 * accuracy, level + 1);
				let (length2, centroid_part2) = recurse(b3, c2, c1, a3, 0.5 * accuracy, level + 1);
				(length1 + length2, centroid_part1 + centroid_part2)
			}

			let (length, centroid_parts) = recurse(p0.to_vec2(), p1.to_vec2(), p2.to_vec2(), p3.to_vec2(), accuracy.unwrap_or_default(), 0);
			(centroid_parts / length, length)
		}
	}
}

/// Return the area centroid, together with the area, of the `BezPath` always considering it as a closed subpath. The area will always be a positive value.
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
pub fn bezpath_area_centroid_and_area(mut bezpath: BezPath, error: Option<f64>, minimum_separation: Option<f64>) -> Option<(Vec2, f64)> {
	let all_intersections = bezpath_all_self_intersections(bezpath.clone(), error, minimum_separation);
	let mut current_sign: f64 = 1.;

	bezpath.close_path();

	let (x_sum, y_sum, area) = bezpath
		.segments()
		.enumerate()
		.map(|(index, segment)| {
			let (f_x, f_y) = pathseg_to_parametric_polynomial(segment);
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
		return Some((Vec2::new(f64::NAN, f64::NAN), 0.));
	}

	Some((Vec2::new(x_sum / area, y_sum / area), area.abs()))
}
