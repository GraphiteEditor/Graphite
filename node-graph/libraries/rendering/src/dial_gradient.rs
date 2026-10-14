//! Gradients whose two ends are each a contour dialed between a line, a circle, and a point, with the contours between
//! them filling in as the pencil of circles the two end contours span.
//!
//! Everything here is in gradient space, where the gradient line runs from the start point at the origin to the end point
//! at (1, 0). Each end's contour passes through its endpoint with its center on the line, and is described by its dial
//! `α ∈ [-1, 1]`: 0 is the line perpendicular to the gradient line, ±1 is the endpoint itself, and in between is a circle
//! bowing toward (positive) or away from (negative) the other endpoint.
//!
//! The contours between are the members of the pencil the end contours span. Each member is spaced by where it crosses
//! the gradient line, so a stop at 0.3 sits where the 0.3 contour crosses the line. This reproduces the linear and radial
//! forms exactly, since their contours cross the line at a distance in proportion to their place along the ramp.

use glam::DVec2;
use std::f64::consts::FRAC_PI_2;

pub use graphic_types::vector_types::gradient::{LINEAR_DIALS, RADIAL_DIALS};

/// Stands in for a t of infinity, on the contours whose crossing with the gradient line is at infinity.
const UNBOUNDED_T: f64 = 1e6;

/// A generalized circle `a(x² + y²) + bx + c = 0` with its center on the gradient line: a circle, a line where `a` is zero,
/// or a point where it has zero radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisCircle {
	pub a: f64,
	pub b: f64,
	pub c: f64,
}

impl AxisCircle {
	/// The contour through the start point at the origin, dialed by `alpha` toward the end point.
	pub fn start(alpha: f64) -> Self {
		let (sin, cos) = (alpha * FRAC_PI_2).sin_cos();
		Self { a: sin, b: -2. * cos, c: 0. }
	}

	/// The contour through the end point at (1, 0), dialed by `alpha` toward the start point.
	pub fn end(alpha: f64) -> Self {
		let (sin, cos) = (alpha * FRAC_PI_2).sin_cos();
		Self {
			a: sin,
			b: 2. * cos - 2. * sin,
			c: sin - 2. * cos,
		}
	}

	pub fn value(&self, point: DVec2) -> f64 {
		self.a * point.length_squared() + self.b * point.x + self.c
	}

	/// The points where it crosses the gradient line, in homogeneous form.
	fn crossings(&self) -> [Homogeneous; 2] {
		solve_quadratic(self.a, self.b, self.c)
	}

	/// The member of the pencil `self·weight + other·other_weight`.
	fn combine(&self, weight: f64, other: &Self, other_weight: f64) -> Self {
		Self {
			a: self.a * weight + other.a * other_weight,
			b: self.b * weight + other.b * other_weight,
			c: self.c * weight + other.c * other_weight,
		}
	}
}

/// A point on the gradient line in homogeneous form, `numerator / denominator`, so the point at infinity is just a zero denominator.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Homogeneous {
	numerator: f64,
	denominator: f64,
}

impl Homogeneous {
	const INFINITY: Self = Self { numerator: 1., denominator: 0. };

	fn value(&self) -> Option<f64> {
		(self.denominator != 0.).then(|| self.numerator / self.denominator)
	}

	/// Scaled so the larger of its two parts has a magnitude of 1, keeping products of points in range.
	fn normalized(self) -> Self {
		let scale = self.numerator.abs().max(self.denominator.abs());
		if scale == 0. {
			return self;
		}
		Self {
			numerator: self.numerator / scale,
			denominator: self.denominator / scale,
		}
	}
}

/// The roots of `a y² + b y + c = 0`, in homogeneous form so a vanishing `a` puts a root at infinity rather than dividing by zero.
fn solve_quadratic(a: f64, b: f64, c: f64) -> [Homogeneous; 2] {
	let discriminant = (b * b - 4. * a * c).max(0.);
	let sign = if b >= 0. { 1. } else { -1. };
	let q = -0.5 * (b + sign * discriminant.sqrt());

	[Homogeneous { numerator: q, denominator: a }.normalized(), Homogeneous { numerator: c, denominator: q }.normalized()]
}

/// How the crossing of a pixel's contour is told apart from the other place the same circle crosses the gradient line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CrossingSelection {
	/// The end contours cross, so every member passes through the two points where they cross, and the gradient line is split by
	/// the line through those points at `radical_x`. Each side of a member circle is its own contour, crossing the gradient line
	/// on the same side of `radical_x` as the pixel.
	SameSideAs { radical_x: f64 },
	/// The end contours don't cross, so the members nest around two limit points that split the gradient line into two arcs, and
	/// every member crosses each arc once. The crossing on the arc holding the gradient's band is the one where
	/// `quadratic[0] y² + quadratic[1] y + quadratic[2]` (with `y` in homogeneous form) is smallest.
	OnArc { quadratic: [f64; 3] },
}

