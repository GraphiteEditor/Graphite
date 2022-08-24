use super::*;
use crate::ToSVGOptions;

/// Functionality relating to core `Bezier` operations, such as constructors and `abs_diff_eq`.
impl Bezier {
	// TODO: Consider removing this function
	/// Create a quadratic bezier using the provided coordinates as the start, handle, and end points.
	pub fn from_linear_coordinates(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
		Bezier {
			start: DVec2::new(x1, y1),
			handles: BezierHandles::Linear,
			end: DVec2::new(x2, y2),
		}
	}

	/// Create a quadratic bezier using the provided DVec2s as the start, handle, and end points.
	pub fn from_linear_dvec2(p1: DVec2, p2: DVec2) -> Self {
		Bezier {
			start: p1,
			handles: BezierHandles::Linear,
			end: p2,
		}
	}

	// TODO: Consider removing this function
	/// Create a quadratic bezier using the provided coordinates as the start, handle, and end points.
	pub fn from_quadratic_coordinates(x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64) -> Self {
		Bezier {
			start: DVec2::new(x1, y1),
			handles: BezierHandles::Quadratic { handle: DVec2::new(x2, y2) },
			end: DVec2::new(x3, y3),
		}
	}

	/// Create a quadratic bezier using the provided DVec2s as the start, handle, and end points.
	pub fn from_quadratic_dvec2(p1: DVec2, p2: DVec2, p3: DVec2) -> Self {
		Bezier {
			start: p1,
			handles: BezierHandles::Quadratic { handle: p2 },
			end: p3,
		}
	}

	// TODO: Consider removing this function
	/// Create a cubic bezier using the provided coordinates as the start, handles, and end points.
	#[allow(clippy::too_many_arguments)]
	pub fn from_cubic_coordinates(x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64, x4: f64, y4: f64) -> Self {
		Bezier {
			start: DVec2::new(x1, y1),
			handles: BezierHandles::Cubic {
				handle_start: DVec2::new(x2, y2),
				handle_end: DVec2::new(x3, y3),
			},
			end: DVec2::new(x4, y4),
		}
	}

	/// Create a cubic bezier using the provided DVec2s as the start, handles, and end points.
	pub fn from_cubic_dvec2(p1: DVec2, p2: DVec2, p3: DVec2, p4: DVec2) -> Self {
		Bezier {
			start: p1,
			handles: BezierHandles::Cubic { handle_start: p2, handle_end: p3 },
			end: p4,
		}
	}

	/// Create a quadratic bezier curve that goes through 3 points, where the middle point will be at the corresponding position `t` on the curve.
	/// - `t` - A representation of how far along the curve the provided point should occur at. The default value is 0.5.
	/// Note that when `t = 0` or `t = 1`, the expectation is that the `point_on_curve` should be equal to `start` and `end` respectively.
	/// In these cases, if the provided values are not equal, this function will use the `point_on_curve` as the `start`/`end` instead.
	pub fn quadratic_through_points(start: DVec2, point_on_curve: DVec2, end: DVec2, t: Option<f64>) -> Self {
		let t = t.unwrap_or(DEFAULT_T_VALUE);
		if t == 0. {
			return Bezier::from_quadratic_dvec2(point_on_curve, point_on_curve, end);
		}
		if t == 1. {
			return Bezier::from_quadratic_dvec2(start, point_on_curve, point_on_curve);
		}
		let [a, _, _] = utils::compute_abc_for_quadratic_through_points(start, point_on_curve, end, t);
		Bezier::from_quadratic_dvec2(start, a, end)
	}

	/// Create a cubic bezier curve that goes through 3 points, where the middle point will be at the corresponding position `t` on the curve.
	/// - `t` - A representation of how far along the curve the provided point should occur at. The default value is 0.5.
	/// Note that when `t = 0` or `t = 1`, the expectation is that the `point_on_curve` should be equal to `start` and `end` respectively.
	/// In these cases, if the provided values are not equal, this function will use the `point_on_curve` as the `start`/`end` instead.
	/// - `midpoint_separation` - A representation of how wide the resulting curve will be around `t` on the curve. This parameter designates the distance between the `e1` and `e2` defined in [the projection identity section](https://pomax.github.io/bezierinfo/#abc) of Pomax's bezier curve primer. It is an optional parameter and the default value is the distance between the points `B` and `C` defined in the primer.
	pub fn cubic_through_points(start: DVec2, point_on_curve: DVec2, end: DVec2, t: Option<f64>, midpoint_separation: Option<f64>) -> Self {
		let t = t.unwrap_or(DEFAULT_T_VALUE);
		if t == 0. {
			return Bezier::from_cubic_dvec2(point_on_curve, point_on_curve, end, end);
		}
		if t == 1. {
			return Bezier::from_cubic_dvec2(start, start, point_on_curve, point_on_curve);
		}
		let [a, b, c] = utils::compute_abc_for_cubic_through_points(start, point_on_curve, end, t);
		let midpoint_separation = midpoint_separation.unwrap_or_else(|| b.distance(c));
		let distance_between_start_and_end = (end - start) / (start.distance(end));
		let e1 = b - (distance_between_start_and_end * midpoint_separation);
		let e2 = b + (distance_between_start_and_end * midpoint_separation * (1. - t) / t);

		// TODO: these functions can be changed to helpers, but need to come up with an appropriate name first
		let v1 = (e1 - t * a) / (1. - t);
		let v2 = (e2 - (1. - t) * a) / t;
		let handle_start = (v1 - (1. - t) * start) / t;
		let handle_end = (v2 - t * end) / (1. - t);
		Bezier::from_cubic_dvec2(start, handle_start, handle_end, end)
	}

