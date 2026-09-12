use crate::list::{Item, List};
use dyn_any::DynAny;
use glam::DVec2;

/// A mapping from an input to output value, drawn as a smooth spline through control points in any x order,
/// which sampling sorts, and held flat beyond the outermost ones. Two points give a straight line and none the identity.
#[derive(Debug, Clone, PartialEq, DynAny, graphene_hash::CacheHash)]
pub struct TransferCurve(pub List<DVec2>);

impl Default for TransferCurve {
	/// The straight line from (0, 0) to (1, 1).
	fn default() -> Self {
		Self::new(vec![DVec2::ZERO, DVec2::ONE])
	}
}

impl TransferCurve {
	/// Builds a curve from points in any order.
	pub fn new(mut points: Vec<DVec2>) -> Self {
		points.sort_by(|a, b| a.x.total_cmp(&b.x));
		Self::from(points)
	}

	/// The control points in the order they are stored, which a drag may carry out of x order.
	pub fn points(&self) -> &[DVec2] {
		self.0.iter_element_values().as_slice()
	}

	/// Whether every control point sits on the y=x diagonal, so the curve leaves the values between them unchanged.
	pub fn is_identity(&self) -> bool {
		self.points().iter().all(|point| point.x == point.y)
	}

	/// Adds a point ahead of the first one to its right, and returns its index.
	pub fn insert_point(&mut self, point: DVec2) -> usize {
		let index = self.points().iter().position(|existing| existing.x > point.x).unwrap_or(self.0.len());

		// The list has no insert of its own, so the points are laid out fresh around the new one
		let mut points = self.points().to_vec();
		points.insert(index, point);
		self.0 = points.into_iter().map(Item::new_from_element).collect();

		index
	}

	pub fn remove_point(&mut self, index: usize) {
		if index >= self.0.len() {
			return;
		}

		let mut points = self.points().to_vec();
		points.remove(index);
		self.0 = points.into_iter().map(Item::new_from_element).collect();
	}

	/// Moves a point, which may carry it past others into a new place along the curve while it keeps its index.
	pub fn move_point(&mut self, index: usize, point: DVec2) {
		let Some(existing) = self.0.element_mut(index) else { return };
		*existing = point;
	}

	/// Prepares the curve for repeated sampling: the spline through the points is solved once here rather than
	/// on every [`TransferCurveEvaluator::evaluate`] call.
	pub fn evaluator(&self) -> TransferCurveEvaluator {
		TransferCurveEvaluator::new(self.points())
	}

	/// Samples the curve at `x`. Looping over many values should be done by holding a [`TransferCurve::evaluator`] instead.
	pub fn evaluate(&self, x: f64) -> f64 {
		self.evaluator().evaluate(x)
	}
}

impl From<Vec<DVec2>> for TransferCurve {
	fn from(points: Vec<DVec2>) -> Self {
		Self(points.into_iter().map(Item::new_from_element).collect())
	}
}

impl From<List<DVec2>> for TransferCurve {
	fn from(points: List<DVec2>) -> Self {
		Self(points)
	}
}

/// A curve prepared for repeated sampling by [`TransferCurve::evaluator`]:
/// a natural cubic spline through the points, whose second derivative vanishes at both ends.
#[derive(Debug, Clone)]
pub struct TransferCurveEvaluator {
	points: Vec<DVec2>,
	second_derivatives: Vec<f64>,
}

impl TransferCurveEvaluator {
	fn new(points: &[DVec2]) -> Self {
		let mut points = points.to_vec();
		points.sort_by(|a, b| a.x.total_cmp(&b.x));

		// Points sharing an x would make the spline's system singular, so the later-stored one at each x stands alone
		points.reverse();
		points.dedup_by(|a, b| a.x == b.x);
		points.reverse();

		let second_derivatives = natural_spline_second_derivatives(&points);
		Self { points, second_derivatives }
	}

	/// Samples the curve at `x`, holding the outermost points' values beyond them.
	pub fn evaluate(&self, x: f64) -> f64 {
		let points = &self.points;
		match points.len() {
			0 => return x,
			1 => return points[0].y,
			_ => {}
		}
		if x <= points[0].x {
			return points[0].y;
		}
		if x >= points[points.len() - 1].x {
			return points[points.len() - 1].y;
		}

		// O(log n) search for the segment holding x
		let upper = points.partition_point(|point| point.x <= x).min(points.len() - 1);
		let lower = upper - 1;
		let (a, b) = (points[lower], points[upper]);
		let width = (b.x - a.x).max(f64::EPSILON);

		// The cubic segment from its two end second derivatives
		let t_b = (x - a.x) / width;
		let t_a = 1. - t_b;
		let (m_a, m_b) = (self.second_derivatives[lower], self.second_derivatives[upper]);
		t_a * a.y + t_b * b.y + ((t_a * t_a * t_a - t_a) * m_a + (t_b * t_b * t_b - t_b) * m_b) * width * width / 6.
	}
}