/// Where the arc holding the band passes through infinity, the crossings beyond infinity on its other side.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wrap {
	/// A point on the gradient line between the two limit points, which separates the arc's two sides of infinity.
	pub reference_x: f64,
	/// Which side of `reference_x` the band is on, `1` or `-1`.
	pub band_side: f64,
	/// The t of the crossings on the far side of infinity.
	pub t: f64,
}

/// Everything needed to find the t of any pixel in a dialed gradient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PencilParameters {
	pub start: AxisCircle,
	pub end: AxisCircle,
	pub selection: CrossingSelection,
	pub wrap: Option<Wrap>,
	/// The t of a crossing at `y` is `y * t_scale + t_offset`.
	pub t_scale: f64,
	pub t_offset: f64,
}

impl PencilParameters {
	/// The parameters of the gradient with these two dials, which must be finite and within -1 to 1.
	pub fn new(start_dial: f64, end_dial: f64) -> Self {
		let start = AxisCircle::start(start_dial);
		let end = AxisCircle::end(end_dial);

		// The members of the pencil that are points have a zero discriminant, which as a quadratic form in the two members'
		// weights has these coefficients. Its determinant says whether the pencil has two such points (it's hyperbolic), none
		// (elliptic, where the end contours cross), or one (parabolic, where they touch).
		let point_quadratic_start = start.b * start.b - 4. * start.a * start.c;
		let point_quadratic_cross = start.b * end.b - 2. * (start.a * end.c + end.a * start.c);
		let point_quadratic_end = end.b * end.b - 4. * end.a * end.c;
		let determinant = point_quadratic_start * point_quadratic_end - point_quadratic_cross * point_quadratic_cross;
		let scale = point_quadratic_start.abs().max(point_quadratic_end.abs()).max(point_quadratic_cross.abs()).max(f64::MIN_POSITIVE);

		if determinant > 1e-12 * scale * scale {
			// The line through the two points where the end contours cross is the member with no curvature
			let line = start.combine(end.a, &end, -start.a);
			let radical_x = -line.c / line.b;

			return Self::with_band(start, end, CrossingSelection::SameSideAs { radical_x }, None, 0., 1.);
		}

		// The limit points are where the point members sit, with a point member of no curvature being the point at infinity
		let limit_points = point_member_weights(point_quadratic_start, point_quadratic_cross, point_quadratic_end).map(|(start_weight, end_weight)| {
			let member = start.combine(start_weight, &end, end_weight);
			let magnitude = member.a.abs().max(member.b.abs()).max(member.c.abs());
			if member.a.abs() <= 1e-12 * magnitude {
				return Homogeneous::INFINITY;
			}

			Homogeneous {
				numerator: -member.b,
				denominator: 2. * member.a,
			}
			.normalized()
		});

		// Each crossing's place relative to the limit points, negative between them and positive beyond them
		let arc_quadratic = arc_quadratic(limit_points);
		let arc_of = |point: Homogeneous| evaluate_homogeneous_quadratic(arc_quadratic, point.normalized());

		// Find each end contour's crossing on each arc, where a point contour (a limit point itself) lies on both
		let choose_on_arc = |crossings: [Homogeneous; 2], negative_arc: bool| {
			crossings
				.into_iter()
				.min_by(|a, b| {
					let (a, b) = (arc_of(*a), arc_of(*b));
					if negative_arc { a.total_cmp(&b) } else { b.total_cmp(&a) }
				})
				.unwrap_or(Homogeneous::INFINITY)
		};
		let candidates = [true, false].map(|negative_arc| {
			let band_start = choose_on_arc(start.crossings(), negative_arc);
			let band_end = choose_on_arc(end.crossings(), negative_arc);
			(negative_arc, band_start, band_end)
		});

		// A band through infinity would give a contour within it an infinite t, so it's unusable. Among the rest, prefer the
		// band that keeps the endpoints at t of 0 and 1.
		let limit_values = limit_points.map(|point| point.value());
		let usable = |band_start: Homogeneous, band_end: Homogeneous| {
			let (Some(band_start), Some(band_end)) = (band_start.value(), band_end.value()) else {
				return false;
			};
			match limit_values {
				[Some(first), Some(second)] => {
					let (low, high) = (first.min(second), first.max(second));
					let beyond = |y: f64| {
						if y > high {
							1
						} else if y < low {
							-1
						} else {
							0
						}
					};
					// On the outer arc, crossings on opposite sides of the limit points lie across infinity from each other
					beyond(band_start) * beyond(band_end) >= 0
				}
				_ => true,
			}
		};
		let truthfulness = |band_start: Homogeneous, band_end: Homogeneous| {
			let is = |point: Homogeneous, target: f64| point.value().is_some_and(|value| (value - target).abs() < 1e-9);
			is(band_start, 0.) as u8 + is(band_end, 1.) as u8
		};
		let chosen = candidates
			.into_iter()
			.filter(|&(_, band_start, band_end)| usable(band_start, band_end))
			.max_by_key(|&(_, band_start, band_end)| truthfulness(band_start, band_end))
			.unwrap_or(candidates[1]);
		let (negative_arc, band_start, band_end) = chosen;

		// Choosing the smallest value picks the crossing on the chosen arc
		let sign = if negative_arc { 1. } else { -1. };
		let selection = CrossingSelection::OnArc {
			quadratic: arc_quadratic.map(|coefficient| coefficient * sign),
		};

		let band_start = band_start.value().unwrap_or(0.);
		let band_end = band_end.value().unwrap_or(1.);

		// An outer arc between two finite limit points passes through infinity, beyond which its crossings belong past the band's far end
		let wrap = match (negative_arc, limit_values) {
			(false, [Some(first), Some(second)]) if (first - second).abs() > 0. => {
				let reference_x = (first + second) / 2.;
				let band_side = if band_start > reference_x { 1. } else { -1. };
				let t_direction = (band_end - band_start).signum();
				Some(Wrap {
					reference_x,
					band_side,
					t: band_side * t_direction * UNBOUNDED_T,
				})
			}
			_ => None,
		};

		Self::with_band(start, end, selection, wrap, band_start, band_end)
	}

