use wasm_bindgen::prelude::*;
pub fn set_panic_hook() {
	// When the `console_error_panic_hook` feature is enabled, we can call the
	// `set_panic_hook` function at least once during initialization, and then
	// we will get better error messages if our code ever panics.
	//
	// For more details see
	// https://github.com/rustwasm/console_error_panic_hook#readme
	#[cfg(feature = "console_error_panic_hook")]
	console_error_panic_hook::set_once();
}

#[wasm_bindgen]
extern "C" {
	#[wasm_bindgen(js_namespace = console)]
	fn debug(msg: &str, format: &str);
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
			log::Level::Trace => (debug, "trace", "color:plum"),
			log::Level::Debug => (debug, "debug", "color:plum"),
			log::Level::Warn => (warn, "warn", "color:#1b8"),
			log::Level::Info => (info, "info", "color:#fa2"),
			log::Level::Error => (error, "error", "color:red"),
		};
		let msg = &format!("{}", format_args!("%c{}%c\t{}", name, record.args()));
		log(msg, color)
	}
	fn flush(&self) {}
}
