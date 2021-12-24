// This file is where functions are defined to be called directly from JS.
// It serves as a thin wrapper over the editor backend API that relies
// on the dispatcher messaging system and more complex Rust data types.

use std::cell::Cell;

use crate::helpers::Error;
use crate::type_translators::{translate_blend_mode, translate_key, translate_tool_type};
use crate::{EDITOR_HAS_CRASHED, EDITOR_INSTANCES};
use editor::consts::FILE_SAVE_SUFFIX;
use editor::input::input_preprocessor::ModifierKeys;
use editor::input::mouse::{EditorMouseState, ScrollDelta, ViewportBounds};
use editor::misc::EditorError;
use editor::tool::{tool_options::ToolOptions, tools, ToolType};
use editor::Color;
use editor::LayerId;

use editor::{message_prelude::*, Editor};
use serde::Serialize;
use serde_wasm_bindgen;
use wasm_bindgen::prelude::*;

// To avoid wasm-bindgen from checking mutable reference issues using WasmRefCell
// we must make all methods take a non mutable reference to self. Not doing this creates
// an issue when rust calls into JS which calls back to rust in the same call stack.
#[wasm_bindgen]
pub struct JsEditorHandle {
	editor_id: u64,
	instance_received_crashed: Cell<bool>,
	handle_response: js_sys::Function,
}

#[wasm_bindgen]
impl JsEditorHandle {
	#[wasm_bindgen(constructor)]
	pub fn new(handle_response: js_sys::Function) -> Self {
		let editor_id = generate_uuid();
		let editor = Editor::new();
		EDITOR_INSTANCES.with(|instances| instances.borrow_mut().insert(editor_id, editor));
		JsEditorHandle {
			editor_id,
			instance_received_crashed: Cell::new(false),
			handle_response,
		}
	}

	// Sends a message to the dispatcher in the Editor Backend
	fn dispatch<T: Into<Message>>(&self, message: T) {
		// Process no further messages after a crash to avoid spamming the console
		let possible_crash_message = EDITOR_HAS_CRASHED.with(|crash_state| crash_state.borrow().clone());
		if let Some(message) = possible_crash_message {
			if !self.instance_received_crashed.get() {
				self.handle_response(message);
				self.instance_received_crashed.set(true);
			}
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
	// Create JS -> Rust wrapper functions below
	// ========================================================================

	pub fn has_crashed(&self) -> JsValue {
		let has_crashed = EDITOR_HAS_CRASHED.with(|crash_state| crash_state.borrow().is_some());
		has_crashed.into()
	}

	/// Modify the currently selected tool in the document state store
	pub fn select_tool(&self, tool: String) -> Result<(), JsValue> {
		match translate_tool_type(&tool) {
			Some(tool) => {
				let message = ToolMessage::ActivateTool(tool);
				self.dispatch(message);

				Ok(())
			}
			None => Err(Error::new(&format!("Couldn't select {} because it was not recognized as a valid tool", tool)).into()),
		}
	}

	/// Update the options for a given tool
	pub fn set_tool_options(&self, tool: String, options: &JsValue) -> Result<(), JsValue> {
		match serde_wasm_bindgen::from_value::<ToolOptions>(options.clone()) {
			Ok(options) => match translate_tool_type(&tool) {
				Some(tool) => {
					let message = ToolMessage::SetToolOptions(tool, options);
					self.dispatch(message);

					Ok(())
				}
				None => Err(Error::new(&format!("Couldn't set options for {} because it was not recognized as a valid tool", tool)).into()),
			},
			Err(err) => Err(Error::new(&format!("Invalid JSON for ToolOptions: {}", err)).into()),
		}
	}

	/// Send a message to a given tool
	pub fn send_tool_message(&self, tool: String, message: &JsValue) -> Result<(), JsValue> {
		let tool_message = match translate_tool_type(&tool) {
			Some(tool) => match tool {
				ToolType::Select => match serde_wasm_bindgen::from_value::<tools::select::SelectMessage>(message.clone()) {
					Ok(select_message) => Ok(ToolMessage::Select(select_message)),
					Err(err) => Err(Error::new(&format!("Invalid message for {}: {}", tool, err)).into()),
				},
				_ => Err(Error::new(&format!("Tool message sending not implemented for {}", tool)).into()),
			},
			None => Err(Error::new(&format!("Couldn't send message for {} because it was not recognized as a valid tool", tool)).into()),
		};

		match tool_message {
			Ok(message) => {
				self.dispatch(message);

				Ok(())
			}
			Err(err) => Err(err),
		}
	}

	pub fn select_document(&self, document_id: u64) {
		let message = DocumentsMessage::SelectDocument(document_id);
		self.dispatch(message);
	}

	pub fn get_open_documents_list(&self) {
		let message = DocumentsMessage::UpdateOpenDocumentsList;
		self.dispatch(message);
	}

	pub fn new_document(&self) {
		let message = DocumentsMessage::NewDocument;
		self.dispatch(message);
	}

	pub fn open_document(&self) {
		let message = DocumentsMessage::OpenDocument;
		self.dispatch(message);
	}

	pub fn open_document_file(&self, name: String, content: String) {
		let message = DocumentsMessage::OpenDocumentFile(name, content);
		self.dispatch(message);
	}

	pub fn save_document(&self) {
		let message = DocumentMessage::SaveDocument;
		self.dispatch(message);
	}

	pub fn close_document(&self, document_id: u64) {
		let message = DocumentsMessage::CloseDocument(document_id);
		self.dispatch(message);
	}

	pub fn close_all_documents(&self) {
		let message = DocumentsMessage::CloseAllDocuments;
		self.dispatch(message);
	}

	pub fn close_active_document_with_confirmation(&self) {
		let message = DocumentsMessage::CloseActiveDocumentWithConfirmation;
		self.dispatch(message);
	}

	pub fn close_document_with_confirmation(&self, document_id: u64) {
		let message = DocumentsMessage::CloseDocumentWithConfirmation(document_id);
		self.dispatch(message);
	}

	pub fn close_all_documents_with_confirmation(&self) {
		let message = DocumentsMessage::CloseAllDocumentsWithConfirmation;
		self.dispatch(message);
	}

	#[wasm_bindgen]
	pub fn request_about_graphite_dialog(&self) {
		let message = DocumentsMessage::RequestAboutGraphiteDialog;
		self.dispatch(message);
	}

	/// Send new bounds when document panel viewports get resized or moved within the editor
	/// [left, top, right, bottom]...
	#[wasm_bindgen]
	pub fn bounds_of_viewports(&self, bounds_of_viewports: &[f64]) {
		let chunked: Vec<_> = bounds_of_viewports.chunks(4).map(ViewportBounds::from_slice).collect();

		let message = InputPreprocessorMessage::BoundsOfViewports(chunked);
		self.dispatch(message);
	}

	/// Mouse movement within the screenspace bounds of the viewport
	pub fn on_mouse_move(&self, x: f64, y: f64, mouse_keys: u8, modifiers: u8) {
		let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());

		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		let message = InputPreprocessorMessage::MouseMove(editor_mouse_state, modifier_keys);
		self.dispatch(message);
	}

	/// Mouse scrolling within the screenspace bounds of the viewport
	pub fn on_mouse_scroll(&self, x: f64, y: f64, mouse_keys: u8, wheel_delta_x: i32, wheel_delta_y: i32, wheel_delta_z: i32, modifiers: u8) {
		let mut editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());
		editor_mouse_state.scroll_delta = ScrollDelta::new(wheel_delta_x, wheel_delta_y, wheel_delta_z);

		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		let message = InputPreprocessorMessage::MouseScroll(editor_mouse_state, modifier_keys);
		self.dispatch(message);
	}

