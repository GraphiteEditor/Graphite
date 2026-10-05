use crate::curve::Curve;
use glam::{DMat2, DVec2};
use kurbo::{ParamCurve, ParamCurveDeriv};

/// How many starting guesses are tried along each curve, which together must land near every solution.
const SEEDS_PER_CURVE: usize = 12;
const MAX_ITERATIONS: usize = 30;

/// Every pair of parameters, one on each curve, at which a single straight line touches both curves. Sorted by the first parameter.
///
/// A curve lying along a line, straight or doubling back, touches only that line along its whole length, so it has no common tangents.
pub fn common_tangents(first: &Curve, second: &Curve) -> Vec<(f64, f64)> {
	if first.lies_along_a_line() || second.lies_along_a_line() {
		return Vec::new();
	}

	// Lengths are judged against the size of the curves, so the same shapes give the same answer at any scale
	let size = first.bounds().union(&second.bounds()).size().max_element();
	if !size.is_finite() || size <= 0. {
		return Vec::new();
	}

	let [first, second] = [Derivatives::of(first), Derivatives::of(second)];
	let mut found: Vec<(f64, f64)> = Vec::new();

	// O(seeds^2) Newton searches, each a handful of cheap evaluations
	for first_seed in 0..SEEDS_PER_CURVE {
		for second_seed in 0..SEEDS_PER_CURVE {
			let seed = DVec2::new(first_seed as f64 + 0.5, second_seed as f64 + 0.5) / SEEDS_PER_CURVE as f64;
			let Some(solution) = refine(&first, &second, seed, size) else { continue };

			let already_found = found.iter().any(|&(s, t)| (s - solution.x).abs() < 1e-6 && (t - solution.y).abs() < 1e-6);
			if !already_found {
				found.push((solution.x, solution.y));
			}
		}
	}

	found.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
	found
}

/// A curve as a cubic, with its first and second derivatives.
struct Derivatives {
	curve: kurbo::CubicBez,
	first: kurbo::QuadBez,
	second: kurbo::Line,
}

impl Derivatives {
	fn of(curve: &Curve) -> Self {
		let curve = curve.segment().to_cubic();
		let first = curve.deriv();
		let second = first.deriv();
		Self { curve, first, second }
	}

	fn at(&self, t: f64) -> [DVec2; 3] {
		[self.curve.eval(t), self.first.eval(t), self.second.eval(t)].map(|point| DVec2::new(point.x, point.y))
	}
}

/// Newton's method on the two conditions for a common tangent: each curve's direction of travel runs along the line joining the two touch points.
fn refine(first: &Derivatives, second: &Derivatives, seed: DVec2, size: f64) -> Option<DVec2> {
	let mut parameters = seed;

	for _ in 0..MAX_ITERATIONS {
		let [first_position, first_velocity, first_acceleration] = first.at(parameters.x);
		let [second_position, second_velocity, second_acceleration] = second.at(parameters.y);
		let between = second_position - first_position;

		let residual = DVec2::new(first_velocity.perp_dot(between), second_velocity.perp_dot(between));
		let jacobian = DMat2::from_cols(
			DVec2::new(first_acceleration.perp_dot(between), first_velocity.perp_dot(second_velocity)),
			DVec2::new(first_velocity.perp_dot(second_velocity), second_acceleration.perp_dot(between)),
		);

		// Touch points that coincide, or a curve at rest, satisfy the conditions without there being any line to speak of
		let scale = between.length();
		let [first_speed, second_speed] = [first_velocity.length(), second_velocity.length()];
		if scale < 1e-9 * size || first_speed < 1e-12 * size || second_speed < 1e-12 * size {
			return None;
		}

		let converged = residual.x.abs() <= 1e-10 * first_speed * scale && residual.y.abs() <= 1e-10 * second_speed * scale;
		if converged {
			let within = |t: f64| (-1e-9..=1. + 1e-9).contains(&t);
			return (within(parameters.x) && within(parameters.y)).then(|| parameters.clamp(DVec2::ZERO, DVec2::ONE));
		}

		if jacobian.determinant().abs() < 1e-300 {
			return None;
		}
		parameters -= jacobian.inverse() * residual;

		// A search that wanders well outside both curves is not going to come back to a solution on them
		if !parameters.is_finite() || parameters.min_element() < -0.5 || parameters.max_element() > 1.5 {
			return None;
		}
	}

	None
}

#[cfg(test)]
mod tests {
	use super::*;

