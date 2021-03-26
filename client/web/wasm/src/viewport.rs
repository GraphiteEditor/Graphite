use crate::wrappers::{translate_tool, Color};
use graphite_editor_core::tools::ToolState;
use wasm_bindgen::prelude::*;

pub static mut TOOL_STATE: ToolState = ToolState::default();

/// Modify the currently selected tool in the document state store
#[wasm_bindgen]
pub fn select_tool(tool: String) -> Result<(), JsValue> {
	let tool_state = unsafe { &mut TOOL_STATE };
	if let Some(tool) = translate_tool(tool.as_str()) {
		Ok(tool_state.select_tool(tool))
	} else {
		Err(JsValue::from(format!("Couldn't select {} because it was not recognized as a valid tool", tool)))
	}
}

/// Mouse movement with the bounds of the canvas
#[wasm_bindgen]
pub fn on_mouse_move(x: u32, y: u32) {
	todo!()
}

/// Update working colors
#[wasm_bindgen]
pub fn update_colors(primary_color: Color, secondary_color: Color) {
	let tool_state = unsafe { &mut TOOL_STATE };
	tool_state.set_primary_color(primary_color.get_inner_color());
	tool_state.set_secondary_color(secondary_color.get_inner_color());
}
