use crate::bounds::Bounds;
use glam::DVec2;
use kurbo::{ParamCurve, ParamCurveDeriv, ParamCurveExtrema, PathSeg};
use std::ops::{Add, Mul, Sub};

/// Distances below this fraction of the size of the geometry involved count as zero.
const COINCIDENCE: f64 = 1e-9;

/// Polynomial coefficients, and polynomial values over [0, 1], below this fraction of the largest coefficient count as zero.
const NEGLIGIBLE: f64 = 1e-12;

/// Roots this far outside [0, 1] are rounding error at an end, so they count as that end.
const PARAMETER_SLACK: f64 = 1e-9;

/// Roots closer than this in t are one root.
const ROOT_SEPARATION: f64 = 1e-9;

/// Intersections closer than this in both parameters are one intersection.
const INTERSECTION_SEPARATION: f64 = 1e-6;

/// Subdivision stops at pieces with a bounding box diagonal below this fraction of the two curves' combined one.
const LEAF_SIZE: f64 = 1e-6;

const MAX_SUBDIVISION_DEPTH: usize = 40;

/// More overlapping pairs of pieces than this at once means the curves coincide along a stretch.
const MAX_LIVE_PAIRS: usize = 4096;

const MAX_ROOT_REFINEMENT_STEPS: usize = 100;

const MAX_NEWTON_STEPS: usize = 50;

/// How far outside [0, 1] Newton iteration may wander before it is considered lost.
const NEWTON_MARGIN: f64 = 1e-3;

/// Enough coefficients for the highest-degree condition, the squared distance of a cubic from a point.
const COEFFICIENT_COUNT: usize = 7;

/// One piece of a path that a point can slide along and that other geometry can meet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Curve {
	segment: PathSeg,
}

/// The point of a curve closest to another point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NearestOnCurve {
	pub t: f64,
	pub position: DVec2,
	pub distance: f64,
}

impl Curve {
	pub fn new(segment: PathSeg) -> Self {
		Self { segment }
	}

	pub fn line(start: DVec2, end: DVec2) -> Self {
		Self::new(PathSeg::Line(kurbo::Line::new(to_point(start), to_point(end))))
	}

	pub fn quadratic(start: DVec2, handle: DVec2, end: DVec2) -> Self {
		Self::new(PathSeg::Quad(kurbo::QuadBez::new(to_point(start), to_point(handle), to_point(end))))
	}

	pub fn cubic(start: DVec2, handle_start: DVec2, handle_end: DVec2, end: DVec2) -> Self {
		Self::new(PathSeg::Cubic(kurbo::CubicBez::new(to_point(start), to_point(handle_start), to_point(handle_end), to_point(end))))
	}

	pub fn segment(&self) -> PathSeg {
		self.segment
	}

	pub fn start(&self) -> DVec2 {
		to_dvec2(self.segment.start())
	}

	pub fn end(&self) -> DVec2 {
		to_dvec2(self.segment.end())
	}

	pub fn evaluate(&self, t: f64) -> DVec2 {
		to_dvec2(self.segment.eval(t))
	}

	/// First derivative with respect to t (not normalized).
	pub fn derivative(&self, t: f64) -> DVec2 {
		let derivative = match self.segment {
			PathSeg::Line(line) => line.deriv().eval(t),
			PathSeg::Quad(quad) => quad.deriv().eval(t),
			PathSeg::Cubic(cubic) => cubic.deriv().eval(t),
		};
		to_dvec2(derivative)
	}

	/// Unit tangent direction at t, which is the limiting direction of travel where the derivative vanishes, as at a retracted handle or a cusp.
	/// `None` if `t` or the curve is not finite, or the whole curve is a single point.
	pub fn tangent(&self, t: f64) -> Option<DVec2> {
		if !t.is_finite() || !self.is_finite() || self.is_single_point() {
			return None;
		}

		// Where B' vanishes, travel follows the first higher derivative that does not, with B'' reversed when arriving at the end
		let [_, _, quadratic, cubic] = self.power_coefficients();
		let first = self.derivative(t);
		let second = 2. * quadratic + 6. * cubic * t;
		let third = 6. * cubic;
		let leaving = if t < 1. { 1. } else { -1. };

		let vanishing = COINCIDENCE * self.size();
		let direction = [first, second * leaving, third].into_iter().find(|derivative| derivative.length() > vanishing).unwrap_or(first);
		direction.try_normalize()
	}

	/// Tight bounding box, or [`Bounds::EMPTY`] if the curve has non-finite coordinates.
	pub fn bounds(&self) -> Bounds {
		if !self.is_finite() {
			return Bounds::EMPTY;
		}

		Bounds::from_rect(self.segment.bounding_box())
	}

	/// Whether the curve is a straight segment traversed without doubling back, with its handles on the chord and in order along it.
	/// A zero-length curve is not straight.
	pub fn is_straight(&self) -> bool {
		let [start, first_handle, second_handle, end] = self.control_points();
		let chord = end - start;
		let chord_length = chord.length();
		if !self.is_finite() || chord_length == 0. {
			return false;
		}

		let mut previous_fraction = 0.;
		for handle in [first_handle, second_handle] {
			let offset = handle - start;
			let fraction = offset.dot(chord) / (chord_length * chord_length);
			let distance_from_chord = offset.perp_dot(chord).abs() / chord_length;

			let out_of_order = fraction < previous_fraction - COINCIDENCE || fraction > 1. + COINCIDENCE;
			if distance_from_chord > COINCIDENCE * chord_length || out_of_order {
				return false;
			}

			previous_fraction = fraction.max(previous_fraction);
		}

		true
	}

	/// Whether the whole curve lies along one line, straight or doubling back. A curve that is a single point does not.
	pub fn lies_along_a_line(&self) -> bool {
		common_line(&self.control_points(), COINCIDENCE * self.size()).is_some()
	}

	/// The closest point of the curve to `point`, chosen exactly among the ends and every foot of a perpendicular. Ties go to the smallest t.
	pub fn nearest(&self, point: DVec2) -> NearestOnCurve {
		let at = |t: f64| {
			let position = self.evaluate(t);
			NearestOnCurve {
				t,
				position,
				distance: position.distance(point),
			}
		};

		if !point.is_finite() || !self.is_finite() {
			return NearestOnCurve { distance: f64::INFINITY, ..at(0.) };
		}

		// Feet where the derivative vanishes stay in, since the nearest point can be the tip of a cusp
		let feet = roots_in_unit_interval(self.perpendicular_polynomial(point));

		// Candidates come in increasing t, and the first of several equally near ones wins
		let candidates = std::iter::once(0.).chain(feet).chain(std::iter::once(1.));
		candidates.map(at).min_by(|a, b| a.distance.total_cmp(&b.distance)).unwrap_or_else(|| at(0.))
	}

	/// Every t where the line from `point` meets the curve at a right angle, sorted ascending, excluding where the derivative vanishes.
	pub fn perpendicular_parameters_from(&self, point: DVec2) -> Vec<f64> {
		if !point.is_finite() || !self.is_finite() {
			return Vec::new();
		}

		let vanishing = COINCIDENCE * self.size();
		let mut parameters = roots_in_unit_interval(self.perpendicular_polynomial(point));
		parameters.retain(|&t| self.derivative(t).length() > vanishing);
		parameters
	}

	/// Every t where the line from `point` touches the curve tangentially, sorted ascending, excluding where the curve passes through `point` or its derivative vanishes.
	/// Empty for straight curves.
	pub fn tangent_parameters_from(&self, point: DVec2) -> Vec<f64> {
		if !point.is_finite() || !self.is_finite() {
			return Vec::new();
		}

		// A curve lying along a line, straight or doubling back, has no isolated points of tangency
		if self.lies_along_a_line() {
			return Vec::new();
		}

		let negligible = COINCIDENCE * self.size();
		let mut parameters = roots_in_unit_interval(self.tangent_polynomial(point));
		parameters.retain(|&t| self.evaluate(t).distance(point) > negligible && self.derivative(t).length() > negligible);
		parameters
	}

