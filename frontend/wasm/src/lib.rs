pub mod api;
mod helpers;
pub mod logging;
pub mod type_translators;

use logging::WasmLog;
use std::panic;
use std::sync::atomic::AtomicBool;
use wasm_bindgen::prelude::*;

// Set up the persistent editor backend state (the thread_local macro provides a way to initialize static variables with non-constant functions)
static LOGGER: WasmLog = WasmLog;
static EDITOR_HAS_CRASHED: AtomicBool = AtomicBool::new(false);

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

	EDITOR_HAS_CRASHED.store(true, std::sync::atomic::Ordering::SeqCst);

	//handle_response(FrontendMessage::DisplayPanic { panic_info, title, description });
	let _ = panicHook(panic_info, title, description);
}

#[wasm_bindgen(module = "/../src/utilities/wasm-loader-exports.ts")]
extern "C" {
	// The JavaScript function to call into with each FrontendMessage
	#[wasm_bindgen(catch)]
	fn handleResponse(callback: &JsValue, responseType: String, responseData: JsValue) -> Result<(), JsValue>;
	// The panic hook
	#[wasm_bindgen(catch)]
	fn panicHook(panic_info: String, title: String, description: String) -> Result<(), JsValue>;
}
