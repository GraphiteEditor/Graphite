/// Default `t` value used for the `curve_through_points` functions
pub const DEFAULT_T_VALUE: f64 = 0.5;

/// Default LUT step size in `compute_lookup_table` function
pub const DEFAULT_LUT_STEP_SIZE: i32 = 10;

/// Number of subdivisions used in `length` calculation
pub const LENGTH_SUBDIVISIONS: i32 = 1000;

/// Number of distances used in search algorithm for `project`
pub const NUM_DISTANCES: usize = 5;

/// Constants used to determine if `f64`'s are equivalent
pub const MAX_ABSOLUTE_DIFFERENCE: f64 = 1e-3;