	/// Every t where the curve meets the infinite line through `point` along `direction`, sorted ascending.
	/// Empty if `direction` is zero or the curve lies along the line.
	pub fn line_intersections(&self, point: DVec2, direction: DVec2) -> Vec<f64> {
		let Some(direction) = direction.try_normalize() else { return Vec::new() };
		if !point.is_finite() || !self.is_finite() {
			return Vec::new();
		}

		// A curve lying along the line meets it everywhere rather than at isolated points
		let tolerance = COINCIDENCE * self.size();
		if self.control_points().iter().all(|control_point| (*control_point - point).perp_dot(direction).abs() <= tolerance) {
			return Vec::new();
		}

		let (x, y) = self.coordinate_polynomials(point);
		roots_in_unit_interval(x * direction.y - y * direction.x)
	}

	/// Every t where the curve meets the circle, sorted ascending. Empty if `radius` is not positive and finite.
	pub fn circle_intersections(&self, center: DVec2, radius: f64) -> Vec<f64> {
		if !radius.is_finite() || radius <= 0. || !center.is_finite() || !self.is_finite() {
			return Vec::new();
		}

		let (x, y) = self.coordinate_polynomials(center);
		let mut polynomial = x * x + y * y;

		// Factoring the constant term avoids cancellation between a squared distance and squared radius that are both large
		let start_distance = self.start().distance(center);
		polynomial.0[0] = (start_distance - radius) * (start_distance + radius);

		roots_in_unit_interval(polynomial)
	}

	/// Every pair of parameters (t on this curve, t on `other`) where the curves meet, sorted by the first.
	/// Coincident stretches contribute nothing, though isolated shared endpoints count.
	pub fn curve_intersections(&self, other: &Curve) -> Vec<(f64, f64)> {
		if !self.is_finite() || !other.is_finite() || self.is_single_point() || other.is_single_point() {
			return Vec::new();
		}

		// A curve coincides with itself, or itself traced backward, everywhere
		if self.segment == other.segment || self.segment == other.segment.reverse() {
			return Vec::new();
		}

		let (bounds, other_bounds) = (self.bounds(), other.bounds());
		let diagonal = bounds.union(&other_bounds).size().length();
		let tolerance = COINCIDENCE * diagonal;
		if !tolerance.is_finite() || !bounds.inflated(tolerance).intersects(&other_bounds) {
			return Vec::new();
		}

		let mut intersections = self.collinear_intersections(other, tolerance).unwrap_or_else(|| match (self.is_straight(), other.is_straight()) {
			(true, true) => self.straight_intersection(other, tolerance).into_iter().collect(),
			(true, false) => other.straight_crossings(self, tolerance).into_iter().map(|(u, t)| (t, u)).collect(),
			(false, true) => self.straight_crossings(other, tolerance),
			(false, false) => self.subdivision_intersections(other, diagonal),
		});

		intersections.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
		intersections
	}

	fn is_finite(&self) -> bool {
		self.segment.is_finite()
	}

	fn is_single_point(&self) -> bool {
		let control_points = self.control_points();
		control_points.iter().all(|control_point| *control_point == control_points[0])
	}

	/// The bounding box diagonal, which scales the tolerances.
	fn size(&self) -> f64 {
		self.bounds().size().length()
	}

	/// The control points, with the end repeated in place of any handles a line or quadratic lacks.
	fn control_points(&self) -> [DVec2; 4] {
		let control_points = match self.segment {
			PathSeg::Line(line) => [line.p0, line.p1, line.p1, line.p1],
			PathSeg::Quad(quad) => [quad.p0, quad.p1, quad.p2, quad.p2],
			PathSeg::Cubic(cubic) => [cubic.p0, cubic.p1, cubic.p2, cubic.p3],
		};
		control_points.map(to_dvec2)
	}

	/// The power basis coefficients of B(t) from the constant term upward, built from differences of control points to keep their precision.
	fn power_coefficients(&self) -> [DVec2; 4] {
		match self.segment {
			PathSeg::Line(line) => {
				let [p0, p1] = [line.p0, line.p1].map(to_dvec2);
				[p0, p1 - p0, DVec2::ZERO, DVec2::ZERO]
			}
			PathSeg::Quad(quad) => {
				let [p0, p1, p2] = [quad.p0, quad.p1, quad.p2].map(to_dvec2);
				[p0, 2. * (p1 - p0), (p2 - p1) - (p1 - p0), DVec2::ZERO]
			}
			PathSeg::Cubic(cubic) => {
				let [p0, p1, p2, p3] = [cubic.p0, cubic.p1, cubic.p2, cubic.p3].map(to_dvec2);
				[p0, 3. * (p1 - p0), 3. * ((p2 - p1) - (p1 - p0)), (p3 - p0) - 3. * (p2 - p1)]
			}
		}
	}

	/// The x and y polynomials of B(t) - origin.
	fn coordinate_polynomials(&self, origin: DVec2) -> (Polynomial, Polynomial) {
		let [constant, linear, quadratic, cubic] = self.power_coefficients();
		let constant = constant - origin;

		(
			Polynomial([constant.x, linear.x, quadratic.x, cubic.x, 0., 0., 0.]),
			Polynomial([constant.y, linear.y, quadratic.y, cubic.y, 0., 0., 0.]),
		)
	}

	/// The dot product of B(t) - point with B'(t), which is zero at the feet of perpendiculars from the point.
	fn perpendicular_polynomial(&self, point: DVec2) -> Polynomial {
		let (x, y) = self.coordinate_polynomials(point);
		x * x.derivative() + y * y.derivative()
	}

	/// The cross product of B(t) - point with B'(t), which is zero where lines from the point touch the curve.
	fn tangent_polynomial(&self, point: DVec2) -> Polynomial {
		let (x, y) = self.coordinate_polynomials(point);
		x * y.derivative() - y * x.derivative()
	}

