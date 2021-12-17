// This file is where functions are defined to be called directly from JS.
// It serves as a thin wrapper over the editor backend API that relies
// on the dispatcher messaging system and more complex Rust data types.

use crate::dispatch;
use crate::helpers::Error;
use crate::type_translators::{translate_blend_mode, translate_key, translate_tool_type, translate_view_mode};
use editor::consts::FILE_SAVE_SUFFIX;
use editor::input::input_preprocessor::ModifierKeys;
use editor::input::mouse::{EditorMouseState, ScrollDelta, ViewportBounds};
use editor::misc::EditorError;
use editor::tool::{tool_options::ToolOptions, tools, ToolType};
use editor::LayerId;
use editor::{message_prelude::*, Color};
use graphene::operation::Operation;
use wasm_bindgen::prelude::*;

/// Intentionally panic for testing purposes
#[wasm_bindgen]
pub fn intentional_panic() {
	panic!();
}

#[wasm_bindgen]
pub fn wasm_memory() -> JsValue {
	wasm_bindgen::memory()
}

/// Modify the currently selected tool in the document state store
#[wasm_bindgen]
pub fn select_tool(tool: String) -> Result<(), JsValue> {
	match translate_tool_type(&tool) {
		Some(tool) => {
			let message = ToolMessage::ActivateTool(tool);
			dispatch(message);

			Ok(())
		}
		None => Err(Error::new(&format!("Couldn't select {} because it was not recognized as a valid tool", tool)).into()),
	}
}

/// Update the options for a given tool
#[wasm_bindgen]
pub fn set_tool_options(tool: String, options: &JsValue) -> Result<(), JsValue> {
	match options.into_serde::<ToolOptions>() {
		Ok(options) => match translate_tool_type(&tool) {
			Some(tool) => {
				let message = ToolMessage::SetToolOptions(tool, options);
				dispatch(message);

				Ok(())
			}
			None => Err(Error::new(&format!("Couldn't set options for {} because it was not recognized as a valid tool", tool)).into()),
		},
		Err(err) => Err(Error::new(&format!("Invalid JSON for ToolOptions: {}", err)).into()),
	}
}

/// Send a message to a given tool
#[wasm_bindgen]
pub fn send_tool_message(tool: String, message: &JsValue) -> Result<(), JsValue> {
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
			dispatch(message);

			Ok(())
		}
		Err(err) => Err(err),
	}
}

#[wasm_bindgen]
pub fn select_document(document: usize) {
	let message = DocumentsMessage::SelectDocument(document);
	dispatch(message);
}

#[wasm_bindgen]
pub fn get_open_documents_list() {
	let message = DocumentsMessage::UpdateOpenDocumentsList;
	dispatch(message);
}

#[wasm_bindgen]
pub fn new_document() {
	let message = DocumentsMessage::NewDocument;
	dispatch(message);
}

#[wasm_bindgen]
pub fn open_document() {
	let message = DocumentsMessage::OpenDocument;
	dispatch(message);
}

#[wasm_bindgen]
pub fn open_document_file(name: String, content: String) {
	let message = DocumentsMessage::OpenDocumentFile(name, content);
	dispatch(message);
}

#[wasm_bindgen]
pub fn save_document() {
	let message = DocumentMessage::SaveDocument;
	dispatch(message);
}

#[wasm_bindgen]
pub fn close_document(document: usize) {
	let message = DocumentsMessage::CloseDocument(document);
	dispatch(message);
}

#[wasm_bindgen]
pub fn close_all_documents() {
	let message = DocumentsMessage::CloseAllDocuments;
	dispatch(message);
}

#[wasm_bindgen]
pub fn close_active_document_with_confirmation() {
	let message = DocumentsMessage::CloseActiveDocumentWithConfirmation;
	dispatch(message);
}

#[wasm_bindgen]
pub fn close_all_documents_with_confirmation() {
	let message = DocumentsMessage::CloseAllDocumentsWithConfirmation;
	dispatch(message);
}

#[wasm_bindgen]
pub fn request_about_graphite_dialog() {
	let message = DocumentsMessage::RequestAboutGraphiteDialog;
	dispatch(message);
}

/// Send new bounds when document panel viewports get resized or moved within the editor
/// [left, top, right, bottom]...
#[wasm_bindgen]
pub fn bounds_of_viewports(bounds_of_viewports: &[f64]) {
	let chunked: Vec<_> = bounds_of_viewports.chunks(4).map(ViewportBounds::from_slice).collect();

	let message = InputPreprocessorMessage::BoundsOfViewports(chunked);
	dispatch(message);
}

