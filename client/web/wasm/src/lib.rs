mod shims;
pub mod utils;
pub mod viewport;
pub mod window;
pub mod wrappers;

use graphite_editor_core::Editor;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;

// the thread_local macro provides a way to initialize static variables with non-constant functions
thread_local! {pub static EDITOR_STATE: RefCell<Editor> = RefCell::new(Editor::new())}

#[wasm_bindgen(start)]
pub fn init() {
	utils::set_panic_hook();
}

/// Send events
#[wasm_bindgen]
pub fn handle_event(event_name: String) {
	// TODO: add payload
	todo!()
}

#[wasm_bindgen]
pub fn greet(name: &str) -> String {
	format!("Hello, {}!", name)
}