	/// For curves lying along one shared line, the single point where they touch, or nothing if they overlap along a stretch or lie apart.
	/// `None` if the curves do not share a line.
	fn collinear_intersections(&self, other: &Curve, tolerance: f64) -> Option<Vec<(f64, f64)>> {
		let (origin, direction) = common_line(&[self.control_points(), other.control_points()].concat(), tolerance)?;

		// Each curve covers an interval of the line, measured from its tight bounds because handles can overshoot the curve
		let extent = |curve: &Curve| {
			let Bounds { min, max } = curve.bounds();
			let corners = [min, max, DVec2::new(min.x, max.y), DVec2::new(max.x, min.y)];
			corners
				.map(|corner| (corner - origin).dot(direction))
				.into_iter()
				.fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), along| (low.min(along), high.max(along)))
		};
		let (start, end) = extent(self);
		let (other_start, other_end) = extent(other);

		let overlap_start = start.max(other_start);
		let overlap_end = end.min(other_end);
		if (overlap_end - overlap_start).abs() > tolerance {
			return Some(Vec::new());
		}

		let touching = origin + direction * (overlap_start + overlap_end) / 2.;
		Some(vec![(self.nearest(touching).t, other.nearest(touching).t)])
	}

	/// Where two straight curves cross, found from their chords.
	fn straight_intersection(&self, other: &Curve, tolerance: f64) -> Option<(f64, f64)> {
		let chord = self.end() - self.start();
		let other_chord = other.end() - other.start();

		let fraction = (other.start() - self.start()).perp_dot(other_chord) / chord.perp_dot(other_chord);
		if !fraction.is_finite() {
			return None;
		}
		let crossing = self.start() + chord * fraction;

		// A straight curve with handles need not move uniformly along its chord, so parameters come from the crossing point
		let nearest = self.nearest(crossing);
		let other_nearest = other.nearest(crossing);
		(nearest.distance <= tolerance && other_nearest.distance <= tolerance).then_some((nearest.t, other_nearest.t))
	}

	/// Where this curve crosses a straight one, found from where it meets the straight one's line.
	fn straight_crossings(&self, straight: &Curve, tolerance: f64) -> Vec<(f64, f64)> {
		let start = straight.start();
		let crossings = self.line_intersections(start, straight.end() - start);

		let on_straight = |t: f64| {
			let nearest = straight.nearest(self.evaluate(t));
			(nearest.distance <= tolerance).then_some((t, nearest.t))
		};
		crossings.into_iter().filter_map(on_straight).collect()
	}

	/// Intersections of two curves that are not straight, by subdividing them against each other until each meeting point is isolated in a pair of pieces, then refining it by Newton iteration.
	// O(k × depth) pair tests for k meeting points, or up to MAX_LIVE_PAIRS per level where the curves coincide
	fn subdivision_intersections(&self, other: &Curve, diagonal: f64) -> Vec<(f64, f64)> {
		let tolerance = COINCIDENCE * diagonal;
		let leaf_size = LEAF_SIZE * diagonal;
		let mut intersections = Vec::new();

		// Newton iteration falters where the curves join smoothly at an end, so ends are matched against the other curve directly
		for t in [0., 1.] {
			let nearest = other.nearest(self.evaluate(t));
			if nearest.distance <= tolerance {
				intersections.push((t, nearest.t));
			}
		}
		for u in [0., 1.] {
			let nearest = self.nearest(other.evaluate(u));
			if nearest.distance <= tolerance {
				intersections.push((nearest.t, u));
			}
		}

		let mut pairs = vec![(Piece::whole(self), Piece::whole(other))];
		for depth in 0..=MAX_SUBDIVISION_DEPTH {
			if pairs.is_empty() {
				break;
			}

			// So many overlapping pairs means the curves coincide along a stretch, where no meeting point is isolated
			if pairs.len() > MAX_LIVE_PAIRS {
				intersections.retain(|&(t, u)| !pairs.iter().any(|(piece, other_piece)| piece.contains(t) && other_piece.contains(u)));
				break;
			}

			let mut next_pairs = Vec::new();
			for (piece, other_piece) in pairs {
				// The bands around chords keep the pairs along a near tangency few, where boxes alone would let them multiply
				let boxes_apart = !piece.bounds.inflated(tolerance).intersects(&other_piece.bounds);
				if boxes_apart || piece.band_excludes(&other_piece, tolerance) || other_piece.band_excludes(&piece, tolerance) {
					continue;
				}

				// A pair holding at most one meeting point is resolved by Newton iteration, if that lands inside the pair
				let is_leaf = depth == MAX_SUBDIVISION_DEPTH || (piece.diagonal() <= leaf_size && other_piece.diagonal() <= leaf_size);
				if is_leaf || piece.meets_at_most_once(&other_piece) {
					let refined = self.refine_intersection(other, piece.middle(), other_piece.middle(), tolerance);
					let refined = refined.filter(|&(t, u)| is_leaf || (piece.contains(t) && other_piece.contains(u)));

					if let Some(intersection) = refined {
						intersections.push(intersection);
					}
					if refined.is_some() || is_leaf {
						continue;
					}
				}

				if piece.diagonal() >= other_piece.diagonal() {
					next_pairs.extend(piece.halves().map(|half| (half, other_piece)));
				} else {
					next_pairs.extend(other_piece.halves().map(|half| (piece, half)));
				}
			}

			pairs = next_pairs;
		}

		deduplicate_intersections(intersections)
	}

	/// Newton iteration on B(t) - B_other(u) = 0 from a starting guess, keeping the result only if the curves meet there within `tolerance`.
	fn refine_intersection(&self, other: &Curve, t: f64, u: f64, tolerance: f64) -> Option<(f64, f64)> {
		let gap = |t: f64, u: f64| self.evaluate(t) - other.evaluate(u);
		let within_reach = |parameter: f64| (-NEWTON_MARGIN..=1. + NEWTON_MARGIN).contains(&parameter);

		let (mut t, mut u) = (t, u);
		let mut difference = gap(t, u);
		let mut best_parameters = (t, u);
		let mut best_distance = difference.length();

		for _ in 0..MAX_NEWTON_STEPS {
			// Solve [B'(t), -B_other'(u)] (step_t, step_u) = -difference by Cramer's rule, where a zero determinant means the curves run parallel
			let derivative = self.derivative(t);
			let other_derivative = -other.derivative(u);
			let determinant = derivative.perp_dot(other_derivative);
			let step_t = other_derivative.perp_dot(difference) / determinant;
			let step_u = difference.perp_dot(derivative) / determinant;

			t += step_t;
			u += step_u;
			if !within_reach(t) || !within_reach(u) {
				break;
			}

			difference = gap(t, u);
			let distance = difference.length();
			if distance < best_distance {
				best_parameters = (t, u);
				best_distance = distance;
			}

			if distance == 0. || step_t.abs() + step_u.abs() <= f64::EPSILON {
				break;
			}
		}

		let (t, u) = (best_parameters.0.clamp(0., 1.), best_parameters.1.clamp(0., 1.));
		(gap(t, u).length() <= tolerance).then_some((t, u))
	}
}

/// A stretch of a curve as a cubic, with its range of the whole curve's parameter, for subdividing two curves against each other.
#[derive(Clone, Copy, Debug)]
struct Piece {
	control_points: [DVec2; 4],
	start: f64,
	end: f64,
	bounds: Bounds,
	/// A cone around every direction the piece travels in, as a central unit direction and a half-angle, if it is narrower than a half-plane.
	tangent_cone: Option<(DVec2, f64)>,
}

impl Piece {
	fn whole(curve: &Curve) -> Self {
		let cubic = curve.segment.to_cubic();
		Self::new([cubic.p0, cubic.p1, cubic.p2, cubic.p3].map(to_dvec2), 0., 1.)
	}

	fn new(control_points: [DVec2; 4], start: f64, end: f64) -> Self {
		let [p0, p1, p2, p3] = control_points;

		// The derivative is a positive blend of the differences between consecutive control points, so it stays inside their cone
		let directions = [p1 - p0, p2 - p1, p3 - p2].map(DVec2::try_normalize);
		let center = directions.iter().flatten().sum::<DVec2>().try_normalize();
		let tangent_cone = center.and_then(|center| {
			let min_cosine = directions.iter().flatten().map(|direction| direction.dot(center)).fold(1., f64::min);
			(min_cosine > 0.).then(|| (center, min_cosine.min(1.).acos()))
		});

		Self {
			control_points,
			start,
			end,
			bounds: Bounds::from_points(control_points),
			tangent_cone,
		}
	}

	/// The two halves from de Casteljau's construction at the middle of the piece.
	fn halves(&self) -> [Self; 2] {
		let [p0, p1, p2, p3] = self.control_points;
		let p01 = (p0 + p1) / 2.;
		let p12 = (p1 + p2) / 2.;
		let p23 = (p2 + p3) / 2.;
		let p012 = (p01 + p12) / 2.;
		let p123 = (p12 + p23) / 2.;
		let split_point = (p012 + p123) / 2.;

		let middle = self.middle();
		[Self::new([p0, p01, p012, split_point], self.start, middle), Self::new([split_point, p123, p23, p3], middle, self.end)]
	}

	fn middle(&self) -> f64 {
		(self.start + self.end) / 2.
	}

	fn diagonal(&self) -> f64 {
		self.bounds.size().length()
	}

	fn contains(&self, t: f64) -> bool {
		(self.start - PARAMETER_SLACK..=self.end + PARAMETER_SLACK).contains(&t)
	}

	/// Whether the other piece lies wholly to one side of the band around this piece's chord that holds its control points, so the two cannot meet.
	fn band_excludes(&self, other: &Piece, tolerance: f64) -> bool {
		let [start, .., end] = self.control_points;
		let Some(normal) = (end - start).perp().try_normalize() else { return false };

		let offset_range = |control_points: [DVec2; 4]| {
			let offsets = control_points.map(|control_point| (control_point - start).dot(normal));
			offsets.into_iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), offset| (low.min(offset), high.max(offset)))
		};
		let (low, high) = offset_range(self.control_points);
		let (other_low, other_high) = offset_range(other.control_points);

		other_low > high + tolerance || other_high < low - tolerance
	}

	/// Whether the pieces can meet at most once, which holds when their tangent cones share no line, since the chord between two meeting points would lie in both.
	fn meets_at_most_once(&self, other: &Piece) -> bool {
		let (Some((center, half_angle)), Some((other_center, other_half_angle))) = (self.tangent_cone, other.tangent_cone) else {
			return false;
		};

		// The other piece may run along that chord either way, so the cones are compared as lines
		let separation = center.dot(other_center).abs().min(1.).acos();
		separation > half_angle + other_half_angle
	}
}

/// A polynomial in t, with coefficients from the constant term upward.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Polynomial([f64; COEFFICIENT_COUNT]);

impl Polynomial {
	fn evaluate(&self, t: f64) -> f64 {
		self.0.iter().rev().fold(0., |sum, coefficient| sum * t + coefficient)
	}

	fn derivative(&self) -> Self {
		Self(std::array::from_fn(|power| self.0.get(power + 1).map_or(0., |coefficient| coefficient * (power + 1) as f64)))
	}

	fn largest_coefficient(&self) -> f64 {
		self.0.iter().fold(0., |largest, coefficient| largest.max(coefficient.abs()))
	}

