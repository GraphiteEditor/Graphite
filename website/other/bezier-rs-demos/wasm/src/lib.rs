pub mod bezier;
pub mod subpath;
mod svg_drawing;
mod utils;

use wasm_bindgen::prelude::*;
pub static LOGGER: WasmLog = WasmLog;

/// Initialize the backend
#[wasm_bindgen(start)]
pub fn init() {
	// Set up the logger with a default level of debug
	log::set_logger(&LOGGER).expect("Failed to set logger");
	log::set_max_level(log::LevelFilter::Trace);
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