	/// A mouse button depressed within screenspace the bounds of the viewport
	pub fn on_mouse_down(&self, x: f64, y: f64, mouse_keys: u8, modifiers: u8) {
		let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());

		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		let message = InputPreprocessorMessage::MouseDown(editor_mouse_state, modifier_keys);
		self.dispatch(message);
	}

	/// A mouse button released
	pub fn on_mouse_up(&self, x: f64, y: f64, mouse_keys: u8, modifiers: u8) {
		let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());

		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		let message = InputPreprocessorMessage::MouseUp(editor_mouse_state, modifier_keys);
		self.dispatch(message);
	}

	/// A keyboard button depressed within screenspace the bounds of the viewport
	pub fn on_key_down(&self, name: String, modifiers: u8) {
		let key = translate_key(&name);
		let modifiers = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		log::trace!("Key down {:?}, name: {}, modifiers: {:?}", key, name, modifiers);

		let message = InputPreprocessorMessage::KeyDown(key, modifiers);
		self.dispatch(message);
	}

	/// A keyboard button released
	pub fn on_key_up(&self, name: String, modifiers: u8) {
		let key = translate_key(&name);
		let modifiers = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		log::trace!("Key up {:?}, name: {}, modifiers: {:?}", key, name, modifiers);

		let message = InputPreprocessorMessage::KeyUp(key, modifiers);
		self.dispatch(message);
	}

	/// Update primary color
	pub fn update_primary_color(&self, red: f32, green: f32, blue: f32, alpha: f32) -> Result<(), JsValue> {
		let primary_color = match Color::from_rgbaf32(red, green, blue, alpha) {
			Some(color) => color,
			None => return Err(Error::new("Invalid color").into()),
		};

		let message = ToolMessage::SelectPrimaryColor(primary_color);
		self.dispatch(message);

		Ok(())
	}

	/// Update secondary color
	pub fn update_secondary_color(&self, red: f32, green: f32, blue: f32, alpha: f32) -> Result<(), JsValue> {
		let secondary_color = match Color::from_rgbaf32(red, green, blue, alpha) {
			Some(color) => color,
			None => return Err(Error::new("Invalid color").into()),
		};

		let message = ToolMessage::SelectSecondaryColor(secondary_color);
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

	pub fn select_layer(&self, paths: Vec<LayerId>, ctrl: bool, shift: bool) {
		let message = DocumentMessage::SelectLayer(paths, ctrl, shift);
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
	pub fn reorder_selected_layers(&self, delta: i32) {
		let message = DocumentMessage::ReorderSelectedLayers(delta);
		self.dispatch(message);
	}

	/// Set the blend mode for the selected layers
	pub fn set_blend_mode_for_selected_layers(&self, blend_mode_svg_style_name: String) -> Result<(), JsValue> {
		let blend_mode = translate_blend_mode(blend_mode_svg_style_name.as_str());

		match blend_mode {
			Some(mode) => {
				let message = DocumentMessage::SetBlendModeForSelectedLayers(mode);
				self.dispatch(message);

				Ok(())
			}
			None => Err(Error::new(&EditorError::Misc("UnknownBlendMode".to_string()).to_string()).into()),
		}
	}

	/// Set the opacity for the selected layers
	pub fn set_opacity_for_selected_layers(&self, opacity_percent: f64) {
		let message = DocumentMessage::SetOpacityForSelectedLayers(opacity_percent / 100.);
		self.dispatch(message);
	}

	/// Export the document
	pub fn export_document(&self) {
		let message = DocumentMessage::ExportDocument;
		self.dispatch(message);
	}

	/// Set snapping disabled / enabled
	pub fn set_snapping(&self, new_status: bool) {
		let message = DocumentMessage::SetSnapping(new_status);
		self.dispatch(message);
	}

	/// Sets the zoom to the value
	pub fn set_canvas_zoom(&self, new_zoom: f64) {
		let message = MovementMessage::SetCanvasZoom(new_zoom);
		self.dispatch(message);
	}

	/// Zoom in to the next step
	pub fn increase_canvas_zoom(&self) {
		let message = MovementMessage::IncreaseCanvasZoom;
		self.dispatch(message);
	}

	/// Zoom out to the next step
	pub fn decrease_canvas_zoom(&self) {
		let message = MovementMessage::DecreaseCanvasZoom;
		self.dispatch(message);
	}

	/// Sets the rotation to the new value (in radians)
	pub fn set_rotation(&self, new_radians: f64) {
		let message = MovementMessage::SetCanvasRotation(new_radians);
		self.dispatch(message);
	}

	/// Translates document (in viewport coords)
	pub fn translate_canvas(&self, delta_x: f64, delta_y: f64) {
		let message = MovementMessage::TranslateCanvas((delta_x, delta_y).into());
		self.dispatch(message);
	}

	/// Translates document (in viewport coords)
	pub fn translate_canvas_by_fraction(&self, delta_x: f64, delta_y: f64) {
		let message = MovementMessage::TranslateCanvasByViewportFraction((delta_x, delta_y).into());
		self.dispatch(message);
	}

	/// Update the list of selected layers. The layer paths have to be stored in one array and are separated by LayerId::MAX
	pub fn select_layers(&self, paths: Vec<LayerId>) {
		let paths = paths.split(|id| *id == LayerId::MAX).map(|path| path.to_vec()).collect();

		let message = DocumentMessage::SetSelectedLayers(paths);
		self.dispatch(message);
	}

	/// Toggle visibility of a layer from the layer list
	pub fn toggle_layer_visibility(&self, path: Vec<LayerId>) {
		let message = DocumentMessage::ToggleLayerVisibility(path);
		self.dispatch(message);
	}

	/// Toggle expansions state of a layer from the layer list
	pub fn toggle_layer_expansion(&self, path: Vec<LayerId>) {
		let message = DocumentMessage::ToggleLayerExpansion(path);
		self.dispatch(message);
	}

	/// Renames a layer from the layer list
	pub fn rename_layer(&self, path: Vec<LayerId>, new_name: String) {
		let message = DocumentMessage::RenameLayer(path, new_name);
		self.dispatch(message);
	}

	/// Deletes a layer from the layer list
	pub fn delete_layer(&self, path: Vec<LayerId>) {
		let message = DocumentMessage::DeleteLayer(path);
		self.dispatch(message);
	}

	/// Requests the backend to add a layer to the layer list
	pub fn add_folder(&self, path: Vec<LayerId>) {
		let message = DocumentMessage::CreateEmptyFolder(path);
		self.dispatch(message);
	}
}

impl Drop for JsEditorHandle {
	fn drop(&mut self) {
		EDITOR_INSTANCES.with(|instances| instances.borrow_mut().remove(&self.editor_id));
	}
}

/// Access a handle to WASM memory
#[wasm_bindgen]
pub fn wasm_memory() -> JsValue {
	wasm_bindgen::memory()
}

/// Intentionally panic for debugging purposes
#[wasm_bindgen]
pub fn intentional_panic() {
	panic!();
}

/// Get the constant FILE_SAVE_SUFFIX
#[wasm_bindgen]
pub fn file_save_suffix() -> String {
	FILE_SAVE_SUFFIX.into()
}

/// Get the constant i32::MAX
#[wasm_bindgen]
pub fn i32_max() -> i32 {
	i32::MAX
}

/// Get the constant i32::MIN
#[wasm_bindgen]
pub fn i32_min() -> i32 {
	i32::MIN
}