/// Second derivatives of the natural cubic spline through sorted `points`, solved by the tridiagonal (Thomas) algorithm in O(n).
fn natural_spline_second_derivatives(points: &[DVec2]) -> Vec<f64> {
	let n = points.len();
	let mut second_derivatives = vec![0.; n];
	if n < 3 {
		return second_derivatives;
	}

	let width = |i: usize| (points[i + 1].x - points[i].x).max(f64::EPSILON);
	let slope = |i: usize| (points[i + 1].y - points[i].y) / width(i);

	// Forward sweep over the interior rows, whose diagonal is 2(h[i-1] + h[i]) with off-diagonals h[i-1] and h[i]
	let mut scratch = vec![0.; n];
	for i in 1..n - 1 {
		let (h_previous, h_next) = (width(i - 1), width(i));
		let denominator = 2. * (h_previous + h_next) - h_previous * scratch[i - 1];
		scratch[i] = h_next / denominator;
		second_derivatives[i] = (6. * (slope(i) - slope(i - 1)) - h_previous * second_derivatives[i - 1]) / denominator;
	}

	// Back substitution, with the natural end conditions leaving both ends at zero
	for i in (1..n - 1).rev() {
		second_derivatives[i] -= scratch[i] * second_derivatives[i + 1];
	}

	second_derivatives
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn identity_and_lines() {
		let identity = TransferCurve::default();
		assert!(identity.is_identity());
		assert!((identity.evaluate(0.3) - 0.3).abs() < 1e-12);

		let line = TransferCurve::new(vec![DVec2::new(1., 0.), DVec2::new(0., 1.)]);
		assert!((line.evaluate(0.25) - 0.75).abs() < 1e-12);
		assert_eq!(line.evaluate(-1.), 1.);
		assert_eq!(line.evaluate(2.), 0.);
	}

	#[test]
	fn spline_passes_through_points_and_stays_smooth() {
		let curve = TransferCurve::new(vec![DVec2::ZERO, DVec2::new(0.25, 0.5), DVec2::new(0.75, 0.6), DVec2::ONE]);
		let evaluator = curve.evaluator();
		for point in curve.points() {
			assert!((evaluator.evaluate(point.x) - point.y).abs() < 1e-12);
		}

		// The first derivative is continuous across the interior points
		let step = 1e-6;
		for point in &curve.points()[1..3] {
			let before = (evaluator.evaluate(point.x) - evaluator.evaluate(point.x - step)) / step;
			let after = (evaluator.evaluate(point.x + step) - evaluator.evaluate(point.x)) / step;
			assert!((before - after).abs() < 1e-3, "kink at {}: {before} vs {after}", point.x);
		}
	}

	#[test]
	fn points_sharing_an_x_leave_the_later_one_standing() {
		let curve = TransferCurve::from(vec![DVec2::ZERO, DVec2::new(0.5, 0.2), DVec2::new(0.5, 0.8), DVec2::ONE]);
		assert!((curve.evaluate(0.5) - 0.8).abs() < 1e-12);

		// A singular system would send the neighboring segments off to enormous values
		for x in [0.1, 0.25, 0.4, 0.6, 0.75, 0.9] {
			assert!(curve.evaluate(x).abs() < 2., "runaway value {} at {x}", curve.evaluate(x));
		}
	}

	#[test]
	fn a_moved_point_may_pass_another_while_keeping_its_index() {
		let mut curve = TransferCurve::default();
		assert_eq!(curve.insert_point(DVec2::new(0.5, 0.7)), 1);

		// Carried past the point that was to its right, it stays at its own index and sampling sorts it into its new place
		curve.move_point(1, DVec2::new(1.5, 0.2));
		assert_eq!(curve.points()[1], DVec2::new(1.5, 0.2));
		assert_eq!(curve.evaluate(2.), 0.2);

		curve.remove_point(1);
		assert!(curve.is_identity());
	}
}