	/// The highest power with a coefficient that is not negligible, or `None` for the zero polynomial.
	fn degree(&self) -> Option<usize> {
		let threshold = NEGLIGIBLE * self.largest_coefficient();
		(0..COEFFICIENT_COUNT).rev().find(|&power| self.0[power].abs() > threshold)
	}

	fn truncated(&self, degree: usize) -> Self {
		let mut truncated = *self;
		truncated.0[degree + 1..].fill(0.);
		truncated
	}
}

impl Add for Polynomial {
	type Output = Self;

	fn add(self, other: Self) -> Self {
		Self(std::array::from_fn(|power| self.0[power] + other.0[power]))
	}
}

impl Sub for Polynomial {
	type Output = Self;

	fn sub(self, other: Self) -> Self {
		Self(std::array::from_fn(|power| self.0[power] - other.0[power]))
	}
}

impl Mul for Polynomial {
	type Output = Self;

	/// Terms past the highest power are dropped, which no condition built from a cubic reaches.
	fn mul(self, other: Self) -> Self {
		let mut product = Self::default();
		for (i, a) in self.0.iter().enumerate() {
			for (j, b) in other.0.iter().enumerate().take(COEFFICIENT_COUNT - i) {
				product.0[i + j] += a * b;
			}
		}
		product
	}
}

impl Mul<f64> for Polynomial {
	type Output = Self;

	fn mul(self, factor: f64) -> Self {
		Self(self.0.map(|coefficient| coefficient * factor))
	}
}

/// Every real root of the polynomial in [0, 1], sorted ascending, including roots where it touches zero without crossing.
fn roots_in_unit_interval(polynomial: Polynomial) -> Vec<f64> {
	if !polynomial.0.iter().all(|coefficient| coefficient.is_finite()) {
		return Vec::new();
	}

	let mut roots = roots_between(polynomial, -PARAMETER_SLACK, 1. + PARAMETER_SLACK);
	for root in &mut roots {
		*root = root.clamp(0., 1.);
	}
	roots.dedup_by(|later, earlier| *later - *earlier < ROOT_SEPARATION);
	roots
}

/// Every root in [lower, upper], sorted ascending, found by splitting the interval at the critical points into stretches where the polynomial is monotonic.
// O(n²) bracket refinements for degree n, summed over the recursion through its derivatives
fn roots_between(polynomial: Polynomial, lower: f64, upper: f64) -> Vec<f64> {
	let Some(degree) = polynomial.degree() else { return Vec::new() };
	let polynomial = polynomial.truncated(degree);

	match degree {
		0 => return Vec::new(),
		1 => {
			let root = -polynomial.0[0] / polynomial.0[1];
			return if (lower..=upper).contains(&root) { vec![root] } else { Vec::new() };
		}
		_ => {}
	}

	let derivative = polynomial.derivative();
	let mut boundaries = vec![lower];
	boundaries.extend(roots_between(derivative, lower, upper));
	boundaries.push(upper);

	// Values this close to zero cannot be told apart from it, which is how roots of even multiplicity show up at critical points
	let tolerance = NEGLIGIBLE * polynomial.largest_coefficient();
	let value_at = |t: f64| {
		let value = polynomial.evaluate(t);
		if value.abs() <= tolerance { 0. } else { value }
	};

	let mut roots = Vec::new();
	let mut left_value = value_at(lower);
	if left_value == 0. {
		roots.push(lower);
	}

	for stretch in boundaries.windows(2) {
		let &[left, right] = stretch else { continue };
		let right_value = value_at(right);

		if left_value != 0. && right_value != 0. && (left_value < 0.) != (right_value < 0.) {
			roots.push(refine_root(polynomial, derivative, left, right, left_value < 0.));
		}
		if right_value == 0. {
			roots.push(right);
		}

		left_value = right_value;
	}

	roots
}

/// The root of a polynomial that is monotonic over [left, right] and changes sign across it, by Newton steps that fall back to bisection when they would leave the shrinking bracket.
fn refine_root(polynomial: Polynomial, derivative: Polynomial, left: f64, right: f64, negative_on_left: bool) -> f64 {
	let (mut left, mut right) = (left, right);
	let mut t = (left + right) / 2.;

	for _ in 0..MAX_ROOT_REFINEMENT_STEPS {
		let value = polynomial.evaluate(t);
		if value == 0. {
			break;
		}

		if (value < 0.) == negative_on_left {
			left = t;
		} else {
			right = t;
		}

		let newton = t - value / derivative.evaluate(t);
		let next = if newton > left && newton < right { newton } else { (left + right) / 2. };
		let step = (next - t).abs();
		t = next;

		if step <= f64::EPSILON {
			break;
		}
	}

	t
}

/// The line through every point within `tolerance`, as an origin and unit direction, or `None` if the points stray from every line or all coincide.
fn common_line(points: &[DVec2], tolerance: f64) -> Option<(DVec2, DVec2)> {
	let origin = *points.first()?;
	let farthest = points.iter().copied().max_by(|a, b| a.distance_squared(origin).total_cmp(&b.distance_squared(origin)))?;
	let direction = (farthest - origin).try_normalize()?;

	points.iter().all(|point| (*point - origin).perp_dot(direction).abs() <= tolerance).then_some((origin, direction))
}

/// Keeps the first of each group of intersections lying within [`INTERSECTION_SEPARATION`] of each other in both parameters.
fn deduplicate_intersections(intersections: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
	let mut unique: Vec<(f64, f64)> = Vec::with_capacity(intersections.len());
	for (t, u) in intersections {
		let repeated = unique
			.iter()
			.any(|&(unique_t, unique_u)| (t - unique_t).abs() <= INTERSECTION_SEPARATION && (u - unique_u).abs() <= INTERSECTION_SEPARATION);
		if !repeated {
			unique.push((t, u));
		}
	}
	unique
}

fn to_point(vector: DVec2) -> kurbo::Point {
	kurbo::Point::new(vector.x, vector.y)
}

fn to_dvec2(point: kurbo::Point) -> DVec2 {
	DVec2::new(point.x, point.y)
}

#[cfg(test)]
mod tests {
	use super::*;
	use glam::DAffine2;
	use std::f64::consts::{FRAC_PI_4, PI, TAU};

	/// A borrowed real function of the curve parameter.
	type ParameterFunction<'a> = &'a dyn Fn(f64) -> f64;

	/// A tiny deterministic generator (SplitMix64), so the randomized tests repeat exactly.
	struct Random(u64);

	impl Random {
		/// A uniformly distributed number in [0, 1).
		fn fraction(&mut self) -> f64 {
			self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
			let mut mixed = self.0;
			mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
			mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
			mixed ^= mixed >> 31;
			(mixed >> 11) as f64 / (1_u64 << 53) as f64
		}

		/// A point in the square from -100 to 100 on each axis.
		fn point(&mut self) -> DVec2 {
			DVec2::new(self.fraction(), self.fraction()) * 200. - 100.
		}

		fn cubic(&mut self) -> Curve {
			Curve::cubic(self.point(), self.point(), self.point(), self.point())
		}
	}

	fn polynomial_with_roots(roots: &[f64]) -> Polynomial {
		let one = Polynomial([1., 0., 0., 0., 0., 0., 0.]);
		roots.iter().fold(one, |product, root| product * Polynomial([-root, 1., 0., 0., 0., 0., 0.]))
	}

	fn constant(value: f64) -> Polynomial {
		Polynomial([value, 0., 0., 0., 0., 0., 0.])
	}

	fn assert_close(found: &[f64], expected: &[f64], tolerance: f64) {
		assert_eq!(found.len(), expected.len(), "found {found:?} but expected {expected:?}");
		for (value, expected_value) in found.iter().zip(expected) {
			assert!((value - expected_value).abs() <= tolerance, "found {found:?} but expected {expected:?}");
		}
	}

	fn assert_intersections(found: &[(f64, f64)], expected: &[(f64, f64)], tolerance: f64) {
		assert_eq!(found.len(), expected.len(), "found {found:?} but expected {expected:?}");
		for (&(t, u), &(expected_t, expected_u)) in found.iter().zip(expected) {
			assert!((t - expected_t).abs() <= tolerance && (u - expected_u).abs() <= tolerance, "found {found:?} but expected {expected:?}");
		}
	}

