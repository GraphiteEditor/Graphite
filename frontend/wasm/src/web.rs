use editor::application::Editor;
use editor::messages::prelude::*;
use graphene_std::Color;
use graphene_std::raster::Image;
use js_sys::{Object, Reflect};
use serde::ser::Serialize;
use std::cell::RefCell;
use std::panic;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, ImageData, window};

use crate::Error;

// Set up the persistent editor backend state
pub static EDITOR_HAS_CRASHED: AtomicBool = AtomicBool::new(false);
pub static NODE_GRAPH_ERROR_DISPLAYED: AtomicBool = AtomicBool::new(false);
pub static LOGGER: WasmLog = WasmLog;
thread_local! {
	pub static EDITOR: Mutex<Option<editor::application::Editor>> = const { Mutex::new(None) };
	pub static EDITOR_HANDLE: Mutex<Option<EditorHandle>> = const { Mutex::new(None) };
}
static IMAGE_DATA_HASH: AtomicU64 = AtomicU64::new(0);

/// Provides access to the `Editor` by calling the given closure with it as an argument.
fn editor<T: Default>(callback: impl FnOnce(&mut editor::application::Editor) -> T) -> T {
	EDITOR.with(|editor| {
		let mut guard = editor.try_lock();
		let Ok(Some(editor)) = guard.as_deref_mut() else { return T::default() };

		callback(editor)
	})
}

/// Provides access to the `Editor` and its `EditorHandle` by calling the given closure with them as arguments.
pub(crate) fn editor_and_handle(mut callback: impl FnMut(&mut Editor, &mut EditorHandle)) {
	EDITOR_HANDLE.with(|editor_handle| {
		editor(|editor| {
			let mut guard = editor_handle.try_lock();
			let Ok(Some(editor_handle)) = guard.as_deref_mut() else {
				log::error!("Failed to borrow editor handle");
				return;
			};

			// Call the closure with the editor and its handle
			callback(editor, editor_handle);
		})
	});
}

