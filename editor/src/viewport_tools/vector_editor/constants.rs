// Helps push values that end in approximately half, plus or minus some floating point imprecision, towards the same side of the round() function
pub const ROUNDING_BIAS: f64 = 0.002;
// The angle threshold in radians that we should mirror handles if we are below
pub const MINIMUM_MIRROR_THRESHOLD: f64 = 0.1;
