// This file is where functions are defined to be called directly from JS.
// It serves as a thin wrapper over the editor backend API that relies
// on the dispatcher messaging system and more complex Rust data types.

use crate::handleResponse;
use crate::helpers::Error;
use crate::type_translators::{translate_blend_mode, translate_key, translate_tool_type};
use crate::EDITOR_HAS_CRASHED;
use editor::consts::FILE_SAVE_SUFFIX;
use editor::input::input_preprocessor::ModifierKeys;
use editor::input::mouse::{EditorMouseState, ScrollDelta, ViewportBounds};
use editor::message_prelude::*;
use editor::misc::EditorError;
use editor::tool::{tool_options::ToolOptions, tools, ToolType};
use editor::Color;
use editor::LayerId;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Editor {
	editor: editor::Editor,
	handle_response: JsValue,
}

#[wasm_bindgen]
impl Editor {
	#[wasm_bindgen(constructor)]
	pub fn new(handle_response: JsValue) -> Editor {
		Editor {
			editor: editor::Editor::new(),
			handle_response,
		}
	}

	// Sends a message to the dispatcher in the Editor Backend
	fn dispatch<T: Into<Message>>(&mut self, message: T) {
		// Process no further messages after a crash to avoid spamming the console
		if EDITOR_HAS_CRASHED.load(std::sync::atomic::Ordering::SeqCst) {
			return;
		}

		// Dispatch the message and receive a vector of FrontendMessage responses
		let responses = self.editor.handle_message(message.into());
		for response in responses.into_iter() {
			// Send each FrontendMessage to the JavaScript frontend
			self.handle_response(response);
		}
	}

	// Sends a FrontendMessage to JavaScript
	fn handle_response(&mut self, message: FrontendMessage) {
		let message_type = message.to_discriminant().local_name();
		let message_data = JsValue::from_serde(&message).expect("Failed to serialize FrontendMessage");

		let js_return_value = handleResponse(&self.handle_response, message_type, message_data);
		if let Err(error) = js_return_value {
			log::error!(
				"While handling FrontendMessage \"{:?}\", JavaScript threw an error: {:?}",
				message.to_discriminant().local_name(),
				error,
			)
		}
	}

