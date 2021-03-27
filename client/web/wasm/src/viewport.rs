use crate::shims::Error;
use crate::wrappers::{translate_tool, Color};
use crate::EDITOR_STATE;
use editor_core::events;
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

static mut POS: (u32, u32) = (0, 0);

/// Mouse movement with the bounds of the canvas
#[wasm_bindgen]
pub fn on_mouse_move(x: u32, y: u32) {
	// SAFETY: This is safe because the code can only ever run in a single thread
	unsafe {
		POS = (x, y);
	}
}

/// Mouse click within the bounds of the canvas
#[wasm_bindgen]
pub fn on_mouse_click(x: u32, y: u32) -> Result<(), JsValue> {
	let ev = events::Event::Click(events::MouseState::from_pos(x, y));
	EDITOR_STATE.with(|editor| editor.borrow_mut().handle_event(ev)).map_err(|err| Error::new(&err.to_string()).into())
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
