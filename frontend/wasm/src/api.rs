// This file is where functions are defined to be called directly from JS.
// It serves as a thin wrapper over the editor backend API that relies
// on the dispatcher messaging system and more complex Rust data types.

use crate::helpers::{translate_key, Error};
use crate::{EDITOR_HAS_CRASHED, EDITOR_INSTANCES, JS_EDITOR_HANDLES};

use editor::consts::{FILE_SAVE_SUFFIX, GRAPHITE_DOCUMENT_VERSION};
use editor::input::input_preprocessor::ModifierKeys;
use editor::input::mouse::{EditorMouseState, ScrollDelta, ViewportBounds};
use editor::message_prelude::*;
use editor::Color;
use editor::Editor;
use editor::LayerId;
use graphene::Operation;

use serde::Serialize;
use serde_wasm_bindgen::{self, from_value};
use std::sync::atomic::Ordering;
use wasm_bindgen::prelude::*;

// To avoid wasm-bindgen from checking mutable reference issues using WasmRefCell we must make all methods take a non mutable reference to self.
// Not doing this creates an issue when rust calls into JS which calls back to rust in the same call stack.
#[wasm_bindgen]
#[derive(Clone)]
pub struct JsEditorHandle {
	editor_id: u64,
	handle_response: js_sys::Function,
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
impl JsEditorHandle {
	#[wasm_bindgen(constructor)]
	pub fn new(handle_response: js_sys::Function) -> Self {
		let editor_id = generate_uuid();
		let editor = Editor::new();
		let editor_handle = JsEditorHandle { editor_id, handle_response };
		EDITOR_INSTANCES.with(|instances| instances.borrow_mut().insert(editor_id, editor));
		JS_EDITOR_HANDLES.with(|instances| instances.borrow_mut().insert(editor_id, editor_handle.clone()));
		editor_handle
	}

	// Sends a message to the dispatcher in the Editor Backend
	fn dispatch<T: Into<Message>>(&self, message: T) {
		// Process no further messages after a crash to avoid spamming the console
		if EDITOR_HAS_CRASHED.load(Ordering::SeqCst) {
			return;
		}

		let responses = EDITOR_INSTANCES.with(|instances| {
			instances
				.borrow_mut()
				.get_mut(&self.editor_id)
				.expect("EDITOR_INSTANCES does not contain the current editor_id")
				.handle_message(message.into())
		});
		for response in responses.into_iter() {
			// Send each FrontendMessage to the JavaScript frontend
			self.handle_response(response);
		}
	}

	// Sends a FrontendMessage to JavaScript
	fn handle_response(&self, message: FrontendMessage) {
		let message_type = message.to_discriminant().local_name();

		let serializer = serde_wasm_bindgen::Serializer::new().serialize_large_number_types_as_bigints(true);
		let message_data = message.serialize(&serializer).expect("Failed to serialize FrontendMessage");

		let js_return_value = self.handle_response.call2(&JsValue::null(), &JsValue::from(message_type), &message_data);

		if let Err(error) = js_return_value {
			log::error!(
				"While handling FrontendMessage \"{:?}\", JavaScript threw an error: {:?}",
				message.to_discriminant().local_name(),
				error,
			)
		}
	}

	// ========================================================================
	// Add additional JS -> Rust wrapper functions below as needed for calling
	// the backend from the web frontend.
	// ========================================================================

	pub fn has_crashed(&self) -> bool {
		EDITOR_HAS_CRASHED.load(Ordering::SeqCst)
	}

	pub fn toggle_node_graph_visibility(&self) {
		self.dispatch(WorkspaceMessage::NodeGraphToggleVisibility);
	}

	/// Update layout of a given UI
	pub fn update_layout(&self, layout_target: JsValue, widget_id: u64, value: JsValue) -> Result<(), JsValue> {
		match (from_value(layout_target), from_value(value)) {
			(Ok(layout_target), Ok(value)) => {
				let message = LayoutMessage::UpdateLayout { layout_target, widget_id, value };
				self.dispatch(message);
				Ok(())
			}
			_ => Err(Error::new("Could not update UI").into()),
		}
	}

	pub fn select_document(&self, document_id: u64) {
		let message = PortfolioMessage::SelectDocument { document_id };
		self.dispatch(message);
	}

	pub fn get_open_documents_list(&self) {
		let message = PortfolioMessage::UpdateOpenDocumentsList;
		self.dispatch(message);
	}

	pub fn request_new_document_dialog(&self) {
		let message = DialogMessage::RequestNewDocumentDialog;
		self.dispatch(message);
	}

	pub fn open_document(&self) {
		let message = PortfolioMessage::OpenDocument;
		self.dispatch(message);
	}

