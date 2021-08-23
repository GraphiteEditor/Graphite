use crate::shims::Error;
use crate::wrappers::{translate_key, translate_tool, Color};
use editor::input::input_preprocessor::ModifierKeys;
use editor::input::mouse::{EditorMouseState, ScrollDelta, ViewportBounds};
use editor::message_prelude::*;
use editor::misc::EditorError;
use editor::tool::{tool_options::ToolOptions, tools, ToolType};
use editor::LayerId;
use graphene::layers::BlendMode;
use wasm_bindgen::prelude::*;

fn convert_error(err: editor::EditorError) -> JsValue {
	Error::new(&err.to_string()).into()
}

fn dispatch<T: Into<Message>>(message: T) -> Result<(), JsValue> {
	let result = crate::EDITOR_STATE.with(|state| state.borrow_mut().handle_message(message.into()));
	if let Ok(messages) = result {
		crate::handle_responses(messages);
	}
	Ok(())
}

/// Modify the currently selected tool in the document state store
#[wasm_bindgen]
pub fn select_tool(tool: String) -> Result<(), JsValue> {
	match translate_tool(&tool) {
		Some(tool) => dispatch(ToolMessage::ActivateTool(tool)),
		None => Err(Error::new(&format!("Couldn't select {} because it was not recognized as a valid tool", tool)).into()),
	}
}

/// Update the options for a given tool
#[wasm_bindgen]
pub fn set_tool_options(tool: String, options: &JsValue) -> Result<(), JsValue> {
	match options.into_serde::<ToolOptions>() {
		Ok(options) => match translate_tool(&tool) {
			Some(tool) => dispatch(ToolMessage::SetToolOptions(tool, options)),
			None => Err(Error::new(&format!("Couldn't set options for {} because it was not recognized as a valid tool", tool)).into()),
		},
		Err(err) => Err(Error::new(&format!("Invalid JSON for ToolOptions: {}", err)).into()),
	}
}

/// Send a message to a given tool
#[wasm_bindgen]
pub fn send_tool_message(tool: String, message: &JsValue) -> Result<(), JsValue> {
	let tool_message = match translate_tool(&tool) {
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
		Ok(tool_message) => dispatch(tool_message),
		Err(err) => Err(err),
	}
}

#[wasm_bindgen]
pub fn select_document(document: usize) -> Result<(), JsValue> {
	dispatch(DocumentsMessage::SelectDocument(document))
}

#[wasm_bindgen]
pub fn get_open_documents_list() -> Result<(), JsValue> {
	dispatch(DocumentsMessage::GetOpenDocumentsList)
}

#[wasm_bindgen]
pub fn new_document() -> Result<(), JsValue> {
	dispatch(DocumentsMessage::NewDocument)
}

#[wasm_bindgen]
pub fn open_document() -> Result<(), JsValue> {
	dispatch(DocumentsMessage::OpenDocument)
}

#[wasm_bindgen]
pub fn open_document_file(name: String, content: String) -> Result<(), JsValue> {
	dispatch(DocumentsMessage::OpenDocumentFile(name, content))
}

#[wasm_bindgen]
pub fn save_document() -> Result<(), JsValue> {
	dispatch(DocumentMessage::SaveDocument)
}

#[wasm_bindgen]
pub fn close_document(document: usize) -> Result<(), JsValue> {
	dispatch(DocumentsMessage::CloseDocument(document))
}

#[wasm_bindgen]
pub fn close_all_documents() -> Result<(), JsValue> {
	dispatch(DocumentsMessage::CloseAllDocuments)
}

#[wasm_bindgen]
pub fn close_active_document_with_confirmation() -> Result<(), JsValue> {
	dispatch(DocumentsMessage::CloseActiveDocumentWithConfirmation)
}

#[wasm_bindgen]
pub fn close_all_documents_with_confirmation() -> Result<(), JsValue> {
	dispatch(DocumentsMessage::CloseAllDocumentsWithConfirmation)
}

/// Send new bounds when document panel viewports get resized or moved within the editor
/// [left, top, right, bottom]...
#[wasm_bindgen]
pub fn bounds_of_viewports(bounds_of_viewports: &[f64]) -> Result<(), JsValue> {
	let chunked: Vec<_> = bounds_of_viewports.chunks(4).map(ViewportBounds::from_slice).collect();
	let ev = InputPreprocessorMessage::BoundsOfViewports(chunked);
	dispatch(ev)
}

