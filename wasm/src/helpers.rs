use crate::JS_EDITOR_HANDLES;

use editor::messages::input_mapper::utility_types::input_keyboard::Key;
use editor::messages::prelude::*;

use std::panic;
use wasm_bindgen::prelude::*;

/// When a panic occurs, notify the user and log the error to the JS console before the backend dies
pub fn panic_hook(info: &panic::PanicInfo) {
	let header = "The editor crashed — sorry about that";
	let description = "
	An internal error occurred. Please report this by filing an issue on GitHub.\n\
	\n\
	Reload the editor to continue. If this happens immediately on repeated reloads, clear saved data.
	"
	.trim();

	error!("{}", info);

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

	trace!("Key event received: {}", name);

	match name {
		// Writing system keys
		"Digit0" | "Numpad0" => Digit0,
		"Digit1" | "Numpad1" => Digit1,
		"Digit2" | "Numpad2" => Digit2,
		"Digit3" | "Numpad3" => Digit3,
		"Digit4" | "Numpad4" => Digit4,
		"Digit5" | "Numpad5" => Digit5,
		"Digit6" | "Numpad6" => Digit6,
		"Digit7" | "Numpad7" => Digit7,
		"Digit8" | "Numpad8" => Digit8,
		"Digit9" | "Numpad9" => Digit9,
		//
		"KeyA" => KeyA,
		"KeyB" => KeyB,
		"KeyC" => KeyC,
		"KeyD" => KeyD,
		"KeyE" => KeyE,
		"KeyF" => KeyF,
		"KeyG" => KeyG,
		"KeyH" => KeyH,
		"KeyI" => KeyI,
		"KeyJ" => KeyJ,
		"KeyK" => KeyK,
		"KeyL" => KeyL,
		"KeyM" => KeyM,
		"KeyN" => KeyN,
		"KeyO" => KeyO,
		"KeyP" => KeyP,
		"KeyQ" => KeyQ,
		"KeyR" => KeyR,
		"KeyS" => KeyS,
		"KeyT" => KeyT,
		"KeyU" => KeyU,
		"KeyV" => KeyV,
		"KeyW" => KeyW,
		"KeyX" => KeyX,
		"KeyY" => KeyY,
		"KeyZ" => KeyZ,
		//
		"Backquote" => Backquote,
		"Backslash" => Backslash,
		"BracketLeft" => BracketLeft,
		"BracketRight" => BracketRight,
		"Comma" | "NumpadComma" => Comma,
		"Equal" | "NumpadEqual" => Equal,
		"Minus" | "NumpadSubtract" => Minus,
		"Period" | "NumpadDecimal" => Period,
		"Quote" => Quote,
		"Semicolon" => Semicolon,
		"Slash" | "NumpadDivide" => Slash,

		// Functional keys
		"AltLeft" | "AltRight" | "AltGraph" => Alt,
		"MetaLeft" | "MetaRight" => Meta,
		"ShiftLeft" | "ShiftRight" => Shift,
		"ControlLeft" | "ControlRight" => Control,
		"Backspace" | "NumpadBackspace" => Backspace,
		"CapsLock" => CapsLock,
		"ContextMenu" => ContextMenu,
		"Enter" | "NumpadEnter" => Enter,
		"Space" => Space,
		"Tab" => Tab,

		// Control pad keys
		"Delete" => Delete,
		"End" => End,
		"Help" => Help,
		"Home" => Home,
		"Insert" => Insert,
		"PageDown" => PageDown,
		"PageUp" => PageUp,

		// Arrow pad keys
		"ArrowDown" => ArrowDown,
		"ArrowLeft" => ArrowLeft,
		"ArrowRight" => ArrowRight,
		"ArrowUp" => ArrowUp,

		// Numpad keys
		// "Numpad0" => KeyNumpad0,
		// "Numpad1" => KeyNumpad1,
		// "Numpad2" => KeyNumpad2,
		// "Numpad3" => KeyNumpad3,
		// "Numpad4" => KeyNumpad4,
		// "Numpad5" => KeyNumpad5,
		// "Numpad6" => KeyNumpad6,
		// "Numpad7" => KeyNumpad7,
		// "Numpad8" => KeyNumpad8,
		// "Numpad9" => KeyNumpad9,
		"NumLock" => NumLock,
		"NumpadAdd" => NumpadAdd,
		// "NumpadBackspace" => KeyNumpadBackspace,
		// "NumpadClear" => NumpadClear,
		// "NumpadClearEntry" => NumpadClearEntry,
		// "NumpadComma" => KeyNumpadComma,
		// "NumpadDecimal" => KeyNumpadDecimal,
		// "NumpadDivide" => KeyNumpadDivide,
		// "NumpadEnter" => KeyNumpadEnter,
		// "NumpadEqual" => KeyNumpadEqual,
		"NumpadHash" => NumpadHash,
		// "NumpadMemoryAdd" => NumpadMemoryAdd,
		// "NumpadMemoryClear" => NumpadMemoryClear,
		// "NumpadMemoryRecall" => NumpadMemoryRecall,
		// "NumpadMemoryStore" => NumpadMemoryStore,
		// "NumpadMemorySubtract" => NumpadMemorySubtract,
		"NumpadMultiply" | "NumpadStar" => NumpadMultiply,
		"NumpadParenLeft" => NumpadParenLeft,
		"NumpadParenRight" => NumpadParenRight,
		// "NumpadStar" => NumpadStar,
		// "NumpadSubtract" => KeyNumpadSubtract,

		// Function keys
		"Escape" => Escape,
		"F1" => F1,
		"F2" => F2,
		"F3" => F3,
		"F4" => F4,
		"F5" => F5,
		"F6" => F6,
		"F7" => F7,
		"F8" => F8,
		"F9" => F9,
		"F10" => F10,
		"F11" => F11,
		"F12" => F12,
		"F13" => F13,
		"F14" => F14,
		"F15" => F15,
		"F16" => F16,
		"F17" => F17,
		"F18" => F18,
		"F19" => F19,
		"F20" => F20,
		"F21" => F21,
		"F22" => F22,
		"F23" => F23,
		"F24" => F24,
		"Fn" => Fn,
		"FnLock" => FnLock,
		"PrintScreen" => PrintScreen,
		"ScrollLock" => ScrollLock,
		"Pause" => Pause,

		// Unidentified keys
		_ => Unidentified,
	}
}
