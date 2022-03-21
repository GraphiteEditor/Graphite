use crate::color::Color;

// RENDERING
pub const LAYER_OUTLINE_STROKE_COLOR: Color = Color::BLACK;
pub const LAYER_OUTLINE_STROKE_WIDTH: f32 = 1.;

// BOOLEAN OPERATIONS

// Bezier curve intersection algorithm
// f64::EPSILON ~= 2^(-52)
pub const F64PRECISE: f64 = f64::EPSILON * 128.0; // ~= 2^(-45) for f64 comparisons, to allow for rounding error
pub const F64LOOSE: f64 = f64::EPSILON * 1048576.0; // ~= 2^(-32)

// Given two curves 'a' and 'b', we guess they intersect at 't_a' and 't_b'.
// 'CURVE_FIDELITY' is the maximum allowable disparity between a(t_a) and b(t_b)
pub const CURVE_FIDELITY: f64 = F64PRECISE * 8.0; // ~= 2^(-42)

// In practice, this makes it less likely that a ray will intersect with a common anchor point between two curves
pub const RAY_FUDGE_FACTOR: f64 = 0.00001;
