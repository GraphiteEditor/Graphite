use crate::document::layer_panel::{LayerPanelEntry, Path};
use crate::message_prelude::*;
use crate::tool::tool_options::ToolOptions;
use crate::Color;
use serde::{Deserialize, Serialize};

#[impl_message(Message, Frontend)]
#[derive(PartialEq, Clone, Deserialize, Serialize, Debug)]
pub enum FrontendMessage {
	CollapseFolder { path: Path },
	ExpandFolder { path: Path, children: Vec<LayerPanelEntry> },
	SetActiveTool { tool_name: String, tool_options: Option<ToolOptions> },
	SetActiveDocument { document_index: usize },
	UpdateOpenDocumentsList { open_documents: Vec<String> },
	DisplayError { description: String },
	DisplayConfirmationToCloseDocument { document_index: usize },
	DisplayConfirmationToCloseAllDocuments,
	UpdateCanvas { document: String },
	UpdateScrollbars { position: (f64, f64), size: (f64, f64), multiplier: (f64, f64) },
	UpdateLayer { path: Path, data: LayerPanelEntry },
	ExportDocument { document: String, name: String },
	SaveDocument { document: String, name: String },
	OpenDocumentBrowse,
	EnableTextInput,
	DisableTextInput,
	UpdateWorkingColors { primary: Color, secondary: Color },
	SetCanvasZoom { new_zoom: f64 },
	SetCanvasRotation { new_radians: f64 },
}
