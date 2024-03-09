use glam::DVec2;
use std::fmt::{Debug, Formatter, Result};

/// Struct used to represent the different strategies for generating arc approximations.
#[derive(Copy, Clone)]
pub enum ArcStrategy {
	/// Start with the greedy strategy of maximizing arc approximations and automatically switch to the divide-and-conquer when the greedy approximations no longer fall within the error bound.
	Automatic,
	/// Use the greedy strategy to maximize approximated arcs, despite potentially erroneous arcs.
	FavorLargerArcs,
	/// Use the divide-and-conquer strategy that prioritizes correctness over maximal arcs.
	FavorCorrectness,
}

/// Struct to represent optional parameters that can be passed to the `arcs` function.
#[derive(Copy, Clone)]
pub struct ArcsOptions {
	/// Determines how the approximated arcs are computed.
	/// When maximizing the arcs, the algorithm may return incorrect arcs when the curve contains any small loops or segments that look like a very thin "U".
	/// The enum options behave as follows:
	/// - `Automatic`: Maximize arcs until an erroneous approximation is found. Compute the arcs of the rest of the curve by first splitting on extremas to ensure no more erroneous cases are encountered.
	/// - `FavorLargerArcs`: Maximize arcs using the original algorithm from the [Approximating a Bezier curve with circular arcs](https://pomax.github.io/bezierinfo/#arcapproximation) section of Pomax's bezier curve primer. Erroneous arcs are possible.
	/// - `FavorCorrectness`: Prioritize correctness by first spliting the curve by its extremas and determine the arc approximation of each segment instead.
	///
	/// The default value is `Automatic`.
	pub strategy: ArcStrategy,
	/// The error used for approximating the arc's fit. The default is `0.5`.
	pub error: f64,
	/// The maximum number of segment iterations used as attempts for arc approximations. The default is `100`.
	pub max_iterations: usize,
}

impl Default for ArcsOptions {
	fn default() -> Self {
		Self {
			strategy: ArcStrategy::Automatic,
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
		Self {
			center: DVec2::ZERO,
			radius: 0.,
			start_angle: 0.,
			end_angle: 0.,
		}
	}
}