	/// Modify the currently selected tool in the document state store
	pub fn select_tool(&mut self, tool: String) -> Result<(), JsValue> {
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
	pub fn set_tool_options(&mut self, tool: String, options: &JsValue) -> Result<(), JsValue> {
		match options.into_serde::<ToolOptions>() {
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
	pub fn send_tool_message(&mut self, tool: String, message: &JsValue) -> Result<(), JsValue> {
		let tool_message = match translate_tool_type(&tool) {
			Some(tool) => match tool {
				ToolType::Select => match message.into_serde::<tools::select::SelectMessage>() {
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

	pub fn select_document(&mut self, document: usize) {
		let message = DocumentsMessage::SelectDocument(document);
		self.dispatch(message);
	}

	pub fn get_open_documents_list(&mut self) {
		let message = DocumentsMessage::GetOpenDocumentsList;
		self.dispatch(message);
	}

	pub fn new_document(&mut self) {
		let message = DocumentsMessage::NewDocument;
		self.dispatch(message);
	}

	pub fn open_document(&mut self) {
		let message = DocumentsMessage::OpenDocument;
		self.dispatch(message);
	}

	pub fn open_document_file(&mut self, name: String, content: String) {
		let message = DocumentsMessage::OpenDocumentFile(name, content);
		self.dispatch(message);
	}

	pub fn save_document(&mut self) {
		let message = DocumentMessage::SaveDocument;
		self.dispatch(message);
	}

	pub fn close_document(&mut self, document: usize) {
		let message = DocumentsMessage::CloseDocument(document);
		self.dispatch(message);
	}

	pub fn close_all_documents(&mut self) {
		let message = DocumentsMessage::CloseAllDocuments;
		self.dispatch(message);
	}

	pub fn close_active_document_with_confirmation(&mut self) {
		let message = DocumentsMessage::CloseActiveDocumentWithConfirmation;
		self.dispatch(message);
	}

	pub fn close_all_documents_with_confirmation(&mut self) {
		let message = DocumentsMessage::CloseAllDocumentsWithConfirmation;
		self.dispatch(message);
	}

	/// Send new bounds when document panel viewports get resized or moved within the editor
	/// [left, top, right, bottom]...
	pub fn bounds_of_viewports(&mut self, bounds_of_viewports: &[f64]) {
		let chunked: Vec<_> = bounds_of_viewports.chunks(4).map(ViewportBounds::from_slice).collect();

		let message = InputPreprocessorMessage::BoundsOfViewports(chunked);
		self.dispatch(message);
	}

	/// Mouse movement within the screenspace bounds of the viewport
	pub fn on_mouse_move(&mut self, x: f64, y: f64, mouse_keys: u8, modifiers: u8) {
		let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());

		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		let message = InputPreprocessorMessage::MouseMove(editor_mouse_state, modifier_keys);
		self.dispatch(message);
	}

	/// Mouse scrolling within the screenspace bounds of the viewport
	pub fn on_mouse_scroll(&mut self, x: f64, y: f64, mouse_keys: u8, wheel_delta_x: i32, wheel_delta_y: i32, wheel_delta_z: i32, modifiers: u8) {
		let mut editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());
		editor_mouse_state.scroll_delta = ScrollDelta::new(wheel_delta_x, wheel_delta_y, wheel_delta_z);

		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		let message = InputPreprocessorMessage::MouseScroll(editor_mouse_state, modifier_keys);
		self.dispatch(message);
	}

	/// A mouse button depressed within screenspace the bounds of the viewport
	pub fn on_mouse_down(&mut self, x: f64, y: f64, mouse_keys: u8, modifiers: u8) {
		let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());

		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		let message = InputPreprocessorMessage::MouseDown(editor_mouse_state, modifier_keys);
		self.dispatch(message);
	}

	/// A mouse button released
	pub fn on_mouse_up(&mut self, x: f64, y: f64, mouse_keys: u8, modifiers: u8) {
		let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());

		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		let message = InputPreprocessorMessage::MouseUp(editor_mouse_state, modifier_keys);
		self.dispatch(message);
	}

	/// A keyboard button depressed within screenspace the bounds of the viewport
	pub fn on_key_down(&mut self, name: String, modifiers: u8) {
		let key = translate_key(&name);
		let modifiers = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		log::trace!("Key down {:?}, name: {}, modifiers: {:?}", key, name, modifiers);

		let message = InputPreprocessorMessage::KeyDown(key, modifiers);
		self.dispatch(message);
	}

	/// A keyboard button released
	pub fn on_key_up(&mut self, name: String, modifiers: u8) {
		let key = translate_key(&name);
		let modifiers = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		log::trace!("Key up {:?}, name: {}, modifiers: {:?}", key, name, modifiers);

		let message = InputPreprocessorMessage::KeyUp(key, modifiers);
		self.dispatch(message);
	}

	/// Update primary color
	pub fn update_primary_color(&mut self, red: f32, green: f32, blue: f32, alpha: f32) -> Result<(), JsValue> {
		let primary_color = match Color::from_rgbaf32(red, green, blue, alpha) {
			Some(color) => color,
			None => return Err(Error::new("Invalid color").into()),
		};

		let message = ToolMessage::SelectPrimaryColor(primary_color);
		self.dispatch(message);

		Ok(())
	}

	/// Update secondary color
	pub fn update_secondary_color(&mut self, red: f32, green: f32, blue: f32, alpha: f32) -> Result<(), JsValue> {
		let secondary_color = match Color::from_rgbaf32(red, green, blue, alpha) {
			Some(color) => color,
			None => return Err(Error::new("Invalid color").into()),
		};

		let message = ToolMessage::SelectSecondaryColor(secondary_color);
		self.dispatch(message);

		Ok(())
	}

	/// Swap primary and secondary color
	pub fn swap_colors(&mut self) {
		let message = ToolMessage::SwapColors;
		self.dispatch(message);
	}

	/// Reset primary and secondary colors to their defaults
	pub fn reset_colors(&mut self) {
		let message = ToolMessage::ResetColors;
		self.dispatch(message);
	}

	/// Undo history one step
	pub fn undo(&mut self) {
		let message = DocumentMessage::Undo;
		self.dispatch(message);
	}

	/// Redo history one step
	pub fn redo(&mut self) {
		let message = DocumentMessage::Redo;
		self.dispatch(message);
	}

	/// Select all layers
	pub fn select_all_layers(&mut self) {
		let message = DocumentMessage::SelectAllLayers;
		self.dispatch(message);
	}

	/// Deselect all layers
	pub fn deselect_all_layers(&mut self) {
		let message = DocumentMessage::DeselectAllLayers;
		self.dispatch(message);
	}

	/// Reorder selected layer
	pub fn reorder_selected_layers(&mut self, delta: i32) {
		let message = DocumentMessage::ReorderSelectedLayers(delta);
		self.dispatch(message);
	}

	/// Set the blend mode for the selected layers
	pub fn set_blend_mode_for_selected_layers(&mut self, blend_mode_svg_style_name: String) -> Result<(), JsValue> {
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
	pub fn set_opacity_for_selected_layers(&mut self, opacity_percent: f64) {
		let message = DocumentMessage::SetOpacityForSelectedLayers(opacity_percent / 100.);
		self.dispatch(message);
	}

	/// Export the document
	pub fn export_document(&mut self) {
		let message = DocumentMessage::ExportDocument;
		self.dispatch(message);
	}

	/// Sets the zoom to the value
	pub fn set_canvas_zoom(&mut self, new_zoom: f64) {
		let message = MovementMessage::SetCanvasZoom(new_zoom);
		self.dispatch(message);
	}

	/// Zoom in to the next step
	pub fn increase_canvas_zoom(&mut self) {
		let message = MovementMessage::IncreaseCanvasZoom;
		self.dispatch(message);
	}

	/// Zoom out to the next step
	pub fn decrease_canvas_zoom(&mut self) {
		let message = MovementMessage::DecreaseCanvasZoom;
		self.dispatch(message);
	}

	/// Sets the rotation to the new value (in radians)
	pub fn set_rotation(&mut self, new_radians: f64) {
		let message = MovementMessage::SetCanvasRotation(new_radians);
		self.dispatch(message);
	}

	/// Translates document (in viewport coords)
	pub fn translate_canvas(&mut self, delta_x: f64, delta_y: f64) {
		let message = MovementMessage::TranslateCanvas((delta_x, delta_y).into());
		self.dispatch(message);
	}

	/// Translates document (in viewport coords)
	pub fn translate_canvas_by_fraction(&mut self, delta_x: f64, delta_y: f64) {
		let message = MovementMessage::TranslateCanvasByViewportFraction((delta_x, delta_y).into());
		self.dispatch(message);
	}

	/// Update the list of selected layers. The layer paths have to be stored in one array and are separated by LayerId::MAX
	pub fn select_layers(&mut self, paths: Vec<LayerId>) {
		let paths = paths.split(|id| *id == LayerId::MAX).map(|path| path.to_vec()).collect();

		let message = DocumentMessage::SetSelectedLayers(paths);
		self.dispatch(message);
	}

	/// Toggle visibility of a layer from the layer list
	pub fn toggle_layer_visibility(&mut self, path: Vec<LayerId>) {
		let message = DocumentMessage::ToggleLayerVisibility(path);
		self.dispatch(message);
	}

	/// Toggle expansions state of a layer from the layer list
	pub fn toggle_layer_expansion(&mut self, path: Vec<LayerId>) {
		let message = DocumentMessage::ToggleLayerExpansion(path);
		self.dispatch(message);
	}

	/// Renames a layer from the layer list
	pub fn rename_layer(&mut self, path: Vec<LayerId>, new_name: String) {
		let message = DocumentMessage::RenameLayer(path, new_name);
		self.dispatch(message);
	}

	/// Deletes a layer from the layer list
	pub fn delete_layer(&mut self, path: Vec<LayerId>) {
		let message = DocumentMessage::DeleteLayer(path);
		self.dispatch(message);
	}

	/// Requests the backend to add a layer to the layer list
	pub fn add_folder(&mut self, path: Vec<LayerId>) {
		let message = DocumentMessage::CreateFolder(path);
		self.dispatch(message);
	}
}

/// Intentionally panic for testing purposes
#[wasm_bindgen]
pub fn intentional_panic() {
	panic!();
}

#[wasm_bindgen]
pub fn wasm_memory() -> JsValue {
	wasm_bindgen::memory()
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
