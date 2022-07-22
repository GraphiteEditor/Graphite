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

/// Struct to represent optional parameters that can be passed to the `arcs` function.
#[derive(Copy, Clone)]
pub struct ArcsOptions {
	/// Determines whether the algorithm tries to greedily maximize the approximated arcs or to prioritize correctness by first splitting the curve between its local extremas.
	/// When maximizing the arcs, the algorithm may return incorrect arcs when the curve contains any small loops or segements that look like a very thin "U".
	/// The default value is `false`.
	pub maximize_arcs: bool,
	/// The error used for approximating the arc's fit. The default is `0.5`.
	pub error: f64,
	/// The maximum number of segment iterations used as attempts for arc approximations. The default is `100`.
	pub max_iterations: i32,
}

impl Default for ArcsOptions {
	fn default() -> Self {
		ArcsOptions {
			maximize_arcs: false,
			error: 0.5,
			max_iterations: 100,
		}
	}
}

/// Struct to represent the circular arc approximation used in the `arcs` bezier function.
#[derive(Copy, Clone)]
pub struct CircleArc {
	// the center point of the circle
	pub center: DVec2,
	// the radius of the circle
	pub radius: f64,
	// the start angle of the circle sector in rad
	pub start_angle: f64,
	// the end angle of the circle sector in rad
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
