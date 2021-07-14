use crate::frontend::layer_panel::LayerPanelEntry;
use crate::message_prelude::*;
use crate::Color;
use serde::{Deserialize, Serialize};

#[impl_message(Message, Frontend)]
#[derive(PartialEq, Clone, Deserialize, Serialize, Debug)]
pub enum FrontendMessage {
	CollapseFolder { path: Vec<LayerId> },
	ExpandFolder { path: Vec<LayerId>, children: Vec<LayerPanelEntry> },
	SetActiveTool { tool_name: String },
	SetActiveDocument { document_index: usize },
	CloseDocument { document_index: usize },
	NewDocument { document_name: String },
	UpdateCanvas { document: String },
	ExportDocument { document: String },
	EnableTextInput,
	DisableTextInput,
	UpdateWorkingColors { primary: Color, secondary: Color },
	PromptCloseConfirmationModal,
	SetCanvasZoom { new_zoom: f64 },
	SetRotation { new_radians: f64 },
}