	/// The intervals between 20,000 evenly spaced samples of [0, 1] across which a function changes sign.
	fn sign_change_intervals(function: impl Fn(f64) -> f64) -> Vec<(f64, f64)> {
		const SAMPLES: usize = 20_000;
		let parameter = |i: usize| i as f64 / SAMPLES as f64;
		let values: Vec<f64> = (0..=SAMPLES).map(|i| function(parameter(i))).collect();

		(1..=SAMPLES).filter(|&i| values[i - 1] * values[i] < 0.).map(|i| (parameter(i - 1), parameter(i))).collect()
	}

	fn sampled_nearest_distance(curve: &Curve, point: DVec2) -> f64 {
		(0..=20_000).map(|i| curve.evaluate(i as f64 / 20_000.).distance(point)).fold(f64::INFINITY, f64::min)
	}

	fn polyline(curve: &Curve, pieces: usize) -> Vec<DVec2> {
		(0..=pieces).map(|i| curve.evaluate(i as f64 / pieces as f64)).collect()
	}

	/// Every proper crossing of two polylines, as the fraction along each and the sine of the crossing angle, skipping chunks of segments with disjoint bounds.
	fn polyline_crossings(first: &[DVec2], second: &[DVec2]) -> Vec<(f64, f64, f64)> {
		const CHUNK: usize = 50;
		let chunks = |points: &[DVec2]| {
			let segments = points.len() - 1;
			(0..segments)
				.step_by(CHUNK)
				.map(|start| (start..(start + CHUNK).min(segments), Bounds::from_points(points[start..=(start + CHUNK).min(segments)].iter().copied())))
				.collect::<Vec<_>>()
		};
		let (first_chunks, second_chunks) = (chunks(first), chunks(second));

		let mut crossings = Vec::new();
		for (first_range, first_bounds) in &first_chunks {
			for (second_range, second_bounds) in &second_chunks {
				if !first_bounds.intersects(second_bounds) {
					continue;
				}

				for i in first_range.clone() {
					for j in second_range.clone() {
						let (a, b) = (first[i + 1] - first[i], second[j + 1] - second[j]);
						let sides_of_first = [second[j], second[j + 1]].map(|point| a.perp_dot(point - first[i]));
						let sides_of_second = [first[i], first[i + 1]].map(|point| b.perp_dot(point - second[j]));
						if sides_of_first[0] * sides_of_first[1] >= 0. || sides_of_second[0] * sides_of_second[1] >= 0. {
							continue;
						}

						let along_first = sides_of_second[0] / (sides_of_second[0] - sides_of_second[1]);
						let along_second = sides_of_first[0] / (sides_of_first[0] - sides_of_first[1]);
						let sine = a.perp_dot(b).abs() / (a.length() * b.length());
						crossings.push(((i as f64 + along_first) / (first.len() - 1) as f64, (j as f64 + along_second) / (second.len() - 1) as f64, sine));
					}
				}
			}
		}
		crossings
	}

	#[test]
	fn the_root_finder_finds_simple_double_and_end_roots() {
		assert_close(&roots_in_unit_interval(polynomial_with_roots(&[0.3, 0.7])), &[0.3, 0.7], 1e-12);
		assert_close(&roots_in_unit_interval(polynomial_with_roots(&[0.2, 0.2, 0.8])), &[0.2, 0.8], 1e-12);
		assert_close(&roots_in_unit_interval(polynomial_with_roots(&[0., 0.5, 1.])), &[0., 0.5, 1.], 1e-12);

		// One third is not exactly representable, so rounding lifts this double root slightly off zero
		assert_close(&roots_in_unit_interval(polynomial_with_roots(&[1. / 3., 1. / 3.])), &[1. / 3.], 1e-12);

		// A touch lifted clearly off zero is no root, while close but distinct roots stay distinct
		assert_close(&roots_in_unit_interval(polynomial_with_roots(&[0.5, 0.5]) + constant(1e-6)), &[], 0.);
		assert_close(&roots_in_unit_interval(polynomial_with_roots(&[0.5 - 1e-4, 0.5 + 1e-4])), &[0.5 - 1e-4, 0.5 + 1e-4], 1e-11);
	}

	#[test]
	fn the_root_finder_separates_six_roots_of_a_sixth_degree_polynomial() {
		let roots = [0.05, 0.2, 0.4, 0.55, 0.75, 0.95];
		assert_close(&roots_in_unit_interval(polynomial_with_roots(&roots)), &roots, 1e-12);

		let scaled = polynomial_with_roots(&roots) * -1e-7;
		assert_close(&roots_in_unit_interval(scaled), &roots, 1e-12);
	}

	#[test]
	fn roots_just_outside_the_interval_count_as_its_ends() {
		assert_close(&roots_in_unit_interval(polynomial_with_roots(&[-1e-11, 0.4])), &[0., 0.4], 1e-12);
		assert_close(&roots_in_unit_interval(polynomial_with_roots(&[0.6, 1. + 1e-11])), &[0.6, 1.], 1e-12);
		assert_close(&roots_in_unit_interval(polynomial_with_roots(&[-1e-6, 1. + 1e-6])), &[], 0.);
		assert_close(&roots_in_unit_interval(polynomial_with_roots(&[-0.5, 1.5])), &[], 0.);
	}

	#[test]
	fn negligible_leading_coefficients_neither_add_roots_nor_hide_real_ones() {
		let nearly_linear = Polynomial([-0.5, 1., 0., 1e-15, 0., 0., 0.]);
		assert_eq!(nearly_linear.degree(), Some(1));
		assert_close(&roots_in_unit_interval(nearly_linear), &[0.5], 1e-12);

		let nearly_quadratic = polynomial_with_roots(&[0.25, 0.25]) + Polynomial([0., 0., 0., 0., 0., 1e-14, 0.]);
		assert_eq!(nearly_quadratic.degree(), Some(2));
		assert_close(&roots_in_unit_interval(nearly_quadratic), &[0.25], 1e-12);
	}

	#[test]
	fn constant_zero_and_broken_polynomials_have_no_roots() {
		assert_close(&roots_in_unit_interval(Polynomial::default()), &[], 0.);
		assert_close(&roots_in_unit_interval(constant(3.)), &[], 0.);
		assert_close(&roots_in_unit_interval(constant(-1e-300)), &[], 0.);
		assert_close(&roots_in_unit_interval(Polynomial([f64::NAN, 1., 0., 0., 0., 0., 0.])), &[], 0.);
		assert_close(&roots_in_unit_interval(Polynomial([-0.5, f64::INFINITY, 0., 0., 0., 0., 0.])), &[], 0.);
	}

	#[test]
	fn evaluation_derivatives_and_the_power_basis_match_kurbo() {
		let mut random = Random(1);

		for _ in 0..50 {
			let curves = [
				Curve::line(random.point(), random.point()),
				Curve::quadratic(random.point(), random.point(), random.point()),
				random.cubic(),
			];
			for curve in curves {
				let (x, y) = curve.coordinate_polynomials(DVec2::ZERO);

				for step in 0..=20 {
					let t = step as f64 / 20.;
					let kurbo_position = to_dvec2(curve.segment().eval(t));
					assert_eq!(curve.evaluate(t), kurbo_position);
					assert!(DVec2::new(x.evaluate(t), y.evaluate(t)).abs_diff_eq(kurbo_position, 1e-10));

					let step_size = 1e-6;
					let central_difference = (curve.evaluate(t + step_size) - curve.evaluate(t - step_size)) / (2. * step_size);
					assert!(curve.derivative(t).abs_diff_eq(central_difference, 1e-5));
				}
			}
		}
	}

	#[test]
	fn tangents_follow_the_direction_of_travel_through_retracted_handles_and_cusps() {
		let retracted_start = Curve::cubic(DVec2::ZERO, DVec2::ZERO, DVec2::new(1., 2.), DVec2::new(3., 0.));
		assert_eq!(retracted_start.derivative(0.), DVec2::ZERO);
		assert!(retracted_start.tangent(0.).unwrap().abs_diff_eq(DVec2::new(1., 2.).normalize(), 1e-12));

		let retracted_end = Curve::cubic(DVec2::ZERO, DVec2::new(1., 2.), DVec2::new(3., 0.), DVec2::new(3., 0.));
		assert!(retracted_end.tangent(1.).unwrap().abs_diff_eq(DVec2::new(2., -2.).normalize(), 1e-12));

		// With both handles on the start, only the third derivative is left to point the way
		let both_retracted = Curve::cubic(DVec2::ZERO, DVec2::ZERO, DVec2::ZERO, DVec2::new(0., 5.));
		assert!(both_retracted.tangent(0.).unwrap().abs_diff_eq(DVec2::Y, 1e-12));

		// This cusp at t = 0.5 arrives heading up and leaves heading down
		let cusp = Curve::cubic(DVec2::ZERO, DVec2::new(1., 1.), DVec2::new(0., 1.), DVec2::new(1., 0.));
		assert_eq!(cusp.derivative(0.5), DVec2::ZERO);
		assert!(cusp.tangent(0.5).unwrap().abs_diff_eq(DVec2::NEG_Y, 1e-12));
		assert!(cusp.tangent(0.3).unwrap().abs_diff_eq(cusp.derivative(0.3).normalize(), 1e-12));

		assert!(Curve::cubic(DVec2::ONE, DVec2::ONE, DVec2::ONE, DVec2::ONE).tangent(0.5).is_none());
	}

