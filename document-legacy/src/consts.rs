// BOOLEAN OPERATIONS

// Bezier curve intersection algorithm
pub const F64PRECISE: f64 = f64::EPSILON * ((1 << 7) as f64); // ~= 2^(-45) - For f64 comparisons to allow for rounding error; note that f64::EPSILON ~= 2^(-52)
pub const F64LOOSE: f64 = f64::EPSILON * ((1 << 20) as f64); // ~= 2^(-32) - For comparisons between values that are a result of complex computations where error accumulates
