#![allow(clippy::too_many_arguments)]
//
// This file is where functions are defined to be called directly from JS.
// It serves as a thin wrapper over the editor backend API that relies
// on the dispatcher messaging system and more complex Rust data types.
//
use crate::helpers::translate_key;
use crate::{EDITOR_HANDLE, EDITOR_HAS_CRASHED, Error, MESSAGE_BUFFER};
use editor::consts::FILE_EXTENSION;
use editor::messages::input_mapper::utility_types::input_keyboard::ModifierKeys;
use editor::messages::input_mapper::utility_types::input_mouse::{EditorMouseState, ScrollDelta, ViewportBounds};
use editor::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use editor::messages::portfolio::document::utility_types::network_interface::ImportOrExport;
use editor::messages::portfolio::utility_types::Platform;
use editor::messages::prelude::*;
use editor::messages::tool::tool_messages::tool_prelude::WidgetId;
use graph_craft::document::NodeId;
use graphene_std::raster::Image;
use graphene_std::raster::color::Color;
use js_sys::{Object, Reflect};
use serde::Serialize;
use serde_wasm_bindgen::{self, from_value};
use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, ImageData, window};

#[cfg(not(feature = "native"))]
use crate::EDITOR;
#[cfg(not(feature = "native"))]
use editor::application::Editor;

static IMAGE_DATA_HASH: AtomicU64 = AtomicU64::new(0);

fn calculate_hash<T: std::hash::Hash>(t: &T) -> u64 {
	use std::collections::hash_map::DefaultHasher;
	use std::hash::Hasher;
	let mut hasher = DefaultHasher::new();
	t.hash(&mut hasher);
	hasher.finish()
}

/// Set the random seed used by the editor by calling this from JS upon initialization.
/// This is necessary because WASM doesn't have a random number generator.
#[wasm_bindgen(js_name = setRandomSeed)]
pub fn set_random_seed(seed: u64) {
	editor::application::set_uuid_seed(seed);
}

/// Provides a handle to access the raw WASM memory.
#[wasm_bindgen(js_name = wasmMemory)]
pub fn wasm_memory() -> JsValue {
	wasm_bindgen::memory()
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

// ============================================================================

/// This struct is, via wasm-bindgen, used by JS to interact with the editor backend. It does this by calling functions, which are `impl`ed
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
}

#[wasm_bindgen]
impl EditorHandle {
	#[cfg(not(feature = "native"))]
	#[wasm_bindgen(constructor)]
	pub fn new(frontend_message_handler_callback: js_sys::Function) -> Self {
		let editor = Editor::new();
		let editor_handle = EditorHandle { frontend_message_handler_callback };
		if EDITOR.with(|handle| handle.lock().ok().map(|mut guard| *guard = Some(editor))).is_none() {
			log::error!("Attempted to initialize the editor more than once");
		}
		if EDITOR_HANDLE.with(|handle| handle.lock().ok().map(|mut guard| *guard = Some(editor_handle.clone()))).is_none() {
			log::error!("Attempted to initialize the editor handle more than once");
		}
		editor_handle
	}

	#[cfg(feature = "native")]
	#[wasm_bindgen(constructor)]
	pub fn new(frontend_message_handler_callback: js_sys::Function) -> Self {
		let editor_handle = EditorHandle { frontend_message_handler_callback };
		if EDITOR_HANDLE.with(|handle| handle.lock().ok().map(|mut guard| *guard = Some(editor_handle.clone()))).is_none() {
			log::error!("Attempted to initialize the editor handle more than once");
		}
		editor_handle
	}

	// Sends a message to the dispatcher in the Editor Backend
	#[cfg(not(feature = "native"))]
	fn dispatch<T: Into<Message>>(&self, message: T) {
		// Process no further messages after a crash to avoid spamming the console

		use crate::MESSAGE_BUFFER;
		if EDITOR_HAS_CRASHED.load(Ordering::SeqCst) {
			return;
		}

		// Get the editor, dispatch the message, and store the `FrontendMessage` queue response
		let frontend_messages = EDITOR.with(|editor| {
			let mut guard = editor.try_lock();
			let Ok(Some(editor)) = guard.as_deref_mut() else {
				// Enqueue messages which can't be procssed currently
				MESSAGE_BUFFER.with_borrow_mut(|buffer| buffer.push(message.into()));
				return vec![];
			};

			editor.handle_message(message)
		});

		// Send each `FrontendMessage` to the JavaScript frontend
		for message in frontend_messages.into_iter() {
			self.send_frontend_message_to_js(message);
		}
	}

