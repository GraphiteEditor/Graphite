use crate::color::Color;

// RENDERING
pub const LAYER_OUTLINE_STROKE_COLOR: Color = Color::BLACK;
pub const LAYER_OUTLINE_STROKE_WIDTH: f32 = 1.;

// Bezier Curve intersection algorithm
pub const F64PRECISION: f64 = f64::EPSILON * 1000.0; // for f64 comparisons
