pub mod api;
pub mod helpers;

use helpers::{panic_hook, WasmLog};
use std::cell::RefCell;
use std::collections::HashMap;
use std::panic;
use std::sync::atomic::AtomicBool;
use wasm_bindgen::prelude::*;

// Set up the persistent editor backend state
pub static EDITOR_HAS_CRASHED: AtomicBool = AtomicBool::new(false);
pub static LOGGER: WasmLog = WasmLog;
thread_local! {
	pub static EDITOR_INSTANCES: RefCell<HashMap<u64, editor::Editor>> = RefCell::new(HashMap::new());
	pub static JS_EDITOR_HANDLES: RefCell<HashMap<u64, api::JsEditorHandle>> = RefCell::new(HashMap::new());
}

/// Initialize the backend
#[wasm_bindgen(start)]
pub fn init() {
	// Set up the panic hook
	panic::set_hook(Box::new(panic_hook));

	// Set up the logger with a default level of debug
	log::set_logger(&LOGGER).expect("Failed to set logger");
	log::set_max_level(log::LevelFilter::Debug);
}
