use crate::shims::Error;
use crate::wrappers::{translate_key, translate_tool, Color};
use crate::EDITOR_STATE;
use editor_core::events;
use wasm_bindgen::prelude::*;

fn convert_error(err: editor_core::EditorError) -> JsValue {
	Error::new(&err.to_string()).into()
}

mod mouse_state {
	pub(super) type MouseKeys = u8;
	use editor_core::events::{self, Event, MouseState, ViewportPosition};
	static mut MOUSE_STATE: MouseKeys = 0;

	pub(super) fn translate_mouse_down(mod_keys: MouseKeys, position: ViewportPosition) -> Event {
		translate_mouse_event(mod_keys, position, true)
	}
	pub(super) fn translate_mouse_up(mod_keys: MouseKeys, position: ViewportPosition) -> Event {
		translate_mouse_event(mod_keys, position, false)
	}

	fn translate_mouse_event(mod_keys: MouseKeys, position: ViewportPosition, down: bool) -> Event {
		let diff = unsafe { MOUSE_STATE } ^ mod_keys;
		unsafe { MOUSE_STATE = mod_keys };
		let mouse_keys = events::MouseKeys::from_bits(mod_keys).expect("invalid modifier keys");
		let state = MouseState { position, mouse_keys };
		match (down, diff) {
			(true, 1) => Event::LmbDown(state),
			(true, 2) => Event::RmbDown(state),
			(true, 4) => Event::MmbDown(state),
			(false, 1) => Event::LmbUp(state),
			(false, 2) => Event::RmbUp(state),
			(false, 4) => Event::MmbUp(state),
			_ => panic!("two buttons where modified at the same time. modification: {:#010b}", diff),
		}
	}
}

/// Modify the currently selected tool in the document state store
#[wasm_bindgen]
pub fn select_tool(tool: String) -> Result<(), JsValue> {
	EDITOR_STATE.with(|editor| match translate_tool(&tool) {
		Some(tool) => editor.borrow_mut().handle_event(events::Event::SelectTool(tool)).map_err(convert_error),
		None => Err(Error::new(&format!("Couldn't select {} because it was not recognized as a valid tool", tool)).into()),
	})
}

// TODO: When a mouse button is down that started in the viewport, this should trigger even when the mouse is outside the viewport (or even the browser window if the browser supports it)
/// Mouse movement within the screenspace bounds of the viewport
#[wasm_bindgen]
pub fn on_mouse_move(x: u32, y: u32) -> Result<(), JsValue> {
	// TODO: Convert these screenspace viewport coordinates to canvas coordinates based on the current zoom and pan
	let ev = events::Event::MouseMove(events::ViewportPosition { x, y });
	EDITOR_STATE.with(|editor| editor.borrow_mut().handle_event(ev)).map_err(convert_error)
}

/// A mouse button depressed within screenspace the bounds of the viewport
#[wasm_bindgen]
pub fn on_mouse_down(x: u32, y: u32, mouse_keys: u8) -> Result<(), JsValue> {
	// TODO: Convert these screenspace viewport coordinates to canvas coordinates based on the current zoom and pan
	let pos = events::ViewportPosition { x, y };
	let ev = mouse_state::translate_mouse_down(mouse_keys, pos);
	EDITOR_STATE.with(|editor| editor.borrow_mut().handle_event(ev)).map_err(convert_error)
}

/// A mouse button released
#[wasm_bindgen]
pub fn on_mouse_up(x: u32, y: u32, mouse_keys: u8) -> Result<(), JsValue> {
	// TODO: Convert these screenspace viewport coordinates to canvas coordinates based on the current zoom and pan
	let pos = events::ViewportPosition { x, y };
	let ev = mouse_state::translate_mouse_up(mouse_keys, pos);
	EDITOR_STATE.with(|editor| editor.borrow_mut().handle_event(ev)).map_err(convert_error)
}

/// A keyboard button depressed within screenspace the bounds of the viewport
#[wasm_bindgen]
pub fn on_key_down(name: String) -> Result<(), JsValue> {
	let key = translate_key(&name);
	log::trace!("key down {:?}, name: {}", key, name);
	let ev = events::Event::KeyDown(key);
	EDITOR_STATE.with(|editor| editor.borrow_mut().handle_event(ev)).map_err(convert_error)
}

/// A keyboard button released
#[wasm_bindgen]
pub fn on_key_up(name: String) -> Result<(), JsValue> {
	let key = translate_key(&name);
	log::trace!("key up {:?}, name: {}", key, name);
	let ev = events::Event::KeyUp(key);
	EDITOR_STATE.with(|editor| editor.borrow_mut().handle_event(ev)).map_err(convert_error)
}

/// Update primary color
#[wasm_bindgen]
pub fn update_primary_color(primary_color: Color) -> Result<(), JsValue> {
	EDITOR_STATE
		.with(|editor| editor.borrow_mut().handle_event(events::Event::SelectPrimaryColor(primary_color.inner())))
		.map_err(convert_error)
}

/// Update secondary color
#[wasm_bindgen]
pub fn update_secondary_color(secondary_color: Color) -> Result<(), JsValue> {
	EDITOR_STATE
		.with(|editor| editor.borrow_mut().handle_event(events::Event::SelectSecondaryColor(secondary_color.inner())))
		.map_err(convert_error)
}

/// Swap primary and secondary color
#[wasm_bindgen]
pub fn swap_colors() -> Result<(), JsValue> {
	EDITOR_STATE.with(|editor| editor.borrow_mut().handle_event(events::Event::SwapColors)).map_err(convert_error)
}

/// Reset primary and secondary colors to their defaults
#[wasm_bindgen]
pub fn reset_colors() -> Result<(), JsValue> {
	EDITOR_STATE.with(|editor| editor.borrow_mut().handle_event(events::Event::ResetColors)).map_err(convert_error)
}