	/// Return the string argument used to create a curve in an SVG `path`, excluding the start point.
	pub(crate) fn svg_curve_argument(&self) -> String {
		let handle_args = match self.handles {
			BezierHandles::Linear => SVG_ARG_LINEAR.to_string(),
			BezierHandles::Quadratic { handle } => {
				format!("{SVG_ARG_QUADRATIC}{} {}", handle.x, handle.y)
			}
			BezierHandles::Cubic { handle_start, handle_end } => {
				format!("{SVG_ARG_CUBIC}{} {} {} {}", handle_start.x, handle_start.y, handle_end.x, handle_end.y)
			}
		};
		format!("{handle_args} {} {}", self.end.x, self.end.y)
	}

	/// Return the string argument used to create the lines connecting handles to endpoints in an SVG `path`
	pub(crate) fn svg_handle_line_argument(&self) -> Option<String> {
		match self.handles {
			BezierHandles::Linear => None,
			BezierHandles::Quadratic { handle } => {
				let handle_line = format!("{SVG_ARG_LINEAR}{} {}", handle.x, handle.y);
				Some(format!(
					"{SVG_ARG_MOVE}{} {} {handle_line} {SVG_ARG_MOVE}{} {} {handle_line}",
					self.start.x, self.start.y, self.end.x, self.end.y
				))
			}
			BezierHandles::Cubic { handle_start, handle_end } => {
				let handle_start_line = format!("{SVG_ARG_LINEAR}{} {}", handle_start.x, handle_start.y);
				let handle_end_line = format!("{SVG_ARG_LINEAR}{} {}", handle_end.x, handle_end.y);
				Some(format!(
					"{SVG_ARG_MOVE}{} {} {handle_start_line} {SVG_ARG_MOVE}{} {} {handle_end_line}",
					self.start.x, self.start.y, self.end.x, self.end.y
				))
			}
		}
	}

	/// Returns an SVG representation of the `Bezier`.
	pub fn to_svg(&self, options: ToSVGOptions) -> String {
		let anchor_arguments = options.formatted_anchor_arguments();
		let anchor_circles = format!(
			r#"<circle cx="{}" cy="{}" {}/><circle cx="{}" cy="{}" {}/>"#,
			self.start.x, self.start.y, anchor_arguments, self.end.x, self.end.y, anchor_arguments
		);

		let handle_point_arguments = options.formatted_handle_point_arguments();
		let handle_circles = match self.handles {
			BezierHandles::Linear => None,
			BezierHandles::Quadratic { handle } => Some(format!(r#"<circle cx="{}" cy="{}" {}/>"#, handle.x, handle.y, handle_point_arguments)),
			BezierHandles::Cubic { handle_start, handle_end } => Some(format!(
				r#"<circle cx="{}" cy="{}" {}/><circle cx="{}" cy="{}" {}/>"#,
				handle_start.x, handle_start.y, handle_point_arguments, handle_end.x, handle_end.y, handle_point_arguments
			)),
		};

		format!(
			r#"<path d="{SVG_ARG_MOVE}{} {} {}" {}/><path d="{}" {}/>{}{}"#,
			self.start.x,
			self.start.y,
			self.svg_curve_argument(),
			options.formatted_curve_arguments(),
			self.svg_handle_line_argument().unwrap_or_else(|| "".to_string()),
			options.formatted_handle_line_arguments(),
			anchor_circles,
			handle_circles.unwrap_or_else(|| "".to_string()),
		)
	}

	/// Returns true if the corresponding points of the two `Bezier`s are within the provided absolute value difference from each other.
	/// The points considered includes the start, end, and any relevant handles.
	pub fn abs_diff_eq(&self, other: &Bezier, max_abs_diff: f64) -> bool {
		let self_points = self.get_points().collect::<Vec<DVec2>>();
		let other_points = other.get_points().collect::<Vec<DVec2>>();

		self_points.len() == other_points.len() && self_points.into_iter().zip(other_points.into_iter()).all(|(a, b)| a.abs_diff_eq(b, max_abs_diff))
	}
}

#[cfg(test)]
mod tests {
	use super::compare::compare_points;
	use super::*;

	#[test]
	fn test_quadratic_from_points() {
		let p1 = DVec2::new(30., 50.);
		let p2 = DVec2::new(140., 30.);
		let p3 = DVec2::new(160., 170.);

		let bezier1 = Bezier::quadratic_through_points(p1, p2, p3, None);
		assert!(compare_points(bezier1.evaluate(0.5), p2));

		let bezier2 = Bezier::quadratic_through_points(p1, p2, p3, Some(0.8));
		assert!(compare_points(bezier2.evaluate(0.8), p2));

		let bezier3 = Bezier::quadratic_through_points(p1, p2, p3, Some(0.));
		assert!(compare_points(bezier3.evaluate(0.), p2));
	}

	#[test]
	fn test_cubic_through_points() {
		let p1 = DVec2::new(30., 30.);
		let p2 = DVec2::new(60., 140.);
		let p3 = DVec2::new(160., 160.);

		let bezier1 = Bezier::cubic_through_points(p1, p2, p3, Some(0.3), Some(10.));
		assert!(compare_points(bezier1.evaluate(0.3), p2));

		let bezier2 = Bezier::cubic_through_points(p1, p2, p3, Some(0.8), Some(91.7));
		assert!(compare_points(bezier2.evaluate(0.8), p2));

		let bezier3 = Bezier::cubic_through_points(p1, p2, p3, Some(0.), Some(91.7));
		assert!(compare_points(bezier3.evaluate(0.), p2));
	}
}
