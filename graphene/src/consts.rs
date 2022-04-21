use crate::color::Color;

// RENDERING
pub const LAYER_OUTLINE_STROKE_COLOR: Color = Color::BLACK;
pub const LAYER_OUTLINE_STROKE_WEIGHT: f64 = 1.;

// BOOLEAN OPERATIONS

// Bezier curve intersection algorithm
// f64::EPSILON ~= 2^(-52)
pub const F64PRECISE: f64 = f64::EPSILON * 128.0; // ~= 2^(-45) for f64 comparisons, to allow for rounding error

// for comparisons between values that are a result of complex computations where error accumulates
pub const F64LOOSE: f64 = f64::EPSILON * 1048576.0; // ~= 2^(-32)

// In practice, this makes it less likely that a ray will intersect with a common anchor point between two curves
pub const RAY_FUDGE_FACTOR: f64 = 0.00001;
