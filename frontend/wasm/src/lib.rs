pub mod document;
mod shims;
pub mod utils;
pub mod wrappers;

use editor::{message_prelude::*, Editor};
use std::cell::RefCell;
use std::sync::atomic::AtomicBool;
use utils::WasmLog;
use wasm_bindgen::prelude::*;

// The thread_local macro provides a way to initialize static variables with non-constant functions
thread_local! {
	pub static EDITOR_STATE: RefCell<Editor> = RefCell::new(Editor::new());
}
static LOGGER: WasmLog = WasmLog;
static EDITOR_HAS_CRASHED: AtomicBool = AtomicBool::new(false);

#[wasm_bindgen(start)]
pub fn init() {
	utils::set_panic_hook();
	log::set_logger(&LOGGER).expect("Failed to set logger");
	log::set_max_level(log::LevelFilter::Debug);
}

// Sends FrontendMessages to JavaScript
fn dispatch<T: Into<Message>>(message: T) {
	// Process no further messages after a crash to avoid spamming the console
	if EDITOR_HAS_CRASHED.load(std::sync::atomic::Ordering::SeqCst) {
		return;
	}

	match EDITOR_STATE.with(|state| state.try_borrow_mut().ok().map(|mut state| state.handle_message(message.into()))) {
		Some(messages) => {
			for message in messages.into_iter() {
				handle_response(message);
			}
		}
		None => {
			EDITOR_HAS_CRASHED.store(true, std::sync::atomic::Ordering::SeqCst);

			let title = "The editor crashed — sorry about that".to_string();
			let description = "An internal error occurred. Reload the editor to continue. Please report this by filing an issue on GitHub.".to_string();

			handle_response(FrontendMessage::DisplayPanic { title, description });
		}
	}
}

// Sends a FrontendMessage to JavaScript
fn handle_response(message: FrontendMessage) {
	let message_type = message.to_discriminant().local_name();
	let message_data = JsValue::from_serde(&message).expect("Failed to serialize response");

	let _ = handleResponse(message_type, message_data).map_err(|error| {
		log::error!(
			"While handling FrontendMessage \"{:?}\", JavaScript threw an error: {:?}",
			message.to_discriminant().local_name(),
			error
		)
	});
}

// The JavaScript function to call into
#[wasm_bindgen(module = "/../src/utilities/response-handler-binding.ts")]
extern "C" {
	#[wasm_bindgen(catch)]
	fn handleResponse(responseType: String, responseData: JsValue) -> Result<(), JsValue>;
}
