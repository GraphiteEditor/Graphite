use crate::color::Color;

// RENDERING
pub const LAYER_OUTLINE_STROKE_COLOR: Color = Color::BLACK;
pub const LAYER_OUTLINE_STROKE_WIDTH: f32 = 1.;

// BOOLEAN OPERATIONS

// Bezier curve intersection algorithm
pub const F64PRECISE: f64 = f64::EPSILON * 100.0; // for f64 comparisons, to allow for rounding error
pub const F64LOOSE: f64 = f64::EPSILON * 1000000.0; // == 0.0000000002220446049250313

// A bezier curve whose `available_precision()` is greater than CURVE_FIDELITY can be evaluated at least 10000 "unique" locations
pub const CURVE_FIDELITY: f64 = F64PRECISE * 100.0;

// In practice, this makes it less likely that a ray will intersect with a common anchor point between two curves
pub const RAY_FUDGE_FACTOR: f64 = 0.00001;
