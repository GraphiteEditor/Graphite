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
pub static NODE_GRAPH_ERROR_DISPLAYED: AtomicBool = AtomicBool::new(false);
pub static LOGGER: WasmLog = WasmLog;
thread_local! {
	pub static EDITOR: OnceCell<RefCell<editor::application::Editor>> = const { OnceCell::new() };
	pub static EDITOR_HANDLE: OnceCell<RefCell<editor_api::EditorHandle>> = const { OnceCell::new() };
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
	let info = info.to_string();
	let e = Error::new("stack");
	let stack = e.stack();
	let backtrace = stack.to_string();
	// error!("{:?}", backtrace);
	if backtrace.to_string().contains("DynAnyNode") {
		log::error!("Node graph evaluation panicked {info}");

		// When the graph panics, the node runtime lock may not be released properly
		if editor::node_graph_executor::NODE_RUNTIME.try_lock().is_none() {
			unsafe { editor::node_graph_executor::NODE_RUNTIME.force_unlock() };
		}

		if !NODE_GRAPH_ERROR_DISPLAYED.load(Ordering::SeqCst) {
			NODE_GRAPH_ERROR_DISPLAYED.store(true, Ordering::SeqCst);
			editor_api::editor_and_handle(|editor, handle| {
				let responses = editor.handle_message(DialogMessage::DisplayDialogError {
					title: "Node graph panicked".into(),
					description: "Node graph evaluation panicked, please check the log for more information.\n Please consider this as a critical error, and save your work before doing anything else. This is an experimental recovery feature, and it's not guaranteed to work.".into(),
				});
				for response in responses {
					handle.send_frontend_message_to_js_rust_proxy(response);
				}
			});
		}

		return;
	} else {
		EDITOR_HAS_CRASHED.store(true, Ordering::SeqCst);
	}

	error!("{info}");

	EDITOR_HANDLE.with(|editor_handle| {
		editor_handle.get().map(|handle| {
			handle
				.borrow_mut()
				.send_frontend_message_to_js_rust_proxy(FrontendMessage::DisplayDialogPanic { panic_info: info.to_string() })
		})
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