	pub fn open_document_file(&self, document_name: String, document_serialized_content: String) {
		let message = PortfolioMessage::OpenDocumentFile {
			document_name,
			document_serialized_content,
		};
		self.dispatch(message);
	}

	pub fn open_auto_saved_document(&self, document_id: u64, document_name: String, document_is_saved: bool, document_serialized_content: String) {
		let message = PortfolioMessage::OpenDocumentFileWithId {
			document_id,
			document_name,
			document_is_saved,
			document_serialized_content,
		};
		self.dispatch(message);
	}

	pub fn save_document(&self) {
		let message = DocumentMessage::SaveDocument;
		self.dispatch(message);
	}

	pub fn trigger_auto_save(&self, document_id: u64) {
		let message = PortfolioMessage::AutoSaveDocument { document_id };
		self.dispatch(message);
	}

	pub fn close_document(&self, document_id: u64) {
		let message = ToolMessage::AbortCurrentTool;
		self.dispatch(message);

		let message = PortfolioMessage::CloseDocument { document_id };
		self.dispatch(message);
	}

	pub fn close_all_documents(&self) {
		let message = PortfolioMessage::CloseAllDocuments;
		self.dispatch(message);
	}

	pub fn close_active_document_with_confirmation(&self) {
		let message = PortfolioMessage::CloseActiveDocumentWithConfirmation;
		self.dispatch(message);
	}

	pub fn close_document_with_confirmation(&self, document_id: u64) {
		let message = PortfolioMessage::CloseDocumentWithConfirmation { document_id };
		self.dispatch(message);
	}

	pub fn close_all_documents_with_confirmation(&self) {
		let message = DialogMessage::CloseAllDocumentsWithConfirmation;
		self.dispatch(message);
	}

	pub fn populate_build_metadata(&self, release: String, timestamp: String, hash: String, branch: String) {
		let new = editor::communication::BuildMetadata { release, timestamp, hash, branch };
		let message = Message::PopulateBuildMetadata { new };
		self.dispatch(message);
	}

	pub fn request_about_graphite_dialog(&self) {
		let message = DialogMessage::RequestAboutGraphiteDialog;
		self.dispatch(message);
	}

	pub fn request_coming_soon_dialog(&self, issue: Option<i32>) {
		let message = DialogMessage::RequestComingSoonDialog { issue };
		self.dispatch(message);
	}

	pub fn log_level_info(&self) {
		let message = GlobalMessage::LogInfo;
		self.dispatch(message);
	}

	pub fn log_level_debug(&self) {
		let message = GlobalMessage::LogDebug;
		self.dispatch(message);
	}

	pub fn log_level_trace(&self) {
		let message = GlobalMessage::LogTrace;
		self.dispatch(message);
	}

	/// Send new bounds when document panel viewports get resized or moved within the editor
	/// [left, top, right, bottom]...
	pub fn bounds_of_viewports(&self, bounds_of_viewports: &[f64]) {
		let chunked: Vec<_> = bounds_of_viewports.chunks(4).map(ViewportBounds::from_slice).collect();

		let message = InputPreprocessorMessage::BoundsOfViewports { bounds_of_viewports: chunked };
		self.dispatch(message);
	}

	/// Mouse movement within the screenspace bounds of the viewport
	pub fn on_mouse_move(&self, x: f64, y: f64, mouse_keys: u8, modifiers: u8) {
		let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());

		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		let message = InputPreprocessorMessage::PointerMove { editor_mouse_state, modifier_keys };
		self.dispatch(message);
	}

