use crate::wrappers::Color;
use wasm_bindgen::prelude::*;

/// Modify the currently selected tool in the document state store
#[wasm_bindgen]
pub fn select_tool(tool: String) {
	todo!()
}

/// Mouse movement with the bounds of the canvas
#[wasm_bindgen]
pub fn on_mouse_move(x: u32, y: u32) {
	todo!()
}

/// Update working colors
#[wasm_bindgen]
pub fn update_colors(primary_color: Color, secondary_color: Color) {
	todo!()
}
