use crate::color::Color;

// RENDERING
pub const LAYER_OUTLINE_STROKE_COLOR: Color = Color::BLACK;
pub const LAYER_OUTLINE_STROKE_WIDTH: f32 = 1.;

// Bezier Curve intersection algorithm
pub const F64PRECISION: f64 = f64::EPSILON * 100.0; // for f64 comparisons, to allow for rounding error

// a bezier curve whose available_precision is greater than CURVE_FIDELITY can be evaluated at least 10000 "unique" locations
pub const CURVE_FIDELITY: f64 = f64::EPSILON * 10000.0;
