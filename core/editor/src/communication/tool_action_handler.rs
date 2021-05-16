use document_core::{color::Color, DocumentResponse, LayerId};
use proc_macros::MessageImpl;

use super::{AsMessage, Message, MessageDiscriminant, MessageHandler};
use crate::{
	events::ToolResponse,
	tools::{ToolFsmState, ToolType},
	SvgDocument,
};
use crate::{
	tools::{DocumentToolData, ToolActionHandlerData},
	EditorError,
};

#[derive(MessageImpl, PartialEq, Clone)]
#[message(Message, Message, Tool)]
pub enum ToolMessage {
	SelectTool(ToolType),
	SelectPrimaryColor(Color),
	SelectSecondaryColor(Color),
	Undo,
	Redo,
	Save,
}

#[derive(Debug, Default)]
pub struct ToolActionHandler {
	tool_state: ToolFsmState,
	actions: Vec<&'static [&'static str]>,
}
impl MessageHandler<ToolMessage, &mut SvgDocument> for ToolActionHandler {
	fn process_action(&mut self, action: ToolMessage, document: &mut SvgDocument, responses: &mut Vec<Message>) {}
	actions_fn!(
		ToolMessageDiscriminant::Undo,
		ToolMessageDiscriminant::Redo,
		ToolMessageDiscriminant::SelectSecondaryColor,
		ToolMessageDiscriminant::SelectPrimaryColor,
		ToolMessageDiscriminant::SelectTool
	);
}