	fn with_band(start: AxisCircle, end: AxisCircle, selection: CrossingSelection, wrap: Option<Wrap>, band_start: f64, band_end: f64) -> Self {
		let width = band_end - band_start;
		let width = if width.abs() < 1e-12 { 1e-12_f64.copysign(width) } else { width };

		Self {
			start,
			end,
			selection,
			wrap,
			t_scale: 1. / width,
			t_offset: -band_start / width,
		}
	}

	/// The gradient's t at `point` in gradient space, by the same steps the renderer's shader takes.
	pub fn t(&self, point: DVec2) -> f64 {
		let start_value = self.start.value(point);
		let end_value = self.end.value(point);
		let scale = start_value.abs().max(end_value.abs());
		if scale == 0. {
			return 0.;
		}
		let (start_value, end_value) = (start_value / scale, end_value / scale);

		// The pixel's contour is the member where the end contours' values keep this ratio, which crosses the gradient line at the
		// roots of this quadratic
		let a = end_value * self.start.a - start_value * self.end.a;
		let b = end_value * self.start.b - start_value * self.end.b;
		let c = end_value * self.start.c - start_value * self.end.c;
		let roots = solve_quadratic(a, b, c);

		let crossing = match self.selection {
			CrossingSelection::SameSideAs { radical_x } => {
				// A pixel on the dividing line itself is on the line member, whose crossing is the finite root
				let side = |root: Homogeneous| (root.numerator - radical_x * root.denominator) * root.denominator * (point.x - radical_x);
				let (first, second) = (side(roots[0]), side(roots[1]));
				let first_wins = if first == second {
					roots[0].denominator.abs() >= roots[1].denominator.abs()
				} else {
					first > second
				};
				if first_wins { roots[0] } else { roots[1] }
			}
			CrossingSelection::OnArc { quadratic } => {
				if evaluate_homogeneous_quadratic(quadratic, roots[0]) <= evaluate_homogeneous_quadratic(quadratic, roots[1]) {
					roots[0]
				} else {
					roots[1]
				}
			}
		};

		if let Some(wrap) = self.wrap
			&& (crossing.numerator - wrap.reference_x * crossing.denominator) * crossing.denominator * wrap.band_side < 0.
		{
			return wrap.t;
		}

		if crossing.denominator == 0. {
			let direction = crossing.numerator * self.t_scale;
			return if direction == 0. { 0. } else { direction.signum() * UNBOUNDED_T };
		}
		crossing.numerator / crossing.denominator * self.t_scale + self.t_offset
	}
}