	#[test]
	fn straight_curves_need_their_handles_on_the_chord_and_in_order() {
		assert!(Curve::line(DVec2::ZERO, DVec2::new(3., 4.)).is_straight());
		assert!(Curve::cubic(DVec2::ZERO, DVec2::new(1., 1.), DVec2::new(2., 2.), DVec2::new(3., 3.)).is_straight());
		assert!(Curve::cubic(DVec2::ZERO, DVec2::ZERO, DVec2::new(3., 3.), DVec2::new(3., 3.)).is_straight());
		assert!(Curve::quadratic(DVec2::ZERO, DVec2::new(1., 0.), DVec2::new(4., 0.)).is_straight());

		// A handle past the end or before the start makes the curve double back over itself
		assert!(!Curve::cubic(DVec2::ZERO, DVec2::new(5., 0.), DVec2::new(2., 0.), DVec2::new(3., 0.)).is_straight());
		assert!(!Curve::quadratic(DVec2::ZERO, DVec2::new(-1., 0.), DVec2::new(4., 0.)).is_straight());
		assert!(!Curve::cubic(DVec2::ZERO, DVec2::new(2., 0.), DVec2::new(1., 0.), DVec2::new(3., 0.)).is_straight());

		assert!(!Curve::cubic(DVec2::ZERO, DVec2::new(1., 1e-6), DVec2::new(2., 0.), DVec2::new(3., 0.)).is_straight());
		assert!(Curve::cubic(DVec2::ZERO, DVec2::new(1., 1e-12), DVec2::new(2., 0.), DVec2::new(3., 0.)).is_straight());

		assert!(!Curve::line(DVec2::ONE, DVec2::ONE).is_straight());
		assert!(!Curve::cubic(DVec2::ZERO, DVec2::new(1., 0.), DVec2::new(2., 0.), DVec2::ZERO).is_straight());
	}

	#[test]
	fn nearest_is_never_farther_than_dense_sampling_finds() {
		let mut random = Random(2);

		for _ in 0..500 {
			let curve = random.cubic();
			let point = random.point();
			let nearest = curve.nearest(point);

			assert!(nearest.distance <= sampled_nearest_distance(&curve, point) + 1e-6 * curve.size());
			assert_eq!(nearest.position, curve.evaluate(nearest.t));
			assert_eq!(nearest.distance, nearest.position.distance(point));
		}
	}

	#[test]
	fn nearest_finds_points_on_s_shaped_and_looped_curves_and_beyond_their_ends() {
		let s_shaped = Curve::cubic(DVec2::ZERO, DVec2::new(100., 0.), DVec2::new(0., 100.), DVec2::new(100., 100.));
		let looped = Curve::cubic(DVec2::ZERO, DVec2::new(200., 200.), DVec2::new(-100., 200.), DVec2::new(100., 0.));
		let mut random = Random(6);

		for curve in [s_shaped, looped] {
			let size = curve.size();

			// The loop passes through its crossing twice, so positions are compared rather than parameters
			for step in 0..=100 {
				let on_curve = curve.evaluate(step as f64 / 100.);
				let nearest = curve.nearest(on_curve);
				assert!(nearest.distance <= 1e-9 * size && nearest.position.distance(on_curve) <= 1e-9 * size);
			}

			let beyond_start = curve.start() - curve.tangent(0.).unwrap() * 10.;
			let beyond_end = curve.end() + curve.tangent(1.).unwrap() * 10.;
			assert_eq!(curve.nearest(beyond_start).t, 0.);
			assert_eq!(curve.nearest(beyond_end).t, 1.);

			for _ in 0..200 {
				let point = random.point() + DVec2::splat(50.);
				assert!(curve.nearest(point).distance <= sampled_nearest_distance(&curve, point) + 1e-6 * size);
			}
		}
	}

	#[test]
	fn nearest_reaches_cusp_tips_and_breaks_ties_toward_the_smallest_parameter() {
		// The cusp tip is the highest point, but no perpendicular foot since the derivative vanishes there
		let cusp = Curve::cubic(DVec2::ZERO, DVec2::new(1., 1.), DVec2::new(0., 1.), DVec2::new(1., 0.));
		let above_tip = DVec2::new(0.5, 1.);
		assert!((cusp.nearest(above_tip).t - 0.5).abs() < 1e-12);
		assert!(cusp.perpendicular_parameters_from(above_tip).iter().all(|t| (t - 0.5).abs() > 1e-6));

		// A closed loop starts and ends at the same point
		let closed = Curve::cubic(DVec2::ZERO, DVec2::new(100., 100.), DVec2::new(-100., 100.), DVec2::ZERO);
		assert_eq!(closed.nearest(DVec2::new(0., -10.)).t, 0.);
	}

	#[test]
	fn perpendicular_and_tangent_parameters_satisfy_their_conditions_and_miss_no_sign_change() {
		let mut random = Random(3);

		for _ in 0..300 {
			let curve = random.cubic();
			let point = random.point();

			let angle_cosine = |t: f64| {
				let (offset, derivative) = (curve.evaluate(t) - point, curve.derivative(t));
				offset.dot(derivative) / (offset.length() * derivative.length())
			};
			let angle_sine = |t: f64| {
				let (offset, derivative) = (curve.evaluate(t) - point, curve.derivative(t));
				offset.perp_dot(derivative) / (offset.length() * derivative.length())
			};

			let cases: [(Vec<f64>, ParameterFunction); 2] = [(curve.perpendicular_parameters_from(point), &angle_cosine), (curve.tangent_parameters_from(point), &angle_sine)];
			for (parameters, condition) in cases {
				assert!(parameters.is_sorted());
				for &t in &parameters {
					assert!(condition(t).abs() <= 1e-7, "t = {t} misses its condition by {}", condition(t));
				}

				for (left, right) in sign_change_intervals(condition) {
					assert!(
						parameters.iter().any(|&t| t >= left - 1e-6 && t <= right + 1e-6),
						"no parameter near the sign change in [{left}, {right}] among {parameters:?}"
					);
				}
			}
		}
	}

	#[test]
	fn tangents_from_an_outside_point_touch_an_arc_where_they_touch_its_circle() {
		// The usual cubic approximation of a quarter of the unit circle, which strays in direction from the circle by up to about 2e-3 radians
		let handle = 4. / 3. * (2_f64.sqrt() - 1.);
		let arc = Curve::cubic(DVec2::X, DVec2::new(1., handle), DVec2::new(handle, 1.), DVec2::Y);
		let point = DVec2::splat(0.9);

		// From a distance d to the center, the true tangents touch the circle acos(1 / d) to either side of the point's direction
		let spread = (1. / point.length()).acos();
		let expected = [FRAC_PI_4 - spread, FRAC_PI_4 + spread].map(DVec2::from_angle);

		let tangents = arc.tangent_parameters_from(point);
		assert_eq!(tangents.len(), 2);
		for (&t, expected_position) in tangents.iter().zip(expected) {
			let (offset, derivative) = (arc.evaluate(t) - point, arc.derivative(t));
			assert!(offset.perp_dot(derivative).abs() <= 1e-12 * offset.length() * derivative.length());
			assert!(arc.evaluate(t).distance(expected_position) < 2e-3);
		}

		// A point on the curve itself has no tangent through it from elsewhere on this arc, though the perpendicular there remains
		let on_arc = arc.evaluate(0.3);
		assert!(arc.tangent_parameters_from(on_arc).is_empty());
		assert_close(&arc.perpendicular_parameters_from(on_arc), &[0.3], 1e-12);
	}

