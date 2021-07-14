use crate::shims::Error;
use crate::wrappers::{translate_key, translate_tool, Color};
use crate::EDITOR_STATE;
use editor_core::input::input_preprocessor::ModifierKeys;
use editor_core::input::mouse::ScrollDelta;
use editor_core::message_prelude::*;
use editor_core::{
	input::mouse::{MouseState, ViewportPosition},
	LayerId,
};
use wasm_bindgen::prelude::*;

fn convert_error(err: editor_core::EditorError) -> JsValue {
	Error::new(&err.to_string()).into()
}

fn dispatch<T: Into<Message>>(message: T) -> Result<(), JsValue> {
	EDITOR_STATE.with(|editor| editor.borrow_mut().handle_message(message).map(|responses| crate::handle_responses(responses)).map_err(convert_error))
}

/// Modify the currently selected tool in the document state store
#[wasm_bindgen]
pub fn select_tool(tool: String) -> Result<(), JsValue> {
	match translate_tool(&tool) {
		Some(tool) => dispatch(ToolMessage::SelectTool(tool)),
		None => Err(Error::new(&format!("Couldn't select {} because it was not recognized as a valid tool", tool)).into()),
	}
}

#[wasm_bindgen]
pub fn select_document(document: usize) -> Result<(), JsValue> {
	dispatch(DocumentMessage::SelectDocument(document))
}

#[wasm_bindgen]
pub fn close_document(document: usize) -> Result<(), JsValue> {
	dispatch(DocumentMessage::CloseDocument(document))
}

#[wasm_bindgen]
pub fn new_document() -> Result<(), JsValue> {
	dispatch(DocumentMessage::NewDocument)
}

// TODO: Call event when the panels are resized
/// Viewport resized
#[wasm_bindgen]
pub fn viewport_resize(new_width: u32, new_height: u32) -> Result<(), JsValue> {
	let ev = InputPreprocessorMessage::ViewportResize(ViewportPosition { x: new_width, y: new_height });
	dispatch(ev)
}

// TODO: When a mouse button is down that started in the viewport, this should trigger even when the mouse is outside the viewport (or even the browser window if the browser supports it)
/// Mouse movement within the screenspace bounds of the viewport
#[wasm_bindgen]
pub fn on_mouse_move(x: u32, y: u32, modifiers: u8) -> Result<(), JsValue> {
	let mods = ModifierKeys::from_bits(modifiers).expect("invalid modifier keys");
	// TODO: Convert these screenspace viewport coordinates to canvas coordinates based on the current zoom and pan
	let ev = InputPreprocessorMessage::MouseMove(ViewportPosition { x, y }, mods);
	dispatch(ev)
}

/// Mouse scrolling within the screenspace bounds of the viewport
#[wasm_bindgen]
pub fn on_mouse_scroll(delta_x: i32, delta_y: i32, delta_z: i32, modifiers: u8) -> Result<(), JsValue> {
	// TODO: Convert these screenspace viewport coordinates to canvas coordinates based on the current zoom and pan
	let mods = ModifierKeys::from_bits(modifiers).expect("invalid modifier keys");
	let ev = InputPreprocessorMessage::MouseScroll(ScrollDelta::new(delta_x, delta_y, delta_z), mods);
	dispatch(ev)
}

/// A mouse button depressed within screenspace the bounds of the viewport
#[wasm_bindgen]
pub fn on_mouse_down(x: u32, y: u32, mouse_keys: u8, modifiers: u8) -> Result<(), JsValue> {
	let pos = ViewportPosition { x, y };
	let mods = ModifierKeys::from_bits(modifiers).expect("invalid modifier keys");
	let ev = InputPreprocessorMessage::MouseDown(MouseState::from_u8_pos(mouse_keys, pos), mods);
	dispatch(ev)
}

/// A mouse button released
#[wasm_bindgen]
pub fn on_mouse_up(x: u32, y: u32, mouse_keys: u8, modifiers: u8) -> Result<(), JsValue> {
	let pos = ViewportPosition { x, y };
	let mods = ModifierKeys::from_bits(modifiers).expect("invalid modifier keys");
	let ev = InputPreprocessorMessage::MouseUp(MouseState::from_u8_pos(mouse_keys, pos), mods);
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

/// Select all layers
#[wasm_bindgen]
pub fn deselect_all_layers() -> Result<(), JsValue> {
	dispatch(DocumentMessage::DeselectAllLayers)
}

/// Export the document
#[wasm_bindgen]
pub fn export_document() -> Result<(), JsValue> {
	dispatch(DocumentMessage::ExportDocument)
}

/// Sets the zoom to the value
#[wasm_bindgen]
pub fn set_zoom(new_zoom: f64) -> Result<(), JsValue> {
	let ev = DocumentMessage::SetCanvasZoom(new_zoom);
	dispatch(ev)
}

/// Sets the rotation to the new value (in radians)
#[wasm_bindgen]
pub fn set_rotation(new_radians: f64) -> Result<(), JsValue> {
	let ev = DocumentMessage::SetRotation(new_radians);
	dispatch(ev)
}

/// Update the list of selected layers. The layer paths have to be stored in one array and are separated by LayerId::MAX
#[wasm_bindgen]
pub fn select_layers(paths: Vec<LayerId>) -> Result<(), JsValue> {
	let paths = paths.split(|id| *id == LayerId::MAX).map(|path| path.to_vec()).collect();
	dispatch(DocumentMessage::SelectLayers(paths))
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

///  Renames a layer from the layer list
#[wasm_bindgen]
pub fn rename_layer(path: Vec<LayerId>, new_name: String) -> Result<(), JsValue> {
	dispatch(DocumentMessage::RenameLayer(path, new_name))
}

///  Deletes a layer from the layer list
#[wasm_bindgen]
pub fn delete_layer(path: Vec<LayerId>) -> Result<(), JsValue> {
	dispatch(DocumentMessage::DeleteLayer(path))
}

///  Requests the backend to add a layer to the layer list
#[wasm_bindgen]
pub fn add_folder(path: Vec<LayerId>) -> Result<(), JsValue> {
	dispatch(DocumentMessage::AddFolder(path))
}
