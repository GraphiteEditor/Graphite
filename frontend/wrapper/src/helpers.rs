#[cfg(not(feature = "native"))]
use crate::EDITOR;
use crate::editor_wrapper::EditorWrapper;
use crate::{EDITOR_HAS_CRASHED, EDITOR_WRAPPER};
#[cfg(not(feature = "native"))]
use editor::application::Editor;
use editor::messages::input_mapper::utility_types::input_keyboard::Key;
use editor::messages::prelude::*;
use graphene_std::raster::Image;
use graphene_std::raster::color::Color;
use js_sys::{Object, Reflect};
use std::sync::atomic::Ordering;
use std::time::Duration;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, ImageData, window};

/// Helper function for calling JS's `requestAnimationFrame` with the given closure
pub(crate) fn request_animation_frame(f: &Closure<dyn FnMut(f64)>) {
	web_sys::window()
		.expect("No global `window` exists")
		.request_animation_frame(f.as_ref().unchecked_ref())
		.expect("Failed to call `requestAnimationFrame`");
}

/// Helper function for calling JS's `setTimeout` with the given closure and delay
pub(crate) fn set_timeout(f: &Closure<dyn FnMut()>, delay: Duration) {
	let delay = delay.clamp(Duration::ZERO, Duration::from_millis(i32::MAX as u64)).as_millis() as i32;
	web_sys::window()
		.expect("No global `window` exists")
		.set_timeout_with_callback_and_timeout_and_arguments_0(f.as_ref().unchecked_ref(), delay)
		.expect("Failed to call `setTimeout`");
}

/// Provides access to the `Editor` by calling the given closure with it as an argument.
#[cfg(not(feature = "native"))]
fn editor<T: Default>(callback: impl FnOnce(&mut editor::application::Editor) -> T) -> T {
	EDITOR.with(|editor| {
		let mut guard = editor.try_lock();
		let Ok(Some(editor)) = guard.as_deref_mut() else {
			log::error!("Failed to borrow editor");
			return T::default();
		};

		callback(editor)
	})
}

/// Provides access to the `Editor` and its `EditorWrapper` by calling the given closure with them as arguments.
#[cfg(not(feature = "native"))]
pub(crate) fn editor_and_wrapper(callback: impl FnOnce(&mut Editor, &mut EditorWrapper)) {
	wrapper(|wrapper| {
		editor(|editor| {
			// Call the closure with the editor and its wrapper
			callback(editor, wrapper);
		})
	});
}
/// Provides access to the `EditorWrapper` by calling the given closure with them as arguments.
pub(crate) fn wrapper(callback: impl FnOnce(&mut EditorWrapper)) {
	EDITOR_WRAPPER.with(|wrapper| {
		let mut guard = wrapper.try_lock();
		let Ok(Some(wrapper)) = guard.as_deref_mut() else {
			log::error!("Failed to borrow editor wrapper");
			return;
		};

		// Call the closure with the editor and its wrapper
		callback(wrapper);
	});
}

#[cfg(not(feature = "native"))]
pub(crate) async fn poll_node_graph_evaluation() {
	// Process no further messages after a crash to avoid spamming the console
	if EDITOR_HAS_CRASHED.load(Ordering::SeqCst) {
		return;
	}

	if !editor::node_graph_executor::run_node_graph().await.0 {
		return;
	}

	editor_and_wrapper(|editor, wrapper| {
		let mut messages = VecDeque::new();
		if let Err(e) = editor.poll_node_graph_evaluation(&mut messages) {
			// TODO: This is a hacky way to suppress the error, but it shouldn't be generated in the first place
			if e != "No active document" {
				error!("Error evaluating node graph:\n{e}");
			}
		}

		// Clear the error display if there are no more errors
		if !messages.is_empty() {
			crate::NODE_GRAPH_ERROR_DISPLAYED.store(false, Ordering::SeqCst);
		}

		// Batch responses to pool frontend updates
		let batched = Message::Batched {
			messages: messages.into_iter().collect(),
		};
		// Send each `FrontendMessage` to the JavaScript frontend
		for response in editor.handle_message(batched) {
			wrapper.send_frontend_message_to_js(response);
		}

		// If the editor cannot be borrowed then it has encountered a panic - we should just ignore new dispatches
	});
}

pub(crate) fn auto_save_all_documents() {
	// Process no further messages after a crash to avoid spamming the console
	if EDITOR_HAS_CRASHED.load(Ordering::SeqCst) {
		return;
	}

	wrapper(|wrapper| {
		wrapper.dispatch(PortfolioMessage::AutoSaveAllDocuments);
	});
}

pub(crate) fn render_image_data_to_canvases(image_data: &[(u64, Image<Color>)]) {
	let window = match window() {
		Some(window) => window,
		None => {
			error!("Cannot render canvas: window object not found");
			return;
		}
	};
	let document = window.document().expect("window should have a document");
	let window_obj = Object::from(window);
	let image_canvases_key = JsValue::from_str("imageCanvases");

	let canvases_obj = match Reflect::get(&window_obj, &image_canvases_key) {
		Ok(obj) if !obj.is_undefined() && !obj.is_null() => obj,
		_ => {
			let new_obj = Object::new();
			if Reflect::set(&window_obj, &image_canvases_key, &new_obj).is_err() {
				error!("Failed to create and set imageCanvases object on window");
				return;
			}
			new_obj.into()
		}
	};
	let canvases_obj = Object::from(canvases_obj);

	for (placeholder_id, image) in image_data.iter() {
		let canvas_name = placeholder_id.to_string();
		let js_key = JsValue::from_str(&canvas_name);

		if Reflect::has(&canvases_obj, &js_key).unwrap_or(false) || image.width == 0 || image.height == 0 {
			continue;
		}

		let canvas: HtmlCanvasElement = document
			.create_element("canvas")
			.expect("Failed to create canvas element")
			.dyn_into::<HtmlCanvasElement>()
			.expect("Failed to cast element to HtmlCanvasElement");

		canvas.set_width(image.width);
		canvas.set_height(image.height);

		let context: CanvasRenderingContext2d = canvas
			.get_context("2d")
			.expect("Failed to get 2d context")
			.expect("2d context was not found")
			.dyn_into::<CanvasRenderingContext2d>()
			.expect("Failed to cast context to CanvasRenderingContext2d");
		let u8_data: Vec<u8> = image.data.iter().flat_map(|color| color.to_rgba8_srgb()).collect();
		let clamped_u8_data = wasm_bindgen::Clamped(&u8_data[..]);
		match ImageData::new_with_u8_clamped_array_and_sh(clamped_u8_data, image.width, image.height) {
			Ok(image_data_obj) => {
				if context.put_image_data(&image_data_obj, 0., 0.).is_err() {
					error!("Failed to put image data on canvas for id: {placeholder_id}");
				}
			}
			Err(e) => {
				error!("Failed to create ImageData for id: {placeholder_id}: {e:?}");
			}
		}

		let js_value = JsValue::from(canvas);

		if Reflect::set(&canvases_obj, &js_key, &js_value).is_err() {
			error!("Failed to set canvas '{canvas_name}' on imageCanvases object");
		}
	}
}

pub(crate) fn calculate_hash<T: std::hash::Hash>(t: &T) -> u64 {
	use std::collections::hash_map::DefaultHasher;
	use std::hash::Hasher;
	let mut hasher = DefaultHasher::new();
	t.hash(&mut hasher);
	hasher.finish()
}

/// Translate a keyboard key from its JS name to its Rust `Key` enum
pub(crate) fn translate_key(name: &str) -> Key {
	use Key::*;

	trace!("Key event received: {name}");

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
