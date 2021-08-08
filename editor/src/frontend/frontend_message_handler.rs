use crate::document::layer_panel::Path;
use crate::frontend::layer_panel::LayerPanelEntry;
use crate::message_prelude::*;
use crate::Color;
use serde::{Deserialize, Serialize};

pub type Callback = Box<dyn Fn(FrontendMessage)>;

#[impl_message(Message, Frontend)]
#[derive(PartialEq, Clone, Deserialize, Serialize, Debug)]
pub enum FrontendMessage {
	CollapseFolder { path: Path },
	ExpandFolder { path: Path, children: Vec<LayerPanelEntry> },
	SetActiveTool { tool_name: String },
	SetActiveDocument { document_index: usize },
	UpdateOpenDocumentsList { open_documents: Vec<String> },
	DisplayError { description: String },
	DisplayConfirmationToCloseDocument { document_index: usize },
	DisplayConfirmationToCloseAllDocuments,
	UpdateCanvas { document: String },
	UpdateLayer { path: Path, data: LayerPanelEntry },
	ExportDocument { document: String },
	EnableTextInput,
	DisableTextInput,
	UpdateWorkingColors { primary: Color, secondary: Color },
	SetCanvasZoom { new_zoom: f64 },
	SetCanvasRotation { new_radians: f64 },
}

pub struct FrontendMessageHandler {
	callback: crate::Callback,
}

impl FrontendMessageHandler {
	pub fn new(callback: Callback) -> Self {
		Self { callback }
	}
}

impl MessageHandler<FrontendMessage, ()> for FrontendMessageHandler {
	fn process_action(&mut self, message: FrontendMessage, _data: (), _responses: &mut VecDeque<Message>) {
		(self.callback)(message)
	}
	advertise_actions!(
		FrontendMessageDiscriminant;

		DisplayError,
		CollapseFolder,
		ExpandFolder,
		SetActiveTool,
		UpdateCanvas,
		EnableTextInput,
		DisableTextInput,
		SetCanvasZoom,
		SetCanvasRotation,
	);
}
