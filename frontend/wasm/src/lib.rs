pub mod document;
mod shims;
pub mod utils;
pub mod wrappers;

use editor::{message_prelude::*, Editor};
use std::cell::RefCell;
use utils::WasmLog;
use wasm_bindgen::prelude::*;

// The thread_local macro provides a way to initialize static variables with non-constant functions
thread_local! {
	pub static EDITOR_STATE: RefCell<Editor> = RefCell::new(Editor::new());
}
static LOGGER: WasmLog = WasmLog;

#[wasm_bindgen(start)]
pub fn init() {
	utils::set_panic_hook();
	log::set_logger(&LOGGER).expect("Failed to set logger");
	log::set_max_level(log::LevelFilter::Debug);
}

// Sends FrontendMessages to JavaScript
pub fn dispatch<T: Into<Message>>(message: T) {
	let messages = EDITOR_STATE.with(|state| state.borrow_mut().handle_message(message.into()));

	for message in messages.into_iter() {
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
}

// The JavaScript function to call into
#[wasm_bindgen(module = "/../src/utilities/response-handler-binding.ts")]
extern "C" {
	#[wasm_bindgen(catch)]
	fn handleResponse(responseType: String, responseData: JsValue) -> Result<(), JsValue>;
}
