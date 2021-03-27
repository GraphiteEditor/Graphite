use crate::shims::Error;
use crate::wrappers::{translate_tool, Color};
use crate::EDITOR_STATE;
use wasm_bindgen::prelude::*;

/// Modify the currently selected tool in the document state store
#[wasm_bindgen]
pub fn select_tool(tool: String) -> Result<(), JsValue> {
	EDITOR_STATE.with(|editor| match translate_tool(&tool) {
		Some(tool) => {
			editor.borrow_mut().tools.active_tool = tool;
			Ok(())
		}
		None => Err(Error::new(&format!("Couldn't select {} because it was not recognized as a valid tool", tool)).into()),
	})
}

/// Mouse movement with the bounds of the canvas
#[wasm_bindgen]
pub fn on_mouse_move(x: u32, y: u32) {
	todo!()
}

/// Update working colors
#[wasm_bindgen]
pub fn update_colors(primary_color: Color, secondary_color: Color) {
	EDITOR_STATE.with(|editor| {
		let mut editor = editor.borrow_mut();
		editor.tools.primary_color = primary_color.inner();
		editor.tools.secondary_color = secondary_color.inner();
	})
}
