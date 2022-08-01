use glam::DVec2;
use std::fmt::{Debug, Formatter, Result};

/// Struct to represent optional parameters that can be passed to the `project` function.
#[derive(Copy, Clone)]
pub struct ProjectionOptions {
	/// Size of the lookup table for the initial passthrough. The default value is `20`.
	pub lut_size: i32,
	/// Difference used between floating point numbers to be considered as equal. The default value is `0.0001`
	pub convergence_epsilon: f64,
	/// Controls the number of iterations needed to consider that minimum distance to have converged. The default value is `3`.
	pub convergence_limit: i32,
	/// Controls the maximum total number of iterations to be used. The default value is `10`.
	pub iteration_limit: i32,
}

impl Default for ProjectionOptions {
	fn default() -> Self {
		ProjectionOptions {
			lut_size: 20,
			convergence_epsilon: 1e-4,
			convergence_limit: 3,
			iteration_limit: 10,
		}
	}
}

/// Struct used to represent the different strategies for generating arc approximations.
#[derive(Copy, Clone)]
pub enum MaximizeArcs {
	/// Start with the greedy strategy of maximizing arc approximations and automatically switch to the divide-and-conquer when the greedy approximations no longer fall within the error bound.
	Automatic,
	/// Use the greedy strategy to maximize approximated arcs, despite potentially erroneous arcs.
	On,
	/// Use the divide-and-conquer strategy that prioritizes correctness over maximal arcs.
	Off,
}

/// Struct to represent optional parameters that can be passed to the `arcs` function.
#[derive(Copy, Clone)]
pub struct ArcsOptions {
	/// Determines how the approximated arcs are computed.
	/// When maximizing the arcs, the algorithm may return incorrect arcs when the curve contains any small loops or segements that look like a very thin "U".
	/// The enum options behave as follows:
	/// - `Automatic`: Maximize arcs until an erroneous approximation is found. Compute the arcs of the rest of the curve by first splitting on extremas to ensure no more erroneous cases are encountered.
	/// - `On`: Maximize arcs using the original algorithm from the [Approximating a Bezier curve with circular arcs](https://pomax.github.io/bezierinfo/#arcapproximation) section of Pomax's bezier curve primer. Erroneous arcs are possible.
	/// - `Off`: Prioritize correctness by first spliting the curve by its extremas and determine the arc approximation of each segment instead.
	///
	/// The default value is `Automatic`.
	pub maximize_arcs: MaximizeArcs,
	/// The error used for approximating the arc's fit. The default is `0.5`.
	pub error: f64,
	/// The maximum number of segment iterations used as attempts for arc approximations. The default is `100`.
	pub max_iterations: i32,
}

impl Default for ArcsOptions {
	fn default() -> Self {
		ArcsOptions {
			maximize_arcs: MaximizeArcs::Automatic,
			error: 0.5,
			max_iterations: 100,
		}
	}
}

/// Struct to represent the circular arc approximation used in the `arcs` bezier function.
#[derive(Copy, Clone, PartialEq)]
pub struct CircleArc {
	/// The center point of the circle.
	pub center: DVec2,
	/// The radius of the circle.
	pub radius: f64,
	/// The start angle of the circle sector in rad.
	pub start_angle: f64,
	/// The end angle of the circle sector in rad.
	pub end_angle: f64,
}

impl Debug for CircleArc {
	fn fmt(&self, f: &mut Formatter<'_>) -> Result {
		write!(f, "Center: {}, radius: {}, start to end angles: {} to {}", self.center, self.radius, self.start_angle, self.end_angle)
	}
}

impl Default for CircleArc {
	fn default() -> Self {
		CircleArc {
			center: DVec2::ZERO,
			radius: 0.,
			start_angle: 0.,
			end_angle: 0.,
		}
	}
}
