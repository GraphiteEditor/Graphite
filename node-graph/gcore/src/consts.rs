use crate::raster::Color;

// RENDERING
pub const LAYER_OUTLINE_STROKE_COLOR: Color = Color::BLACK;
pub const LAYER_OUTLINE_STROKE_WEIGHT: f64 = 1.;

// Fonts
pub const DEFAULT_FONT_FAMILY: &str = "Cabin";
pub const DEFAULT_FONT_STYLE: &str = "Normal (400)";

// Zoom
pub const VIEWPORT_ZOOM_SCALE_MIN: f64 = 0.000_000_1;
pub const VIEWPORT_ZOOM_SCALE_MAX: f64 = 10_000.;
