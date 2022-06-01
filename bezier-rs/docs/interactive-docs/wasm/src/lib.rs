use bezier_rs::Bezier;
use wasm_bindgen::prelude::*;

/// Convert to SVG
#[wasm_bindgen]
// TODO: Allow modifying the viewport, width and height
pub fn quad_to_svg(x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64) -> String {
	Bezier::from_quadratic_coordinates(x1, y1, x2, y2, x3, y3).to_svg()
}
