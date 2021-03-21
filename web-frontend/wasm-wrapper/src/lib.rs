mod utils;
mod viewport;
mod window;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
	fn alert(s: &str);
}

#[wasm_bindgen(start)]
pub fn init() {
	utils::set_panic_hook();
	alert("Hello, Graphite!");
}

/// Send events
#[wasm_bindgen]
pub fn handle_event(event_name: String) {
	// TODO: add payload
	todo!()
}
