pub mod bezier;
pub mod subpath;
mod svg_drawing;
mod utils;

use wasm_bindgen::prelude::*;

#[cfg(feature = "logging")]
pub static LOGGER: WasmLog = WasmLog;
#[cfg(feature = "logging")]
thread_local! { pub static HAS_CRASHED: std::cell::RefCell<bool> = const { std::cell::RefCell::new(false) } }

/// Initialize the backend
#[wasm_bindgen(start)]
pub fn init() {
	#[cfg(feature = "logging")]
	{
		// Set up the logger with a default level of debug
		log::set_logger(&LOGGER).expect("Failed to set logger");
		log::set_max_level(log::LevelFilter::Trace);

		fn panic_hook(info: &std::panic::PanicHookInfo<'_>) {
			// Skip if we have already panicked
			if HAS_CRASHED.with(|cell| cell.replace(true)) {
				return;
			}
			log::error!("{}", info);
		}

		std::panic::set_hook(Box::new(panic_hook));
	}
}

/// Logging to the JS console
#[cfg(feature = "logging")]
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

#[cfg(feature = "logging")]
#[derive(Default)]
pub struct WasmLog;

#[cfg(feature = "logging")]
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