/// Mouse movement within the screenspace bounds of the viewport
#[wasm_bindgen]
pub fn on_mouse_move(x: f64, y: f64, mouse_keys: u8, modifiers: u8) -> Result<(), JsValue> {
	let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());

	let modifier_keys = ModifierKeys::from_bits(modifiers).expect("invalid modifier keys");

	let ev = InputPreprocessorMessage::MouseMove(editor_mouse_state, modifier_keys);
	dispatch(ev)
}

/// Mouse scrolling within the screenspace bounds of the viewport
#[wasm_bindgen]
pub fn on_mouse_scroll(x: f64, y: f64, mouse_keys: u8, wheel_delta_x: i32, wheel_delta_y: i32, wheel_delta_z: i32, modifiers: u8) -> Result<(), JsValue> {
	let mut editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());
	editor_mouse_state.scroll_delta = ScrollDelta::new(wheel_delta_x, wheel_delta_y, wheel_delta_z);

	let modifier_keys = ModifierKeys::from_bits(modifiers).expect("invalid modifier keys");

	let ev = InputPreprocessorMessage::MouseScroll(editor_mouse_state, modifier_keys);
	dispatch(ev)
}

/// A mouse button depressed within screenspace the bounds of the viewport
#[wasm_bindgen]
pub fn on_mouse_down(x: f64, y: f64, mouse_keys: u8, modifiers: u8) -> Result<(), JsValue> {
	let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());

	let modifier_keys = ModifierKeys::from_bits(modifiers).expect("invalid modifier keys");

	let ev = InputPreprocessorMessage::MouseDown(editor_mouse_state, modifier_keys);
	dispatch(ev)
}

/// A mouse button released
#[wasm_bindgen]
pub fn on_mouse_up(x: f64, y: f64, mouse_keys: u8, modifiers: u8) -> Result<(), JsValue> {
	let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());

	let modifier_keys = ModifierKeys::from_bits(modifiers).expect("invalid modifier keys");

	let ev = InputPreprocessorMessage::MouseUp(editor_mouse_state, modifier_keys);
	dispatch(ev)
}

/// A keyboard button depressed within screenspace the bounds of the viewport
#[wasm_bindgen]
pub fn on_key_down(name: String, modifiers: u8) -> Result<(), JsValue> {
	let key = translate_key(&name);
	let mods = ModifierKeys::from_bits(modifiers).expect("invalid modifier keys");
	log::trace!("key down {:?}, name: {}, modifiers: {:?}", key, name, mods);
	let ev = InputPreprocessorMessage::KeyDown(key, mods);
	dispatch(ev)
}

/// A keyboard button released
#[wasm_bindgen]
pub fn on_key_up(name: String, modifiers: u8) -> Result<(), JsValue> {
	let key = translate_key(&name);
	let mods = ModifierKeys::from_bits(modifiers).expect("invalid modifier keys");
	log::trace!("key up {:?}, name: {}, modifiers: {:?}", key, name, mods);
	let ev = InputPreprocessorMessage::KeyUp(key, mods);
	dispatch(ev)
}

/// Update primary color
#[wasm_bindgen]
pub fn update_primary_color(primary_color: Color) -> Result<(), JsValue> {
	dispatch(ToolMessage::SelectPrimaryColor(primary_color.inner()))
}

/// Update secondary color
#[wasm_bindgen]
pub fn update_secondary_color(secondary_color: Color) -> Result<(), JsValue> {
	dispatch(ToolMessage::SelectSecondaryColor(secondary_color.inner()))
}

/// Swap primary and secondary color
#[wasm_bindgen]
pub fn swap_colors() -> Result<(), JsValue> {
	dispatch(ToolMessage::SwapColors)
}

/// Reset primary and secondary colors to their defaults
#[wasm_bindgen]
pub fn reset_colors() -> Result<(), JsValue> {
	dispatch(ToolMessage::ResetColors)
}

/// Undo history one step
#[wasm_bindgen]
pub fn undo() -> Result<(), JsValue> {
	dispatch(DocumentMessage::Undo)
}

/// Select all layers
#[wasm_bindgen]
pub fn select_all_layers() -> Result<(), JsValue> {
	dispatch(DocumentMessage::SelectAllLayers)
}

/// Deselect all layers
#[wasm_bindgen]
pub fn deselect_all_layers() -> Result<(), JsValue> {
	dispatch(DocumentMessage::DeselectAllLayers)
}

/// Reorder selected layer
#[wasm_bindgen]
pub fn reorder_selected_layers(delta: i32) -> Result<(), JsValue> {
	dispatch(DocumentMessage::ReorderSelectedLayers(delta))
}