	/// The upper half of a circle approximated by one cubic, running from its left end to its right.
	fn dome(center: DVec2, radius: f64) -> Curve {
		// This handle length gives the closest single cubic to a half circle
		let handle = radius * 4. / 3.;
		Curve::cubic(
			center + DVec2::new(-radius, 0.),
			center + DVec2::new(-radius, handle),
			center + DVec2::new(radius, handle),
			center + DVec2::new(radius, 0.),
		)
	}

	fn assert_is_common_tangent(first: &Curve, second: &Curve, (s, t): (f64, f64)) {
		let between = (second.evaluate(t) - first.evaluate(s)).normalize();
		assert!(first.tangent(s).unwrap().perp_dot(between).abs() < 1e-9);
		assert!(second.tangent(t).unwrap().perp_dot(between).abs() < 1e-9);
	}

	#[test]
	fn two_humps_side_by_side_share_one_line_resting_across_their_tops() {
		let [left, right] = [dome(DVec2::ZERO, 10.), dome(DVec2::new(40., 0.), 10.)];

		let tangents = common_tangents(&left, &right);

		assert_eq!(tangents.len(), 1);
		assert_is_common_tangent(&left, &right, tangents[0]);

		// Equal humps on level ground are touched at their peaks, in the middle of each curve
		assert!((tangents[0].0 - 0.5).abs() < 1e-9 && (tangents[0].1 - 0.5).abs() < 1e-9);
	}

	#[test]
	fn humps_of_different_heights_share_a_sloping_line() {
		let [low, high] = [dome(DVec2::ZERO, 6.), dome(DVec2::new(40., 0.), 14.)];

		let tangents = common_tangents(&low, &high);

		assert_eq!(tangents.len(), 1);
		assert_is_common_tangent(&low, &high, tangents[0]);

		// The line climbs toward the taller hump, so it touches each on the side facing away from the other's peak
		let touch = [low.evaluate(tangents[0].0), high.evaluate(tangents[0].1)];
		assert!(touch[1].y > touch[0].y);
		assert!(tangents[0].0 < 0.5 && tangents[0].1 < 0.5);
	}

	#[test]
	fn the_same_shapes_share_the_same_line_at_any_scale() {
		let at_scale = |scale: f64| common_tangents(&dome(DVec2::ZERO, 6. * scale), &dome(DVec2::new(40. * scale, 0.), 14. * scale));
		let expected = at_scale(1.);
		assert_eq!(expected.len(), 1);

		for scale in [1e-9, 1e-4, 1e5, 1e12] {
			let tangents = at_scale(scale);
			assert_eq!(tangents.len(), 1, "at a scale of {scale}");
			assert!((tangents[0].0 - expected[0].0).abs() < 1e-9 && (tangents[0].1 - expected[0].1).abs() < 1e-9);
		}
	}

	#[test]
	fn a_hump_and_a_hollow_share_the_line_that_crosses_between_them() {
		let hump = dome(DVec2::ZERO, 10.);
		let hollow = Curve::cubic(DVec2::new(30., 0.), DVec2::new(30., -40. / 3.), DVec2::new(50., -40. / 3.), DVec2::new(50., 0.));

		let tangents = common_tangents(&hump, &hollow);

		assert_eq!(tangents.len(), 1);
		assert_is_common_tangent(&hump, &hollow, tangents[0]);
	}

	#[test]
	fn straight_curves_and_curves_that_share_no_line_give_nothing() {
		let hump = dome(DVec2::ZERO, 10.);
		let straight = Curve::line(DVec2::new(0., 30.), DVec2::new(50., 30.));
		assert!(common_tangents(&hump, &straight).is_empty());
		assert!(common_tangents(&straight, &hump).is_empty());

		// A small hump tucked under a large one cannot be touched by any line that also touches the large one
		let inner = dome(DVec2::ZERO, 2.);
		assert!(common_tangents(&hump, &inner).is_empty());
	}

	#[test]
	fn curves_doubling_back_along_a_line_give_nothing() {
		let doubling_back = |y: f64, x: f64| Curve::cubic(DVec2::new(x, y), DVec2::new(x + 5., y), DVec2::new(x + 2., y), DVec2::new(x + 3., y));
		assert!(common_tangents(&doubling_back(0., 0.), &doubling_back(0., 10.)).is_empty());

		// The line touches the hump's peak, yet the curve lying along it touches it everywhere rather than at one point
		let [along_the_peak, hump] = [doubling_back(10., 0.), dome(DVec2::new(40., 0.), 10.)];
		assert!(common_tangents(&along_the_peak, &hump).is_empty());
		assert!(common_tangents(&hump, &along_the_peak).is_empty());
	}
}
