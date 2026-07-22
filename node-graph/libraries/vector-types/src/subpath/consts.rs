// Implementation constants

/// Constant used to determine if `f64`s are equivalent.
pub const MAX_ABSOLUTE_DIFFERENCE: f64 = 1e-3;

// Constant to approximate a quarter circle with a cubic Bézier curve, from https://pomax.github.io/bezierinfo/#circles_cubic
pub const HANDLE_OFFSET_FACTOR: f64 = 0.551784777779014;
