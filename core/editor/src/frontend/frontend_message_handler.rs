use crate::message_prelude::*;
use document_core::{response::LayerPanelEntry, DocumentResponse, LayerId};
use serde::{Deserialize, Serialize};

pub type Callback = Box<dyn Fn(FrontendMessage)>;

#[impl_message(Message, Frontend)]
#[derive(PartialEq, Clone, Deserialize, Serialize, Debug)]
pub enum FrontendMessage {
	CollapseFolder { path: Vec<LayerId> },
	ExpandFolder { path: Vec<LayerId>, children: Vec<LayerPanelEntry> },
	SetActiveTool { tool_name: String },
	SetActiveDocument { document_index: usize },
	UpdateCanvas { document: String },
	ExportDocument { document: String },
	EnableTextInput,
	DisableTextInput,
}

impl From<DocumentResponse> for Message {
	fn from(response: DocumentResponse) -> Self {
		let frontend: FrontendMessage = response.into();
		frontend.into()
	}
}
impl From<DocumentResponse> for FrontendMessage {
	fn from(response: DocumentResponse) -> Self {
		match response {
			DocumentResponse::ExpandFolder { path, children } => Self::ExpandFolder { path, children },
			DocumentResponse::CollapseFolder { path } => Self::CollapseFolder { path },
			_ => unimplemented!("The frontend does not handle {:?}", response),
		}
	}
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
		log::trace!("Sending {} Response", message.to_discriminant().global_name());
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
