use crate::curve::Curve;
use glam::{DMat2, DVec2};

/// The positions a point may occupy while it stays snapped to something that leaves it one freedom.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Locus {
	/// An endless straight line through a point, along a unit direction.
	Line {
		point: DVec2,
		direction: DVec2,
	},
	Circle {
		center: DVec2,
		radius: f64,
	},
	/// A bounded piece of a path.
	Curve(Curve),
}

impl Locus {
	/// The endless line through `point` along `direction`. Returns `None` if the direction has no length.
	pub fn line(point: DVec2, direction: DVec2) -> Option<Self> {
		let direction = direction.try_normalize()?;
		point.is_finite().then_some(Self::Line { point, direction })
	}

	/// The endless line through two points. Returns `None` if they coincide.
	pub fn line_through(a: DVec2, b: DVec2) -> Option<Self> {
		Self::line(a, b - a)
	}

	/// Returns `None` unless the radius is positive and finite.
	pub fn circle(center: DVec2, radius: f64) -> Option<Self> {
		(center.is_finite() && radius.is_finite() && radius > 0.).then_some(Self::Circle { center, radius })
	}

	pub fn is_straight(&self) -> bool {
		match self {
			Self::Line { .. } => true,
			Self::Circle { .. } => false,
			Self::Curve(curve) => curve.is_straight(),
		}
	}

	/// The position on the locus closest to a point.
	pub fn nearest(&self, point: DVec2) -> DVec2 {
		match *self {
			Self::Line { point: origin, direction } => origin + direction * (point - origin).dot(direction),
			Self::Circle { center, radius } => center + (point - center).try_normalize().unwrap_or(DVec2::X) * radius,
			Self::Curve(curve) => curve.nearest(point).position,
		}
	}

	/// The position on the locus that appears closest to a point, when `seen` carries small steps in this space to steps on screen.
	///
	/// A view that only turns and scales evenly agrees with [`Locus::nearest`], while one that stretches or foreshortens does not.
	pub fn nearest_as_seen(&self, point: DVec2, seen: DMat2) -> DVec2 {
		if keeps_shape(seen) || seen.determinant() == 0. || !seen.is_finite() {
			return self.nearest(point);
		}

		let nearest = match *self {
			Self::Line { point: origin, direction } => {
				let seen_direction = seen * direction;
				origin + direction * (seen * (point - origin)).dot(seen_direction) / seen_direction.length_squared()
			}
			Self::Circle { center, radius } => center + DVec2::from_angle(nearest_angle_as_seen(seen * (center - point), seen * radius)) * radius,
			Self::Curve(curve) => {
				// A straight-line map keeps every parameter of a curve where it was, so the nearest one is found on the curve as seen
				let affine = kurbo::Affine::new([seen.x_axis.x, seen.x_axis.y, seen.y_axis.x, seen.y_axis.y, 0., 0.]);
				curve.evaluate(Curve::new(affine * curve.segment()).nearest(seen * point).t)
			}
		};

		if nearest.is_finite() { nearest } else { self.nearest(point) }
	}

	/// The unit direction of travel along the locus where it passes closest to a point. Returns `None` for a curve with no length or with non-finite coordinates.
	pub fn direction_near(&self, point: DVec2) -> Option<DVec2> {
		match *self {
			Self::Line { direction, .. } => Some(direction),
			Self::Circle { center, .. } => Some((point - center).try_normalize().unwrap_or(DVec2::X).perp()),
			Self::Curve(curve) => curve.tangent(curve.nearest(point).t),
		}
	}

	/// The locus after every position `p` on it is carried to `matrix * p + offset`.
	/// Returns `None` if that collapses a line to a point, or would squash a circle into an ellipse.
	pub fn transformed(&self, matrix: DMat2, offset: DVec2) -> Option<Self> {
		match *self {
			Self::Line { point, direction } => Self::line(matrix * point + offset, matrix * direction),
			Self::Circle { center, radius } => keeps_shape(matrix).then(|| Self::circle(matrix * center + offset, radius * matrix.x_axis.length())).flatten(),
			Self::Curve(curve) => {
				let affine = kurbo::Affine::new([matrix.x_axis.x, matrix.x_axis.y, matrix.y_axis.x, matrix.y_axis.y, offset.x, offset.y]);
				Some(Self::Curve(Curve::new(affine * curve.segment())))
			}
		}
	}