impl PencilParameters {
	/// The same parameters for Vello GPU's pencil gradient paint.
	pub fn to_vello(&self) -> vello_gpu::PencilGeometry {
		let circle = |circle: &AxisCircle| [circle.a as f32, circle.b as f32, circle.c as f32];

		vello_gpu::PencilGeometry {
			start: circle(&self.start),
			end: circle(&self.end),
			selection: match self.selection {
				CrossingSelection::SameSideAs { radical_x } => vello_gpu::PencilSelection::SameSideAs { radical_x: radical_x as f32 },
				CrossingSelection::OnArc { quadratic } => vello_gpu::PencilSelection::OnArc {
					quadratic: quadratic.map(|coefficient| coefficient as f32),
				},
			},
			wrap: self.wrap.map(|wrap| vello_gpu::PencilWrap {
				reference_x: wrap.reference_x as f32,
				band_side: wrap.band_side as f32,
				t: wrap.t as f32,
			}),
			t_scale: self.t_scale as f32,
			t_offset: self.t_offset as f32,
		}
	}

	/// The closest gradient SVG can express: a radial gradient from the start circle to the end circle when one nests inside
	/// the other, or else the linear gradient along the gradient line.
	pub fn svg_approximation(&self) -> SvgApproximation {
		let (Some(start), Some(end)) = (self.start.center_and_radius(), self.end.center_and_radius()) else {
			return SvgApproximation::Linear;
		};

		let distance = (start.0 - end.0).abs();
		let nested = distance <= (start.1 - end.1).abs();
		if !nested {
			return SvgApproximation::Linear;
		}

		SvgApproximation::Radial {
			start_center: start.0,
			start_radius: start.1,
			end_center: end.0,
			end_radius: end.1,
		}
	}
}

/// A gradient SVG can draw in place of a dialed one, in gradient space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SvgApproximation {
	Linear,
	/// From the start circle, as SVG's focal circle, out to the end circle, both centered on the gradient line.
	Radial {
		start_center: f64,
		start_radius: f64,
		end_center: f64,
		end_radius: f64,
	},
}

impl AxisCircle {
	/// Where its center sits on the gradient line, and its radius, unless it's a line.
	fn center_and_radius(&self) -> Option<(f64, f64)> {
		if self.a.abs() < 1e-9 {
			return None;
		}
		let center = -self.b / (2. * self.a);
		let radius = (center * center - self.c / self.a).max(0.).sqrt();
		Some((center, radius))
	}
}

/// The weights of the two members that are points, the roots of the homogeneous quadratic form `start s² + 2·cross·s·e + end e²`.
fn point_member_weights(start: f64, cross: f64, end: f64) -> [(f64, f64); 2] {
	let discriminant = (cross * cross - start * end).max(0.).sqrt();

	if start.abs() < 1e-12 && end.abs() < 1e-12 {
		// Both end contours are points already
		[(1., 0.), (0., 1.)]
	} else if start.abs() >= end.abs() {
		[((-cross + discriminant) / start, 1.), ((-cross - discriminant) / start, 1.)]
	} else {
		[(1., (-cross + discriminant) / end), (1., (-cross - discriminant) / end)]
	}
}

/// The quadratic in a homogeneous point that is negative between the two limit points and positive beyond them.
fn arc_quadratic([first, second]: [Homogeneous; 2]) -> [f64; 3] {
	// (y·d₁ - n₁)(y·d₂ - n₂) with the denominators kept nonnegative, so its sign doesn't depend on how each point is scaled
	let orient = |point: Homogeneous| {
		if point.denominator < 0. {
			Homogeneous {
				numerator: -point.numerator,
				denominator: -point.denominator,
			}
		} else {
			point
		}
	};
	let (first, second) = (orient(first), orient(second));

	[
		first.denominator * second.denominator,
		-(first.denominator * second.numerator + first.numerator * second.denominator),
		first.numerator * second.numerator,
	]
}

fn evaluate_homogeneous_quadratic([a, b, c]: [f64; 3], point: Homogeneous) -> f64 {
	let Homogeneous { numerator, denominator } = point;
	a * numerator * numerator + b * numerator * denominator + c * denominator * denominator
}

#[cfg(test)]
mod tests {
	use super::*;

	fn assert_close(actual: f64, expected: f64, context: &str) {
		assert!((actual - expected).abs() < 1e-6, "{context}: expected {expected}, got {actual}");
	}

	#[test]
	fn radial_dials_space_contours_by_radius() {
		let parameters = PencilParameters::new(RADIAL_DIALS[0], RADIAL_DIALS[1]);
		for radius in [0.1, 0.3, 0.75, 1., 1.5, 4.] {
			for angle in [0., 0.7, 2., 3.1, 4.5] {
				let point = DVec2::from_angle(angle) * radius;
				assert_close(parameters.t(point), radius, &format!("radius {radius} at angle {angle}"));
			}
		}
	}

