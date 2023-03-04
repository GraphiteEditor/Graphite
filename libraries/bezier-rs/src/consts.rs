// Implementation constants

/// Constant used to determine if `f64`s are equivalent.
pub const MAX_ABSOLUTE_DIFFERENCE: f64 = 1e-3;
/// A stricter constant used to determine if `f64`s are equivalent.
pub const STRICT_MAX_ABSOLUTE_DIFFERENCE: f64 = 1e-6;
/// Number of distances used in search algorithm for `project`.
pub const NUM_DISTANCES: usize = 5;
/// Maximum allowed angle that the normal of the `start` or `end` point can make with the normal of the corresponding handle for a curve to be considered scalable/simple.
pub const SCALABLE_CURVE_MAX_ENDPOINT_NORMAL_ANGLE: f64 = std::f64::consts::PI / 3.;
/// Minimum allowable separation between adjacent `t` values when calculating curve intersections
pub const MIN_SEPARATION_VALUE: f64 = 5. * 1e-3;
/// Default error bound for `t_value_to_parametric` function when TValue argument is Euclidean
pub const DEFAULT_EUCLIDEAN_ERROR_BOUND: f64 = 0.001;

// Method argument defaults

/// Default `t` value used for the `curve_through_points` functions.
pub const DEFAULT_T_VALUE: f64 = 0.5;
/// Default LUT step size in `compute_lookup_table` function.
pub const DEFAULT_LUT_STEP_SIZE: usize = 10;
/// Default number of subdivisions used in `length` calculation.
pub const DEFAULT_LENGTH_SUBDIVISIONS: usize = 1000;
/// Default step size for `reduce` function.
pub const DEFAULT_REDUCE_STEP_SIZE: f64 = 0.01;

// SVG constants
pub const SVG_ARG_CUBIC: &str = "C";
pub const SVG_ARG_LINEAR: &str = "L";
pub const SVG_ARG_MOVE: &str = "M";
pub const SVG_ARG_QUADRATIC: &str = "Q";
pub const SVG_ARG_CLOSED: &str = "Z";
