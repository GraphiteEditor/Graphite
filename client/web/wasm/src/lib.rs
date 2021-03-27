mod shims;
pub mod utils;
pub mod viewport;
pub mod window;
pub mod wrappers;

use graphite_editor_core::{events::Response, Callback, Editor};
use std::cell::RefCell;
use wasm_bindgen::prelude::*;

// the thread_local macro provides a way to initialize static variables with non-constant functions
thread_local! {pub static EDITOR_STATE: RefCell<Editor> = RefCell::new(Editor::new(Box::new(handle_response)))}

#[wasm_bindgen(start)]
pub fn init() {
	utils::set_panic_hook();
}

fn handle_response(response: Response) {
	match response {
		Response::UpdateCanvas => update_canvas(),
	}
}

#[wasm_bindgen(module = "/../src/wasm-callback-processor.js")]
extern "C" {
	fn update_canvas();
}

#[wasm_bindgen]
pub fn greet(name: &str) -> String {
	format!("Hello, {}!", name)
}