/// Mouse movement within the screenspace bounds of the viewport
#[wasm_bindgen]
pub fn on_mouse_move(x: f64, y: f64, mouse_keys: u8, modifiers: u8) {
	let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());

	let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

	let message = InputPreprocessorMessage::MouseMove(editor_mouse_state, modifier_keys);
	dispatch(message);
}

/// Mouse scrolling within the screenspace bounds of the viewport
#[wasm_bindgen]
pub fn on_mouse_scroll(x: f64, y: f64, mouse_keys: u8, wheel_delta_x: i32, wheel_delta_y: i32, wheel_delta_z: i32, modifiers: u8) {
	let mut editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());
	editor_mouse_state.scroll_delta = ScrollDelta::new(wheel_delta_x, wheel_delta_y, wheel_delta_z);

	let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

	let message = InputPreprocessorMessage::MouseScroll(editor_mouse_state, modifier_keys);
	dispatch(message);
}

/// A mouse button depressed within screenspace the bounds of the viewport
#[wasm_bindgen]
pub fn on_mouse_down(x: f64, y: f64, mouse_keys: u8, modifiers: u8) {
	let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());

	let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

	let message = InputPreprocessorMessage::MouseDown(editor_mouse_state, modifier_keys);
	dispatch(message);
}

/// A mouse button released
#[wasm_bindgen]
pub fn on_mouse_up(x: f64, y: f64, mouse_keys: u8, modifiers: u8) {
	let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());

	let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

	let message = InputPreprocessorMessage::MouseUp(editor_mouse_state, modifier_keys);
	dispatch(message);
}

/// A keyboard button depressed within screenspace the bounds of the viewport
#[wasm_bindgen]
pub fn on_key_down(name: String, modifiers: u8) {
	let key = translate_key(&name);
	let modifiers = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

	log::trace!("Key down {:?}, name: {}, modifiers: {:?}", key, name, modifiers);

	let message = InputPreprocessorMessage::KeyDown(key, modifiers);
	dispatch(message);
}

/// A keyboard button released
#[wasm_bindgen]
pub fn on_key_up(name: String, modifiers: u8) {
	let key = translate_key(&name);
	let modifiers = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

	log::trace!("Key up {:?}, name: {}, modifiers: {:?}", key, name, modifiers);

	let message = InputPreprocessorMessage::KeyUp(key, modifiers);
	dispatch(message);
}

/// Update primary color
#[wasm_bindgen]
pub fn update_primary_color(red: f32, green: f32, blue: f32, alpha: f32) -> Result<(), JsValue> {
	let primary_color = match Color::from_rgbaf32(red, green, blue, alpha) {
		Some(color) => color,
		None => return Err(Error::new("Invalid color").into()),
	};

	let message = ToolMessage::SelectPrimaryColor(primary_color);
	dispatch(message);

	Ok(())
}

/// Update secondary color
#[wasm_bindgen]
pub fn update_secondary_color(red: f32, green: f32, blue: f32, alpha: f32) -> Result<(), JsValue> {
	let secondary_color = match Color::from_rgbaf32(red, green, blue, alpha) {
		Some(color) => color,
		None => return Err(Error::new("Invalid color").into()),
	};

	let message = ToolMessage::SelectSecondaryColor(secondary_color);
	dispatch(message);

	Ok(())
}

/// Swap primary and secondary color
#[wasm_bindgen]
pub fn swap_colors() {
	let message = ToolMessage::SwapColors;
	dispatch(message);
}

/// Reset primary and secondary colors to their defaults
#[wasm_bindgen]
pub fn reset_colors() {
	let message = ToolMessage::ResetColors;
	dispatch(message);
}

/// Undo history one step
#[wasm_bindgen]
pub fn undo() {
	let message = DocumentMessage::Undo;
	dispatch(message);
}

/// Redo history one step
#[wasm_bindgen]
pub fn redo() {
	let message = DocumentMessage::Redo;
	dispatch(message);
}

/// Select all layers
#[wasm_bindgen]
pub fn select_all_layers() {
	let message = DocumentMessage::SelectAllLayers;
	dispatch(message);
}

/// Deselect all layers
#[wasm_bindgen]
pub fn deselect_all_layers() {
	let message = DocumentMessage::DeselectAllLayers;
	dispatch(message);
}

/// Reorder selected layer
#[wasm_bindgen]
pub fn reorder_selected_layers(delta: i32) {
	let message = DocumentMessage::ReorderSelectedLayers(delta);
	dispatch(message);
}

