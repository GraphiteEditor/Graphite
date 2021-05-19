use wasm_bindgen::prelude::*;

type DocumentId = u32;

/// Modify the active Document in the editor state store
#[wasm_bindgen]
pub fn set_active_document(document_id: DocumentId) {
	todo!("set_active_document {}", document_id)
}

/// Query the name of a specific document
#[wasm_bindgen]
pub fn get_document_name(document_id: DocumentId) -> String {
	todo!("get_document_name {}", document_id)
}

/// Query the id of the most recently interacted with document
#[wasm_bindgen]
pub fn get_active_document() -> DocumentId {
	todo!("get_active_document")
}

/*use editor_core::workspace::PanelId;
/// Notify the editor that the mouse hovers above a panel
#[wasm_bindgen]
pub fn panel_hover_enter(panel_id: PanelId) {
	todo!("panel_hover_enter {}", panel_id)
}*/

/// Query a list of currently available operations
#[wasm_bindgen]
pub fn get_available_operations() -> Vec<JsValue> {
	todo!("get_available_operations")
	// vec!["example1", "example2"].into_iter().map(JsValue::from).collect()
}

/*
/// Load a new .gdd file into the editor
/// Returns a unique document identifier
#[wasm_bindgen]
pub fn load_document(raw_data: &[u8]) -> DocumentId {
	todo!()
}*/
