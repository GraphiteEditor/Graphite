//! Bezier-rs: A Bezier Math Library f&or Rust

mod consts;
mod structs;
pub mod subpath;
mod utils;

use consts::*;
pub use structs::*;
pub use subpath::*;

use glam::{DMat2, DVec2};
use std::f64::consts::PI;
use std::fmt::{Debug, Formatter, Result};
use std::ops::Range;

/// Representation of the handle point(s) in a bezier segment.
#[derive(Copy, Clone, PartialEq)]
enum BezierHandles {
	Linear,
	/// Handles for a quadratic curve.
	Quadratic {
		/// Point representing the location of the single handle.
		handle: DVec2,
	},
	/// Handles for a cubic curve.
	Cubic {
		/// Point representing the location of the handle associated to the start point.
		handle_start: DVec2,
		/// Point representing the location of the handle associated to the end point.
		handle_end: DVec2,
	},
}

/// Representation of a bezier curve with 2D points.
#[derive(Copy, Clone, PartialEq)]
pub struct Bezier {
	/// Start point of the bezier curve.
	start: DVec2,
	/// Start point of the bezier curve.
	end: DVec2,
	/// Handles of the bezier curve.
	handles: BezierHandles,
}

impl Debug for Bezier {
	fn fmt(&self, f: &mut Formatter<'_>) -> Result {
		write!(f, "{:?}", self.get_points().collect::<Vec<DVec2>>())
	}
}

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

	/// Convert `Bezier` to SVG `path`.
	pub fn to_svg(&self) -> String {
		format!(
			r#"<path d="{SVG_ARG_MOVE}{} {} {}" stroke="black" fill="none"/>"#,
			self.start.x,
			self.start.y,
			self.svg_curve_argument()
		)
	}

	/// Set the coordinates of the start point.
	pub fn set_start(&mut self, s: DVec2) {
		self.start = s;
	}

	/// Set the coordinates of the end point.
	pub fn set_end(&mut self, e: DVec2) {
		self.end = e;
	}

	/// Set the coordinates of the first handle point. This represents the only handle in a quadratic segment. If used on a linear segment, it will be changed to a quadratic.
	pub fn set_handle_start(&mut self, h1: DVec2) {
		match self.handles {
			BezierHandles::Linear => {
				self.handles = BezierHandles::Quadratic { handle: h1 };
			}
			BezierHandles::Quadratic { ref mut handle } => {
				*handle = h1;
			}
			BezierHandles::Cubic { ref mut handle_start, .. } => {
				*handle_start = h1;
			}
		};
	}

	/// Set the coordinates of the second handle point. This will convert both linear and quadratic segments into cubic ones. For a linear segment, the first handle will be set to the start point.
	pub fn set_handle_end(&mut self, h2: DVec2) {
		match self.handles {
			BezierHandles::Linear => {
				self.handles = BezierHandles::Cubic {
					handle_start: self.start,
					handle_end: h2,
				};
			}
			BezierHandles::Quadratic { handle } => {
				self.handles = BezierHandles::Cubic { handle_start: handle, handle_end: h2 };
			}
			BezierHandles::Cubic { ref mut handle_end, .. } => {
				*handle_end = h2;
			}
		};
	}

	/// Get the coordinates of the bezier segment's start point.
	pub fn start(&self) -> DVec2 {
		self.start
	}

	/// Get the coordinates of the bezier segment's end point.
	pub fn end(&self) -> DVec2 {
		self.end
	}

	/// Get the coordinates of the bezier segment's first handle point. This represents the only handle in a quadratic segment.
	pub fn handle_start(&self) -> Option<DVec2> {
		match self.handles {
			BezierHandles::Linear => None,
			BezierHandles::Quadratic { handle } => Some(handle),
			BezierHandles::Cubic { handle_start, .. } => Some(handle_start),
		}
	}

	/// Get the coordinates of the second handle point. This will return `None` for a quadratic segment.
	pub fn handle_end(&self) -> Option<DVec2> {
		match self.handles {
			BezierHandles::Linear { .. } => None,
			BezierHandles::Quadratic { .. } => None,
			BezierHandles::Cubic { handle_end, .. } => Some(handle_end),
		}
	}

	/// Get an iterator over the coordinates of all points in a vector.
	/// - For a linear segment, the order of the points will be: `start`, `end`.
	/// - For a quadratic segment, the order of the points will be: `start`, `handle`, `end`.
	/// - For a cubic segment, the order of the points will be: `start`, `handle_start`, `handle_end`, `end`.
	pub fn get_points(&self) -> impl Iterator<Item = DVec2> {
		match self.handles {
			BezierHandles::Linear => [self.start, self.end, DVec2::ZERO, DVec2::ZERO].into_iter().take(2),
			BezierHandles::Quadratic { handle } => [self.start, handle, self.end, DVec2::ZERO].into_iter().take(3),
			BezierHandles::Cubic { handle_start, handle_end } => [self.start, handle_start, handle_end, self.end].into_iter().take(4),
		}
	}

	/// Returns true if the corresponding points of the two `Bezier`s are within the provided absolute value difference from each other.
	/// The points considered includes the start, end, and any relevant handles.
	pub fn abs_diff_eq(&self, other: &Bezier, max_abs_diff: f64) -> bool {
		let self_points = self.get_points().collect::<Vec<DVec2>>();
		let other_points = other.get_points().collect::<Vec<DVec2>>();

		self_points.len() == other_points.len() && self_points.into_iter().zip(other_points.into_iter()).all(|(a, b)| a.abs_diff_eq(b, max_abs_diff))
	}

	/// Calculate the point on the curve based on the `t`-value provided.
	fn unrestricted_evaluate(&self, t: f64) -> DVec2 {
		// Basis code based off of pseudocode found here: <https://pomax.github.io/bezierinfo/#explanation>.

		let t_squared = t * t;
		let one_minus_t = 1. - t;
		let squared_one_minus_t = one_minus_t * one_minus_t;

		match self.handles {
			BezierHandles::Linear => self.start.lerp(self.end, t),
			BezierHandles::Quadratic { handle } => squared_one_minus_t * self.start + 2. * one_minus_t * t * handle + t_squared * self.end,
			BezierHandles::Cubic { handle_start, handle_end } => {
				let t_cubed = t_squared * t;
				let cubed_one_minus_t = squared_one_minus_t * one_minus_t;
				cubed_one_minus_t * self.start + 3. * squared_one_minus_t * t * handle_start + 3. * one_minus_t * t_squared * handle_end + t_cubed * self.end
			}
		}
	}

	/// Calculate the point on the curve based on the `t`-value provided.
	/// Expects `t` to be within the inclusive range `[0, 1]`.
	pub fn evaluate(&self, t: f64) -> DVec2 {
		assert!((0.0..=1.).contains(&t));
		self.unrestricted_evaluate(t)
	}

	/// Return a selection of equidistant points on the bezier curve.
	/// If no value is provided for `steps`, then the function will default `steps` to be 10.
	pub fn compute_lookup_table(&self, steps: Option<usize>) -> Vec<DVec2> {
		let steps_unwrapped = steps.unwrap_or(DEFAULT_LUT_STEP_SIZE);
		let ratio: f64 = 1. / (steps_unwrapped as f64);
		let mut steps_array = Vec::with_capacity(steps_unwrapped + 1);

		for t in 0..steps_unwrapped + 1 {
			steps_array.push(self.evaluate(f64::from(t as i32) * ratio))
		}

		steps_array
	}

	/// Return an approximation of the length of the bezier curve.
	/// - `num_subdivisions` - Number of subdivisions used to approximate the curve. The default value is 1000.
	pub fn length(&self, num_subdivisions: Option<usize>) -> f64 {
		match self.handles {
			BezierHandles::Linear => self.start.distance(self.end),
			_ => {
				// Code example from <https://gamedev.stackexchange.com/questions/5373/moving-ships-between-two-planets-along-a-bezier-missing-some-equations-for-acce/5427#5427>.

				// We will use an approximate approach where we split the curve into many subdivisions
				// and calculate the euclidean distance between the two endpoints of the subdivision
				let lookup_table = self.compute_lookup_table(Some(num_subdivisions.unwrap_or(DEFAULT_LENGTH_SUBDIVISIONS)));
				let mut approx_curve_length = 0.;
				let mut previous_point = lookup_table[0];
				// Calculate approximate distance between subdivision
				for current_point in lookup_table.iter().skip(1) {
					// Calculate distance of subdivision
					approx_curve_length += (*current_point - previous_point).length();
					// Update the previous point
					previous_point = *current_point;
				}

				approx_curve_length
			}
		}
	}

	/// Returns a Bezier representing the derivative of the original curve.
	/// - This function returns `None` for a linear segment.
	pub fn derivative(&self) -> Option<Bezier> {
		match self.handles {
			BezierHandles::Linear => None,
			BezierHandles::Quadratic { handle } => {
				let p1_minus_p0 = handle - self.start;
				let p2_minus_p1 = self.end - handle;
				Some(Bezier::from_linear_dvec2(2. * p1_minus_p0, 2. * p2_minus_p1))
			}
			BezierHandles::Cubic { handle_start, handle_end } => {
				let p1_minus_p0 = handle_start - self.start;
				let p2_minus_p1 = handle_end - handle_start;
				let p3_minus_p2 = self.end - handle_end;
				Some(Bezier::from_quadratic_dvec2(3. * p1_minus_p0, 3. * p2_minus_p1, 3. * p3_minus_p2))
			}
		}
	}

	/// Returns a normalized unit vector representing the tangent at the point designated by `t` on the curve.
	pub fn tangent(&self, t: f64) -> DVec2 {
		match self.handles {
			BezierHandles::Linear => self.end - self.start,
			_ => self.derivative().unwrap().evaluate(t),
		}
		.normalize()
	}

	/// Returns a normalized unit vector representing the direction of the normal at the point designated by `t` on the curve.
	pub fn normal(&self, t: f64) -> DVec2 {
		self.tangent(t).perp()
	}

	/// Returns the curvature, a scalar value for the derivative at the given `t`-value along the curve.
	/// Curvature is 1 over the radius of a circle with an equivalent derivative.
	pub fn curvature(&self, t: f64) -> f64 {
		let (d, dd) = match &self.derivative() {
			Some(first_derivative) => match first_derivative.derivative() {
				Some(second_derivative) => (first_derivative.evaluate(t), second_derivative.evaluate(t)),
				None => (first_derivative.evaluate(t), first_derivative.end - first_derivative.start),
			},
			None => (self.end - self.start, DVec2::new(0., 0.)),
		};

		let numerator = d.x * dd.y - d.y * dd.x;
		let denominator = (d.x.powf(2.) + d.y.powf(2.)).powf(1.5);
		if denominator == 0. {
			0.
		} else {
			numerator / denominator
		}
	}

	/// Returns the pair of Bezier curves that result from splitting the original curve at the point corresponding to `t`.
	pub fn split(&self, t: f64) -> [Bezier; 2] {
		let split_point = self.evaluate(t);

		match self.handles {
			BezierHandles::Linear => [Bezier::from_linear_dvec2(self.start, split_point), Bezier::from_linear_dvec2(split_point, self.end)],
			// TODO: Actually calculate the correct handle locations
			BezierHandles::Quadratic { handle } => {
				let t_minus_one = t - 1.;
				[
					Bezier::from_quadratic_dvec2(self.start, t * handle - t_minus_one * self.start, split_point),
					Bezier::from_quadratic_dvec2(split_point, t * self.end - t_minus_one * handle, self.end),
				]
			}
			BezierHandles::Cubic { handle_start, handle_end } => {
				let t_minus_one = t - 1.;
				[
					Bezier::from_cubic_dvec2(
						self.start,
						t * handle_start - t_minus_one * self.start,
						(t * t) * handle_end - 2. * t * t_minus_one * handle_start + (t_minus_one * t_minus_one) * self.start,
						split_point,
					),
					Bezier::from_cubic_dvec2(
						split_point,
						(t * t) * self.end - 2. * t * t_minus_one * handle_end + (t_minus_one * t_minus_one) * handle_start,
						t * self.end - t_minus_one * handle_end,
						self.end,
					),
				]
			}
		}
	}

	/// Returns the Bezier curve representing the sub-curve starting at the point corresponding to `t1` and ending at the point corresponding to `t2`.
	pub fn trim(&self, t1: f64, t2: f64) -> Bezier {
		// Depending on the order of `t1` and `t2`, determine which half of the split we need to keep
		let t1_split_side = if t1 <= t2 { 1 } else { 0 };
		let t2_split_side = if t1 <= t2 { 0 } else { 1 };
		let bezier_starting_at_t1 = self.split(t1)[t1_split_side];
		// Adjust the ratio `t2` to its corresponding value on the new curve that was split on `t1`
		let adjusted_t2 = if t1 < t2 || (t1 == t2 && t1 == 0.) {
			// Case where we took the split from t1 to the end
			// Also cover the `t1` == t2 case where there would otherwise be a divide by 0
			(t2 - t1) / (1. - t1)
		} else {
			// Case where we took the split from the beginning to `t1`
			t2 / t1
		};
		bezier_starting_at_t1.split(adjusted_t2)[t2_split_side]
	}

	/// Returns the `t` value that corresponds to the closest point on the curve to the provided point.
	/// Uses a searching algorithm akin to binary search that can be customized using the [ProjectionOptions] structure.
	pub fn project(&self, point: DVec2, options: ProjectionOptions) -> f64 {
		let ProjectionOptions {
			lut_size,
			convergence_epsilon,
			convergence_limit,
			iteration_limit,
		} = options;

		// TODO: Consider optimizations from precomputing useful values, or using the GPU
		// First find the closest point from the results of a lookup table
		let lut = self.compute_lookup_table(Some(lut_size));
		let (minimum_position, minimum_distance) = utils::get_closest_point_in_lut(&lut, point);

		// Get the t values to the left and right of the closest result in the lookup table
		let lut_size_f64 = lut_size as f64;
		let minimum_position_f64 = minimum_position as f64;
		let mut left_t = (minimum_position_f64 - 1.).max(0.) / lut_size_f64;
		let mut right_t = (minimum_position_f64 + 1.).min(lut_size_f64) / lut_size_f64;

		// Perform a finer search by finding closest t from 5 points between [left_t, right_t] inclusive
		// Choose new left_t and right_t for a smaller range around the closest t and repeat the process
		let mut final_t = left_t;
		let mut distance;

		// Increment minimum_distance to ensure that the distance < minimum_distance comparison will be true for at least one iteration
		let mut new_minimum_distance = minimum_distance + 1.;
		// Maintain the previous distance to identify convergence
		let mut previous_distance;
		// Counter to limit the number of iterations
		let mut iteration_count = 0;
		// Counter to identify how many iterations have had a similar result. Used for convergence test
		let mut convergence_count = 0;

		// Store calculated distances to minimize unnecessary recomputations
		let mut distances: [f64; NUM_DISTANCES] = [
			point.distance(lut[(minimum_position as i64 - 1).max(0) as usize]),
			0.,
			0.,
			0.,
			point.distance(lut[lut_size.min(minimum_position + 1)]),
		];

		while left_t <= right_t && convergence_count < convergence_limit && iteration_count < iteration_limit {
			previous_distance = new_minimum_distance;
			let step = (right_t - left_t) / (NUM_DISTANCES as f64 - 1.);
			let mut iterator_t = left_t;
			let mut target_index = 0;
			// Iterate through first 4 points and will handle the right most point later
			for (step_index, table_distance) in distances.iter_mut().enumerate().take(4) {
				// Use previously computed distance for the left most point, and compute new values for the others
				if step_index == 0 {
					distance = *table_distance;
				} else {
					distance = point.distance(self.evaluate(iterator_t));
					*table_distance = distance;
				}
				if distance < new_minimum_distance {
					new_minimum_distance = distance;
					target_index = step_index;
					final_t = iterator_t
				}
				iterator_t += step;
			}
			// Check right most edge separately since step may not perfectly add up to it (floating point errors)
			if distances[NUM_DISTANCES - 1] < new_minimum_distance {
				new_minimum_distance = distances[NUM_DISTANCES - 1];
				final_t = right_t;
			}

			// Update left_t and right_t to be the t values (final_t +/- step), while handling the edges (i.e. if final_t is 0, left_t will be 0 instead of -step)
			// Ensure that the t values never exceed the [0, 1] range
			left_t = (final_t - step).max(0.);
			right_t = (final_t + step).min(1.);

			// Re-use the corresponding computed distances (target_index is the index corresponding to final_t)
			// Since target_index is a u_size, can't subtract one if it is zero
			distances[0] = distances[if target_index == 0 { 0 } else { target_index - 1 }];
			distances[NUM_DISTANCES - 1] = distances[(target_index + 1).min(NUM_DISTANCES - 1)];

			iteration_count += 1;
			// update count for consecutive iterations of similar minimum distances
			if previous_distance - new_minimum_distance < convergence_epsilon {
				convergence_count += 1;
			} else {
				convergence_count = 0;
			}
		}

		final_t
	}

	/// Returns two lists of `t`-values representing the local extrema of the `x` and `y` parametric curves respectively.
	/// The local extrema are defined to be points at which the derivative of the curve is equal to zero.
	fn unrestricted_local_extrema(&self) -> [Vec<f64>; 2] {
		match self.handles {
			BezierHandles::Linear => [Vec::new(), Vec::new()],
			BezierHandles::Quadratic { handle } => {
				let a = handle - self.start;
				let b = self.end - handle;
				let b_minus_a = b - a;
				[utils::solve_linear(b_minus_a.x, a.x), utils::solve_linear(b_minus_a.y, a.y)]
			}
			BezierHandles::Cubic { handle_start, handle_end } => {
				let a = 3. * (-self.start + 3. * handle_start - 3. * handle_end + self.end);
				let b = 6. * (self.start - 2. * handle_start + handle_end);
				let c = 3. * (handle_start - self.start);
				let discriminant = b * b - 4. * a * c;
				let two_times_a = 2. * a;
				[
					utils::solve_quadratic(discriminant.x, two_times_a.x, b.x, c.x),
					utils::solve_quadratic(discriminant.y, two_times_a.y, b.y, c.y),
				]
			}
		}
	}

	/// Returns two lists of `t`-values representing the local extrema of the `x` and `y` parametric curves respectively.
	/// The list of `t`-values returned are filtered such that they fall within the range `[0, 1]`.
	pub fn local_extrema(&self) -> [Vec<f64>; 2] {
		self.unrestricted_local_extrema()
			.into_iter()
			.map(|t_values| t_values.into_iter().filter(|&t| t > 0. && t < 1.).collect::<Vec<f64>>())
			.collect::<Vec<Vec<f64>>>()
			.try_into()
			.unwrap()
	}

	/// Returns a Bezier curve that results from applying the transformation function to each point in the Bezier.
	pub fn apply_transformation(&self, transformation_function: &dyn Fn(DVec2) -> DVec2) -> Bezier {
		let transformed_start = transformation_function(self.start);
		let transformed_end = transformation_function(self.end);
		match self.handles {
			BezierHandles::Linear => Bezier::from_linear_dvec2(transformed_start, transformed_end),
			BezierHandles::Quadratic { handle } => {
				let transformed_handle = transformation_function(handle);
				Bezier::from_quadratic_dvec2(transformed_start, transformed_handle, transformed_end)
			}
			BezierHandles::Cubic { handle_start, handle_end } => {
				let transformed_handle_start = transformation_function(handle_start);
				let transformed_handle_end = transformation_function(handle_end);
				Bezier::from_cubic_dvec2(transformed_start, transformed_handle_start, transformed_handle_end, transformed_end)
			}
		}
	}

	/// Returns a Bezier curve that results from rotating the curve around the origin by the given angle (in radians).
	pub fn rotate(&self, angle: f64) -> Bezier {
		let rotation_matrix = DMat2::from_angle(angle);
		self.apply_transformation(&|point| rotation_matrix.mul_vec2(point))
	}

	/// Returns a Bezier curve that results from translating the curve by the given `DVec2`.
	pub fn translate(&self, translation: DVec2) -> Bezier {
		self.apply_transformation(&|point| point + translation)
	}

	/// Implementation of the algorithm to find curve intersections by iterating on bounding boxes.
	/// - `self_original_t_interval` - Used to identify the `t` values of the original parent of `self` that the current iteration is representing.
	/// - `other_original_t_interval` - Used to identify the `t` values of the original parent of `other` that the current iteration is representing.
	fn intersections_between_subcurves(&self, self_original_t_interval: Range<f64>, other: &Bezier, other_original_t_interval: Range<f64>, error: f64) -> Vec<[f64; 2]> {
		let bounding_box1 = self.bounding_box();
		let bounding_box2 = other.bounding_box();

		// Get the `t` interval of the original parent of `self` and determine the middle `t` value
		let Range { start: self_start_t, end: self_end_t } = self_original_t_interval;
		let self_mid_t = (self_start_t + self_end_t) / 2.;

		// Get the `t` interval of the original parent of `other` and determine the middle `t` value
		let Range {
			start: other_start_t,
			end: other_end_t,
		} = other_original_t_interval;
		let other_mid_t = (other_start_t + other_end_t) / 2.;

		let error_threshold = DVec2::new(error, error);

		// Check if the bounding boxes overlap
		if utils::do_rectangles_overlap(bounding_box1, bounding_box2) {
			// If bounding boxes are within the error threshold (i.e. are small enough), we have found an intersection
			if (bounding_box1[1] - bounding_box1[0]).lt(&error_threshold) && (bounding_box2[1] - bounding_box2[0]).lt(&error_threshold) {
				// Use the middle t value, return the corresponding `t` value for `self` and `other`
				return vec![[self_mid_t, other_mid_t]];
			}

			// Split curves in half and repeat with the combinations of the two halves of each curve
			let [split_1_a, split_1_b] = self.split(0.5);
			let [split_2_a, split_2_b] = other.split(0.5);

			[
				split_1_a.intersections_between_subcurves(self_start_t..self_mid_t, &split_2_a, other_start_t..other_mid_t, error),
				split_1_a.intersections_between_subcurves(self_start_t..self_mid_t, &split_2_b, other_mid_t..other_end_t, error),
				split_1_b.intersections_between_subcurves(self_mid_t..self_end_t, &split_2_a, other_start_t..other_mid_t, error),
				split_1_b.intersections_between_subcurves(self_mid_t..self_end_t, &split_2_b, other_mid_t..other_end_t, error),
			]
			.concat()
		} else {
			vec![]
		}
	}

	// TODO: Use an `impl Iterator` return type instead of a `Vec`
	/// Returns a list of `t` values that correspond to intersection points between the current bezier curve and the provided one. The returned `t` values are with respect to the current bezier, not the provided parameter.
	/// If the provided curve is linear, then zero intersection points will be returned along colinear segments.
	/// - `error` - For intersections where the provided bezier is non-linear, `error` defines the threshold for bounding boxes to be considered an intersection point.
	pub fn intersections(&self, other: &Bezier, error: Option<f64>) -> Vec<f64> {
		let error = error.unwrap_or(0.5);
		if other.handles == BezierHandles::Linear {
			// Rotate the bezier and the line by the angle that the line makes with the x axis
			let line_directional_vector = other.end - other.start;
			let angle = line_directional_vector.angle_between(DVec2::new(1., 0.));
			let rotation_matrix = DMat2::from_angle(angle);
			let rotated_bezier = self.apply_transformation(&|point| rotation_matrix.mul_vec2(point));
			let rotated_line = [rotation_matrix.mul_vec2(other.start), rotation_matrix.mul_vec2(other.end)];

			// Translate the bezier such that the line becomes aligned on top of the x-axis
			let vertical_distance = rotated_line[0].y;
			let translated_bezier = rotated_bezier.translate(DVec2::new(0., -vertical_distance));

			// Compute the roots of the resulting bezier curve
			let list_intersection_t = match translated_bezier.handles {
				BezierHandles::Linear => {
					// If the transformed linear bezier is on the x-axis, `a` and `b` will both be zero and `solve_linear` will return no roots
					let a = translated_bezier.end.y - translated_bezier.start.y;
					let b = translated_bezier.start.y;
					utils::solve_linear(a, b)
				}
				BezierHandles::Quadratic { handle } => {
					let a = translated_bezier.start.y - 2. * handle.y + translated_bezier.end.y;
					let b = 2. * (handle.y - translated_bezier.start.y);
					let c = translated_bezier.start.y;

					let discriminant = b * b - 4. * a * c;
					let two_times_a = 2. * a;

					utils::solve_quadratic(discriminant, two_times_a, b, c)
				}
				BezierHandles::Cubic { handle_start, handle_end } => {
					let start_y = translated_bezier.start.y;
					let a = -start_y + 3. * handle_start.y - 3. * handle_end.y + translated_bezier.end.y;
					let b = 3. * start_y - 6. * handle_start.y + 3. * handle_end.y;
					let c = -3. * start_y + 3. * handle_start.y;
					let d = start_y;

					utils::solve_cubic(a, b, c, d)
				}
			};

			let min = other.start.min(other.end);
			let max = other.start.max(other.end);

			return list_intersection_t
				.into_iter()
				// Accept the t value if it is approximately in [0, 1] and if the corresponding coordinates are within the range of the linear line
				.filter(|&t| {
					utils::f64_approximately_in_range(t, 0., 1., MAX_ABSOLUTE_DIFFERENCE)
						&& utils::dvec2_approximately_in_range(self.unrestricted_evaluate(t), min, max, MAX_ABSOLUTE_DIFFERENCE).all()
				})
				// Ensure the returned value is within the correct range
				.map(|t| t.clamp(0., 1.))
				.collect::<Vec<f64>>();
		}

		// TODO: Consider using the `intersections_between_vectors_of_curves` helper function here
		// Otherwise, use bounding box to determine intersections
		self.intersections_between_subcurves(0. ..1., other, 0. ..1., error).iter().map(|t_values| t_values[0]).collect()
	}

	/// Helper function to compute intersections between lists of subcurves.
	/// This function uses the algorithm implemented in `intersections_between_subcurves`.
	fn intersections_between_vectors_of_curves(subcurves1: &[(Bezier, Range<f64>)], subcurves2: &[(Bezier, Range<f64>)], error: f64) -> Vec<[f64; 2]> {
		let segment_pairs = subcurves1.iter().flat_map(move |(curve1, curve1_t_pair)| {
			subcurves2
				.iter()
				.filter_map(move |(curve2, curve2_t_pair)| utils::do_rectangles_overlap(curve1.bounding_box(), curve2.bounding_box()).then(|| (curve1, curve1_t_pair, curve2, curve2_t_pair)))
		});
		segment_pairs
			.flat_map(|(curve1, curve1_t_pair, curve2, curve2_t_pair)| curve1.intersections_between_subcurves(curve1_t_pair.clone(), curve2, curve2_t_pair.clone(), error))
			.collect::<Vec<[f64; 2]>>()
	}

	// TODO: Use an `impl Iterator` return type instead of a `Vec`
	/// Returns a list of `t` values that correspond to the self intersection points of the current bezier curve. For each intersection point, the returned `t` value is the smaller of the two that correspond to the point.
	/// - `error` - For intersections with non-linear beziers, `error` defines the threshold for bounding boxes to be considered an intersection point.
	pub fn self_intersections(&self, error: Option<f64>) -> Vec<[f64; 2]> {
		if self.handles == BezierHandles::Linear || matches!(self.handles, BezierHandles::Quadratic { .. }) {
			return vec![];
		}

		let error = error.unwrap_or(0.5);

		// Get 2 copies of the reduced curves
		let (self1, self1_t_values) = self.reduced_curves_and_t_values(None);
		let (self2, self2_t_values) = (self1.clone(), self1_t_values.clone());
		let num_curves = self1.len();

		// Create iterators that combine a subcurve with the `t` value pair that it was trimmed with
		let combined_iterator1 = self1.into_iter().zip(self1_t_values.windows(2).map(|t_pair| Range { start: t_pair[0], end: t_pair[1] }));
		// Second one needs to be a list because Iterator does not implement copy
		let combined_list2: Vec<(Bezier, Range<f64>)> = self2.into_iter().zip(self2_t_values.windows(2).map(|t_pair| Range { start: t_pair[0], end: t_pair[1] })).collect();

		// Adjacent reduced curves cannot intersect
		// So for each curve, look for intersections with every curve that is at least 2 indices away
		combined_iterator1
			.take(num_curves - 2)
			.enumerate()
			.flat_map(|(index, (subcurve, t_pair))| Bezier::intersections_between_vectors_of_curves(&[(subcurve, t_pair)], &combined_list2[index + 2..], error))
			.collect()
	}

	/// Returns a list of lists of points representing the De Casteljau points for all iterations at the point corresponding to `t` using De Casteljau's algorithm.
	/// The `i`th element of the list represents the set of points in the `i`th iteration.
	/// More information on the algorithm can be found in the [De Casteljau section](https://pomax.github.io/bezierinfo/#decasteljau) in Pomax's primer.
	pub fn de_casteljau_points(&self, t: f64) -> Vec<Vec<DVec2>> {
		let bezier_points = match self.handles {
			BezierHandles::Linear => vec![self.start, self.end],
			BezierHandles::Quadratic { handle } => vec![self.start, handle, self.end],
			BezierHandles::Cubic { handle_start, handle_end } => vec![self.start, handle_start, handle_end, self.end],
		};
		let mut de_casteljau_points = vec![bezier_points];
		let mut current_points = de_casteljau_points.last().unwrap();

		// Iterate until one point is left, that point will be equal to `evaluate(t)`
		while current_points.len() > 1 {
			// Map from every adjacent pair of points to their respective midpoints, which decrements by 1 the number of points for the next iteration
			let next_points: Vec<DVec2> = current_points.as_slice().windows(2).map(|pair| DVec2::lerp(pair[0], pair[1], t)).collect();
			de_casteljau_points.push(next_points);

			current_points = de_casteljau_points.last().unwrap();
		}

		de_casteljau_points
	}

	/// Determine if it is possible to scale the given curve, using the following conditions:
	/// 1. All the handles are located on a single side of the curve.
	/// 2. The on-curve point for `t = 0.5` must occur roughly in the center of the polygon defined by the curve's endpoint normals.
	/// See [the offset section](https://pomax.github.io/bezierinfo/#offsetting) of Pomax's bezier curve primer for more details.
	fn is_scalable(&self) -> bool {
		if self.handles == BezierHandles::Linear {
			return true;
		}
		// Verify all the handles are located on a single side of the curve.
		if let BezierHandles::Cubic { handle_start, handle_end } = self.handles {
			let angle_1 = (self.end - self.start).angle_between(handle_start - self.start);
			let angle_2 = (self.end - self.start).angle_between(handle_end - self.start);
			if (angle_1 > 0. && angle_2 < 0.) || (angle_1 < 0. && angle_2 > 0.) {
				return false;
			}
		}
		// Verify the angle formed by the endpoint normals is sufficiently small, ensuring the on-curve point for `t = 0.5` occurs roughly in the center of the polygon.
		let normal_0 = self.normal(0.);
		let normal_1 = self.normal(1.);
		let endpoint_normal_angle = (normal_0.x * normal_1.x + normal_0.y * normal_1.y).acos();
		endpoint_normal_angle < SCALABLE_CURVE_MAX_ENDPOINT_NORMAL_ANGLE
	}

	/// Add the bezier endpoints if not already present, and combine and sort the dimensional extrema.
	fn get_extrema_t_list(&self) -> Vec<f64> {
		let mut extrema = self.local_extrema().into_iter().flatten().collect::<Vec<f64>>();
		extrema.append(&mut vec![0., 1.]);
		extrema.dedup();
		extrema.sort_by(|ex1, ex2| ex1.partial_cmp(ex2).unwrap());
		extrema
	}

	/// Returns a tuple of the scalable subcurves and the corresponding `t` values that were used to split the curve.
	/// This function may introduce gaps if subsections of the curve are not reducible.
	/// The function takes the following parameter:
	/// - `step_size` - Dictates the granularity at which the function searches for reducible subcurves. The default value is `0.01`.
	///   A small granularity may increase the chance the function does not introduce gaps, but will increase computation time.
	fn reduced_curves_and_t_values(&self, step_size: Option<f64>) -> (Vec<Bezier>, Vec<f64>) {
		// A linear segment is scalable, so return itself
		if let BezierHandles::Linear = self.handles {
			return (vec![*self], vec![0., 1.]);
		}

		let step_size = step_size.unwrap_or(DEFAULT_REDUCE_STEP_SIZE);

		let extrema = self.get_extrema_t_list();

		// Split each subcurve such that each resulting segment is scalable.
		let mut result_beziers: Vec<Bezier> = Vec::new();
		let mut result_t_values: Vec<f64> = vec![extrema[0]];

		extrema.windows(2).for_each(|t_pair| {
			let t_subcurve_start = t_pair[0];
			let t_subcurve_end = t_pair[1];
			let subcurve = self.trim(t_subcurve_start, t_subcurve_end);
			// Perform no processing on the subcurve if it's already scalable.
			if subcurve.is_scalable() {
				result_beziers.push(subcurve);
				result_t_values.push(t_subcurve_end);
				return;
			}
			// According to <https://pomax.github.io/bezierinfo/#offsetting>, it is generally sufficient to split subcurves with no local extrema at `t = 0.5` to generate two scalable segments.
			let [first_half, second_half] = subcurve.split(0.5);
			if first_half.is_scalable() && second_half.is_scalable() {
				result_beziers.push(first_half);
				result_beziers.push(second_half);
				result_t_values.push(t_subcurve_start + (t_subcurve_end - t_subcurve_start) / 2.);
				result_t_values.push(t_subcurve_end);
				return;
			}

			// Greedily iterate across the subcurve at intervals of size `step_size` to break up the curve into maximally large segments
			let mut segment: Bezier;
			let mut t1 = 0.;
			let mut t2 = step_size;
			while t2 <= 1. + step_size {
				segment = subcurve.trim(t1, f64::min(t2, 1.));
				if !segment.is_scalable() {
					t2 -= step_size;

					// If the previous step does not exist, the start of the subcurve is irreducible.
					// Otherwise, add the valid segment from the previous step to the result.
					if f64::abs(t1 - t2) >= step_size {
						segment = subcurve.trim(t1, t2);
						result_beziers.push(segment);
						result_t_values.push(t_subcurve_start + t2 * (t_subcurve_end - t_subcurve_start));
					} else {
						return;
					}
					t1 = t2;
				}
				t2 += step_size;
			}
			// Collect final remainder of the curve.
			if t1 < 1. {
				segment = subcurve.trim(t1, 1.);
				if segment.is_scalable() {
					result_beziers.push(segment);
					result_t_values.push(t_subcurve_end);
				}
			}
		});
		(result_beziers, result_t_values)
	}

	/// Split the curve into a number of scalable subcurves. This function may introduce gaps if subsections of the curve are not reducible.
	/// The function takes the following parameter:
	/// - `step_size` - Dictates the granularity at which the function searches for reducible subcurves. The default value is `0.01`.
	///   A small granularity may increase the chance the function does not introduce gaps, but will increase computation time.
	pub fn reduce(&self, step_size: Option<f64>) -> Vec<Bezier> {
		self.reduced_curves_and_t_values(step_size).0
	}

	/// Approximate a bezier curve with circular arcs.
	/// The algorithm can be customized using the [ArcsOptions] structure.
	pub fn arcs(&self, arcs_options: ArcsOptions) -> Vec<CircleArc> {
		let ArcsOptions {
			strategy: maximize_arcs,
			error,
			max_iterations,
		} = arcs_options;

		match maximize_arcs {
			ArcStrategy::Automatic => {
				let (auto_arcs, final_low_t) = self.approximate_curve_with_arcs(0., 1., error, max_iterations, true);
				let arc_approximations = self.split(final_low_t)[1].arcs(ArcsOptions {
					strategy: ArcStrategy::FavorCorrectness,
					error,
					max_iterations,
				});
				if final_low_t != 1. {
					[auto_arcs, arc_approximations].concat()
				} else {
					auto_arcs
				}
			}
			ArcStrategy::FavorLargerArcs => self.approximate_curve_with_arcs(0., 1., error, max_iterations, false).0,
			ArcStrategy::FavorCorrectness => self
				.get_extrema_t_list()
				.windows(2)
				.flat_map(|t_pair| self.approximate_curve_with_arcs(t_pair[0], t_pair[1], error, max_iterations, false).0)
				.collect::<Vec<CircleArc>>(),
		}
	}

	/// Implements an algorithm that approximates a bezier curve with circular arcs.
	/// This algorithm uses a method akin to binary search to find an arc that approximates a maximal segment of the curve.
	/// Once a maximal arc has been found for a sub-segment of the curve, the algorithm continues by starting again at the end of the previous approximation.
	/// More details can be found in the [Approximating a Bezier curve with circular arcs](https://pomax.github.io/bezierinfo/#arcapproximation) section of Pomax's bezier curve primer.
	/// A caveat with this algorithm is that it is possible to find erroneous approximations in cases such as in a very narrow `U`.
	/// - `stop_when_invalid`: Used to determine whether the algorithm should terminate early if erroneous approximations are encountered.
	///
	/// Returns a tuple where the first element is the list of circular arcs and the second is the `t` value where the next segment should start from.
	/// The second value will be `1.` except for when `stop_when_invalid` is true and an invalid approximation is encountered.
	fn approximate_curve_with_arcs(&self, local_low: f64, local_high: f64, error: f64, max_iterations: usize, stop_when_invalid: bool) -> (Vec<CircleArc>, f64) {
		let mut low = local_low;
		let mut middle = (local_low + local_high) / 2.;
		let mut high = local_high;
		let mut previous_high = local_high;

		let mut iterations = 0;
		let mut previous_arc = CircleArc::default();
		let mut was_previous_good = false;
		let mut arcs = Vec::new();

		// Outer loop to iterate over the curve
		while low < local_high {
			// Inner loop to find the next maximal segment of the curve that can be approximated with a circular arc
			while iterations <= max_iterations {
				iterations += 1;
				let p1 = self.evaluate(low);
				let p2 = self.evaluate(middle);
				let p3 = self.evaluate(high);

				let wrapped_center = utils::compute_circle_center_from_points(p1, p2, p3);
				// If the segment is linear, move on to next segment
				if wrapped_center.is_none() {
					previous_high = high;
					low = high;
					high = 1.;
					middle = (low + high) / 2.;
					was_previous_good = false;
					break;
				}

				let center = wrapped_center.unwrap();
				let radius = center.distance(p1);

				let angle_p1 = DVec2::new(1., 0.).angle_between(p1 - center);
				let angle_p2 = DVec2::new(1., 0.).angle_between(p2 - center);
				let angle_p3 = DVec2::new(1., 0.).angle_between(p3 - center);

				let mut start_angle = angle_p1;
				let mut end_angle = angle_p3;

				// Adjust start and end angles of the arc to ensure that it travels in the counter-clockwise direction
				if angle_p1 < angle_p3 {
					if angle_p2 < angle_p1 || angle_p3 < angle_p2 {
						std::mem::swap(&mut start_angle, &mut end_angle);
					}
				} else if angle_p2 < angle_p1 && angle_p3 < angle_p2 {
					std::mem::swap(&mut start_angle, &mut end_angle);
				}

				let new_arc = CircleArc {
					center,
					radius,
					start_angle,
					end_angle,
				};

				// Use points in between low, middle, and high to evaluate how well the arc approximates the curve
				let e1 = self.evaluate((low + middle) / 2.);
				let e2 = self.evaluate((middle + high) / 2.);

				// Iterate until we find the largest good approximation such that the next iteration is not a good approximation with an arc
				if utils::f64_compare(radius, e1.distance(center), error) && utils::f64_compare(radius, e2.distance(center), error) {
					// Check if the good approximation is actually valid: the sector angle cannot be larger than 180 degrees (PI radians)
					let mut sector_angle = end_angle - start_angle;
					if sector_angle < 0. {
						sector_angle += 2. * PI;
					}
					if stop_when_invalid && sector_angle > PI {
						return (arcs, low);
					}
					if high == local_high {
						// Found the final arc approximation
						arcs.push(new_arc);
						low = high;
						break;
					}
					// If the approximation is good, expand the segment by half to try finding a larger good approximation
					previous_high = high;
					high = (high + (high - low) / 2.).min(local_high);
					middle = (low + high) / 2.;
					previous_arc = new_arc;
					was_previous_good = true;
				} else if was_previous_good {
					// If the previous approximation was good and the current one is bad, then we use the previous good approximation
					arcs.push(previous_arc);

					// Continue searching for approximations for the rest of the curve
					low = previous_high;
					high = local_high;
					middle = low + (high - low) / 2.;
					was_previous_good = false;
					break;
				} else {
					// If no good approximation has been seen yet, try again with half the segment
					previous_high = high;
					high = middle;
					middle = low + (high - low) / 2.;
					previous_arc = new_arc;
				}
			}
		}

		(arcs, low)
	}

	/// Return the min and max corners that represent the bounding box of the curve.
	pub fn bounding_box(&self) -> [DVec2; 2] {
		// Start by taking min/max of endpoints.
		let mut endpoints_min = self.start.min(self.end);
		let mut endpoints_max = self.start.max(self.end);

		// Iterate through extrema points.
		let extrema = self.local_extrema();
		for t_values in extrema {
			for t in t_values {
				let point = self.evaluate(t);
				// Update bounding box if new min/max is found.
				endpoints_min = endpoints_min.min(point);
				endpoints_max = endpoints_max.max(point);
			}
		}

		[endpoints_min, endpoints_max]
	}

	// TODO: Use an `impl Iterator` return type instead of a `Vec`
	/// Returns list of `t`-values representing the inflection points of the curve.
	/// The inflection points are defined to be points at which the second derivative of the curve is equal to zero.
	pub fn unrestricted_inflections(&self) -> Vec<f64> {
		match self.handles {
			// There exists no inflection points for linear and quadratic beziers.
			BezierHandles::Linear => Vec::new(),
			BezierHandles::Quadratic { .. } => Vec::new(),
			BezierHandles::Cubic { .. } => {
				// Axis align the curve.
				let translated_bezier = self.translate(-self.start);
				let angle = translated_bezier.end.angle_between(DVec2::new(1., 0.));
				let rotated_bezier = translated_bezier.rotate(angle);
				if let BezierHandles::Cubic { handle_start, handle_end } = rotated_bezier.handles {
					// These formulas and naming conventions follows https://pomax.github.io/bezierinfo/#inflections
					let a = handle_end.x * handle_start.y;
					let b = rotated_bezier.end.x * handle_start.y;
					let c = handle_start.x * handle_end.y;
					let d = rotated_bezier.end.x * handle_end.y;

					let x = -3. * a + 2. * b + 3. * c - d;
					let y = 3. * a - b - 3. * c;
					let z = c - a;

					let discriminant = y * y - 4. * x * z;
					utils::solve_quadratic(discriminant, 2. * x, y, z)
				} else {
					unreachable!("shouldn't happen")
				}
			}
		}
	}

	/// Returns list of `t`-values representing the inflection points of the curve.
	/// The list of `t`-values returned are filtered such that they fall within the range `[0, 1]`.
	pub fn inflections(&self) -> Vec<f64> {
		self.unrestricted_inflections().into_iter().filter(|&t| t > 0. && t < 1.).collect::<Vec<f64>>()
	}

	/// Scale will translate a bezier curve a fixed distance away from its original position, and stretch/compress the transformed curve to match the translation ratio.
	/// Note that not all bezier curves are possible to scale, so this function asserts that the provided curve is scalable.
	/// A proof for why this is true can be found in the [Curve offsetting section](https://pomax.github.io/bezierinfo/#offsetting) of Pomax's bezier curve primer.
	/// `scale` takes the parameter `distance`, which is the distance away from the curve that the new one will be scaled to. Positive values will scale the curve in the
	/// same direction as the endpoint normals, while negative values will scale in the opposite direction.
	fn scale(&self, distance: f64) -> Bezier {
		assert!(self.is_scalable(), "The curve provided to scale is not scalable. Reduce the curve first.");

		let normal_start = self.normal(0.);
		let normal_end = self.normal(1.);

		// If normal unit vectors are equal, then the lines are parallel
		if normal_start.abs_diff_eq(normal_end, MAX_ABSOLUTE_DIFFERENCE) {
			return self.translate(distance * normal_start);
		}

		// Find the intersection point of the endpoint normals
		let intersection = utils::line_intersection(self.start, normal_start, self.end, normal_end);

		let should_flip_direction = (self.start - intersection).normalize().abs_diff_eq(normal_start, MAX_ABSOLUTE_DIFFERENCE);
		self.apply_transformation(&|point| {
			let mut direction_unit_vector = (intersection - point).normalize();
			if should_flip_direction {
				direction_unit_vector *= -1.;
			}
			point + distance * direction_unit_vector
		})
	}

	/// Offset will get all the reduceable subcurves, and for each subcurve, it will scale the subcurve a set distance away from the original curve.
	/// Note that not all bezier curves are possible to offset, so this function first reduces the curve to scalable segments and then offsets those segments.
	/// A proof for why this is true can be found in the [Curve offsetting section](https://pomax.github.io/bezierinfo/#offsetting) of Pomax's bezier curve primer.
	/// Offset takes the following parameter:
	/// - `distance` - The distance away from the curve that the new one will be offset to. Positive values will offset the curve in the same direction as the endpoint normals,
	/// while negative values will offset in the opposite direction.
	pub fn offset(&self, distance: f64) -> Vec<Bezier> {
		let mut reduced = self.reduce(None);
		reduced.iter_mut().for_each(|bezier| *bezier = bezier.scale(distance));
		reduced
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::consts::MAX_ABSOLUTE_DIFFERENCE;

	use glam::DVec2;

	// Compare points by allowing some maximum absolute difference to account for floating point errors
	fn compare_points(p1: DVec2, p2: DVec2) -> bool {
		p1.abs_diff_eq(p2, MAX_ABSOLUTE_DIFFERENCE)
	}

	// Compare vectors of points by allowing some maximum absolute difference to account for floating point errors
	fn compare_vector_of_points(a: Vec<DVec2>, b: Vec<DVec2>) -> bool {
		a.len() == b.len() && a.into_iter().zip(b.into_iter()).all(|(p1, p2)| p1.abs_diff_eq(p2, MAX_ABSOLUTE_DIFFERENCE))
	}

	// Compare vectors of beziers by allowing some maximum absolute difference between points to account for floating point errors
	fn compare_vector_of_beziers(beziers: &[Bezier], expected_bezier_points: Vec<Vec<DVec2>>) -> bool {
		beziers
			.iter()
			.zip(expected_bezier_points.iter())
			.all(|(&a, b)| compare_vector_of_points(a.get_points().collect::<Vec<DVec2>>(), b.to_vec()))
	}

	// Compare circle arcs by allowing some maximum absolute difference between values to account for floating point errors
	fn compare_arcs(arc1: CircleArc, arc2: CircleArc) -> bool {
		compare_points(arc1.center, arc2.center)
			&& utils::f64_compare(arc1.radius, arc1.radius, MAX_ABSOLUTE_DIFFERENCE)
			&& utils::f64_compare(arc1.start_angle, arc2.start_angle, MAX_ABSOLUTE_DIFFERENCE)
			&& utils::f64_compare(arc1.end_angle, arc2.end_angle, MAX_ABSOLUTE_DIFFERENCE)
	}

	// Compare vectors of points with some maximum allowed absolute difference between the values
	fn compare_vec_of_points(vec1: Vec<DVec2>, vec2: Vec<DVec2>, max_absolute_difference: f64) -> bool {
		vec1.into_iter().zip(vec2).all(|(p1, p2)| p1.abs_diff_eq(p2, max_absolute_difference))
	}

	#[test]
	fn test_quadratic_through_points() {
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

	#[test]
	fn evaluate() {
		let p1 = DVec2::new(3., 5.);
		let p2 = DVec2::new(14., 3.);
		let p3 = DVec2::new(19., 14.);
		let p4 = DVec2::new(30., 21.);

		let bezier1 = Bezier::from_quadratic_dvec2(p1, p2, p3);
		assert!(compare_points(bezier1.evaluate(0.5), DVec2::new(3., 3.)));

		let bezier2 = Bezier::from_cubic_dvec2(p1, p2, p3, p4);
		assert!(compare_points(bezier1.evaluate(0.8), DVec2::new(3., 3.)));
	}

	#[test]
	fn compute_lookup_table() {
		println!("compute_lookup_table");

		let bezier1 = Bezier::from_quadratic_coordinates(10., 10., 30., 30., 50., 10.);
		let expected_lookup_table1 = Vec::from([DVec2::new(0., 0.), DVec2::new(0., 0.), DVec2::new(0., 0.), DVec2::new(0., 0.), DVec2::new(0., 0.)]);
		let actual_lookup_table1 = bezier1.compute_lookup_table(Some(4));
		println!(
			"{:?}, {:?}, {:?}, {:?}",
			actual_lookup_table1[0], actual_lookup_table1[1], actual_lookup_table1[2], actual_lookup_table1[3]
		);
		// let lookup_table_iter1 = expected_lookup_table1.iter().zip(actual_lookup_table1.iter());
		// for (_, (actual, expected)) in lookup_table_iter1.iter() {
		// 	assert!(compare_points(actual, expected));
		// }

		let bezier2 = Bezier::from_cubic_coordinates(10., 10., 30., 30., 70., 70., 90., 10.);
		let expected_lookup_table2 = Vec::from([DVec2::new(0., 0.), DVec2::new(0., 0.), DVec2::new(0., 0.), DVec2::new(0., 0.), DVec2::new(0., 0.)]);
		let actual_lookup_table2 = bezier2.compute_lookup_table(Some(4));
		println!(
			"{:?}, {:?}, {:?}, {:?}",
			actual_lookup_table2[0], actual_lookup_table2[1], actual_lookup_table2[2], actual_lookup_table2[3]
		);
		// let lookup_table_iter2 = expected_lookup_table2.iter().zip(actual_lookup_table2.iter());
		// for (_, (actual, expected)) in lookup_table_iter.iter() {
		// 	assert!(compare_points(actual, expected));
		// }
	}

	#[test]
	fn length() {
		let p1 = DVec2::new(30., 50.);
		let p2 = DVec2::new(140., 30.);
		let p3 = DVec2::new(160., 170.);
		let p4 = DVec2::new(77., 129.);

		let bezier_linear = Bezier::from_linear_dvec2(p1, p2);
		assert!(utils::f64_compare(bezier_linear.length(None), p1.distance(p2), MAX_ABSOLUTE_DIFFERENCE));

		let bezier_quadratic = Bezier::from_quadratic_dvec2(p1, p2, p3);
		assert!(utils::f64_compare(bezier_quadratic.length(None), 204., MAX_ABSOLUTE_DIFFERENCE));

		let bezier_cubic = Bezier::from_cubic_dvec2(p1, p2, p3, p4);
		assert!(utils::f64_compare(bezier_cubic.length(None), 199., MAX_ABSOLUTE_DIFFERENCE));
	}

	#[test]
	fn derivative() {
		let p1 = DVec2::new(10., 10.);
		let p2 = DVec2::new(40., 30.);
		let p3 = DVec2::new(60., 60.);
		let p4 = DVec2::new(70., 100.);

		let linear = Bezier::from_linear_dvec2(p1, p2);
		assert!(linear.derivative().is_none());

		let quadratic = Bezier::from_quadratic_dvec2(p1, p2, p3);
		let derivative_quadratic = quadratic.derivative().unwrap();
		assert!(derivative_quadratic.abs_diff_eq(&Bezier::from_linear_coordinates(60., 40., 40., 60.), MAX_ABSOLUTE_DIFFERENCE));

		let cubic = Bezier::from_cubic_dvec2(p1, p2, p3, p4);
		let derivative_cubic = cubic.derivative().unwrap();
		assert!(derivative_cubic.abs_diff_eq(&Bezier::from_quadratic_coordinates(90., 60., 60., 90., 30., 120.), MAX_ABSOLUTE_DIFFERENCE));
	}

	#[test]
	fn tangent() {}

	#[test]
	fn normal() {}

	#[test]
	fn split() {
		println!("split");

		let bezier1 = Bezier::from_quadratic_coordinates(10., 10., 50., 50., 90., 10.);
		let expected_split1 = Vec::from([
			Bezier::from_quadratic_coordinates(10., 10., 50., 50., 90., 10.),
			Bezier::from_quadratic_coordinates(90., 10., 50., 50., 90., 10.),
		]);
		let actual_split1 = bezier1.split(0.3);
		println!(
			"actual_split1 {:?}, {:?}",
			actual_split1[0].get_points().collect::<Vec<DVec2>>(),
			actual_split1[1].get_points().collect::<Vec<DVec2>>()
		);
		// let split_iter1 = expected_split1.iter().zip(actual_split1.iter());
		// for (_, (actual, expected)) in split_iter1.iter() {
		// 	assert!(compare_points(actual, expected));
		// }

		let bezier2 = Bezier::from_cubic_coordinates(10., 10., 50., 50., 90., 10., 100., 100.);
		let expected_split2 = Vec::from([
			Bezier::from_cubic_coordinates(10., 10., 50., 50., 90., 10., 100., 100.),
			Bezier::from_cubic_coordinates(90., 10., 50., 50., 90., 10., 100., 100.),
		]);
		let actual_split2 = bezier2.split(0.8);
		println!(
			"actual_split2 {:?}, {:?}",
			actual_split2[0].get_points().collect::<Vec<DVec2>>(),
			actual_split2[1].get_points().collect::<Vec<DVec2>>()
		);
		// let split_iter2 = expected_split2.iter().zip(actual_split2.iter());
		// for (_, (actual, expected)) in split_iter2.iter() {
		// 	assert!(compare_points(actual, expected));
		// }
	}

	#[test]
	fn split_at_anchors() {
		let start = DVec2::new(30., 50.);
		let end = DVec2::new(160., 170.);

		let bezier_quadratic = Bezier::from_quadratic_dvec2(start, DVec2::new(140., 30.), end);

		// Test splitting a quadratic bezier at the startpoint
		let [point_bezier1, remainder1] = bezier_quadratic.split(0.);
		assert!(point_bezier1.start() == start);
		assert!(point_bezier1.end() == start);
		assert!(point_bezier1.handle_start().unwrap() == start);
		assert!(remainder1.abs_diff_eq(&bezier_quadratic, MAX_ABSOLUTE_DIFFERENCE));

		// Test splitting a quadratic bezier at the endpoint
		let [remainder2, point_bezier2] = bezier_quadratic.split(1.);
		assert!(point_bezier2.start() == end);
		assert!(point_bezier2.handle_start().unwrap() == end);
		assert!(point_bezier2.end() == end);
		assert!(remainder2.abs_diff_eq(&bezier_quadratic, MAX_ABSOLUTE_DIFFERENCE));

		let bezier_cubic = Bezier::from_cubic_dvec2(start, DVec2::new(60., 140.), DVec2::new(150., 30.), end);

		// Test splitting a cubic bezier at the startpoint
		let [point_bezier3, remainder3] = bezier_cubic.split(0.);
		assert!(point_bezier2.start() == start);
		assert!(point_bezier2.handle_start().unwrap() == start);
		assert!(point_bezier2.handle_end().unwrap() == start);
		assert!(point_bezier2.end() == start);
		assert!(remainder3.abs_diff_eq(&bezier_cubic, MAX_ABSOLUTE_DIFFERENCE));

		// Test splitting a cubic bezier at the endpoint
		let [remainder4, point_bezier4] = bezier_cubic.split(1.);
		assert!(point_bezier4.start() == end);
		assert!(point_bezier4.handle_start().unwrap() == end);
		assert!(point_bezier4.handle_end().unwrap() == end);
		assert!(point_bezier4.end() == end);
		assert!(remainder4.abs_diff_eq(&bezier_cubic, MAX_ABSOLUTE_DIFFERENCE));
	}

	#[test]
	fn trim() {
		let linear = Bezier::from_linear_coordinates(4., 4., 5., 4.);
		let trimmed_linear = linear.trim(0.25, 0.75);
		assert!(trimmed_linear.start == DVec2::new(4.25, 4.) && trimmed_linear.end == DVec2::new(4.75, 4.));

		let quadratic = Bezier::from_quadratic_coordinates(0., 0., 1., 1., 2., 0.);
		let trimmed_quadratic = quadratic.trim(0.25, 0.75);

		let cubic = Bezier::from_cubic_coordinates(0., 0., 1., 1., 2., 1., 2., 0.);
		let trimmed_cubic = cubic.trim(0.25, 0.75);
	}

	#[test]
	fn trim_t2_greater_than_t1() {
		// Test trimming quadratic curve when t2 > t1
		let bezier_quadratic = Bezier::from_quadratic_coordinates(30., 50., 140., 30., 160., 170.);
		let trim1 = bezier_quadratic.trim(0.25, 0.75);
		let trim2 = bezier_quadratic.trim(0.75, 0.25);
		assert!(trim1.abs_diff_eq(&trim2, MAX_ABSOLUTE_DIFFERENCE));

		// Test trimming cubic curve when t2 > t1
		let bezier_cubic = Bezier::from_cubic_coordinates(30., 30., 60., 140., 150., 30., 160., 160.);
		let trim3 = bezier_cubic.trim(0.25, 0.75);
		let trim4 = bezier_cubic.trim(0.75, 0.25);
		assert!(trim3.abs_diff_eq(&trim4, MAX_ABSOLUTE_DIFFERENCE));
	}

	#[test]
	fn test_project() {
		let project_options = ProjectionOptions::default();

		let bezier1 = Bezier::from_cubic_coordinates(4., 4., 23., 45., 10., 30., 56., 90.);
		assert!(bezier1.evaluate(bezier1.project(DVec2::new(100., 100.), project_options)) == DVec2::new(56., 90.));
		assert!(bezier1.evaluate(bezier1.project(DVec2::new(0., 0.), project_options)) == DVec2::new(4., 4.));

		let bezier2 = Bezier::from_quadratic_coordinates(0., 0., 0., 100., 100., 100.);
		assert!(bezier2.evaluate(bezier2.project(DVec2::new(100., 0.), project_options)) == DVec2::new(0., 0.));
	}

	#[test]
	fn extrema_linear() {
		// Linear bezier cannot have extrema
		let line = Bezier::from_linear_dvec2(DVec2::new(10., 10.), DVec2::new(50., 50.));
		assert!(line.local_extrema().is_empty());
	}

	#[test]
	fn extrema_quadratic() {
		println!("extrema_quadratic");

		// no x-extrema, no y-extrema
		let bezier1 = Bezier::from_quadratic_dvec2(DVec2::new(3., 5.), DVec2::new(7., 10.), DVec2::new(10., 14.));
		let extrema1 = bezier1.local_extrema();
		println!("{:?}", extrema1);
		assert!(extrema1[0].is_empty() && extrema1[1].is_empty());

		// 1 x-extrema, no y-extrema
		let bezier2 = Bezier::from_quadratic_dvec2(DVec2::new(3., 5.), DVec2::new(1., 10.), DVec2::new(19., 14.));
		let extrema2 = bezier2.local_extrema();
		println!("{:?}", extrema2);
		assert!(extrema2[0].len() == 1 && extrema2[1].is_empty());

		// no x-extrema, 1 y-extrema
		let bezier3 = Bezier::from_quadratic_dvec2(DVec2::new(3., 3.), DVec2::new(10., 125.), DVec2::new(20., 15.));
		let extrema3 = bezier3.local_extrema();
		println!("{:?}", extrema3);
		assert!(extrema3[0].is_empty() && extrema3[1].is_empty());

		// 1 x-extrema, 1 y-extrema
		let bezier4 = Bezier::from_quadratic_dvec2(DVec2::new(30., 3.), DVec2::new(10., 125.), DVec2::new(5., 15.));
		let extrema4 = bezier4.local_extrema();
		println!("{:?}", extrema4);
		assert!(extrema4[0].len() == 1 && extrema4[1].len() == 1);
	}

	#[test]
	fn extrema_cubic() {
		println!("extrema_quadratic");

		// 0 x-extrema, 0 y-extrema
		let bezier0 = Bezier::from_cubic_coordinates(100., 105., 250., 250., 110., 150., 260., 260.);
		let extrema0 = bezier0.local_extrema();
		println!("{:?}", extrema0);
		assert!(extrema0[0].is_empty() && extrema0[1].is_empty());

		// 1 x-extrema, 0 y-extrema
		let bezier1 = Bezier::from_cubic_coordinates(177., 135., 170., 10., 24., 18., 20., 117.);
		let extrema1 = bezier1.local_extrema();
		println!("{:?}", extrema1);
		assert!(extrema1[0].len() == 1 && extrema1[1].is_empty());

		// 1 x-extrema, 1 y-extrema
		let bezier2 = Bezier::from_cubic_coordinates(100., 105., 170., 10., 24., 18., 20., 117.);
		let extrema2 = bezier2.local_extrema();
		println!("{:?}", extrema2);
		assert!(extrema2[0].len() == 1 && extrema2[1].len() == 1);

		// 1 x-extrema, 2 y-extrema
		let bezier3 = Bezier::from_cubic_coordinates(47., 91., 119., 16., 152., 192., 45., 153.);
		let extrema3 = bezier3.local_extrema();
		println!("{:?}", extrema3);
		assert!(extrema3[0].len() == 1 && extrema3[1].len() == 2);

		// 2 x-extrema, 0 y-extrema
		let bezier4 = Bezier::from_cubic_coordinates(48., 34., 136., 62., 48., 95., 48., 95.);
		let extrema4 = bezier4.local_extrema();
		println!("{:?}", extrema4);
		assert!(extrema4[0].len() == 2 && extrema4[1].is_empty());

		// 2 x-extrema, 1 y-extrema
		let bezier5 = Bezier::from_cubic_coordinates(100., 100., 100., 300., 300., 300., 300., 100.);
		let extrema5 = bezier5.local_extrema();
		println!("{:?}", extrema5);
		assert!(extrema5[0].len() == 2 && extrema5[1].len() == 1);

		// 2 x-extrema, 2 y-extrema
		let bezier6 = Bezier::from_cubic_coordinates(46., 60., 140., 8., 52., 159., 120., 118.);
		let extrema6 = bezier6.local_extrema();
		println!("{:?}", extrema6);
		assert!(extrema6[0].len() == 2 && extrema6[1].len() == 2);
	}

	#[test]
	fn test_intersect_line_segment_linear() {
		let p1 = DVec2::new(30., 60.);
		let p2 = DVec2::new(140., 120.);

		// Intersection at edge of curve
		let bezier = Bezier::from_linear_dvec2(p1, p2);
		let line1 = Bezier::from_linear_coordinates(20., 60., 70., 60.);
		let intersections1 = bezier.intersections(&line1, None);
		assert!(intersections1.len() == 1);
		assert!(compare_points(bezier.evaluate(intersections1[0]), DVec2::new(30., 60.)));

		// Intersection in the middle of curve
		let line2 = Bezier::from_linear_coordinates(150., 150., 30., 30.);
		let intersections2 = bezier.intersections(&line2, None);
		assert!(compare_points(bezier.evaluate(intersections2[0]), DVec2::new(96., 96.)));
	}

	#[test]
	fn test_intersect_line_segment_quadratic() {
		let p1 = DVec2::new(30., 50.);
		let p2 = DVec2::new(140., 30.);
		let p3 = DVec2::new(160., 170.);

		// Intersection at edge of curve
		let bezier = Bezier::from_quadratic_dvec2(p1, p2, p3);
		let line1 = Bezier::from_linear_coordinates(20., 50., 40., 50.);
		let intersections1 = bezier.intersections(&line1, None);
		assert!(intersections1.len() == 1);
		assert!(compare_points(bezier.evaluate(intersections1[0]), p1));

		// Intersection in the middle of curve
		let line2 = Bezier::from_linear_coordinates(150., 150., 30., 30.);
		let intersections2 = bezier.intersections(&line2, None);
		assert!(compare_points(bezier.evaluate(intersections2[0]), DVec2::new(47.77355, 47.77354)));
	}

	#[test]
	fn test_intersect_line_segment_cubic() {
		let p1 = DVec2::new(30., 30.);
		let p2 = DVec2::new(60., 140.);
		let p3 = DVec2::new(150., 30.);
		let p4 = DVec2::new(160., 160.);

		let bezier = Bezier::from_cubic_dvec2(p1, p2, p3, p4);
		// Intersection at edge of curve, Discriminant > 0
		let line1 = Bezier::from_linear_coordinates(20., 30., 40., 30.);
		let intersections1 = bezier.intersections(&line1, None);
		assert!(intersections1.len() == 1);
		assert!(compare_points(bezier.evaluate(intersections1[0]), p1));

		// Intersection at edge and in middle of curve, Discriminant < 0
		let line2 = Bezier::from_linear_coordinates(150., 150., 30., 30.);
		let intersections2 = bezier.intersections(&line2, None);
		assert!(intersections2.len() == 2);
		assert!(compare_points(bezier.evaluate(intersections2[0]), p1));
		assert!(compare_points(bezier.evaluate(intersections2[1]), DVec2::new(85.84, 85.84)));
	}

	#[test]
	fn test_intersect_curve() {
		let bezier1 = Bezier::from_cubic_coordinates(30., 30., 60., 140., 150., 30., 160., 160.);
		let bezier2 = Bezier::from_quadratic_coordinates(175., 140., 20., 20., 120., 20.);

		let intersections = bezier1.intersections(&bezier2, None);
		let intersections2 = bezier2.intersections(&bezier1, None);
		assert!(compare_vec_of_points(
			intersections.iter().map(|&t| bezier1.evaluate(t)).collect(),
			intersections2.iter().map(|&t| bezier2.evaluate(t)).collect(),
			2.
		));
	}

	#[test]
	fn test_intersect_with_self() {
		let bezier = Bezier::from_cubic_coordinates(160., 180., 170., 10., 30., 90., 180., 140.);
		let intersections = bezier.self_intersections(Some(0.5));
		assert!(compare_vec_of_points(
			intersections.iter().map(|&t| bezier.evaluate(t[0])).collect(),
			intersections.iter().map(|&t| bezier.evaluate(t[1])).collect(),
			2.
		));
		assert!(Bezier::from_linear_coordinates(160., 180., 170., 10.).self_intersections(None).is_empty());
		assert!(Bezier::from_quadratic_coordinates(160., 180., 170., 10., 30., 90.).self_intersections(None).is_empty());
	}

	#[test]
	fn test_offset() {
		let p1 = DVec2::new(30., 50.);
		let p2 = DVec2::new(140., 30.);
		let p3 = DVec2::new(160., 170.);
		let bezier1 = Bezier::from_quadratic_dvec2(p1, p2, p3);
		let expected_bezier_points1 = vec![
			vec![DVec2::new(31.7888, 59.8387), DVec2::new(44.5924, 57.46446), DVec2::new(56.09375, 57.5)],
			vec![DVec2::new(56.09375, 57.5), DVec2::new(94.94197, 56.5019), DVec2::new(117.6473, 84.5936)],
			vec![DVec2::new(117.6473, 84.5936), DVec2::new(142.3985, 113.403), DVec2::new(150.1005, 171.4142)],
		];
		assert!(compare_vector_of_beziers(&bezier1.offset(10.), expected_bezier_points1));

		let p4 = DVec2::new(32., 77.);
		let p5 = DVec2::new(169., 25.);
		let p6 = DVec2::new(164., 157.);
		let bezier2 = Bezier::from_quadratic_dvec2(p4, p5, p6);
		let expected_bezier_points2 = vec![
			vec![DVec2::new(42.6458, 105.04758), DVec2::new(75.0218, 91.9939), DVec2::new(98.09357, 92.3043)],
			vec![DVec2::new(98.09357, 92.3043), DVec2::new(116.5995, 88.5479), DVec2::new(123.9055, 102.0401)],
			vec![DVec2::new(123.9055, 102.0401), DVec2::new(136.6087, 116.9522), DVec2::new(134.1761, 147.9324)],
			vec![DVec2::new(134.1761, 147.9324), DVec2::new(134.1812, 151.7987), DVec2::new(134.0215, 155.86445)],
		];
		assert!(compare_vector_of_beziers(&bezier2.offset(30.), expected_bezier_points2));
	}

	#[test]
	fn test_reduce() {
		let p1 = DVec2::new(0., 0.);
		let p2 = DVec2::new(50., 50.);
		let p3 = DVec2::new(0., 0.);
		let bezier = Bezier::from_quadratic_dvec2(p1, p2, p3);

		let expected_bezier_points = vec![
			vec![DVec2::new(0., 0.), DVec2::new(0.5, 0.5), DVec2::new(0.989, 0.989)],
			vec![DVec2::new(0.989, 0.989), DVec2::new(2.705, 2.705), DVec2::new(4.2975, 4.2975)],
			vec![DVec2::new(4.2975, 4.2975), DVec2::new(5.6625, 5.6625), DVec2::new(6.9375, 6.9375)],
		];
		let reduced_curves = bezier.reduce(None);
		assert!(compare_vector_of_beziers(&reduced_curves, expected_bezier_points));

		// Check that the reduce helper is correct
		let (helper_curves, helper_t_values) = bezier.reduced_curves_and_t_values(None);
		assert_eq!(&reduced_curves, &helper_curves);
		assert!(reduced_curves
			.iter()
			.zip(helper_t_values.windows(2))
			.all(|(curve, t_pair)| curve.abs_diff_eq(&bezier.trim(t_pair[0], t_pair[1]), MAX_ABSOLUTE_DIFFERENCE)))
	}

	#[test]
	fn test_arcs_linear() {
		let bezier = Bezier::from_linear_coordinates(30., 60., 140., 120.);
		let linear_arcs = bezier.arcs(ArcsOptions::default());
		assert!(linear_arcs.is_empty());
	}

	#[test]
	fn test_arcs_quadratic() {
		let bezier1 = Bezier::from_quadratic_coordinates(30., 30., 50., 50., 100., 100.);
		assert!(bezier1.arcs(ArcsOptions::default()).is_empty());

		let bezier2 = Bezier::from_quadratic_coordinates(50., 50., 85., 65., 100., 100.);
		let actual_arcs = bezier2.arcs(ArcsOptions::default());
		let expected_arc = CircleArc {
			center: DVec2::new(15., 135.),
			radius: 91.92388,
			start_angle: -1.18019,
			end_angle: -0.39061,
		};
		assert_eq!(actual_arcs.len(), 1);
		assert!(compare_arcs(actual_arcs[0], expected_arc));
	}

	#[test]
	fn test_arcs_cubic() {
		let bezier = Bezier::from_cubic_coordinates(30., 30., 30., 80., 60., 80., 60., 140.);
		let actual_arcs = bezier.arcs(ArcsOptions::default());
		let expected_arcs = vec![
			CircleArc {
				center: DVec2::new(122.394877, 30.7777189),
				radius: 92.39815,
				start_angle: 2.5637146,
				end_angle: -3.1331755,
			},
			CircleArc {
				center: DVec2::new(-47.54881, 136.169378),
				radius: 107.61701,
				start_angle: -0.53556,
				end_angle: 0.0356025,
			},
		];

		assert_eq!(actual_arcs.len(), 2);
		assert!(compare_arcs(actual_arcs[0], expected_arcs[0]));
		assert!(compare_arcs(actual_arcs[1], expected_arcs[1]));

		// Bezier that contains the erroneous case when maximizing arcs
		let bezier2 = Bezier::from_cubic_coordinates(48., 176., 170., 10., 30., 90., 180., 160.);
		let auto_arcs = bezier2.arcs(ArcsOptions::default());

		let extrema_arcs = bezier2.arcs(ArcsOptions {
			strategy: ArcStrategy::FavorCorrectness,
			..ArcsOptions::default()
		});

		let maximal_arcs = bezier2.arcs(ArcsOptions {
			strategy: ArcStrategy::FavorLargerArcs,
			..ArcsOptions::default()
		});

		// Resulting automatic arcs match the maximal results until the bad arc (in this case, only index 0 should match)
		assert_eq!(auto_arcs[0], maximal_arcs[0]);
		// Check that the first result from MaximizeArcs::Automatic should not equal the first results from MaximizeArcs::Off
		assert_ne!(auto_arcs[0], extrema_arcs[0]);
		// The remaining results (index 2 onwards) should match the results where MaximizeArcs::Off from the next extrema point onwards (after index 2).
		assert!(auto_arcs.iter().skip(2).zip(extrema_arcs.iter().skip(2)).all(|(arc1, arc2)| compare_arcs(*arc1, *arc2)));
	}
}
