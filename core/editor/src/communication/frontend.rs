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
