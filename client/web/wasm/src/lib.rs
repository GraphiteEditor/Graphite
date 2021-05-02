pub mod document;
mod shims;
pub mod utils;
pub mod window;
pub mod wrappers;

use editor_core::{events::Response, Editor};
use std::cell::RefCell;
use utils::WasmLog;
use wasm_bindgen::prelude::*;
use wrappers::WasmResponse;

// the thread_local macro provides a way to initialize static variables with non-constant functions
thread_local! { pub static EDITOR_STATE: RefCell<Editor> = RefCell::new(Editor::new(Box::new(handle_response))) }
static LOGGER: WasmLog = WasmLog;

#[wasm_bindgen(start)]
pub fn init() {
	utils::set_panic_hook();
	log::set_logger(&LOGGER).expect("Failed to set logger");
	log::set_max_level(log::LevelFilter::Debug);
}

fn handle_response(response: Response) {
	let response_type = response.to_string();
	send_response(response_type, response);
}

fn send_response(response_type: String, response_data: Response) {
	let response_data = JsValue::from_serde(&WasmResponse::new(response_data)).expect("Failed to serialize response");
	handleResponse(response_type, response_data);
}

#[wasm_bindgen(module = "/../src/response-handler.ts")]
extern "C" {
	fn handleResponse(responseType: String, responseData: JsValue);
}

#[wasm_bindgen]
pub fn greet(name: &str) -> String {
	format!("Hello, {}!", name)
}
