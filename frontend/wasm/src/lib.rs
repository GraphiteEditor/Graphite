#![doc = include_str!("../README.md")]

// `macro_use` puts the log macros (`error!`, `warn!`, `debug!`, `info!` and `trace!`) in scope for the crate
#[macro_use]
extern crate log;

pub mod editor_api;
pub mod helpers;

use editor::messages::prelude::*;

use std::cell::{OnceCell, RefCell};
use std::panic;
use std::sync::atomic::{AtomicBool, Ordering};
use wasm_bindgen::prelude::*;

// Set up the persistent editor backend state
pub static EDITOR_HAS_CRASHED: AtomicBool = AtomicBool::new(false);
pub static LOGGER: WasmLog = WasmLog;
thread_local! {
	pub static EDITOR: OnceCell<RefCell<editor::application::Editor>> = OnceCell::new();
	pub static EDITOR_HANDLE: OnceCell<RefCell<editor_api::EditorHandle>> = OnceCell::new();
}

/// Initialize the backend
#[wasm_bindgen(start)]
pub fn init_graphite() {
	// Set up the panic hook
	panic::set_hook(Box::new(panic_hook));

	// Set up the logger with a default level of debug
	log::set_logger(&LOGGER).expect("Failed to set logger");
	log::set_max_level(log::LevelFilter::Debug);
}

/// When a panic occurs, notify the user and log the error to the JS console before the backend dies
pub fn panic_hook(info: &panic::PanicInfo) {
	EDITOR_HAS_CRASHED.store(true, Ordering::SeqCst);

	error!("{info}");

	EDITOR_HANDLE.with(|editor_handle| {
		editor_handle.get().map(|handle| {
			handle
				.borrow_mut()
				.send_frontend_message_to_js_rust_proxy(FrontendMessage::DisplayDialogPanic { panic_info: info.to_string() })
		})
	});
}

/// The JavaScript `Error` type
#[wasm_bindgen]
extern "C" {
	#[derive(Clone, Debug)]
	pub type Error;

	#[wasm_bindgen(constructor)]
	pub fn new(msg: &str) -> Error;
}

/// Logging to the JS console
#[wasm_bindgen]
extern "C" {
	#[wasm_bindgen(js_namespace = console)]
	fn log(msg: &str, format: &str);
	#[wasm_bindgen(js_namespace = console)]
	fn info(msg: &str, format: &str);
	#[wasm_bindgen(js_namespace = console)]
	fn warn(msg: &str, format: &str);
	#[wasm_bindgen(js_namespace = console)]
	fn error(msg: &str, format: &str);
}

#[derive(Default)]
pub struct WasmLog;

impl log::Log for WasmLog {
	fn enabled(&self, metadata: &log::Metadata) -> bool {
		metadata.level() <= log::Level::Info
	}

	fn log(&self, record: &log::Record) {
		let (log, name, color): (fn(&str, &str), &str, &str) = match record.level() {
			log::Level::Trace => (log, "trace", "color:plum"),
			log::Level::Debug => (log, "debug", "color:cyan"),
			log::Level::Warn => (warn, "warn", "color:goldenrod"),
			log::Level::Info => (info, "info", "color:mediumseagreen"),
			log::Level::Error => (error, "error", "color:red"),
		};
		let msg = &format!("%c{}\t{}", name, record.args());
		log(msg, color)
	}

	fn flush(&self) {}
}