/// Set the blend mode for the selected layers
#[wasm_bindgen]
pub fn set_blend_mode_for_selected_layers(blend_mode_svg_style_name: String) -> Result<(), JsValue> {
	let blend_mode = match blend_mode_svg_style_name.as_str() {
		"normal" => BlendMode::Normal,
		"multiply" => BlendMode::Multiply,
		"darken" => BlendMode::Darken,
		"color-burn" => BlendMode::ColorBurn,
		"screen" => BlendMode::Screen,
		"lighten" => BlendMode::Lighten,
		"color-dodge" => BlendMode::ColorDodge,
		"overlay" => BlendMode::Overlay,
		"soft-light" => BlendMode::SoftLight,
		"hard-light" => BlendMode::HardLight,
		"difference" => BlendMode::Difference,
		"exclusion" => BlendMode::Exclusion,
		"hue" => BlendMode::Hue,
		"saturation" => BlendMode::Saturation,
		"color" => BlendMode::Color,
		"luminosity" => BlendMode::Luminosity,
		_ => return Err(convert_error(EditorError::Misc("UnknownBlendMode".to_string()))),
	};

	dispatch(DocumentMessage::SetBlendModeForSelectedLayers(blend_mode))
}

/// Set the opacity for the selected layers
#[wasm_bindgen]
pub fn set_opacity_for_selected_layers(opacity_percent: f64) -> Result<(), JsValue> {
	dispatch(DocumentMessage::SetOpacityForSelectedLayers(opacity_percent / 100.))
}

/// Export the document
#[wasm_bindgen]
pub fn export_document() -> Result<(), JsValue> {
	dispatch(DocumentMessage::ExportDocument)
}

/// Sets the zoom to the value
#[wasm_bindgen]
pub fn set_canvas_zoom(new_zoom: f64) -> Result<(), JsValue> {
	let ev = MovementMessage::SetCanvasZoom(new_zoom);
	dispatch(ev)
}

/// Zoom in to the next step
#[wasm_bindgen]
pub fn increase_canvas_zoom() -> Result<(), JsValue> {
	let ev = MovementMessage::IncreaseCanvasZoom;
	dispatch(ev)
}

/// Zoom out to the next step
#[wasm_bindgen]
pub fn decrease_canvas_zoom() -> Result<(), JsValue> {
	let ev = MovementMessage::DecreaseCanvasZoom;
	dispatch(ev)
}

/// Sets the rotation to the new value (in radians)
#[wasm_bindgen]
pub fn set_rotation(new_radians: f64) -> Result<(), JsValue> {
	let ev = MovementMessage::SetCanvasRotation(new_radians);
	dispatch(ev)
}

/// Translates document (in viewport coords)
#[wasm_bindgen]
pub fn translate_canvas(delta_x: f64, delta_y: f64) -> Result<(), JsValue> {
	let ev = MovementMessage::TranslateCanvas((delta_x, delta_y).into());
	dispatch(ev)
}

/// Translates document (in viewport coords)
#[wasm_bindgen]
pub fn translate_canvas_by_fraction(delta_x: f64, delta_y: f64) -> Result<(), JsValue> {
	let ev = MovementMessage::TranslateCanvasByViewportFraction((delta_x, delta_y).into());
	dispatch(ev)
}

/// Update the list of selected layers. The layer paths have to be stored in one array and are separated by LayerId::MAX
#[wasm_bindgen]
pub fn select_layers(paths: Vec<LayerId>) -> Result<(), JsValue> {
	let paths = paths.split(|id| *id == LayerId::MAX).map(|path| path.to_vec()).collect();
	dispatch(DocumentMessage::SetSelectedLayers(paths))
}

/// Toggle visibility of a layer from the layer list
#[wasm_bindgen]
pub fn toggle_layer_visibility(path: Vec<LayerId>) -> Result<(), JsValue> {
	dispatch(DocumentMessage::ToggleLayerVisibility(path))
}

/// Toggle expansions state of a layer from the layer list
#[wasm_bindgen]
pub fn toggle_layer_expansion(path: Vec<LayerId>) -> Result<(), JsValue> {
	dispatch(DocumentMessage::ToggleLayerExpansion(path))
}

/// Renames a layer from the layer list
#[wasm_bindgen]
pub fn rename_layer(path: Vec<LayerId>, new_name: String) -> Result<(), JsValue> {
	dispatch(DocumentMessage::RenameLayer(path, new_name))
}

/// Deletes a layer from the layer list
#[wasm_bindgen]
pub fn delete_layer(path: Vec<LayerId>) -> Result<(), JsValue> {
	dispatch(DocumentMessage::DeleteLayer(path))
}

/// Requests the backend to add a layer to the layer list
#[wasm_bindgen]
pub fn add_folder(path: Vec<LayerId>) -> Result<(), JsValue> {
	dispatch(DocumentMessage::AddFolder(path))
}
