#[cfg(not(feature = "native"))]
use crate::EDITOR_HAS_CRASHED;
use crate::EDITOR_WRAPPER;
use crate::editor_wrapper::EditorWrapper;
#[cfg(feature = "editor")]
use editor::messages::input_mapper::utility_types::keyboard::Key;
#[cfg(all(not(feature = "native"), target_family = "wasm"))]
use editor::messages::prelude::*;
#[cfg(all(not(feature = "native"), target_family = "wasm"))]
use editor::task::{Spawn, Task};
#[cfg(all(not(feature = "native"), target_family = "wasm"))]
use graphene_std::runtime::Instant;
use js_sys::{Object, Reflect};
#[cfg(all(not(feature = "native"), target_family = "wasm"))]
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashSet;
#[cfg(not(feature = "native"))]
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
pub(crate) async fn run_node_runtime(update: bool) {
	// Process no further messages after a crash to avoid spamming the console
	if EDITOR_HAS_CRASHED.load(Ordering::SeqCst) {
		return;
	}

	let Some(mut runtime) = crate::NODE_RUNTIME.try_lock() else { return };
	let Some(runtime) = runtime.as_mut() else { return };
	if update {
		runtime.update();
	}
	runtime.run().await;
}

#[cfg(all(not(feature = "native"), target_family = "wasm"))]
pub(crate) struct WasmSpawner;
#[cfg(all(not(feature = "native"), target_family = "wasm"))]
impl Spawn for WasmSpawner {
	fn spawn(&self, task: Task) {
		wasm_bindgen_futures::spawn_local(task);
	}
}

/// Web editor wake: queues a microtask that dispatches [`Message::Wake`].
#[cfg(all(not(feature = "native"), target_family = "wasm"))]
pub(crate) fn wake_editor() {
	wasm_bindgen_futures::spawn_local(async {
		wrapper(|wrapper| wrapper.dispatch(Message::Wake));
	});
}

#[cfg(all(not(feature = "native"), target_family = "wasm"))]
thread_local! {
	static IDLE_RUN_QUEUED: Cell<bool> = const { Cell::new(false) };
	static IDLE_PERIOD: RefCell<Option<(web_sys::IdleDeadline, Instant)>> = const { RefCell::new(None) };
	static IDLE_RUN: Closure<dyn FnMut(web_sys::IdleDeadline)> = Closure::new(|deadline| {
		IDLE_RUN_QUEUED.set(false);
		IDLE_PERIOD.set(Some((deadline, Instant::now())));
		wasm_bindgen_futures::spawn_local(run_node_runtime(false));
	});
}

/// Web runtime wake: runs the node runtime in the time left before the next frame, so it can't hold up the browser's painting
/// Within an idle period it runs again at once if the time the last run took still fits.
#[cfg(all(not(feature = "native"), target_family = "wasm"))]
pub(crate) fn wake_runtime() {
	let fits = IDLE_PERIOD.with_borrow_mut(|period| {
		period.as_mut().is_some_and(|(deadline, run_start)| {
			let now = Instant::now();
			let fits = deadline.time_remaining() > (now - *run_start).as_secs_f64() * 1000.;
			if fits {
				*run_start = now;
			}
			fits
		})
	});
	if fits {
		wasm_bindgen_futures::spawn_local(run_node_runtime(false));
		return;
	}

	let Some(window) = window() else { return };
	if Reflect::has(&window, &"requestIdleCallback".into()).unwrap_or(false) && !IDLE_RUN_QUEUED.replace(true) {
		IDLE_RUN.with(|run| {
			let _ = window.request_idle_callback(run.as_ref().unchecked_ref());
		});
	}
}

thread_local! {
	/// The canvases put on `window.imageCanvases` for images, the only ones dropped from it here, since other canvases share it.
	static IMAGE_CANVAS_IDS: RefCell<HashSet<u64>> = RefCell::new(HashSet::new());
}

/// A canvas image's ID, width, height, and RGBA8 pixels.
pub(crate) type CanvasImagePixels<'a> = (u64, u32, u32, &'a [u8]);

/// Blits each new image to a canvas registered on `window.imageCanvases`, and drops the image canvases no render uses anymore.
pub(crate) fn render_image_data_to_canvases<'a>(images: impl IntoIterator<Item = CanvasImagePixels<'a>>, canvas_ids_in_use: &[u64]) {
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

	IMAGE_CANVAS_IDS.with_borrow_mut(|registered| {
		let in_use: HashSet<u64> = canvas_ids_in_use.iter().copied().collect();
		registered.retain(|id| {
			let used = in_use.contains(id);
			if !used {
				let _ = Reflect::delete_property(&canvases_obj, &JsValue::from_str(&format!("image-{id}")));
			}
			used
		});
	});

	for (placeholder_id, width, height, pixels) in images {
		// Matches the renderer's `data-canvas-placeholder` value
		let canvas_name = format!("image-{placeholder_id}");
		let js_key = JsValue::from_str(&canvas_name);

		if Reflect::has(&canvases_obj, &js_key).unwrap_or(false) || width == 0 || height == 0 {
			continue;
		}

		let canvas: HtmlCanvasElement = document
			.create_element("canvas")
			.expect("Failed to create canvas element")
			.dyn_into::<HtmlCanvasElement>()
			.expect("Failed to cast element to HtmlCanvasElement");

		canvas.set_width(width);
		canvas.set_height(height);

		let context: CanvasRenderingContext2d = canvas
			.get_context("2d")
			.expect("Failed to get 2d context")
			.expect("2d context was not found")
			.dyn_into::<CanvasRenderingContext2d>()
			.expect("Failed to cast context to CanvasRenderingContext2d");
		let clamped_pixels = wasm_bindgen::Clamped(pixels);
		match ImageData::new_with_u8_clamped_array_and_sh(clamped_pixels, width, height) {
			Ok(image_data_obj) => {
				if context.put_image_data(&image_data_obj, 0, 0).is_err() {
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
			continue;
		}
		IMAGE_CANVAS_IDS.with_borrow_mut(|registered| registered.insert(placeholder_id));
	}
}

/// Translate a keyboard key from its JS name to its Rust `Key` enum
#[cfg(feature = "editor")]
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
