use crate::document::layer_panel::{LayerPanelEntry, RawBuffer};
use crate::message_prelude::*;
use crate::tool::tool_options::ToolOptions;
use crate::Color;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Clone, Deserialize, Serialize, Debug)]
pub struct FrontendDocumentState {
	pub is_saved: bool,
	pub name: String,
	pub id: i32,
}

#[impl_message(Message, Frontend)]
#[derive(PartialEq, Clone, Deserialize, Serialize, Debug)]
pub enum FrontendMessage {
	DisplayFolderTreeStructure { data_buffer: RawBuffer },
	SetActiveTool { tool_name: String, tool_options: Option<ToolOptions> },
	SetActiveDocument { document_id: i32 },
	UpdateOpenDocumentsList { open_documents: Vec<FrontendDocumentState> },
	DisplayError { title: String, description: String },
	DisplayPanic { panic_info: String, title: String, description: String },
	DisplayConfirmationToCloseDocument { document_id: i32 },
	DisplayConfirmationToCloseAllDocuments,
	UpdateLayer { data: LayerPanelEntry },
	UpdateCanvas { document: String },
	UpdateScrollbars { position: (f64, f64), size: (f64, f64), multiplier: (f64, f64) },
	UpdateRulers { origin: (f64, f64), spacing: f64, interval: f64 },
	ExportDocument { document: String, name: String },
	SaveDocument { document: String, name: String },
	OpenDocumentBrowse,
	EnableTextInput,
	DisableTextInput,
	UpdateWorkingColors { primary: Color, secondary: Color },
	SetCanvasZoom { new_zoom: f64 },
	SetCanvasRotation { new_radians: f64 },
}
