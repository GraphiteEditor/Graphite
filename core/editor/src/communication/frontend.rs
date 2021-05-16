use super::{AsMessage, Message, MessageDiscriminant};
use crate::communication::events::ToolResponse;
use document_core::DocumentResponse;
use proc_macros::MessageImpl;
use serde::{Deserialize, Serialize};

#[derive(MessageImpl, PartialEq, Clone, Deserialize, Serialize)]
#[message(Message, Message, Frontend)]
pub enum FrontendMessage {
	Document(DocumentResponse),
	Tool(ToolResponse),
}

impl From<DocumentResponse> for Message {
	fn from(response: DocumentResponse) -> Self {
		let frontend: FrontendMessage = response.into();
		frontend.into()
	}
}

impl From<DocumentResponse> for FrontendMessage {
	fn from(response: DocumentResponse) -> Self {
		Self::Document(response)
	}
}

impl From<ToolResponse> for Message {
	fn from(response: ToolResponse) -> Self {
		let frontend: FrontendMessage = response.into();
		frontend.into()
	}
}

impl From<ToolResponse> for FrontendMessage {
	fn from(response: ToolResponse) -> Self {
		Self::Tool(response)
	}
}
