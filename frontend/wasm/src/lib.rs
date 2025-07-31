#![doc = include_str!("../README.md")]

// `macro_use` puts the log macros (`error!`, `warn!`, `debug!`, `info!` and `trace!`) in scope for the crate
#[macro_use]
extern crate log;

pub mod editor_api;
pub mod helpers;
pub mod native_communcation;

use editor::messages::prelude::*;
use std::panic;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use wasm_bindgen::prelude::*;

// Set up the persistent editor backend state
pub static EDITOR_HAS_CRASHED: AtomicBool = AtomicBool::new(false);
pub static NODE_GRAPH_ERROR_DISPLAYED: AtomicBool = AtomicBool::new(false);
pub static LOGGER: WasmLog = WasmLog;

thread_local! {
	pub static EDITOR: Mutex<Option<editor::application::Editor>> = const { Mutex::new(None) };
	pub static EDITOR_HANDLE: Mutex<Option<editor_api::EditorHandle>> = const { Mutex::new(None) };
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
pub fn panic_hook(info: &panic::PanicHookInfo) {
	let info = info.to_string();
	let backtrace = Error::new("stack").stack().to_string();
	if backtrace.contains("DynAnyNode") {
		log::error!("Node graph evaluation panicked {info}");

		// When the graph panics, the node runtime lock may not be released properly
		if editor::node_graph_executor::NODE_RUNTIME.try_lock().is_none() {
			unsafe { editor::node_graph_executor::NODE_RUNTIME.force_unlock() };
		}

		if !NODE_GRAPH_ERROR_DISPLAYED.load(Ordering::SeqCst) {
			NODE_GRAPH_ERROR_DISPLAYED.store(true, Ordering::SeqCst);
			editor_api::editor_and_handle(|_, handle| {
				let error = r#"
				<rect x="50%" y="50%" width="600" height="100" transform="translate(-300 -50)" rx="4" fill="var(--color-error-red)" />
				<text x="50%" y="50%" dominant-baseline="middle" text-anchor="middle" font-size="18" fill="var(--color-2-mildblack)">
					<tspan x="50%" dy="-24" font-weight="bold">The document crashed while being rendered in its current state.</tspan>
					<tspan x="50%" dy="24">The editor is now unstable! Undo your last action to restore the artwork,</tspan>
					<tspan x="50%" dy="24">then save your document and restart the editor before continuing work.</tspan>
				/text>"#
				// It's a mystery why the `/text>` tag above needs to be missing its `<`, but when it exists it prints the `<` character in the text. However this works with it removed.
				.to_string();
				handle.send_frontend_message_to_js_rust_proxy(FrontendMessage::UpdateDocumentArtwork { svg: error });
			});
		}

		return;
	} else {
		EDITOR_HAS_CRASHED.store(true, Ordering::SeqCst);
	}

	log::error!("{info}");

	EDITOR_HANDLE.with(|editor_handle| {
		let mut guard = editor_handle.lock();
		if let Ok(Some(handle)) = guard.as_deref_mut() {
			handle.send_frontend_message_to_js_rust_proxy(FrontendMessage::DisplayDialogPanic { panic_info: info.to_string() });
		}
	});
}

#[wasm_bindgen]
extern "C" {
	/// The JavaScript `Error` type
	#[derive(Clone, Debug)]
	pub type Error;

	#[wasm_bindgen(constructor)]
	pub fn new(msg: &str) -> Error;

	#[wasm_bindgen(structural, method, getter)]
	fn stack(error: &Error) -> String;
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
	#[wasm_bindgen(js_namespace = console)]
	fn trace(msg: &str, format: &str);
}

#[derive(Default)]
pub struct WasmLog;

impl log::Log for WasmLog {
	#[inline]
	fn enabled(&self, metadata: &log::Metadata) -> bool {
		metadata.level() <= log::max_level()
	}

	fn log(&self, record: &log::Record) {
		if !self.enabled(record.metadata()) {
			return;
		}

		let (log, name, color): (fn(&str, &str), &str, &str) = match record.level() {
			log::Level::Trace => (log, "trace", "color:plum"),
			log::Level::Debug => (log, "debug", "color:cyan"),
			log::Level::Warn => (warn, "warn", "color:goldenrod"),
			log::Level::Info => (info, "info", "color:mediumseagreen"),
			log::Level::Error => (error, "error", "color:red"),
		};

		// The %c is replaced by the message color
		if record.level() == log::Level::Info {
			// We don't print the file name and line number for info-level logs because it's used for printing the message system logs
			log(&format!("%c{}\t{}", name, record.args()), color);
		} else {
			let file = record.file().unwrap_or_else(|| record.target());
			let line = record.line().map_or_else(|| "[Unknown]".to_string(), |line| line.to_string());
			let args = record.args();

			log(&format!("%c{name}\t{file}:{line}\n{args}"), color);
		}
	}

	fn flush(&self) {}
}
