use glam::DVec2;
use std::fmt::{Debug, Formatter, Result};

/// Struct to represent optional parameters that can be passed to the `project` function.
#[derive(Copy, Clone)]
pub struct ProjectionOptions {
	/// Size of the lookup table for the initial passthrough. The default value is 20.
	pub lut_size: i32,
	/// Difference used between floating point numbers to be considered as equal. The default value is `0.0001`
	pub convergence_epsilon: f64,
	/// Controls the number of iterations needed to consider that minimum distance to have converged. The default value is 3.
	pub convergence_limit: i32,
	/// Controls the maximum total number of iterations to be used. The default value is 10.
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

#[derive(Copy, Clone)]
pub struct CircleArc {
	pub center: DVec2,
	pub radius: f64,
	pub start_angle: f64,
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