/// Set the blend mode for the selected layers
#[wasm_bindgen]
pub fn set_blend_mode_for_selected_layers(blend_mode_svg_style_name: String) -> Result<(), JsValue> {
	let blend_mode = translate_blend_mode(blend_mode_svg_style_name.as_str());

	match blend_mode {
		Some(mode) => {
			let message = DocumentMessage::SetBlendModeForSelectedLayers(mode);
			dispatch(message);

			Ok(())
		}
		None => Err(Error::new(&EditorError::Misc("UnknownBlendMode".to_string()).to_string()).into()),
	}
}

/// Set the opacity for the selected layers
#[wasm_bindgen]
pub fn set_opacity_for_selected_layers(opacity_percent: f64) {
	let message = DocumentMessage::SetOpacityForSelectedLayers(opacity_percent / 100.);
	dispatch(message);
}

/// Export the document
#[wasm_bindgen]
pub fn export_document() {
	let message = DocumentMessage::ExportDocument;
	dispatch(message);
}

/// Set snapping disabled / enabled
#[wasm_bindgen]
pub fn set_snapping(new_status: bool) {
	let message = DocumentMessage::SetSnapping(new_status);
	dispatch(message);
}

/// Swap between view modes
#[wasm_bindgen]
pub fn set_view_mode(new_mode: String) -> Result<(), JsValue> {
	match translate_view_mode(new_mode.as_str()) {
		Some(mode) => dispatch(DocumentMessage::DispatchOperation(Box::from(Operation::SetViewMode { mode }))),
		None => return Err(Error::new("Invalid view mode").into()),
	};
	Ok(())
}

/// Sets the zoom to the value
#[wasm_bindgen]
pub fn set_canvas_zoom(new_zoom: f64) {
	let message = MovementMessage::SetCanvasZoom(new_zoom);
	dispatch(message);
}

/// Zoom in to the next step
#[wasm_bindgen]
pub fn increase_canvas_zoom() {
	let message = MovementMessage::IncreaseCanvasZoom;
	dispatch(message);
}

/// Zoom out to the next step
#[wasm_bindgen]
pub fn decrease_canvas_zoom() {
	let message = MovementMessage::DecreaseCanvasZoom;
	dispatch(message);
}

/// Sets the rotation to the new value (in radians)
#[wasm_bindgen]
pub fn set_rotation(new_radians: f64) {
	let message = MovementMessage::SetCanvasRotation(new_radians);
	dispatch(message);
}

/// Translates document (in viewport coords)
#[wasm_bindgen]
pub fn translate_canvas(delta_x: f64, delta_y: f64) {
	let message = MovementMessage::TranslateCanvas((delta_x, delta_y).into());
	dispatch(message);
}

/// Translates document (in viewport coords)
#[wasm_bindgen]
pub fn translate_canvas_by_fraction(delta_x: f64, delta_y: f64) {
	let message = MovementMessage::TranslateCanvasByViewportFraction((delta_x, delta_y).into());
	dispatch(message);
}

/// Update the list of selected layers. The layer paths have to be stored in one array and are separated by LayerId::MAX
#[wasm_bindgen]
pub fn select_layers(paths: Vec<LayerId>) {
	let paths = paths.split(|id| *id == LayerId::MAX).map(|path| path.to_vec()).collect();

	let message = DocumentMessage::SetSelectedLayers(paths);
	dispatch(message);
}

/// Toggle visibility of a layer from the layer list
#[wasm_bindgen]
pub fn toggle_layer_visibility(path: Vec<LayerId>) {
	let message = DocumentMessage::ToggleLayerVisibility(path);
	dispatch(message);
}

/// Toggle expansions state of a layer from the layer list
#[wasm_bindgen]
pub fn toggle_layer_expansion(path: Vec<LayerId>) {
	let message = DocumentMessage::ToggleLayerExpansion(path);
	dispatch(message);
}

/// Renames a layer from the layer list
#[wasm_bindgen]
pub fn rename_layer(path: Vec<LayerId>, new_name: String) {
	let message = DocumentMessage::RenameLayer(path, new_name);
	dispatch(message);
}

/// Deletes a layer from the layer list
#[wasm_bindgen]
pub fn delete_layer(path: Vec<LayerId>) {
	let message = DocumentMessage::DeleteLayer(path);
	dispatch(message);
}

/// Requests the backend to add a layer to the layer list
#[wasm_bindgen]
pub fn add_folder(path: Vec<LayerId>) {
	let message = DocumentMessage::CreateFolder(path);
	dispatch(message);
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
