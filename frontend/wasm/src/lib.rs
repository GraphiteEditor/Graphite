pub mod api;
mod helpers;
pub mod logging;
pub mod type_translators;

use editor::message_prelude::FrontendMessage;
use logging::WasmLog;
use std::cell::RefCell;
use std::panic;
use wasm_bindgen::prelude::*;

// Set up the persistent editor backend state
static LOGGER: WasmLog = WasmLog;
thread_local! { pub static EDITOR_HAS_CRASHED: RefCell<Option<FrontendMessage>> = RefCell::new(None); }

// Initialize the backend
#[wasm_bindgen(start)]
pub fn init() {
	panic::set_hook(Box::new(panic_hook));

	log::set_logger(&LOGGER).expect("Failed to set logger");
	log::set_max_level(log::LevelFilter::Debug);
}

// When a panic occurs, close up shop before the backend dies
fn panic_hook(info: &panic::PanicInfo) {
	let panic_info = info.to_string();
	let title = "The editor crashed — sorry about that".to_string();
	let description = "An internal error occurred. Reload the editor to continue. Please report this by filing an issue on GitHub.".to_string();

	EDITOR_HAS_CRASHED.with(|crash_status| crash_status.borrow_mut().replace(FrontendMessage::DisplayPanic { panic_info, title, description }));
}

// The TS file that wasm-bindgen instantiates and calls into
#[wasm_bindgen(module = "/../src/utilities/wasm-loader-exports.ts")]
extern "C" {
	// The JavaScript function to call into with each FrontendMessage
	#[wasm_bindgen(catch)]
	fn handleJsMessage(callback: &JsValue, responseType: String, responseData: JsValue) -> Result<(), JsValue>;
}
