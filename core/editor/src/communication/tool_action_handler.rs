use document_core::color::Color;
use graphite_proc_macros::*;

use super::{message::prelude::*, InputPreprocessor, MessageHandler};
use crate::{
	tools::{rectangle::RectangleMessage, ToolFsmState, ToolType},
	SvgDocument,
};
use std::collections::VecDeque;

#[impl_message(Message, Tool)]
#[derive(PartialEq, Clone, Debug)]
pub enum ToolMessage {
	SelectTool(ToolType),
	SelectPrimaryColor(Color),
	SelectSecondaryColor(Color),
	SwapColors,
	ResetColors,
	#[child]
	Rectangle(RectangleMessage),
}

#[derive(Debug, Default)]
pub struct ToolActionHandler {
	tool_state: ToolFsmState,
	actions: Vec<&'static [&'static str]>,
}
impl MessageHandler<ToolMessage, (&SvgDocument, &InputPreprocessor)> for ToolActionHandler {
	fn process_action(&mut self, action: ToolMessage, data: (&SvgDocument, &InputPreprocessor), responses: &mut VecDeque<Message>) {
		let (document, input) = data;
		use ToolMessage::*;
		match action {
			SelectPrimaryColor(c) => self.tool_state.document_tool_data.primary_color = c,
			SelectSecondaryColor(c) => self.tool_state.document_tool_data.secondary_color = c,
			SelectTool(tool) => {
				self.tool_state.tool_data.active_tool_type = tool;
				responses.push_back(FrontendMessage::SetActiveTool { tool_name: tool.to_string() }.into())
			}
			SwapColors => {
				let doc_data = &mut self.tool_state.document_tool_data;
				std::mem::swap(&mut doc_data.primary_color, &mut doc_data.secondary_color);
			}
			ResetColors => {
				let doc_data = &mut self.tool_state.document_tool_data;
				doc_data.primary_color = Color::WHITE;
				doc_data.secondary_color = Color::BLACK;
			}
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
