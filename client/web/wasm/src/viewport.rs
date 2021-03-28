use crate::shims::Error;
use crate::wrappers::{translate_tool, Color};
use crate::EDITOR_STATE;
use editor_core::events;
use wasm_bindgen::prelude::*;

/// Modify the currently selected tool in the document state store
#[wasm_bindgen]
pub fn select_tool(tool: String) -> Result<(), JsValue> {
	EDITOR_STATE.with(|editor| match translate_tool(&tool) {
		Some(tool) => editor.borrow_mut().handle_event(events::Event::SelectTool(tool)).map_err(|err| Error::new(&err.to_string()).into()),
		None => Err(Error::new(&format!("Couldn't select {} because it was not recognized as a valid tool", tool)).into()),
	})
}

/// Mouse movement with the bounds of the canvas
#[wasm_bindgen]
pub fn on_mouse_move(x: u32, y: u32) -> Result<(), JsValue> {
	let ev = events::Event::MouseMovement(events::CanvasPosition { x, y });
	EDITOR_STATE.with(|editor| editor.borrow_mut().handle_event(ev)).map_err(|err| Error::new(&err.to_string()).into())
}

/// Mouse click within the bounds of the canvas
#[wasm_bindgen]
pub fn on_mouse_down(x: u32, y: u32, mouse_keys: u8) -> Result<(), JsValue> {
	let mouse_keys = events::MouseKeys::from_bits(mouse_keys).expect("invalid modifier keys");
	let ev = events::Event::MouseDown(events::MouseState {
		position: events::CanvasPosition { x, y },
		mouse_keys,
	});
	EDITOR_STATE.with(|editor| editor.borrow_mut().handle_event(ev)).map_err(|err| Error::new(&err.to_string()).into())
}

/// Mouse released
#[wasm_bindgen]
pub fn on_mouse_up(x: u32, y: u32, mouse_keys: u8) -> Result<(), JsValue> {
	let mouse_keys = events::MouseKeys::from_bits(mouse_keys).expect("invalid modifier keys");
	let ev = events::Event::MouseDown(events::MouseState {
		position: events::CanvasPosition { x, y },
		mouse_keys,
	});
	EDITOR_STATE.with(|editor| editor.borrow_mut().handle_event(ev)).map_err(|err| Error::new(&err.to_string()).into())
}

/// Update primary color
#[wasm_bindgen]
pub fn update_primary_color(primary_color: Color) -> Result<(), JsValue> {
	EDITOR_STATE
		.with(|editor| editor.borrow_mut().handle_event(events::Event::SelectPrimaryColor(primary_color.inner())))
		.map_err(|err: editor_core::EditorError| Error::new(&err.to_string()).into())
}

/// Update secondary color
#[wasm_bindgen]
pub fn update_secondary_color(secondary_color: Color) -> Result<(), JsValue> {
	EDITOR_STATE
		.with(|editor| editor.borrow_mut().handle_event(events::Event::SelectSecondaryColor(secondary_color.inner())))
		.map_err(|err: editor_core::EditorError| Error::new(&err.to_string()).into())
}