	/// Mouse scrolling within the screenspace bounds of the viewport
	pub fn on_mouse_scroll(&self, x: f64, y: f64, mouse_keys: u8, wheel_delta_x: i32, wheel_delta_y: i32, wheel_delta_z: i32, modifiers: u8) {
		let mut editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());
		editor_mouse_state.scroll_delta = ScrollDelta::new(wheel_delta_x, wheel_delta_y, wheel_delta_z);

		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		let message = InputPreprocessorMessage::MouseScroll { editor_mouse_state, modifier_keys };
		self.dispatch(message);
	}

	/// A mouse button depressed within screenspace the bounds of the viewport
	pub fn on_mouse_down(&self, x: f64, y: f64, mouse_keys: u8, modifiers: u8) {
		let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());

		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		let message = InputPreprocessorMessage::PointerDown { editor_mouse_state, modifier_keys };
		self.dispatch(message);
	}

	/// A mouse button released
	pub fn on_mouse_up(&self, x: f64, y: f64, mouse_keys: u8, modifiers: u8) {
		let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());

		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		let message = InputPreprocessorMessage::PointerUp { editor_mouse_state, modifier_keys };
		self.dispatch(message);
	}

	/// Mouse double clicked
	pub fn on_double_click(&self, x: f64, y: f64, mouse_keys: u8, modifiers: u8) {
		let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());
		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		let message = InputPreprocessorMessage::DoubleClick { editor_mouse_state, modifier_keys };
		self.dispatch(message);
	}

	/// A keyboard button depressed within screenspace the bounds of the viewport
	pub fn on_key_down(&self, name: String, modifiers: u8) {
		let key = translate_key(&name);
		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		log::trace!("Key down {:?}, name: {}, modifiers: {:?}", key, name, modifiers);

		let message = InputPreprocessorMessage::KeyDown { key, modifier_keys };
		self.dispatch(message);
	}

	/// A keyboard button released
	pub fn on_key_up(&self, name: String, modifiers: u8) {
		let key = translate_key(&name);
		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		log::trace!("Key up {:?}, name: {}, modifiers: {:?}", key, name, modifier_keys);

		let message = InputPreprocessorMessage::KeyUp { key, modifier_keys };
		self.dispatch(message);
	}

	/// A text box was committed
	pub fn on_change_text(&self, new_text: String) -> Result<(), JsValue> {
		let message = TextMessage::TextChange { new_text };
		self.dispatch(message);

		Ok(())
	}

	/// A font has been downloaded
	pub fn on_font_load(&self, font: String, data: Vec<u8>, is_default: bool) -> Result<(), JsValue> {
		let message = DocumentMessage::FontLoaded { font, data, is_default };
		self.dispatch(message);

		Ok(())
	}

	/// A text box was changed
	pub fn update_bounds(&self, new_text: String) -> Result<(), JsValue> {
		let message = TextMessage::UpdateBounds { new_text };
		self.dispatch(message);

		Ok(())
	}

	/// Update primary color
	pub fn update_primary_color(&self, red: f32, green: f32, blue: f32, alpha: f32) -> Result<(), JsValue> {
		let primary_color = match Color::from_rgbaf32(red, green, blue, alpha) {
			Some(color) => color,
			None => return Err(Error::new("Invalid color").into()),
		};

		let message = ToolMessage::SelectPrimaryColor { color: primary_color };
		self.dispatch(message);

		Ok(())
	}

	/// Update secondary color
	pub fn update_secondary_color(&self, red: f32, green: f32, blue: f32, alpha: f32) -> Result<(), JsValue> {
		let secondary_color = match Color::from_rgbaf32(red, green, blue, alpha) {
			Some(color) => color,
			None => return Err(Error::new("Invalid color").into()),
		};

		let message = ToolMessage::SelectSecondaryColor { color: secondary_color };
		self.dispatch(message);

		Ok(())
	}

	/// Swap primary and secondary color
	pub fn swap_colors(&self) {
		let message = ToolMessage::SwapColors;
		self.dispatch(message);
	}

	/// Reset primary and secondary colors to their defaults
	pub fn reset_colors(&self) {
		let message = ToolMessage::ResetColors;
		self.dispatch(message);
	}

	/// Undo history one step
	pub fn undo(&self) {
		let message = DocumentMessage::Undo;
		self.dispatch(message);
	}

	/// Redo history one step
	pub fn redo(&self) {
		let message = DocumentMessage::Redo;
		self.dispatch(message);
	}

	/// Cut selected layers
	pub fn cut(&self) {
		let message = PortfolioMessage::Cut { clipboard: Clipboard::Device };
		self.dispatch(message);
	}

	/// Copy selected layers
	pub fn copy(&self) {
		let message = PortfolioMessage::Copy { clipboard: Clipboard::Device };
		self.dispatch(message);
	}

	/// Paste layers from a serialized json representation
	pub fn paste_serialized_data(&self, data: String) {
		let message = PortfolioMessage::PasteSerializedData { data };
		self.dispatch(message);
	}

	/// Modify the layer selection based on the layer which is clicked while holding down the <kbd>Ctrl</kbd> and/or <kbd>Shift</kbd> modifier keys used for range selection behavior
	pub fn select_layer(&self, layer_path: Vec<LayerId>, ctrl: bool, shift: bool) {
		let message = DocumentMessage::SelectLayer { layer_path, ctrl, shift };
		self.dispatch(message);
	}

	/// Select all layers
	pub fn select_all_layers(&self) {
		let message = DocumentMessage::SelectAllLayers;
		self.dispatch(message);
	}

	/// Deselect all layers
	pub fn deselect_all_layers(&self) {
		let message = DocumentMessage::DeselectAllLayers;
		self.dispatch(message);
	}

	/// Reorder selected layer
	pub fn reorder_selected_layers(&self, relative_index_offset: isize) {
		let message = DocumentMessage::ReorderSelectedLayers { relative_index_offset };
		self.dispatch(message);
	}

	/// Move a layer to be next to the specified neighbor
	pub fn move_layer_in_tree(&self, folder_path: Vec<LayerId>, insert_index: isize) {
		let message = DocumentMessage::MoveSelectedLayersTo {
			folder_path,
			insert_index,
			reverse_index: true,
		};
		self.dispatch(message);
	}

	/// Set the name for the layer
	pub fn set_layer_name(&self, layer_path: Vec<LayerId>, name: String) {
		let message = DocumentMessage::SetLayerName { layer_path, name };
		self.dispatch(message);
	}

	/// Export the document
	pub fn export_document(&self) {
		let message = DialogMessage::RequestExportDialog;
		self.dispatch(message);
	}

	/// Translates document (in viewport coords)
	pub fn translate_canvas(&self, delta_x: f64, delta_y: f64) {
		let message = MovementMessage::TranslateCanvas { delta: (delta_x, delta_y).into() };
		self.dispatch(message);
	}

	/// Translates document (in viewport coords)
	pub fn translate_canvas_by_fraction(&self, delta_x: f64, delta_y: f64) {
		let message = MovementMessage::TranslateCanvasByViewportFraction { delta: (delta_x, delta_y).into() };
		self.dispatch(message);
	}

	/// Sends the blob url generated by js
	pub fn set_image_blob_url(&self, path: Vec<LayerId>, blob_url: String, width: f64, height: f64) {
		let dimensions = (width, height);
		let message = Operation::SetImageBlobUrl { path, blob_url, dimensions };
		self.dispatch(message);
	}

	/// Pastes an image
	pub fn paste_image(&self, mime: String, image_data: Vec<u8>, mouse_x: Option<f64>, mouse_y: Option<f64>) {
		let mouse = mouse_x.and_then(|x| mouse_y.map(|y| (x, y)));
		let message = DocumentMessage::PasteImage { mime, image_data, mouse };
		self.dispatch(message);
	}

	/// Toggle visibility of a layer from the layer list
	pub fn toggle_layer_visibility(&self, layer_path: Vec<LayerId>) {
		let message = DocumentMessage::ToggleLayerVisibility { layer_path };
		self.dispatch(message);
	}

	/// Toggle expansions state of a layer from the layer list
	pub fn toggle_layer_expansion(&self, layer_path: Vec<LayerId>) {
		let message = DocumentMessage::ToggleLayerExpansion { layer_path };
		self.dispatch(message);
	}

	// TODO: Replace with initialization system, issue #524
	pub fn init_app(&self) {
		let message = PortfolioMessage::UpdateDocumentWidgets;
		self.dispatch(message);

		let message = ToolMessage::InitTools;
		self.dispatch(message);
	}
}

