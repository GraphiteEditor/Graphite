pub mod document;
mod shims;
pub mod utils;
pub mod window;
pub mod wrappers;

use editor_core::{message_prelude::*, Editor};
use std::cell::RefCell;
use utils::WasmLog;
use wasm_bindgen::prelude::*;

// the thread_local macro provides a way to initialize static variables with non-constant functions
thread_local! { pub static EDITOR_STATE: RefCell<Editor> = RefCell::new(Editor::new(Box::new(handle_response))) }
static LOGGER: WasmLog = WasmLog;

#[wasm_bindgen(start)]
pub fn init() {
	utils::set_panic_hook();
	log::set_logger(&LOGGER).expect("Failed to set logger");
	log::set_max_level(log::LevelFilter::Debug);
}

#[wasm_bindgen(module = "/../src/utilities/response-handler-binding.ts")]
extern "C" {
	#[wasm_bindgen(catch)]
	fn handleResponse(responseType: String, responseData: JsValue) -> Result<(), JsValue>;
}

fn handle_response(response: FrontendMessage) {
	let response_type = response.to_discriminant().local_name();
	send_response(response_type, response);
}

fn send_response(response_type: String, response_data: FrontendMessage) {
	let response_data = JsValue::from_serde(&response_data).expect("Failed to serialize response");
	let _ = handleResponse(response_type, response_data).map_err(|error| log::error!("javascript threw an error: {:?}", error));
}