	#[test]
	fn line_and_circle_intersections_lie_on_them_and_match_a_dense_scan() {
		let mut random = Random(4);
		let mut compared = 0;

		for _ in 0..300 {
			let curve = random.cubic();
			let size = curve.size();
			let point = random.point() / 2.;
			let direction = DVec2::from_angle(random.fraction() * TAU);
			let radius = 10. + random.fraction() * 90.;

			let line_offset = |t: f64| (curve.evaluate(t) - point).perp_dot(direction);
			let circle_offset = |t: f64| curve.evaluate(t).distance(point) - radius;
			let line_crossing_sine = |t: f64| curve.tangent(t).map_or(0., |tangent| tangent.perp_dot(direction).abs());
			let circle_crossing_sine = |t: f64| {
				curve
					.tangent(t)
					.zip((curve.evaluate(t) - point).try_normalize())
					.map_or(0., |(tangent, outward)| tangent.dot(outward).abs())
			};

			let cases: [(Vec<f64>, ParameterFunction, ParameterFunction); 2] = [
				(curve.line_intersections(point, direction), &line_offset, &line_crossing_sine),
				(curve.circle_intersections(point, radius), &circle_offset, &circle_crossing_sine),
			];
			for (parameters, offset, crossing_sine) in cases {
				assert!(parameters.is_sorted());
				for &t in &parameters {
					assert!(offset(t).abs() <= 1e-9 * size, "t = {t} is {} away", offset(t));
				}

				// Grazing crossings and ones at the ends cannot be counted reliably by sampling
				if parameters.iter().any(|&t| crossing_sine(t) < 1e-3 || !(1e-3..=1. - 1e-3).contains(&t)) {
					continue;
				}
				assert_eq!(parameters.len(), sign_change_intervals(offset).len());
				compared += 1;
			}
		}

		assert!(compared >= 500, "only {compared} cases were unambiguous");
	}

	#[test]
	fn touching_lines_and_circles_count_once_and_a_line_along_a_straight_curve_counts_never() {
		// The parabola y = (2x - 1)² over [0, 1], which touches the X axis and the unit circle below it at its lowest point
		let parabola = Curve::cubic(DVec2::Y, DVec2::new(1. / 3., -1. / 3.), DVec2::new(2. / 3., -1. / 3.), DVec2::ONE);
		assert_close(&parabola.line_intersections(DVec2::ZERO, DVec2::X), &[0.5], 1e-9);
		assert_close(&parabola.circle_intersections(DVec2::new(0.5, -1.), 1.), &[0.5], 1e-9);

		// Moved and rotated, rounding keeps the touches from being exact
		let transform = DAffine2::from_angle_translation(0.7, DVec2::new(3., 1.));
		let [start, handle_start, handle_end, end] = parabola.control_points().map(|point| transform.transform_point2(point));
		let moved = Curve::cubic(start, handle_start, handle_end, end);
		assert_close(&moved.line_intersections(transform.transform_point2(DVec2::ZERO), transform.transform_vector2(DVec2::X)), &[0.5], 1e-9);
		assert_close(&moved.circle_intersections(transform.transform_point2(DVec2::new(0.5, -1.)), 1.), &[0.5], 1e-9);

		let straight = Curve::cubic(DVec2::ZERO, DVec2::new(1., 1.), DVec2::new(2., 2.), DVec2::new(3., 3.));
		assert!(straight.line_intersections(DVec2::new(-1., -1.), DVec2::ONE).is_empty());
		assert_eq!(straight.line_intersections(DVec2::new(0., 2.), DVec2::X).len(), 1);
	}

	#[test]
	fn crossing_lines_meet_once_while_parallel_and_overlapping_ones_do_not() {
		let line = Curve::line(DVec2::ZERO, DVec2::new(2., 2.));

		assert_intersections(&line.curve_intersections(&Curve::line(DVec2::new(0., 2.), DVec2::new(2., 0.))), &[(0.5, 0.5)], 1e-12);
		assert_intersections(&line.curve_intersections(&Curve::line(DVec2::new(0., 3.), DVec2::new(1.5, 0.))), &[(0.5, 2. / 3.)], 1e-12);

		// These would cross only if extended
		assert!(line.curve_intersections(&Curve::line(DVec2::new(2., 0.), DVec2::new(1.5, 0.5))).is_empty());

		assert!(line.curve_intersections(&Curve::line(DVec2::new(1., 0.), DVec2::new(3., 2.))).is_empty());
		assert!(line.curve_intersections(&Curve::line(DVec2::ONE, DVec2::new(3., 3.))).is_empty());
		assert!(
			line.curve_intersections(&Curve::cubic(DVec2::ONE, DVec2::new(1.5, 1.5), DVec2::new(1.6, 1.6), DVec2::new(3., 3.)))
				.is_empty()
		);
	}

	#[test]
	fn collinear_segments_meeting_end_to_end_meet_at_that_end() {
		let first = Curve::line(DVec2::ZERO, DVec2::new(1., 2.));
		let next = Curve::line(DVec2::new(1., 2.), DVec2::new(2., 4.));
		let head_to_head = Curve::line(DVec2::new(2., 4.), DVec2::new(1., 2.));

		assert_intersections(&first.curve_intersections(&next), &[(1., 0.)], 1e-12);
		assert_intersections(&next.curve_intersections(&first), &[(0., 1.)], 1e-12);
		assert_intersections(&first.curve_intersections(&head_to_head), &[(1., 1.)], 1e-12);
	}

	#[test]
	fn a_line_through_a_loop_crosses_it_three_times() {
		let looped = Curve::cubic(DVec2::ZERO, DVec2::new(200., 200.), DVec2::new(-100., 200.), DVec2::new(100., 0.));
		let line = Curve::line(DVec2::new(40., -10.), DVec2::new(40., 200.));

		let intersections = looped.curve_intersections(&line);
		assert_eq!(intersections.len(), 3);
		for &(t, u) in &intersections {
			assert!(looped.evaluate(t).distance(line.evaluate(u)) <= 1e-9);
			assert!((looped.evaluate(t).x - 40.).abs() <= 1e-9);
		}

		let mut swapped: Vec<(f64, f64)> = line.curve_intersections(&looped).into_iter().map(|(u, t)| (t, u)).collect();
		swapped.sort_by(|a, b| a.0.total_cmp(&b.0));
		assert_intersections(&swapped, &intersections, 1e-12);
	}

	#[test]
	fn two_arcs_cross_twice_symmetrically() {
		let arch = Curve::cubic(DVec2::new(-1., 0.), DVec2::new(-1., 4. / 3.), DVec2::new(1., 4. / 3.), DVec2::new(1., 0.));
		let flatter = Curve::cubic(DVec2::new(-1.5, 0.3), DVec2::new(-0.5, 0.8), DVec2::new(0.5, 0.8), DVec2::new(1.5, 0.3));

		let intersections = arch.curve_intersections(&flatter);
		assert_eq!(intersections.len(), 2);
		for &(t, u) in &intersections {
			assert!(arch.evaluate(t).distance(flatter.evaluate(u)) <= 1e-12);
		}

		// Both curves are mirror images of themselves across the Y axis
		let [(t0, u0), (t1, u1)] = [intersections[0], intersections[1]];
		assert!((t0 + t1 - 1.).abs() <= 1e-12 && (u0 + u1 - 1.).abs() <= 1e-12);
	}

	#[test]
	fn two_s_curves_cross_nine_times() {
		// The graphs of y = T(x) and x = T(y) over [-1, 1], for the Chebyshev polynomial T(x) = 4x³ - 3x
		let chebyshev = |x: f64| 4. * x * x * x - 3. * x;
		let across = Curve::cubic(DVec2::new(-1., -1.), DVec2::new(-1. / 3., 5.), DVec2::new(1. / 3., -5.), DVec2::ONE);
		let upright = Curve::cubic(DVec2::new(-1., -1.), DVec2::new(5., -1. / 3.), DVec2::new(-5., 1. / 3.), DVec2::ONE);

		// They meet where x = T(T(x)), which for x = cos(θ) means cos(9θ) = cos(θ), so θ is a multiple of π/4 or π/5
		let angles = (0..=4).map(|k| k as f64 * PI / 4.).chain((1..=4).map(|k| k as f64 * PI / 5.));
		let mut expected: Vec<(f64, f64)> = angles.map(|angle| (angle.cos(), chebyshev(angle.cos()))).map(|(x, y)| ((x + 1.) / 2., (y + 1.) / 2.)).collect();
		expected.sort_by(|a, b| a.0.total_cmp(&b.0));

		assert_intersections(&across.curve_intersections(&upright), &expected, 1e-9);
	}