	#[cfg(feature = "native")]
	fn dispatch<T: Into<Message>>(&self, message: T) {
		let message: Message = message.into();
		let Ok(serialized_message) = ron::to_string(&message) else {
			log::error!("Failed to serialize message");
			return;
		};
		crate::native_communcation::send_message_to_cef(serialized_message)
	}

	// Sends a FrontendMessage to JavaScript
	fn send_frontend_message_to_js(&self, mut message: FrontendMessage) {
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
			error!("While handling FrontendMessage {:?}, JavaScript threw an error:\n{:?}", message.to_discriminant().local_name(), error,)
		}
	}

	// ========================================================================
	// Add additional JS -> Rust wrapper functions below as needed for calling
	// the backend from the web frontend.
	// ========================================================================

	#[wasm_bindgen(js_name = initAfterFrontendReady)]
	pub fn init_after_frontend_ready(&self, platform: String) {
		#[cfg(feature = "native")]
		crate::native_communcation::initialize_native_communication();

		// Send initialization messages
		let platform = match platform.as_str() {
			"Windows" => Platform::Windows,
			"Mac" => Platform::Mac,
			"Linux" => Platform::Linux,
			_ => Platform::Unknown,
		};
		self.dispatch(GlobalsMessage::SetPlatform { platform });
		self.dispatch(PortfolioMessage::Init);

		// Poll node graph evaluation on `requestAnimationFrame`
		{
			let f = std::rc::Rc::new(RefCell::new(None));
			let g = f.clone();

			*g.borrow_mut() = Some(Closure::new(move |_timestamp| {
				#[cfg(not(feature = "native"))]
				wasm_bindgen_futures::spawn_local(poll_node_graph_evaluation());

				if !EDITOR_HAS_CRASHED.load(Ordering::SeqCst) {
					handle(|handle| {
						// Process all messages that have been queued up
						let messages = MESSAGE_BUFFER.take();

						for message in messages {
							handle.dispatch(message);
						}

						handle.dispatch(InputPreprocessorMessage::CurrentTime {
							timestamp: js_sys::Date::now() as u64,
						});
						handle.dispatch(AnimationMessage::IncrementFrameCounter);

						// Used by auto-panning, but this could possibly be refactored in the future, see:
						// <https://github.com/GraphiteEditor/Graphite/pull/2562#discussion_r2041102786>
						handle.dispatch(BroadcastMessage::TriggerEvent(EventMessage::AnimationFrame));
					});
				}

				// Schedule ourself for another requestAnimationFrame callback
				request_animation_frame(f.borrow().as_ref().unwrap());
			}));

			request_animation_frame(g.borrow().as_ref().unwrap());
		}

		// Auto save all documents on `setTimeout`
		{
			let f = std::rc::Rc::new(RefCell::new(None));
			let g = f.clone();

			*g.borrow_mut() = Some(Closure::new(move || {
				auto_save_all_documents();

				// Schedule ourself for another setTimeout callback
				set_timeout(f.borrow().as_ref().unwrap(), Duration::from_secs(editor::consts::AUTO_SAVE_TIMEOUT_SECONDS));
			}));

			set_timeout(g.borrow().as_ref().unwrap(), Duration::from_secs(editor::consts::AUTO_SAVE_TIMEOUT_SECONDS));
		}
	}

	#[wasm_bindgen(js_name = addPrimaryImport)]
	pub fn add_primary_import(&self) {
		self.dispatch(DocumentMessage::AddTransaction);
		self.dispatch(NodeGraphMessage::AddPrimaryImport);
	}

	#[wasm_bindgen(js_name = addSecondaryImport)]
	pub fn add_secondary_import(&self) {
		self.dispatch(DocumentMessage::AddTransaction);
		self.dispatch(NodeGraphMessage::AddSecondaryImport);
	}

	#[wasm_bindgen(js_name = addPrimaryExport)]
	pub fn add_primary_export(&self) {
		self.dispatch(DocumentMessage::AddTransaction);
		self.dispatch(NodeGraphMessage::AddPrimaryExport);
	}

	#[wasm_bindgen(js_name = addSecondaryExport)]
	pub fn add_secondary_export(&self) {
		self.dispatch(DocumentMessage::AddTransaction);
		self.dispatch(NodeGraphMessage::AddSecondaryExport);
	}

	/// Minimizes the application window to the taskbar or dock
	#[wasm_bindgen(js_name = appWindowMinimize)]
	pub fn app_window_minimize(&self) {
		let message = AppWindowMessage::AppWindowMinimize;
		self.dispatch(message);
	}

	/// Toggles minimizing or restoring down the application window
	#[wasm_bindgen(js_name = appWindowMaximize)]
	pub fn app_window_maximize(&self) {
		let message = AppWindowMessage::AppWindowMaximize;
		self.dispatch(message);
	}

	/// Closes the application window
	#[wasm_bindgen(js_name = appWindowClose)]
	pub fn app_window_close(&self) {
		let message = AppWindowMessage::AppWindowClose;
		self.dispatch(message);
	}

	/// Drag the application window
	#[wasm_bindgen(js_name = appWindowDrag)]
	pub fn app_window_start_drag(&self) {
		let message = AppWindowMessage::AppWindowDrag;
		self.dispatch(message);
	}

	/// Displays a dialog with an error message
	#[wasm_bindgen(js_name = errorDialog)]
	pub fn error_dialog(&self, title: String, description: String) {
		let message = DialogMessage::DisplayDialogError { title, description };
		self.dispatch(message);
	}

	/// Answer whether or not the editor has crashed
	#[wasm_bindgen(js_name = hasCrashed)]
	pub fn has_crashed(&self) -> bool {
		EDITOR_HAS_CRASHED.load(Ordering::SeqCst)
	}

	/// Answer whether or not the editor is in development mode
	#[wasm_bindgen(js_name = inDevelopmentMode)]
	pub fn in_development_mode(&self) -> bool {
		cfg!(debug_assertions)
	}

	/// Get the constant `FILE_EXTENSION`
	#[wasm_bindgen(js_name = fileExtension)]
	pub fn file_extension(&self) -> String {
		FILE_EXTENSION.into()
	}

	/// Update the value of a given UI widget, but don't commit it to the history (unless `commit_layout()` is called, which handles that)
	#[wasm_bindgen(js_name = widgetValueUpdate)]
	pub fn widget_value_update(&self, layout_target: JsValue, widget_id: u64, value: JsValue) -> Result<(), JsValue> {
		let widget_id = WidgetId(widget_id);
		match (from_value(layout_target), from_value(value)) {
			(Ok(layout_target), Ok(value)) => {
				let message = LayoutMessage::WidgetValueUpdate { layout_target, widget_id, value };
				self.dispatch(message);
				Ok(())
			}
			(target, val) => Err(Error::new(&format!("Could not update UI\nDetails:\nTarget: {target:?}\nValue: {val:?}")).into()),
		}
	}

	/// Commit the value of a given UI widget to the history
	#[wasm_bindgen(js_name = widgetValueCommit)]
	pub fn widget_value_commit(&self, layout_target: JsValue, widget_id: u64, value: JsValue) -> Result<(), JsValue> {
		let widget_id = WidgetId(widget_id);
		match (from_value(layout_target), from_value(value)) {
			(Ok(layout_target), Ok(value)) => {
				let message = LayoutMessage::WidgetValueCommit { layout_target, widget_id, value };
				self.dispatch(message);
				Ok(())
			}
			(target, val) => Err(Error::new(&format!("Could not commit UI\nDetails:\nTarget: {target:?}\nValue: {val:?}")).into()),
		}
	}

	/// Update the value of a given UI widget, and commit it to the history
	#[wasm_bindgen(js_name = widgetValueCommitAndUpdate)]
	pub fn widget_value_commit_and_update(&self, layout_target: JsValue, widget_id: u64, value: JsValue) -> Result<(), JsValue> {
		self.widget_value_commit(layout_target.clone(), widget_id, value.clone())?;
		self.widget_value_update(layout_target, widget_id, value)?;
		Ok(())
	}

	#[wasm_bindgen(js_name = loadPreferences)]
	pub fn load_preferences(&self, preferences: Option<String>) {
		let preferences = if let Some(preferences) = preferences {
			let Ok(preferences) = serde_json::from_str(&preferences) else {
				log::error!("Failed to deserialize preferences");
				return;
			};
			Some(preferences)
		} else {
			None
		};

		let message = PreferencesMessage::Load { preferences };
		self.dispatch(message);
	}

	#[wasm_bindgen(js_name = selectDocument)]
	pub fn select_document(&self, document_id: u64) {
		let document_id = DocumentId(document_id);
		let message = PortfolioMessage::SelectDocument { document_id };
		self.dispatch(message);
	}

	#[wasm_bindgen(js_name = newDocumentDialog)]
	pub fn new_document_dialog(&self) {
		let message = DialogMessage::RequestNewDocumentDialog;
		self.dispatch(message);
	}

	#[wasm_bindgen(js_name = openDocument)]
	pub fn open_document(&self) {
		let message = PortfolioMessage::OpenDocument;
		self.dispatch(message);
	}

	#[wasm_bindgen(js_name = demoArtworkDialog)]
	pub fn demo_artwork_dialog(&self) {
		let message = DialogMessage::RequestDemoArtworkDialog;
		self.dispatch(message);
	}

	#[wasm_bindgen(js_name = openDocumentFile)]
	pub fn open_document_file(&self, document_name: String, document_serialized_content: String) {
		let message = PortfolioMessage::OpenDocumentFile {
			document_name: Some(document_name),
			document_path: None,
			document_serialized_content,
		};
		self.dispatch(message);
	}

	#[wasm_bindgen(js_name = openAutoSavedDocument)]
	pub fn open_auto_saved_document(&self, document_id: u64, document_name: String, document_is_saved: bool, document_serialized_content: String, to_front: bool) {
		let document_id = DocumentId(document_id);
		let message = PortfolioMessage::OpenDocumentFileWithId {
			document_id,
			document_name: Some(document_name),
			document_path: None,
			document_is_auto_saved: true,
			document_is_saved,
			document_serialized_content,
			to_front,
			select_after_open: false,
		};
		self.dispatch(message);
	}

	#[wasm_bindgen(js_name = triggerAutoSave)]
	pub fn trigger_auto_save(&self, document_id: u64) {
		let document_id = DocumentId(document_id);
		let message = PortfolioMessage::AutoSaveDocument { document_id };
		self.dispatch(message);
	}

	#[wasm_bindgen(js_name = closeDocumentWithConfirmation)]
	pub fn close_document_with_confirmation(&self, document_id: u64) {
		let document_id = DocumentId(document_id);
		let message = PortfolioMessage::CloseDocumentWithConfirmation { document_id };
		self.dispatch(message);
	}

	#[wasm_bindgen(js_name = requestAboutGraphiteDialogWithLocalizedCommitDate)]
	pub fn request_about_graphite_dialog_with_localized_commit_date(&self, localized_commit_date: String, localized_commit_year: String) {
		let message = DialogMessage::RequestAboutGraphiteDialogWithLocalizedCommitDate {
			localized_commit_date,
			localized_commit_year,
		};
		self.dispatch(message);
	}

	#[wasm_bindgen(js_name = requestLicensesThirdPartyDialogWithLicenseText)]
	pub fn request_licenses_third_party_dialog_with_license_text(&self, license_text: String) {
		let message = DialogMessage::RequestLicensesThirdPartyDialogWithLicenseText { license_text };
		self.dispatch(message);
	}

	/// Send new bounds when document panel viewports get resized or moved within the editor
	/// [left, top, right, bottom]...
	#[wasm_bindgen(js_name = boundsOfViewports)]
	pub fn bounds_of_viewports(&self, bounds_of_viewports: &[f64]) {
		let chunked: Vec<_> = bounds_of_viewports.chunks(4).map(ViewportBounds::from_slice).collect();

		let message = InputPreprocessorMessage::BoundsOfViewports { bounds_of_viewports: chunked };
		self.dispatch(message);
	}

	/// Zoom the canvas to fit all content
	#[wasm_bindgen(js_name = zoomCanvasToFitAll)]
	pub fn zoom_canvas_to_fit_all(&self) {
		let message = DocumentMessage::ZoomCanvasToFitAll;
		self.dispatch(message);
	}

	/// Inform the overlays system of the current device pixel ratio
	#[wasm_bindgen(js_name = setDevicePixelRatio)]
	pub fn set_device_pixel_ratio(&self, ratio: f64) {
		let message = PortfolioMessage::SetDevicePixelRatio { ratio };
		self.dispatch(message);
	}

	/// Mouse movement within the screenspace bounds of the viewport
	#[wasm_bindgen(js_name = onMouseMove)]
	pub fn on_mouse_move(&self, x: f64, y: f64, mouse_keys: u8, modifiers: u8) {
		let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());

		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		let message = InputPreprocessorMessage::PointerMove { editor_mouse_state, modifier_keys };
		self.dispatch(message);
	}

	/// Mouse scrolling within the screenspace bounds of the viewport
	#[wasm_bindgen(js_name = onWheelScroll)]
	pub fn on_wheel_scroll(&self, x: f64, y: f64, mouse_keys: u8, wheel_delta_x: f64, wheel_delta_y: f64, wheel_delta_z: f64, modifiers: u8) {
		let mut editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());
		editor_mouse_state.scroll_delta = ScrollDelta::new(wheel_delta_x, wheel_delta_y, wheel_delta_z);

		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		let message = InputPreprocessorMessage::WheelScroll { editor_mouse_state, modifier_keys };
		self.dispatch(message);
	}

	/// A mouse button depressed within screenspace the bounds of the viewport
	#[wasm_bindgen(js_name = onMouseDown)]
	pub fn on_mouse_down(&self, x: f64, y: f64, mouse_keys: u8, modifiers: u8) {
		let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());

		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		let message = InputPreprocessorMessage::PointerDown { editor_mouse_state, modifier_keys };
		self.dispatch(message);
	}

	/// A mouse button released
	#[wasm_bindgen(js_name = onMouseUp)]
	pub fn on_mouse_up(&self, x: f64, y: f64, mouse_keys: u8, modifiers: u8) {
		let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());

		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		let message = InputPreprocessorMessage::PointerUp { editor_mouse_state, modifier_keys };
		self.dispatch(message);
	}

	/// Mouse shaken
	#[wasm_bindgen(js_name = onMouseShake)]
	pub fn on_mouse_shake(&self, x: f64, y: f64, mouse_keys: u8, modifiers: u8) {
		let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());

		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		let message = InputPreprocessorMessage::PointerShake { editor_mouse_state, modifier_keys };
		self.dispatch(message);
	}

	/// Mouse double clicked
	#[wasm_bindgen(js_name = onDoubleClick)]
	pub fn on_double_click(&self, x: f64, y: f64, mouse_keys: u8, modifiers: u8) {
		let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());

		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		let message = InputPreprocessorMessage::DoubleClick { editor_mouse_state, modifier_keys };
		self.dispatch(message);
	}

	/// A keyboard button depressed within screenspace the bounds of the viewport
	#[wasm_bindgen(js_name = onKeyDown)]
	pub fn on_key_down(&self, name: String, modifiers: u8, key_repeat: bool) {
		let key = translate_key(&name);
		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		trace!("Key down {key:?}, name: {name}, modifiers: {modifiers:?}, key repeat: {key_repeat}");

		let message = InputPreprocessorMessage::KeyDown { key, key_repeat, modifier_keys };
		self.dispatch(message);
	}

	/// A keyboard button released
	#[wasm_bindgen(js_name = onKeyUp)]
	pub fn on_key_up(&self, name: String, modifiers: u8, key_repeat: bool) {
		let key = translate_key(&name);
		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		trace!("Key up {key:?}, name: {name}, modifiers: {modifier_keys:?}, key repeat: {key_repeat}");

		let message = InputPreprocessorMessage::KeyUp { key, key_repeat, modifier_keys };
		self.dispatch(message);
	}

	/// A text box was committed
	#[wasm_bindgen(js_name = onChangeText)]
	pub fn on_change_text(&self, new_text: String, is_left_or_right_click: bool) -> Result<(), JsValue> {
		let message = TextToolMessage::TextChange { new_text, is_left_or_right_click };
		self.dispatch(message);

		Ok(())
	}

	/// A font has been downloaded
	#[wasm_bindgen(js_name = onFontLoad)]
	pub fn on_font_load(&self, font_family: String, font_style: String, preview_url: String, data: Vec<u8>) -> Result<(), JsValue> {
		let message = PortfolioMessage::FontLoaded {
			font_family,
			font_style,
			preview_url,
			data,
		};
		self.dispatch(message);

		Ok(())
	}

	/// A text box was changed
	#[wasm_bindgen(js_name = updateBounds)]
	pub fn update_bounds(&self, new_text: String) -> Result<(), JsValue> {
		let message = TextToolMessage::UpdateBounds { new_text };
		self.dispatch(message);

		Ok(())
	}

	/// Begin sampling a pixel color from the document by entering eyedropper sampling mode
	#[wasm_bindgen(js_name = eyedropperSampleForColorPicker)]
	pub fn eyedropper_sample_for_color_picker(&self) -> Result<(), JsValue> {
		let message = DialogMessage::RequestComingSoonDialog { issue: Some(832) };
		self.dispatch(message);

		Ok(())
	}

	/// Update primary color with values on a scale from 0 to 1.
	#[wasm_bindgen(js_name = updatePrimaryColor)]
	pub fn update_primary_color(&self, red: f32, green: f32, blue: f32, alpha: f32) -> Result<(), JsValue> {
		let Some(primary_color) = Color::from_rgbaf32(red, green, blue, alpha) else {
			return Err(Error::new("Invalid color").into());
		};

		let message = ToolMessage::SelectWorkingColor {
			color: primary_color.to_linear_srgb(),
			primary: true,
		};
		self.dispatch(message);

		Ok(())
	}

	/// Update secondary color with values on a scale from 0 to 1.
	#[wasm_bindgen(js_name = updateSecondaryColor)]
	pub fn update_secondary_color(&self, red: f32, green: f32, blue: f32, alpha: f32) -> Result<(), JsValue> {
		let Some(secondary_color) = Color::from_rgbaf32(red, green, blue, alpha) else {
			return Err(Error::new("Invalid color").into());
		};

		let message = ToolMessage::SelectWorkingColor {
			color: secondary_color.to_linear_srgb(),
			primary: false,
		};
		self.dispatch(message);

		Ok(())
	}

	/// Visit the given URL
	#[wasm_bindgen(js_name = visitUrl)]
	pub fn visit_url(&self, url: String) {
		let message = FrontendMessage::TriggerVisitLink { url };
		self.dispatch(message);
	}

	/// Paste layers from a serialized JSON representation
	#[wasm_bindgen(js_name = pasteSerializedData)]
	pub fn paste_serialized_data(&self, data: String) {
		let message = PortfolioMessage::PasteSerializedData { data };
		self.dispatch(message);
	}

	/// Paste vector into a new layer from a serialized JSON representation
	#[wasm_bindgen(js_name = pasteSerializedVector)]
	pub fn paste_serialized_vector(&self, data: String) {
		let message = PortfolioMessage::PasteSerializedVector { data };
		self.dispatch(message);
	}

	#[wasm_bindgen(js_name = clipLayer)]
	pub fn clip_layer(&self, id: u64) {
		let id = NodeId(id);
		let message = DocumentMessage::ClipLayer { id };
		self.dispatch(message);
	}

	/// Modify the layer selection based on the layer which is clicked while holding down the <kbd>Ctrl</kbd> and/or <kbd>Shift</kbd> modifier keys used for range selection behavior
	#[wasm_bindgen(js_name = selectLayer)]
	pub fn select_layer(&self, id: u64, ctrl: bool, shift: bool) {
		let id = NodeId(id);
		let message = DocumentMessage::SelectLayer { id, ctrl, shift };
		self.dispatch(message);
	}

	/// Deselect all layers
	#[wasm_bindgen(js_name = deselectAllLayers)]
	pub fn deselect_all_layers(&self) {
		let message = DocumentMessage::DeselectAllLayers;
		self.dispatch(message);
	}

	/// Move a layer to within a folder and placed down at the given index.
	/// If the folder is `None`, it is inserted into the document root.
	/// If the insert index is `None`, it is inserted at the start of the folder.
	#[wasm_bindgen(js_name = moveLayerInTree)]
	pub fn move_layer_in_tree(&self, insert_parent_id: Option<u64>, insert_index: Option<usize>) {
		let insert_parent_id = insert_parent_id.map(NodeId);
		let parent = insert_parent_id.map(LayerNodeIdentifier::new_unchecked).unwrap_or_default();

		let message = DocumentMessage::MoveSelectedLayersTo {
			parent,
			insert_index: insert_index.unwrap_or_default(),
		};
		self.dispatch(message);
	}

	/// Set the name for the layer
	#[wasm_bindgen(js_name = setLayerName)]
	pub fn set_layer_name(&self, id: u64, name: String) {
		let layer = LayerNodeIdentifier::new_unchecked(NodeId(id));
		let message = NodeGraphMessage::SetDisplayName {
			node_id: layer.to_node(),
			alias: name,
			skip_adding_history_step: false,
		};
		self.dispatch(message);
	}

	/// Translates document (in viewport coords)
	#[wasm_bindgen(js_name = panCanvasAbortPrepare)]
	pub fn pan_canvas_abort_prepare(&self, x_not_y_axis: bool) {
		let message = NavigationMessage::CanvasPanAbortPrepare { x_not_y_axis };
		self.dispatch(message);
	}

	#[wasm_bindgen(js_name = panCanvasAbort)]
	pub fn pan_canvas_abort(&self, x_not_y_axis: bool) {
		let message = NavigationMessage::CanvasPanAbort { x_not_y_axis };
		self.dispatch(message);
	}

	/// Translates document (in viewport coords)
	#[wasm_bindgen(js_name = panCanvas)]
	pub fn pan_canvas(&self, delta_x: f64, delta_y: f64) {
		let message = NavigationMessage::CanvasPan { delta: (delta_x, delta_y).into() };
		self.dispatch(message);
	}

	/// Translates document (in viewport coords)
	#[wasm_bindgen(js_name = panCanvasByFraction)]
	pub fn pan_canvas_by_fraction(&self, delta_x: f64, delta_y: f64) {
		let message = NavigationMessage::CanvasPanByViewportFraction { delta: (delta_x, delta_y).into() };
		self.dispatch(message);
	}

	/// Snaps the import/export edges to a grid space when the scroll bar is released
	#[wasm_bindgen(js_name = setGridAlignedEdges)]
	pub fn set_grid_aligned_edges(&self) {
		let message = NodeGraphMessage::SetGridAlignedEdges;
		self.dispatch(message);
	}

	/// Merge the selected nodes into a subnetwork
	#[wasm_bindgen(js_name = mergeSelectedNodes)]
	pub fn merge_nodes(&self) {
		let message = NodeGraphMessage::MergeSelectedNodes;
		self.dispatch(message);
	}

	/// Creates a new document node in the node graph
	#[wasm_bindgen(js_name = createNode)]
	pub fn create_node(&self, node_type: String, x: i32, y: i32) {
		let id = NodeId::new();
		let message = NodeGraphMessage::CreateNodeFromContextMenu {
			node_id: Some(id),
			node_type,
			xy: Some((x / 24, y / 24)),
			add_transaction: true,
		};
		self.dispatch(message);
	}

	/// Pastes the nodes based on serialized data
	#[wasm_bindgen(js_name = pasteSerializedNodes)]
	pub fn paste_serialized_nodes(&self, serialized_nodes: String) {
		let message = NodeGraphMessage::PasteNodes { serialized_nodes };
		self.dispatch(message);
	}

	/// Pastes an image
	#[wasm_bindgen(js_name = pasteImage)]
	pub fn paste_image(
		&self,
		name: Option<String>,
		image_data: Vec<u8>,
		width: u32,
		height: u32,
		mouse_x: Option<f64>,
		mouse_y: Option<f64>,
		insert_parent_id: Option<u64>,
		insert_index: Option<usize>,
	) {
		let mouse = mouse_x.and_then(|x| mouse_y.map(|y| (x, y)));
		let image = graphene_std::raster::Image::from_image_data(&image_data, width, height);

		let parent_and_insert_index = if let (Some(insert_parent_id), Some(insert_index)) = (insert_parent_id, insert_index) {
			let insert_parent_id = NodeId(insert_parent_id);
			let parent = LayerNodeIdentifier::new_unchecked(insert_parent_id);
			Some((parent, insert_index))
		} else {
			None
		};

		let message = PortfolioMessage::PasteImage {
			name,
			image,
			mouse,
			parent_and_insert_index,
		};
		self.dispatch(message);
	}

	#[wasm_bindgen(js_name = pasteSvg)]
	pub fn paste_svg(&self, name: Option<String>, svg: String, mouse_x: Option<f64>, mouse_y: Option<f64>, insert_parent_id: Option<u64>, insert_index: Option<usize>) {
		let mouse = mouse_x.and_then(|x| mouse_y.map(|y| (x, y)));

		let parent_and_insert_index = if let (Some(insert_parent_id), Some(insert_index)) = (insert_parent_id, insert_index) {
			let insert_parent_id = NodeId(insert_parent_id);
			let parent = LayerNodeIdentifier::new_unchecked(insert_parent_id);
			Some((parent, insert_index))
		} else {
			None
		};

		let message = PortfolioMessage::PasteSvg {
			name,
			svg,
			mouse,
			parent_and_insert_index,
		};
		self.dispatch(message);
	}

	/// Toggle visibility of a layer or node given its node ID
	#[wasm_bindgen(js_name = toggleNodeVisibilityLayerPanel)]
	pub fn toggle_node_visibility_layer(&self, id: u64) {
		let node_id = NodeId(id);
		let message = NodeGraphMessage::ToggleVisibility { node_id };
		self.dispatch(message);
	}

	/// Pin or unpin a node given its node ID
	#[wasm_bindgen(js_name = setNodePinned)]
	pub fn set_node_pinned(&self, id: u64, pinned: bool) {
		self.dispatch(DocumentMessage::SetNodePinned { node_id: NodeId(id), pinned });
	}

	/// Delete a layer or node given its node ID
	#[wasm_bindgen(js_name = deleteNode)]
	pub fn delete_node(&self, id: u64) {
		self.dispatch(DocumentMessage::DeleteNode { node_id: NodeId(id) });
	}

	/// Toggle lock state of a layer from the layer list
	#[wasm_bindgen(js_name = toggleLayerLock)]
	pub fn toggle_layer_lock(&self, node_id: u64) {
		let message = NodeGraphMessage::ToggleLocked { node_id: NodeId(node_id) };
		self.dispatch(message);
	}

	/// Toggle expansions state of a layer from the layer list
	#[wasm_bindgen(js_name = toggleLayerExpansion)]
	pub fn toggle_layer_expansion(&self, id: u64, recursive: bool) {
		let id = NodeId(id);
		let message = DocumentMessage::ToggleLayerExpansion { id, recursive };
		self.dispatch(message);
	}

	/// Set the active panel to the most recently clicked panel
	#[wasm_bindgen(js_name = setActivePanel)]
	pub fn set_active_panel(&self, panel: String) {
		let message = PortfolioMessage::SetActivePanel { panel: panel.into() };
		self.dispatch(message);
	}

	/// Toggle display type for a layer
	#[wasm_bindgen(js_name = setToNodeOrLayer)]
	pub fn set_to_node_or_layer(&self, id: u64, is_layer: bool) {
		self.dispatch(DocumentMessage::SetToNodeOrLayer { node_id: NodeId(id), is_layer });
	}

	/// Set the name of an import or export
	#[wasm_bindgen(js_name = setImportName)]
	pub fn set_import_name(&self, index: usize, name: String) {
		let message = NodeGraphMessage::SetImportExportName {
			name,
			index: ImportOrExport::Import(index),
		};
		self.dispatch(message);
	}

	/// Set the name of an export
	#[wasm_bindgen(js_name = setExportName)]
	pub fn set_export_name(&self, index: usize, name: String) {
		let message = NodeGraphMessage::SetImportExportName {
			name,
			index: ImportOrExport::Export(index),
		};
		self.dispatch(message);
	}
}

