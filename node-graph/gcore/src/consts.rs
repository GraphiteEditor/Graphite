use crate::raster::Color;

// RENDERING
pub const LAYER_OUTLINE_STROKE_COLOR: Color = Color::BLACK;
pub const LAYER_OUTLINE_STROKE_WEIGHT: f64 = 0.5;

// Fonts
pub const DEFAULT_FONT_FAMILY: &str = "Cabin";
pub const DEFAULT_FONT_STYLE: &str = "Regular (400)";

// Load Source Sans Pro font data
// TODO: Grab this from the node_modules folder (either with `include_bytes!` or ideally at runtime) instead of checking the font file into the repo.
// TODO: And maybe use the WOFF2 version (if it's supported) for its smaller, compressed file size.
pub const SOURCE_SANS_FONT_DATA: &[u8] = include_bytes!("text/source-sans-pro-regular.ttf");
pub const SOURCE_SANS_FONT_FAMILY: &str = "Source Sans Pro";
pub const SOURCE_SANS_FONT_STYLE: &str = "Regular (400)";
