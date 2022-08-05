use crate::JS_EDITOR_HANDLES;

use editor::messages::input_mapper::utility_types::input_keyboard::Key;
use editor::messages::prelude::*;

use std::panic;
use wasm_bindgen::prelude::*;

/// When a panic occurs, notify the user and log the error to the JS console before the backend dies
pub fn panic_hook(info: &panic::PanicInfo) {
	let header = "The editor crashed — sorry about that";
	let description = "An internal error occurred. Reload the editor to continue. Please report this by filing an issue on GitHub.";

	log::error!("{}", info);

	JS_EDITOR_HANDLES.with(|instances| {
		instances.borrow_mut().values_mut().for_each(|instance| {
			instance.send_frontend_message_to_js_rust_proxy(FrontendMessage::DisplayDialogPanic {
				panic_info: info.to_string(),
				header: header.to_string(),
				description: description.to_string(),
			})
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

/// Translate a keyboard key from its JS name to its Rust `Key` enum
pub fn translate_key(name: &str) -> Key {
	use Key::*;

	log::trace!("Key event received: {}", name);

	match name.to_lowercase().as_str() {
		"a" => KeyA,
		"b" => KeyB,
		"c" => KeyC,
		"d" => KeyD,
		"e" => KeyE,
		"f" => KeyF,
		"g" => KeyG,
		"h" => KeyH,
		"i" => KeyI,
		"j" => KeyJ,
		"k" => KeyK,
		"l" => KeyL,
		"m" => KeyM,
		"n" => KeyN,
		"o" => KeyO,
		"p" => KeyP,
		"q" => KeyQ,
		"r" => KeyR,
		"s" => KeyS,
		"t" => KeyT,
		"u" => KeyU,
		"v" => KeyV,
		"w" => KeyW,
		"x" => KeyX,
		"y" => KeyY,
		"z" => KeyZ,
		"0" => Key0,
		"1" => Key1,
		"2" => Key2,
		"3" => Key3,
		"4" => Key4,
		"5" => Key5,
		"6" => Key6,
		"7" => Key7,
		"8" => Key8,
		"9" => Key9,
		"enter" => KeyEnter,
		"=" => KeyEquals,
		"+" => KeyPlus,
		"-" => KeyMinus,
		"shift" => KeyShift,
		// When using linux + chrome + the neo keyboard layout, the shift key is recognized as caps
		"capslock" => KeyShift,
		" " => KeySpace,
		"control" => KeyControl,
		"command" => KeyCommand,
		"delete" => KeyDelete,
		"backspace" => KeyBackspace,
		"alt" => KeyAlt,
		"escape" => KeyEscape,
		"tab" => KeyTab,
		"arrowup" => KeyArrowUp,
		"arrowdown" => KeyArrowDown,
		"arrowleft" => KeyArrowLeft,
		"arrowright" => KeyArrowRight,
		"[" => KeyLeftBracket,
		"]" => KeyRightBracket,
		"{" => KeyLeftCurlyBracket,
		"}" => KeyRightCurlyBracket,
		"pageup" => KeyPageUp,
		"pagedown" => KeyPageDown,
		"," => KeyComma,
		"." => KeyPeriod,
		_ => UnknownKey,
	}
}
