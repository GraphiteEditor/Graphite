pub mod document;
mod shims;
pub mod utils;
pub mod wrappers;

use editor::{message_prelude::*, Editor};
use std::cell::RefCell;
use std::panic;
use std::sync::atomic::AtomicBool;
use utils::WasmLog;
use wasm_bindgen::prelude::*;

// Set up the persistent editor backend state (the thread_local macro provides a way to initialize static variables with non-constant functions)
thread_local! { pub static EDITOR_STATE: RefCell<Editor> = RefCell::new(Editor::new()); }
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

	handle_response(FrontendMessage::DisplayPanic { panic_info, title, description });

	EDITOR_HAS_CRASHED.store(true, std::sync::atomic::Ordering::SeqCst);
}

// Sends a message to the dispatcher in the Editor Backend
fn dispatch<T: Into<Message>>(message: T) {
	// Process no further messages after a crash to avoid spamming the console
	if EDITOR_HAS_CRASHED.load(std::sync::atomic::Ordering::SeqCst) {
		return;
	}

	// Dispatch the message and receive a vector of FrontendMessage responses
	let responses = EDITOR_STATE.with(|state| state.try_borrow_mut().ok().map(|mut state| state.handle_message(message.into())));
	for response in responses.unwrap_or_default().into_iter() {
		// Send each FrontendMessage to the JavaScript frontend
		handle_response(response);
	}
}

// Sends a FrontendMessage to JavaScript
fn handle_response(message: FrontendMessage) {
	let message_type = message.to_discriminant().local_name();
	let message_data = JsValue::from_serde(&message).expect("Failed to serialize FrontendMessage");

	let js_return_value = handleResponse(message_type, message_data);
	if let Err(error) = js_return_value {
		log::error!(
			"While handling FrontendMessage \"{:?}\", JavaScript threw an error: {:?}",
			message.to_discriminant().local_name(),
			error,
		)
	}
}

// The JavaScript function to call into with each FrontendMessage
#[wasm_bindgen(module = "/../src/utilities/response-handler-binding.ts")]
extern "C" {
	#[wasm_bindgen(catch)]
	fn handleResponse(responseType: String, responseData: JsValue) -> Result<(), JsValue>;
}
