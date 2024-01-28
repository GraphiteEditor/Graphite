use super::*;
use std::fmt::Write;

/// Functionality relating to core `Bezier` operations, such as constructors and `abs_diff_eq`.
impl Bezier {
	// TODO: Consider removing this function
	/// Create a linear bezier using the provided coordinates as the start and end points.
	pub fn from_linear_coordinates(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
		Bezier {
			start: DVec2::new(x1, y1),
			handles: BezierHandles::Linear,
			end: DVec2::new(x2, y2),
		}
	}

	/// Create a linear bezier using the provided DVec2s as the start and end points.
	/// <iframe frameBorder="0" width="100%" height="300px" src="https://graphite.rs/libraries/bezier-rs#bezier/constructor/solo" title="Constructor Demo"></iframe>
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
	/// <iframe frameBorder="0" width="100%" height="375px" src="https://graphite.rs/libraries/bezier-rs#bezier/bezier-through-points/solo" title="Through Points Demo"></iframe>
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
	pub fn svg_curve_argument(&self) -> String {
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

	/// Write the curve argument to the string
	pub fn write_curve_argument(&self, svg: &mut String) -> std::fmt::Result {
		match self.handles {
			BezierHandles::Linear => svg.push_str(SVG_ARG_LINEAR),
			BezierHandles::Quadratic { handle } => write!(svg, "{SVG_ARG_QUADRATIC}{:.6},{:.6}", handle.x, handle.y)?,
			BezierHandles::Cubic { handle_start, handle_end } => write!(svg, "{SVG_ARG_CUBIC}{:.6},{:.6} {:.6},{:.6}", handle_start.x, handle_start.y, handle_end.x, handle_end.y)?,
		}
		write!(svg, " {:.6},{:.6}", self.end.x, self.end.y)
	}

	/// Return the string argument used to create the lines connecting handles to endpoints in an SVG `path`
	pub(crate) fn svg_handle_line_argument(&self) -> Option<String> {
		match self.handles {
			BezierHandles::Linear => None,
			BezierHandles::Quadratic { handle } => {
				let handle_line = format!("{SVG_ARG_LINEAR}{:.6} {:.6}", handle.x, handle.y);
				Some(format!(
					"{SVG_ARG_MOVE}{:.6} {:.6} {handle_line} {SVG_ARG_MOVE}{:.6} {:.6} {handle_line}",
					self.start.x, self.start.y, self.end.x, self.end.y
				))
			}
			BezierHandles::Cubic { handle_start, handle_end } => {
				let handle_start_line = format!("{SVG_ARG_LINEAR}{:.6} {:.6}", handle_start.x, handle_start.y);
				let handle_end_line = format!("{SVG_ARG_LINEAR}{} {}", handle_end.x, handle_end.y);
				Some(format!(
					"{SVG_ARG_MOVE}{:.6} {:.6} {handle_start_line} {SVG_ARG_MOVE}{:.6} {:.6} {handle_end_line}",
					self.start.x, self.start.y, self.end.x, self.end.y
				))
			}
		}
	}

	/// Appends to the `svg` mutable string with an SVG shape representation of the curve.
	pub fn curve_to_svg(&self, svg: &mut String, attributes: String) {
		let _ = write!(svg, r#"<path d="{SVG_ARG_MOVE}{} {} {}" {}/>"#, self.start.x, self.start.y, self.svg_curve_argument(), attributes);
	}

	/// Appends to the `svg` mutable string with an SVG shape representation of the handle lines.
	pub fn handle_lines_to_svg(&self, svg: &mut String, attributes: String) {
		let _ = write!(svg, r#"<path d="{}" {}/>"#, self.svg_handle_line_argument().unwrap_or_default(), attributes);
	}

	/// Appends to the `svg` mutable string with an SVG shape representation of the anchors.
	pub fn anchors_to_svg(&self, svg: &mut String, attributes: String) {
		let _ = write!(
			svg,
			r#"<circle cx="{}" cy="{}" {attributes}/><circle cx="{}" cy="{}" {attributes}/>"#,
			self.start.x, self.start.y, self.end.x, self.end.y
		);
	}

	/// Appends to the `svg` mutable string with an SVG shape representation of the handles.
	pub fn handles_to_svg(&self, svg: &mut String, attributes: String) {
		if let BezierHandles::Quadratic { handle } = self.handles {
			let _ = write!(svg, r#"<circle cx="{}" cy="{}" {attributes}/>"#, handle.x, handle.y);
		} else if let BezierHandles::Cubic { handle_start, handle_end } = self.handles {
			let _ = write!(
				svg,
				r#"<circle cx="{}" cy="{}" {attributes}/><circle cx="{}" cy="{}" {attributes}/>"#,
				handle_start.x, handle_start.y, handle_end.x, handle_end.y
			);
		};
	}

	/// Appends to the `svg` mutable string with an SVG shape representation that includes the curve, the handle lines, the anchors, and the handles.
	pub fn to_svg(&self, svg: &mut String, curve_attributes: String, anchor_attributes: String, handle_attributes: String, handle_line_attributes: String) {
		if !curve_attributes.is_empty() {
			self.curve_to_svg(svg, curve_attributes);
		}
		if !handle_line_attributes.is_empty() {
			self.handle_lines_to_svg(svg, handle_line_attributes);
		}
		if !anchor_attributes.is_empty() {
			self.anchors_to_svg(svg, anchor_attributes);
		}
		if !handle_attributes.is_empty() {
			self.handles_to_svg(svg, handle_attributes);
		}
	}

	/// Returns true if the corresponding points of the two `Bezier`s are within the provided absolute value difference from each other.
	/// The points considered includes the start, end, and any relevant handles.
	pub fn abs_diff_eq(&self, other: &Bezier, max_abs_diff: f64) -> bool {
		let self_points = self.get_points().collect::<Vec<DVec2>>();
		let other_points = other.get_points().collect::<Vec<DVec2>>();

		self_points.len() == other_points.len() && self_points.into_iter().zip(other_points).all(|(a, b)| a.abs_diff_eq(b, max_abs_diff))
	}

	/// Returns true if the start, end and handles of the Bezier are all at the same location
	pub fn is_point(&self) -> bool {
		let start = self.start();

		self.get_points().all(|point| point.abs_diff_eq(start, MAX_ABSOLUTE_DIFFERENCE))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::compare::compare_points;
	use crate::utils::TValue;

	#[test]
	fn test_quadratic_from_points() {
		let p1 = DVec2::new(30., 50.);
		let p2 = DVec2::new(140., 30.);
		let p3 = DVec2::new(160., 170.);

		let bezier1 = Bezier::quadratic_through_points(p1, p2, p3, None);
		assert!(compare_points(bezier1.evaluate(TValue::Parametric(0.5)), p2));

		let bezier2 = Bezier::quadratic_through_points(p1, p2, p3, Some(0.8));
		assert!(compare_points(bezier2.evaluate(TValue::Parametric(0.8)), p2));

		let bezier3 = Bezier::quadratic_through_points(p1, p2, p3, Some(0.));
		assert!(compare_points(bezier3.evaluate(TValue::Parametric(0.)), p2));
	}

	#[test]
	fn test_cubic_through_points() {
		let p1 = DVec2::new(30., 30.);
		let p2 = DVec2::new(60., 140.);
		let p3 = DVec2::new(160., 160.);

		let bezier1 = Bezier::cubic_through_points(p1, p2, p3, Some(0.3), Some(10.));
		assert!(compare_points(bezier1.evaluate(TValue::Parametric(0.3)), p2));

		let bezier2 = Bezier::cubic_through_points(p1, p2, p3, Some(0.8), Some(91.7));
		assert!(compare_points(bezier2.evaluate(TValue::Parametric(0.8)), p2));

		let bezier3 = Bezier::cubic_through_points(p1, p2, p3, Some(0.), Some(91.7));
		assert!(compare_points(bezier3.evaluate(TValue::Parametric(0.)), p2));
	}
}
