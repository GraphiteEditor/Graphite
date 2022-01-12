use crate::document::{LayerPanelEntry, RawBuffer};
use crate::message_prelude::*;
use crate::misc::HintData;
use crate::tool::tool_options::ToolOptions;
use crate::Color;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Clone, Deserialize, Serialize, Debug)]
pub struct FrontendDocumentDetails {
	pub is_saved: bool,
	pub name: String,
	pub id: u64,
}

#[impl_message(Message, Frontend)]
#[derive(PartialEq, Clone, Deserialize, Serialize, Debug)]
pub enum FrontendMessage {
	// Display prefix: make the frontend show something, like a dialog
	// Update prefix: give the frontend a new value or state for it to use
	// Trigger prefix: cause a browser API to do something
	DisplayDocumentLayerTreeStructure { data_buffer: RawBuffer },
	UpdateActiveTool { tool_name: String, tool_options: Option<ToolOptions> },
	UpdateActiveDocument { document_id: u64 },
	UpdateOpenDocumentsList { open_documents: Vec<FrontendDocumentDetails> },
	UpdateInputHints { hint_data: HintData },
	DisplayDialogError { title: String, description: String },
	DisplayDialogPanic { panic_info: String, title: String, description: String },
	DisplayConfirmationToCloseDocument { document_id: u64 },
	DisplayConfirmationToCloseAllDocuments,
	DisplayDialogAboutGraphite,
	UpdateDocumentLayer { data: LayerPanelEntry },
	UpdateDocumentArtwork { svg: String },
	UpdateDocumentOverlays { svg: String },
	UpdateDocumentArtboards { svg: String },
	UpdateDocumentScrollbars { position: (f64, f64), size: (f64, f64), multiplier: (f64, f64) },
	UpdateDocumentRulers { origin: (f64, f64), spacing: f64, interval: f64 },
	TriggerFileUpload,
	TriggerFileDownload { document: String, name: String },
	TriggerIndexedDbWriteDocument { document: String, details: FrontendDocumentDetails, version: String },
	TriggerIndexedDbRemoveDocument { document_id: u64 },
	UpdateWorkingColors { primary: Color, secondary: Color },
	UpdateCanvasZoom { factor: f64 },
	UpdateCanvasRotation { angle_radians: f64 },
}