	/// Every position where two loci meet. Loci that coincide along a stretch, such as overlapping lines, meet nowhere in particular and give nothing.
	pub fn crossings(&self, other: &Self) -> Vec<DVec2> {
		match (*self, *other) {
			(
				Self::Line { point, direction },
				Self::Line {
					point: other_point,
					direction: other_direction,
				},
			) => line_line_crossing(point, direction, other_point, other_direction).into_iter().collect(),
			(Self::Line { point, direction }, Self::Circle { center, radius }) | (Self::Circle { center, radius }, Self::Line { point, direction }) => {
				line_circle_crossings(point, direction, center, radius)
			}
			(
				Self::Circle { center, radius },
				Self::Circle {
					center: other_center,
					radius: other_radius,
				},
			) => circle_circle_crossings(center, radius, other_center, other_radius),
			(Self::Line { point, direction }, Self::Curve(curve)) | (Self::Curve(curve), Self::Line { point, direction }) => {
				curve.line_intersections(point, direction).into_iter().map(|t| curve.evaluate(t)).collect()
			}
			(Self::Circle { center, radius }, Self::Curve(curve)) | (Self::Curve(curve), Self::Circle { center, radius }) => {
				curve.circle_intersections(center, radius).into_iter().map(|t| curve.evaluate(t)).collect()
			}
			(Self::Curve(curve), Self::Curve(other_curve)) => curve.curve_intersections(&other_curve).into_iter().map(|(t, _)| curve.evaluate(t)).collect(),
		}
	}
}

/// How nearly parallel two unit directions must be, as the sine of the angle between them, before their lines are treated as never meeting.
const PARALLEL_SINE: f64 = 1e-12;

/// Whether a matrix only turns, mirrors, and scales evenly, so that circles stay circles and the nearest of two positions stays the nearest.
fn keeps_shape(matrix: DMat2) -> bool {
	let scale = matrix.x_axis.length();
	scale > 0. && (matrix.y_axis.length() - scale).abs() <= 1e-9 * scale && matrix.x_axis.dot(matrix.y_axis).abs() <= 1e-9 * scale * scale
}

/// The angle around a circle at which it comes closest to the origin, for a circle that the view has turned into an ellipse.
/// The ellipse is given by where its center appears and by the matrix carrying unit directions to its rim.
fn nearest_angle_as_seen(seen_center: DVec2, to_rim: DMat2) -> f64 {
	let squared_distance = |angle: f64| (seen_center + to_rim * DVec2::from_angle(angle)).length_squared();

	const SAMPLES: usize = 32;
	let spacing = std::f64::consts::TAU / SAMPLES as f64;
	let samples: [f64; SAMPLES] = std::array::from_fn(|sample| squared_distance(sample as f64 * spacing));

	// The distance dips at most twice around an ellipse, and the lowest sample need not be in the deeper dip, so each is followed to its bottom
	let mut nearest = (f64::INFINITY, 0.);
	for sample in 0..SAMPLES {
		let [before, after] = [samples[(sample + SAMPLES - 1) % SAMPLES], samples[(sample + 1) % SAMPLES]];
		let in_a_dip = samples[sample] <= before && samples[sample] < after;
		if !in_a_dip {
			continue;
		}

		let angle = bottom_of_dip(seen_center, to_rim, sample as f64 * spacing, spacing);
		if squared_distance(angle) < nearest.0 {
			nearest = (squared_distance(angle), angle);
		}
	}

	nearest.1
}

/// The angle within `spacing` of `around` at which a circle seen as an ellipse comes closest to the origin, given that it dips somewhere in that span.
fn bottom_of_dip(seen_center: DVec2, to_rim: DMat2, around: f64, spacing: f64) -> f64 {
	let squared_distance = |angle: f64| (seen_center + to_rim * DVec2::from_angle(angle)).length_squared();

	// A golden section search narrows the span without ever leaving the dip
	let golden = (5_f64.sqrt() - 1.) / 2.;
	let (mut low, mut high) = (around - spacing, around + spacing);
	for _ in 0..40 {
		let [inner_low, inner_high] = [high - (high - low) * golden, low + (high - low) * golden];
		if squared_distance(inner_low) < squared_distance(inner_high) {
			high = inner_high;
		} else {
			low = inner_low;
		}
	}

	// The distance is too flat at the bottom for that search to place it finely, which Newton's method on the slope then does
	let mut angle = (low + high) / 2.;
	for _ in 0..4 {
		let rim = to_rim * DVec2::from_angle(angle);
		let along_rim = to_rim * DVec2::from_angle(angle).perp();
		let toward = seen_center + rim;

		let slope = toward.dot(along_rim);
		let curvature = along_rim.length_squared() - toward.dot(rim);
		let step = slope / curvature;
		if !step.is_finite() || curvature <= 0. || (angle - step - around).abs() > spacing {
			break;
		}
		angle -= step;
	}

	angle
}

