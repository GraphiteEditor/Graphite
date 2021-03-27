mod shims;
pub mod utils;
pub mod viewport;
pub mod window;
pub mod wrappers;

use wasm_bindgen::prelude::*;

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
