use document_core::color::Color;

use super::{message::prelude::*, AsMessage, InputPreprocessor, Message, MessageDiscriminant, MessageHandler, MessageImpl};
use crate::{
	tools::{rectangle::RectangleMessage, ToolFsmState, ToolType},
	SvgDocument,
};

#[derive(MessageImpl, PartialEq, Clone)]
#[message(Message, Message, Tool)]
pub enum ToolMessage {
	SelectTool(ToolType),
	SelectPrimaryColor(Color),
	SelectSecondaryColor(Color),
	#[child]
	Rectangle(RectangleMessage),
}

#[derive(Debug, Default)]
pub struct ToolActionHandler {
	tool_state: ToolFsmState,
	actions: Vec<&'static [&'static str]>,
}
impl MessageHandler<ToolMessage, (&SvgDocument, &InputPreprocessor)> for ToolActionHandler {
	fn process_action(&mut self, action: ToolMessage, data: (&SvgDocument, &InputPreprocessor), responses: &mut Vec<Message>) {
		let (document, input) = data;
		use ToolMessage::*;
		match action {
			SelectPrimaryColor(c) => self.tool_state.document_tool_data.primary_color = c,
			SelectSecondaryColor(c) => self.tool_state.document_tool_data.secondary_color = c,
			SelectTool(tool) => self.tool_state.tool_data.active_tool_type = tool,
			Rectangle(_) => {
				let tool = self.tool_state.tool_data.tools.get_mut(&ToolType::Rectangle).unwrap();
				tool.process_action(action, (&document, &self.tool_state.document_tool_data, input), responses);
			}
		}
	}
	actions_fn!(
		ToolMessageDiscriminant::SelectSecondaryColor,
		ToolMessageDiscriminant::SelectPrimaryColor,
		ToolMessageDiscriminant::SelectTool
	);
}