fn line_line_crossing(point: DVec2, direction: DVec2, other_point: DVec2, other_direction: DVec2) -> Option<DVec2> {
	let sine = direction.perp_dot(other_direction);
	if sine.abs() < PARALLEL_SINE {
		return None;
	}

	let along = (other_point - point).perp_dot(other_direction) / sine;
	let crossing = point + direction * along;
	crossing.is_finite().then_some(crossing)
}

fn line_circle_crossings(point: DVec2, direction: DVec2, center: DVec2, radius: f64) -> Vec<DVec2> {
	let foot = point + direction * (center - point).dot(direction);
	let offset = foot.distance(center);

	// A line grazing the circle within rounding error of its radius touches it at the foot of the perpendicular
	let grazing_tolerance = radius * 1e-12;
	if offset > radius + grazing_tolerance {
		return Vec::new();
	}
	if offset >= radius - grazing_tolerance {
		return vec![foot];
	}

	let half_chord = (radius * radius - offset * offset).sqrt();
	vec![foot - direction * half_chord, foot + direction * half_chord]
}

fn circle_circle_crossings(center: DVec2, radius: f64, other_center: DVec2, other_radius: f64) -> Vec<DVec2> {
	let between = other_center - center;
	let distance = between.length();
	let Some(toward) = between.try_normalize() else { return Vec::new() };

	// The crossings lie on the line perpendicular to the one joining the centers, at this distance along it from the first center
	let along = (distance * distance + radius * radius - other_radius * other_radius) / (2. * distance);
	line_circle_crossings(center + toward * along, toward.perp(), center, radius)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn assert_points_match(actual: Vec<DVec2>, expected: &[DVec2]) {
		assert_eq!(actual.len(), expected.len(), "{actual:?} should match {expected:?}");
		for point in expected {
			assert!(actual.iter().any(|found| found.abs_diff_eq(*point, 1e-9)), "{actual:?} should contain {point:?}");
		}
	}

	#[test]
	fn degenerate_loci_are_rejected() {
		assert!(Locus::line(DVec2::ZERO, DVec2::ZERO).is_none());
		assert!(Locus::line_through(DVec2::ONE, DVec2::ONE).is_none());
		assert!(Locus::circle(DVec2::ZERO, 0.).is_none());
		assert!(Locus::circle(DVec2::ZERO, f64::NAN).is_none());
	}

	#[test]
	fn nearest_positions_lie_on_each_kind_of_locus() {
		let line = Locus::line(DVec2::new(0., 2.), DVec2::new(3., 0.)).unwrap();
		assert!(line.nearest(DVec2::new(7., 9.)).abs_diff_eq(DVec2::new(7., 2.), 1e-12));

		let circle = Locus::circle(DVec2::new(1., 1.), 5.).unwrap();
		assert!(circle.nearest(DVec2::new(1., 20.)).abs_diff_eq(DVec2::new(1., 6.), 1e-12));

		// The center has no nearest position, so one is chosen rather than producing a result that is not a number
		assert!(circle.nearest(DVec2::new(1., 1.)).is_finite());

		let curve = Locus::Curve(Curve::line(DVec2::ZERO, DVec2::new(10., 0.)));
		assert!(curve.nearest(DVec2::new(15., 3.)).abs_diff_eq(DVec2::new(10., 0.), 1e-12));
	}

	#[test]
	fn the_nearest_position_as_seen_is_the_one_closest_on_screen() {
		// A view that stretches one direction three times as much as the other, and turns a little
		let seen = DMat2::from_cols(DVec2::new(1., 0.2), DVec2::new(-0.6, 3.));
		let point = DVec2::new(6., 1.);

		let loci = [
			Locus::line(DVec2::ZERO, DVec2::ONE).unwrap(),
			Locus::circle(DVec2::new(1., -2.), 4.).unwrap(),
			Locus::Curve(Curve::cubic(DVec2::ZERO, DVec2::new(1., 5.), DVec2::new(6., 5.), DVec2::new(9., -1.))),
		];
		for locus in loci {
			let nearest = locus.nearest_as_seen(point, seen);
			let seen_distance = |position: DVec2| (seen * (position - point)).length();

			// It lies on the locus, and no position along the locus around it appears any closer
			assert!(locus.nearest(nearest).abs_diff_eq(nearest, 1e-9));
			for step in -200..=200 {
				let along = locus.nearest(nearest + locus.direction_near(nearest).unwrap() * step as f64 * 0.05);
				assert!(seen_distance(nearest) <= seen_distance(along) + 1e-9);
			}

			// The plain nearest position is a different one under such a view
			assert!(seen_distance(nearest) < seen_distance(locus.nearest(point)) - 1e-3);
		}
	}

	#[test]
	fn the_nearest_position_on_a_circle_is_found_however_much_the_view_stretches() {
		let (center, radius) = (DVec2::new(3., -2.), 5.);
		let circle = Locus::circle(center, radius).unwrap();
		let around = |step: usize, steps: usize, distance: f64| center + DVec2::from_angle(step as f64 / steps as f64 * std::f64::consts::TAU) * distance;
		let all_around: Vec<DVec2> = (0..20_000).map(|sample| around(sample, 20_000, radius)).collect();

		for stretch in [2., 6., 8., 16., 40.] {
			let seen = DMat2::from_cols(DVec2::new(stretch, 0.3), DVec2::new(-0.2, 1.));
			let seen_distance = |from: DVec2, to: DVec2| (seen * (to - from)).length();

			// From a little way inside and outside the circle all around it, where a stretched view makes two sides of it nearly as close
			for step in 0..90 {
				for offset in [-0.9, -0.3, 0.2, 0.7] {
					let point = around(step, 90, radius + offset);

					let found = seen_distance(point, circle.nearest_as_seen(point, seen));
					let slowly_found = all_around.iter().map(|&position| seen_distance(point, position)).fold(f64::INFINITY, f64::min);
					assert!(found <= slowly_found + 1e-9, "{found} should be {slowly_found} for {point:?} at a stretch of {stretch}");
				}
			}
		}
	}

	#[test]
	fn a_view_that_keeps_shapes_sees_the_same_nearest_position() {
		let seen = DMat2::from_scale_angle(DVec2::splat(2.5), 0.8);
		let point = DVec2::new(6., 1.);

		let circle = Locus::circle(DVec2::new(1., -2.), 4.).unwrap();
		assert_eq!(circle.nearest_as_seen(point, seen), circle.nearest(point));
		assert_eq!(circle.nearest_as_seen(point, DMat2::ZERO), circle.nearest(point));
	}

	#[test]
	fn directions_follow_the_locus() {
		let circle = Locus::circle(DVec2::ZERO, 2.).unwrap();
		let direction = circle.direction_near(DVec2::new(5., 0.)).unwrap();
		assert!(direction.dot(DVec2::X).abs() < 1e-12);

		let curve = Locus::Curve(Curve::line(DVec2::ZERO, DVec2::new(0., 4.)));
		assert!(curve.direction_near(DVec2::new(1., 1.)).unwrap().abs_diff_eq(DVec2::Y, 1e-12));
	}

	#[test]
	fn lines_cross_once_unless_parallel() {
		let horizontal = Locus::line(DVec2::new(0., 3.), DVec2::X).unwrap();
		let diagonal = Locus::line(DVec2::ZERO, DVec2::ONE).unwrap();
		assert_points_match(horizontal.crossings(&diagonal), &[DVec2::new(3., 3.)]);

		let parallel = Locus::line(DVec2::new(0., 5.), DVec2::NEG_X).unwrap();
		assert!(horizontal.crossings(&parallel).is_empty());
		assert!(horizontal.crossings(&horizontal).is_empty());
	}

	#[test]
	fn a_line_meets_a_circle_twice_once_when_grazing_or_not_at_all() {
		let circle = Locus::circle(DVec2::ZERO, 5.).unwrap();

		let through = Locus::line(DVec2::new(-9., 3.), DVec2::X).unwrap();
		assert_points_match(through.crossings(&circle), &[DVec2::new(-4., 3.), DVec2::new(4., 3.)]);
		assert_points_match(circle.crossings(&through), &[DVec2::new(-4., 3.), DVec2::new(4., 3.)]);

		let grazing = Locus::line(DVec2::new(-9., 5.), DVec2::X).unwrap();
		assert_points_match(grazing.crossings(&circle), &[DVec2::new(0., 5.)]);

		let missing = Locus::line(DVec2::new(-9., 5.1), DVec2::X).unwrap();
		assert!(missing.crossings(&circle).is_empty());
	}

	#[test]
	fn circles_meet_twice_once_when_touching_or_not_at_all() {
		let circle = Locus::circle(DVec2::ZERO, 5.).unwrap();

		let overlapping = Locus::circle(DVec2::new(8., 0.), 5.).unwrap();
		assert_points_match(circle.crossings(&overlapping), &[DVec2::new(4., 3.), DVec2::new(4., -3.)]);

		let touching = Locus::circle(DVec2::new(10., 0.), 5.).unwrap();
		assert_points_match(circle.crossings(&touching), &[DVec2::new(5., 0.)]);

		let inside = Locus::circle(DVec2::new(1., 0.), 2.).unwrap();
		assert!(circle.crossings(&inside).is_empty());

		let apart = Locus::circle(DVec2::new(20., 0.), 5.).unwrap();
		assert!(circle.crossings(&apart).is_empty());

		let concentric = Locus::circle(DVec2::ZERO, 3.).unwrap();
		assert!(circle.crossings(&concentric).is_empty());
	}

	#[test]
	fn curves_meet_lines_circles_and_each_other() {
		let arch = Curve::cubic(DVec2::new(-5., 0.), DVec2::new(-5., 8.), DVec2::new(5., 8.), DVec2::new(5., 0.));
		let locus = Locus::Curve(arch);

		// The arch peaks at a height of 6, so a line at a height of 3 crosses each side of it once, mirrored about the Y axis
		let across = locus.crossings(&Locus::line(DVec2::new(0., 3.), DVec2::X).unwrap());
		assert_eq!(across.len(), 2);
		assert!(across.iter().all(|crossing| (crossing.y - 3.).abs() < 1e-9));
		assert!((across[0].x + across[1].x).abs() < 1e-9);

		let around = locus.crossings(&Locus::circle(DVec2::ZERO, 5.5).unwrap());
		assert_eq!(around.len(), 2);
		assert!(around.iter().all(|crossing| (crossing.length() - 5.5).abs() < 1e-9));

		let chord = Locus::Curve(Curve::line(DVec2::new(-10., 3.), DVec2::new(0., 3.)));
		let left_side_only = locus.crossings(&chord);
		assert_eq!(left_side_only.len(), 1);
		assert!(left_side_only[0].x < 0.);
	}

	#[test]
	fn a_map_that_keeps_shapes_carries_every_kind_of_locus() {
		// A half turn about a point with a doubling in size, as when a feature mirrors the pointer about a center
		let (matrix, offset) = (DMat2::from_diagonal(DVec2::splat(-2.)), DVec2::new(3., -2.));
		let carry = |point: DVec2| matrix * point + offset;
		let probe = DVec2::new(1., 7.);

		let loci = [
			Locus::line(DVec2::ZERO, DVec2::ONE).unwrap(),
			Locus::circle(DVec2::ONE, 4.).unwrap(),
			Locus::Curve(Curve::cubic(DVec2::ZERO, DVec2::new(1., 3.), DVec2::new(4., 3.), DVec2::new(5., 0.))),
		];
		for locus in loci {
			let carried = locus.transformed(matrix, offset).unwrap();
			assert!(carried.nearest(carry(probe)).abs_diff_eq(carry(locus.nearest(probe)), 1e-9));
		}
	}

	#[test]
	fn a_map_that_squashes_is_refused_only_by_circles() {
		let squash = DMat2::from_diagonal(DVec2::new(1., 3.));

		assert!(Locus::circle(DVec2::ONE, 4.).unwrap().transformed(squash, DVec2::ZERO).is_none());
		assert!(Locus::line(DVec2::ZERO, DVec2::ONE).unwrap().transformed(squash, DVec2::ZERO).is_some());
		assert!(Locus::line(DVec2::ZERO, DVec2::X).unwrap().transformed(DMat2::from_diagonal(DVec2::new(0., 1.)), DVec2::ZERO).is_none());
	}
}