	#[test]
	fn adjacent_segments_meet_once_at_their_shared_end() {
		let first = Curve::cubic(DVec2::ZERO, DVec2::new(1., 1.), DVec2::new(2., 1.), DVec2::new(3., 0.));
		let smooth = Curve::cubic(DVec2::new(3., 0.), DVec2::new(4., -1.), DVec2::new(5., -1.), DVec2::new(6., 0.));
		let cornered = Curve::cubic(DVec2::new(3., 0.), DVec2::new(3., 2.), DVec2::new(5., 2.), DVec2::new(6., 0.));

		for next in [smooth, cornered] {
			assert_intersections(&first.curve_intersections(&next), &[(1., 0.)], 1e-12);
			assert_intersections(&next.curve_intersections(&first), &[(0., 1.)], 1e-12);
		}

		let straight_continuation = Curve::line(DVec2::new(3., 0.), DVec2::new(4., -1.));
		assert_intersections(&first.curve_intersections(&straight_continuation), &[(1., 0.)], 1e-9);
	}

	#[test]
	fn coincident_curves_meet_nowhere() {
		let curve = Curve::cubic(DVec2::ZERO, DVec2::new(30., 90.), DVec2::new(70., -40.), DVec2::new(100., 20.));
		let reversed = Curve::new(curve.segment().reverse());
		let part = Curve::new(curve.segment().subsegment(0.2..0.7));
		let quadratic = Curve::quadratic(DVec2::ZERO, DVec2::new(50., 100.), DVec2::new(100., 0.));
		let elevated = Curve::new(PathSeg::Cubic(quadratic.segment().to_cubic()));

		assert!(curve.curve_intersections(&curve).is_empty());
		assert!(curve.curve_intersections(&reversed).is_empty());
		assert!(curve.curve_intersections(&part).is_empty());
		assert!(part.curve_intersections(&curve).is_empty());
		assert!(quadratic.curve_intersections(&elevated).is_empty());
	}

	#[test]
	fn curve_intersections_meet_and_match_fine_polylines_in_generic_cases() {
		let mut random = Random(5);
		let mut compared = 0;

		for _ in 0..200 {
			let (first, second) = (random.cubic(), random.cubic());
			let intersections = first.curve_intersections(&second);
			let diagonal = first.bounds().union(&second.bounds()).size().length();

			assert!(intersections.is_sorted_by(|a, b| a.0 <= b.0));
			for &(t, u) in &intersections {
				assert!(first.evaluate(t).distance(second.evaluate(u)) <= 1e-7 * diagonal);
			}

			// Grazing meetings, and ones near an end, can fall either way between the polylines
			let crossings = polyline_crossings(&polyline(&first, 2000), &polyline(&second, 2000));
			let crossing_sine = |t: f64, u: f64| first.tangent(t).zip(second.tangent(u)).map_or(0., |(a, b)| a.perp_dot(b).abs());
			let near_an_end = |parameter: f64| !(1e-3..=1. - 1e-3).contains(&parameter);
			let ambiguous_intersection = intersections.iter().any(|&(t, u)| crossing_sine(t, u) < 1e-2 || near_an_end(t) || near_an_end(u));
			let ambiguous_crossing = crossings.iter().any(|&(t, u, sine)| sine < 1e-2 || near_an_end(t) || near_an_end(u));
			if ambiguous_intersection || ambiguous_crossing {
				continue;
			}

			assert_eq!(intersections.len(), crossings.len(), "found {intersections:?} but the polylines cross at {crossings:?}");
			for &(t, u, _) in &crossings {
				assert!(intersections.iter().any(|&(found_t, found_u)| (found_t - t).abs() < 1e-3 && (found_u - u).abs() < 1e-3));
			}
			compared += 1;
		}

		assert!(compared >= 150, "only {compared} cases were unambiguous");
	}

	#[test]
	fn curves_touching_tangentially_meet_once_where_they_touch() {
		// Quarter circles from the usual cubic approximation, which is exact at their middles where they touch
		let handle = 4. / 3. * (2_f64.sqrt() - 1.);
		let quarter_circle = |center: DVec2, radius: f64, start_angle: f64| {
			let [start, end] = [start_angle, start_angle + PI / 2.].map(DVec2::from_angle);
			Curve::cubic(
				center + start * radius,
				center + (start + start.perp() * handle) * radius,
				center + (end - end.perp() * handle) * radius,
				center + end * radius,
			)
		};
		let touching_direction = DVec2::from_angle(3. * FRAC_PI_4);
		let circle = quarter_circle(DVec2::ZERO, 1., PI / 2.);

		let outside = quarter_circle(touching_direction * 2., 1., -PI / 2.);
		let around = quarter_circle(touching_direction * -0.1, 1.1, PI / 2.);
		let closely_around = quarter_circle(touching_direction * -0.01, 1.01, PI / 2.);

		for other in [outside, around, closely_around] {
			let intersections = circle.curve_intersections(&other);
			assert_intersections(&intersections, &[(0.5, 0.5)], 1e-6);
			assert!(circle.evaluate(intersections[0].0).distance(touching_direction) <= 1e-7);
		}
	}

	#[test]
	fn degenerate_inputs_give_empty_results() {
		let single_point = Curve::cubic(DVec2::ONE, DVec2::ONE, DVec2::ONE, DVec2::ONE);
		assert_eq!(
			single_point.nearest(DVec2::new(4., 5.)),
			NearestOnCurve {
				t: 0.,
				position: DVec2::ONE,
				distance: 5.
			}
		);
		assert!(single_point.perpendicular_parameters_from(DVec2::ZERO).is_empty());
		assert!(single_point.tangent_parameters_from(DVec2::ZERO).is_empty());
		assert!(single_point.line_intersections(DVec2::ZERO, DVec2::ONE).is_empty());
		assert!(single_point.line_intersections(DVec2::ZERO, DVec2::X).is_empty());
		assert!(single_point.circle_intersections(DVec2::ZERO, 2_f64.sqrt()).is_empty());
		assert!(single_point.curve_intersections(&Curve::line(DVec2::ZERO, DVec2::splat(2.))).is_empty());
		assert!(Curve::line(DVec2::ZERO, DVec2::splat(2.)).curve_intersections(&single_point).is_empty());
		assert!(single_point.tangent(0.5).is_none());

		let curve = Curve::cubic(DVec2::ZERO, DVec2::new(1., 2.), DVec2::new(3., 2.), DVec2::new(4., 0.));
		let nan = DVec2::NAN;
		assert_eq!(curve.nearest(nan).t, 0.);
		assert_eq!(curve.nearest(nan).distance, f64::INFINITY);
		assert!(curve.perpendicular_parameters_from(nan).is_empty());
		assert!(curve.tangent_parameters_from(nan).is_empty());
		assert!(curve.line_intersections(nan, DVec2::X).is_empty());
		assert!(curve.line_intersections(DVec2::ZERO, nan).is_empty());
		assert!(curve.line_intersections(DVec2::ZERO, DVec2::ZERO).is_empty());
		assert!(curve.circle_intersections(nan, 1.).is_empty());
		assert!(curve.tangent(f64::NAN).is_none());
		for radius in [0., -1., f64::NAN, f64::INFINITY] {
			assert!(curve.circle_intersections(DVec2::ZERO, radius).is_empty());
		}

		for broken_point in [nan, DVec2::new(f64::INFINITY, 0.)] {
			let broken = Curve::cubic(DVec2::ZERO, broken_point, DVec2::ONE, DVec2::X);
			assert_eq!(broken.nearest(DVec2::ZERO).t, 0.);
			assert_eq!(broken.bounds(), Bounds::EMPTY);
			assert!(broken.tangent(0.5).is_none());
			assert!(!broken.is_straight());
			assert!(broken.perpendicular_parameters_from(DVec2::ZERO).is_empty());
			assert!(broken.tangent_parameters_from(DVec2::ZERO).is_empty());
			assert!(broken.line_intersections(DVec2::ZERO, DVec2::X).is_empty());
			assert!(broken.circle_intersections(DVec2::ZERO, 1.).is_empty());
			assert!(broken.curve_intersections(&curve).is_empty());
			assert!(curve.curve_intersections(&broken).is_empty());
		}
	}
}
