pub mod api;
mod helpers;
pub mod logging;
pub mod type_translators;

use editor::message_prelude::FrontendMessage;
use logging::WasmLog;
use std::cell::RefCell;
use std::collections::HashMap;
use std::panic;
use std::sync::atomic::AtomicBool;
use wasm_bindgen::prelude::*;

// Set up the persistent editor backend state
static LOGGER: WasmLog = WasmLog;
thread_local! {
	pub static EDITOR_INSTANCES: RefCell<HashMap<u64, (editor::Editor, api::JsEditorHandle)>> = RefCell::new(HashMap::new());
}

pub static EDITOR_HAS_CRASHED: AtomicBool = AtomicBool::new(false);

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
	log::error!("{}", info);
	EDITOR_INSTANCES.with(|instances| {
		instances.borrow_mut().values_mut().for_each(|instance| {
			instance.1.handle_response_rust_proxy(FrontendMessage::DisplayPanic {
				panic_info: panic_info.clone(),
				title: title.clone(),
				description: description.clone(),
			})
		})
	});
}
