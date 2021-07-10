use crate::shims::Error;
use crate::wrappers::{translate_key, translate_tool, Color};
use crate::EDITOR_STATE;
use editor_core::message_prelude::*;
use editor_core::{
	input::mouse::{MouseState, ViewportPosition},
	LayerId,
};
use wasm_bindgen::prelude::*;

fn convert_error(err: editor_core::EditorError) -> JsValue {
	Error::new(&err.to_string()).into()
}

/// Modify the currently selected tool in the document state store
#[wasm_bindgen]
pub fn select_tool(tool: String) -> Result<(), JsValue> {
	EDITOR_STATE.with(|editor| match translate_tool(&tool) {
		Some(tool) => editor.borrow_mut().handle_message(ToolMessage::SelectTool(tool)).map_err(convert_error),
		None => Err(Error::new(&format!("Couldn't select {} because it was not recognized as a valid tool", tool)).into()),
	})
}

#[wasm_bindgen]
pub fn select_document(document: usize) -> Result<(), JsValue> {
	EDITOR_STATE.with(|editor| editor.borrow_mut().handle_message(DocumentMessage::SelectDocument(document)).map_err(convert_error))
}

#[wasm_bindgen]
pub fn close_document(document: usize) -> Result<(), JsValue> {
	EDITOR_STATE.with(|editor| editor.borrow_mut().handle_message(DocumentMessage::CloseDocument(document)).map_err(convert_error))
}

#[wasm_bindgen]
pub fn new_document() -> Result<(), JsValue> {
	EDITOR_STATE.with(|editor| editor.borrow_mut().handle_message(DocumentMessage::NewDocument).map_err(convert_error))
}

// TODO: When a mouse button is down that started in the viewport, this should trigger even when the mouse is outside the viewport (or even the browser window if the browser supports it)
/// Mouse movement within the screenspace bounds of the viewport
#[wasm_bindgen]
pub fn on_mouse_move(x: u32, y: u32) -> Result<(), JsValue> {
	// TODO: Convert these screenspace viewport coordinates to canvas coordinates based on the current zoom and pan
	let ev = InputPreprocessorMessage::MouseMove(ViewportPosition { x, y });
	EDITOR_STATE.with(|editor| editor.borrow_mut().handle_message(ev)).map_err(convert_error)
}

/// A mouse button depressed within screenspace the bounds of the viewport
#[wasm_bindgen]
pub fn on_mouse_down(x: u32, y: u32, mouse_keys: u8) -> Result<(), JsValue> {
	let pos = ViewportPosition { x, y };
	let ev = InputPreprocessorMessage::MouseDown(MouseState::from_u8_pos(mouse_keys, pos));
	EDITOR_STATE.with(|editor| editor.borrow_mut().handle_message(ev)).map_err(convert_error)
}

/// A mouse button released
#[wasm_bindgen]
pub fn on_mouse_up(x: u32, y: u32, mouse_keys: u8) -> Result<(), JsValue> {
	let pos = ViewportPosition { x, y };
	let ev = InputPreprocessorMessage::MouseUp(MouseState::from_u8_pos(mouse_keys, pos));
	EDITOR_STATE.with(|editor| editor.borrow_mut().handle_message(ev)).map_err(convert_error)
}

/// A keyboard button depressed within screenspace the bounds of the viewport
#[wasm_bindgen]
pub fn on_key_down(name: String) -> Result<(), JsValue> {
	let key = translate_key(&name);
	log::trace!("key down {:?}, name: {}", key, name);
	let ev = InputPreprocessorMessage::KeyDown(key);
	EDITOR_STATE.with(|editor| editor.borrow_mut().handle_message(ev)).map_err(convert_error)
}

/// A keyboard button released
#[wasm_bindgen]
pub fn on_key_up(name: String) -> Result<(), JsValue> {
	let key = translate_key(&name);
	log::trace!("key up {:?}, name: {}", key, name);
	let ev = InputPreprocessorMessage::KeyUp(key);
	EDITOR_STATE.with(|editor| editor.borrow_mut().handle_message(ev)).map_err(convert_error)
}