// ============================================================================

#[wasm_bindgen(js_name = evaluateMathExpression)]
pub fn evaluate_math_expression(expression: &str) -> Option<f64> {
	let value = math_parser::evaluate(expression)
		.inspect_err(|err| error!("Math parser error on \"{expression}\": {err}"))
		.ok()?
		.0
		.inspect_err(|err| error!("Math evaluate error on \"{expression}\": {err} "))
		.ok()?;
	let Some(real) = value.as_real() else {
		error!("{value} was not a real; skipping.");
		return None;
	};
	Some(real)
}

/// Helper function for calling JS's `requestAnimationFrame` with the given closure
fn request_animation_frame(f: &Closure<dyn FnMut(f64)>) {
	web_sys::window()
		.expect("No global `window` exists")
		.request_animation_frame(f.as_ref().unchecked_ref())
		.expect("Failed to call `requestAnimationFrame`");
}

/// Helper function for calling JS's `setTimeout` with the given closure and delay
fn set_timeout(f: &Closure<dyn FnMut()>, delay: Duration) {
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

/// Provides access to the `Editor` and its `EditorHandle` by calling the given closure with them as arguments.
#[cfg(not(feature = "native"))]
pub(crate) fn editor_and_handle(callback: impl FnOnce(&mut Editor, &mut EditorHandle)) {
	handle(|editor_handle| {
		editor(|editor| {
			// Call the closure with the editor and its handle
			callback(editor, editor_handle);
		})
	});
}
/// Provides access to the `EditorHandle` by calling the given closure with them as arguments.
pub(crate) fn handle(callback: impl FnOnce(&mut EditorHandle)) {
	EDITOR_HANDLE.with(|editor_handle| {
		let mut guard = editor_handle.try_lock();
		let Ok(Some(editor_handle)) = guard.as_deref_mut() else {
			log::error!("Failed to borrow editor handle");
			return;
		};

		// Call the closure with the editor and its handle
		callback(editor_handle);
	});
}

#[cfg(not(feature = "native"))]
async fn poll_node_graph_evaluation() {
	// Process no further messages after a crash to avoid spamming the console
	if EDITOR_HAS_CRASHED.load(Ordering::SeqCst) {
		return;
	}

	if !editor::node_graph_executor::run_node_graph().await.0 {
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

fn auto_save_all_documents() {
	// Process no further messages after a crash to avoid spamming the console
	if EDITOR_HAS_CRASHED.load(Ordering::SeqCst) {
		return;
	}

	handle(|handle| {
		handle.dispatch(PortfolioMessage::AutoSaveAllDocuments);
	});
}
