use crate::frontend::layer_panel::LayerPanelEntry;
use crate::message_prelude::*;
use serde::{Deserialize, Serialize};

pub type Callback = Box<dyn Fn(FrontendMessage)>;

#[impl_message(Message, Frontend)]
#[derive(PartialEq, Clone, Deserialize, Serialize, Debug)]
pub enum FrontendMessage {
	CollapseFolder { path: Vec<LayerId> },
	ExpandFolder { path: Vec<LayerId>, children: Vec<LayerPanelEntry> },
	SetActiveTool { tool_name: String },
	UpdateCanvas { document: String },
	ExportDocument { document: String },
	EnableTextInput,
	DisableTextInput,
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

		CollapseFolder,
		ExpandFolder,
		SetActiveTool,
		UpdateCanvas,
		EnableTextInput,
		DisableTextInput,
	);
}