fn calculate_hash<T: std::hash::Hash>(t: &T) -> u64 {
	use std::collections::hash_map::DefaultHasher;
	use std::hash::Hasher;
	let mut hasher = DefaultHasher::new();
	t.hash(&mut hasher);
	hasher.finish()
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

/// This struct is, via wasm-bindgen, used by JS to interact with the editor backend.
#[wasm_bindgen]
#[derive(Clone)]
pub struct EditorHandle {
	/// This callback is called by the editor's dispatcher when directing FrontendMessages from Rust to JS
	frontend_message_handler_callback: js_sys::Function,
}

// Defined separately from the `impl` block below since this `impl` block lacks the `#[wasm_bindgen]` attribute.
// Quirks in wasm-bindgen prevent functions in `#[wasm_bindgen]` `impl` blocks from being made publicly accessible from Rust.
impl EditorHandle {
	pub fn send_frontend_message_to_js_rust_proxy(&self, message: FrontendMessage) {
		self.send_frontend_message_to_js(message);
	}

	pub fn dispatch<T: Into<Message>>(&self, message: T) {
		// Process no further messages after a crash to avoid spamming the console
		if EDITOR_HAS_CRASHED.load(Ordering::SeqCst) {
			return;
		}

		// Get the editor, dispatch the message, and store the `FrontendMessage` queue response
		let frontend_messages = editor(|editor| editor.handle_message(message.into()));

		// Send each `FrontendMessage` to the JavaScript frontend
		for message in frontend_messages.into_iter() {
			self.send_frontend_message_to_js(message);
		}
	}

	// Sends a FrontendMessage to JavaScript, which is only possible on web
	pub fn send_frontend_message_to_js(&self, mut message: FrontendMessage) {
		if let FrontendMessage::UpdateImageData { ref image_data } = message {
			let new_hash = calculate_hash(image_data);
			let prev_hash = IMAGE_DATA_HASH.load(Ordering::Relaxed);

			if new_hash != prev_hash {
				render_image_data_to_canvases(image_data.as_slice());
				IMAGE_DATA_HASH.store(new_hash, Ordering::Relaxed);
			}
			return;
		}

		if let FrontendMessage::UpdateDocumentLayerStructure { data_buffer } = message {
			message = FrontendMessage::UpdateDocumentLayerStructureJs { data_buffer: data_buffer.into() };
		}

		let message_type = message.to_discriminant().local_name();

		let serializer = serde_wasm_bindgen::Serializer::new().serialize_large_number_types_as_bigints(true);
		let message_data = message.serialize(&serializer).expect("Failed to serialize FrontendMessage");

		let js_return_value = self.frontend_message_handler_callback.call2(&JsValue::null(), &JsValue::from(message_type), &message_data);

		if let Err(error) = js_return_value {
			error!(
				"While handling FrontendMessage \"{:?}\", JavaScript threw an error: {:?}",
				message.to_discriminant().local_name(),
				error,
			)
		}
	}
}

#[wasm_bindgen]
impl EditorHandle {
	#[wasm_bindgen(constructor)]
	pub fn new(frontend_message_handler_callback: js_sys::Function) -> Self {
		let editor = Editor::new();
		let editor_handle = EditorHandle { frontend_message_handler_callback };
		// If on native, all messages passed into wasm from the browser just get forwarded to the main native thread, so theres no need to create an editor
		if EDITOR.with(|handle| handle.lock().ok().map(|mut guard| *guard = Some(editor))).is_none() {
			log::error!("Attempted to initialize the editor more than once");
		}
		if EDITOR_HANDLE.with(|handle| handle.lock().ok().map(|mut guard| *guard = Some(editor_handle.clone()))).is_none() {
			log::error!("Attempted to initialize the editor handle more than once");
		}
		editor_handle
	}

	/// Answer whether or not the editor has crashed
	#[wasm_bindgen(js_name = hasCrashed)]
	pub fn has_crashed(&self) -> bool {
		EDITOR_HAS_CRASHED.load(Ordering::SeqCst)
	}
}

pub fn run_and_poll_node_graph_evaluation_loop() {
	let f = std::rc::Rc::new(RefCell::new(None));
	let g = f.clone();

	*g.borrow_mut() = Some(Closure::new(move |_timestamp| {
		// On native we run the node graph in another thread and block until messages are received, but on web we need to run it in the main thread and poll it
		wasm_bindgen_futures::spawn_local(run_and_poll_node_graph_evaluation());
		// Schedule ourself for another requestAnimationFrame callback
		request_animation_frame(f.borrow().as_ref().unwrap());
	}));

	request_animation_frame(g.borrow().as_ref().unwrap());
}

/// Helper function for calling JS's `requestAnimationFrame` with the given closure
fn request_animation_frame(f: &Closure<dyn FnMut(f64)>) {
	web_sys::window()
		.expect("No global `window` exists")
		.request_animation_frame(f.as_ref().unchecked_ref())
		.expect("Failed to call `requestAnimationFrame`");
}

// Used to run the node graph from WASM, which cannot be in another thread and block until requests are made
// Returns a boolean since this is run from request animation frame, and we do not want to poll the runtime after requests which get cancelled
async fn run_and_poll_node_graph_evaluation() {
	// Process no further messages after a crash to avoid spamming the console
	if EDITOR_HAS_CRASHED.load(Ordering::SeqCst) {
		return;
	}

	async fn try_run_runtime() -> bool {
		let Some(mut runtime) = editor::node_graph_executor::NODE_RUNTIME.try_lock() else { return true };
		if let Some(ref mut runtime) = runtime.as_mut() {
			runtime.run().await;
		}
		false
	}

	let runtime_busy = try_run_runtime().await;

	if runtime_busy {
		return;
	};

	editor_and_handle(|editor, handle| {
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

		// Send each `FrontendMessage` to the JavaScript frontend
		for response in messages.into_iter().flat_map(|message| editor.handle_message(message)) {
			handle.send_frontend_message_to_js(response);
		}

		// If the editor cannot be borrowed then it has encountered a panic - we should just ignore new dispatches
	});
}

fn render_image_data_to_canvases(image_data: &[(u64, Image<Color>)]) {
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
			editor_and_handle(|_, handle| {
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