/// Update primary color
#[wasm_bindgen]
pub fn update_primary_color(primary_color: Color) -> Result<(), JsValue> {
	EDITOR_STATE
		.with(|editor| editor.borrow_mut().handle_message(ToolMessage::SelectPrimaryColor(primary_color.inner())))
		.map_err(convert_error)
}

/// Update secondary color
#[wasm_bindgen]
pub fn update_secondary_color(secondary_color: Color) -> Result<(), JsValue> {
	EDITOR_STATE
		.with(|editor| editor.borrow_mut().handle_message(ToolMessage::SelectSecondaryColor(secondary_color.inner())))
		.map_err(convert_error)
}

/// Swap primary and secondary color
#[wasm_bindgen]
pub fn swap_colors() -> Result<(), JsValue> {
	EDITOR_STATE.with(|editor| editor.borrow_mut().handle_message(ToolMessage::SwapColors)).map_err(convert_error)
}

/// Reset primary and secondary colors to their defaults
#[wasm_bindgen]
pub fn reset_colors() -> Result<(), JsValue> {
	EDITOR_STATE.with(|editor| editor.borrow_mut().handle_message(ToolMessage::ResetColors)).map_err(convert_error)
}

/// Undo history one step
#[wasm_bindgen]
pub fn undo() -> Result<(), JsValue> {
	EDITOR_STATE.with(|editor| editor.borrow_mut().handle_message(DocumentMessage::Undo)).map_err(convert_error)
}

/// Select all layers
#[wasm_bindgen]
pub fn select_all_layers() -> Result<(), JsValue> {
	EDITOR_STATE.with(|editor| editor.borrow_mut().handle_message(DocumentMessage::SelectAllLayers)).map_err(convert_error)
}

/// Export the document
#[wasm_bindgen]
pub fn export_document() -> Result<(), JsValue> {
	EDITOR_STATE.with(|editor| editor.borrow_mut().handle_message(DocumentMessage::ExportDocument)).map_err(convert_error)
}

/// Update the list of selected layers. The layer paths have to be stored in one array and are separated by LayerId::MAX
#[wasm_bindgen]
pub fn select_layers(paths: Vec<LayerId>) -> Result<(), JsValue> {
	let paths = paths.split(|id| *id == LayerId::MAX).map(|path| path.to_vec()).collect();
	EDITOR_STATE
		.with(|editor| editor.borrow_mut().handle_message(DocumentMessage::SelectLayers(paths)))
		.map_err(convert_error)
}

/// Toggle visibility of a layer from the layer list
#[wasm_bindgen]
pub fn toggle_layer_visibility(path: Vec<LayerId>) -> Result<(), JsValue> {
	EDITOR_STATE
		.with(|editor| editor.borrow_mut().handle_message(DocumentMessage::ToggleLayerVisibility(path)))
		.map_err(convert_error)
}

/// Toggle expansions state of a layer from the layer list
#[wasm_bindgen]
pub fn toggle_layer_expansion(path: Vec<LayerId>) -> Result<(), JsValue> {
	EDITOR_STATE
		.with(|editor| editor.borrow_mut().handle_message(DocumentMessage::ToggleLayerExpansion(path)))
		.map_err(convert_error)
}

///  Renames a layer from the layer list
#[wasm_bindgen]
pub fn rename_layer(path: Vec<LayerId>, new_name: String) -> Result<(), JsValue> {
	EDITOR_STATE
		.with(|editor| editor.borrow_mut().handle_message(DocumentMessage::RenameLayer(path, new_name)))
		.map_err(convert_error)
}

///  Deletes a layer from the layer list
#[wasm_bindgen]
pub fn delete_layer(path: Vec<LayerId>) -> Result<(), JsValue> {
	EDITOR_STATE
		.with(|editor| editor.borrow_mut().handle_message(DocumentMessage::DeleteLayer(path)))
		.map_err(convert_error)
}

///  Requests the backend to add a layer to the layer list
#[wasm_bindgen]
pub fn add_folder(path: Vec<LayerId>) -> Result<(), JsValue> {
	EDITOR_STATE.with(|editor| editor.borrow_mut().handle_message(DocumentMessage::AddFolder(path))).map_err(convert_error)
}