	#[test]
	fn linear_dials_space_contours_by_distance_along_the_line() {
		let parameters = PencilParameters::new(LINEAR_DIALS[0], LINEAR_DIALS[1]);
		for point in [DVec2::new(0.25, 0.), DVec2::new(0.6, 3.), DVec2::new(-2., -7.), DVec2::new(5., 0.5)] {
			assert_close(parameters.t(point), point.x, &format!("point {point}"));
		}
	}

	#[test]
	fn concentric_dials_space_contours_by_radius_between_the_ends() {
		// The donut: a hole of radius 1 centered 1 before the start, and an end circle of radius 2 around the same center
		let parameters = PencilParameters::new(-0.5, 0.5_f64.atan() / FRAC_PI_2);
		let center = DVec2::new(-1., 0.);
		for radius in [1., 1.25, 1.5, 2., 3.] {
			for angle in [0., 1., 2.5, 4.] {
				let point = center + DVec2::from_angle(angle) * radius;
				assert_close(parameters.t(point), radius - 1., &format!("radius {radius} at angle {angle}"));
			}
		}
	}

	#[test]
	fn point_dials_space_contours_by_distance_ratio() {
		let parameters = PencilParameters::new(1., 1.);
		for point in [DVec2::new(0.5, 0.), DVec2::new(0.5, 2.), DVec2::new(0.2, 0.3), DVec2::new(3., -1.)] {
			// The contours are the circles of Apollonius, each crossing the line between the points at y / (1 - y) = ratio
			let ratio = point.length() / (point - DVec2::X).length();
			assert_close(parameters.t(point), ratio / (1. + ratio), &format!("point {point}"));
		}
	}

	#[test]
	fn dials_near_a_preset_render_like_it() {
		let radial = PencilParameters::new(RADIAL_DIALS[0], RADIAL_DIALS[1]);
		let near_radial = PencilParameters::new(RADIAL_DIALS[0] - 1e-7, RADIAL_DIALS[1] + 1e-7);
		let linear = PencilParameters::new(LINEAR_DIALS[0], LINEAR_DIALS[1]);
		let near_linear = PencilParameters::new(LINEAR_DIALS[0] + 1e-7, LINEAR_DIALS[1] - 1e-7);

		for point in [DVec2::new(0.3, 0.2), DVec2::new(-0.5, 0.4), DVec2::new(0.9, -0.1)] {
			assert!((radial.t(point) - near_radial.t(point)).abs() < 1e-4, "near radial at {point}");
			assert!((linear.t(point) - near_linear.t(point)).abs() < 1e-4, "near linear at {point}");
		}
	}

	#[test]
	fn focal_gradient_pads_everywhere_outside_its_end_circle() {
		// The start point sits inside the end circle, off its center
		let parameters = PencilParameters::new(1., 0.6);

		assert_close(parameters.t(DVec2::X), 1., "end point");
		let inside = parameters.t(DVec2::new(0.5, 0.));
		assert!(inside > 0. && inside < 1., "inside: {inside}");
		for point in [DVec2::new(-5., 0.), DVec2::new(-0.5, 0.), DVec2::new(5., 0.), DVec2::new(0.3, 4.), DVec2::new(-40., 30.)] {
			let t = parameters.t(point);
			assert!(t > 1., "outside at {point}: {t}");
		}
	}

	#[test]
	fn crossing_contours_fan_through_their_lens() {
		let parameters = PencilParameters::new(0.6, 0.6);
		assert!(matches!(parameters.selection, CrossingSelection::SameSideAs { .. }));

		for x in [0., 0.1, 0.25, 0.5, 0.8, 1.] {
			assert_close(parameters.t(DVec2::new(x, 0.)), x, &format!("on the line at {x}"));
		}
		// Mirrored across the gradient line
		assert_close(parameters.t(DVec2::new(0.3, 0.2)), parameters.t(DVec2::new(0.3, -0.2)), "mirror");
	}

	#[test]
	fn every_dial_pair_gives_a_number_everywhere() {
		for start_step in -10..=10 {
			for end_step in -10..=10 {
				let parameters = PencilParameters::new(start_step as f64 / 10., end_step as f64 / 10.);
				for x in -8..=8 {
					for y in -8..=8 {
						let point = DVec2::new(x as f64 * 0.37, y as f64 * 0.41);
						let t = parameters.t(point);
						assert!(!t.is_nan(), "dials ({start_step}, {end_step}) at {point}");
					}
				}
			}
		}
	}
}