// Needed to make JsEditorHandle functions pub to rust. Do not fully
// understand reason but has to do with #[wasm_bindgen] procedural macro.
impl JsEditorHandle {
	pub fn handle_response_rust_proxy(&self, message: FrontendMessage) {
		self.handle_response(message);
	}
}

impl Drop for JsEditorHandle {
	fn drop(&mut self) {
		EDITOR_INSTANCES.with(|instances| instances.borrow_mut().remove(&self.editor_id));
	}
}

/// Set the random seed used by the editor by calling this from JS upon initialization.
/// This is necessary because WASM doesn't have a random number generator.
#[wasm_bindgen]
pub fn set_random_seed(seed: u64) {
	editor::communication::set_uuid_seed(seed)
}

/// Intentionally panic for debugging purposes
#[wasm_bindgen]
pub fn intentional_panic() {
	panic!();
}

/// Access a handle to WASM memory
#[wasm_bindgen]
pub fn wasm_memory() -> JsValue {
	wasm_bindgen::memory()
}

/// Get the constant `FILE_SAVE_SUFFIX`
#[wasm_bindgen]
pub fn file_save_suffix() -> String {
	FILE_SAVE_SUFFIX.into()
}

/// Get the constant `GRAPHITE_DOCUMENT_VERSION`
#[wasm_bindgen]
pub fn graphite_version() -> String {
	GRAPHITE_DOCUMENT_VERSION.to_string()
}

/// Get the constant `i32::MAX`
#[wasm_bindgen]
pub fn i32_max() -> i32 {
	i32::MAX
}

/// Get the constant `i32::MIN`
#[wasm_bindgen]
pub fn i32_min() -> i32 {
	i32::MIN
}
