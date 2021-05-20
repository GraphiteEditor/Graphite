use crate::message_prelude::*;
use document_core::color::Color;

use crate::input::InputPreprocessor;
use crate::{
	tool::{ToolFsmState, ToolType},
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
	MouseMove,
	#[child]
	Rectangle(RectangleMessage),
	#[child]
	Ellipse(EllipseMessage),
}

#[derive(Debug, Default)]
pub struct ToolMessageHandler {
	tool_state: ToolFsmState,
	actions: Vec<&'static [&'static str]>,
}
impl MessageHandler<ToolMessage, (&SvgDocument, &InputPreprocessor)> for ToolMessageHandler {
	fn process_action(&mut self, message: ToolMessage, data: (&SvgDocument, &InputPreprocessor), responses: &mut VecDeque<Message>) {
		let (document, input) = data;
		use ToolMessage::*;
		match message {
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
			message => {
				let tool_type = match message {
					Rectangle(_) => ToolType::Rectangle,
					Ellipse(_) => ToolType::Ellipse,
					_ => unreachable!(),
				};
				let tool = self.tool_state.tool_data.tools.get_mut(&tool_type).unwrap();
				tool.process_action(message, (&document, &self.tool_state.document_tool_data, input), responses);
			}
		}
	}
	fn actions(&self) -> ActionList {
		let mut list = actions!(ToolMessageDiscriminant; ResetColors, SwapColors);
		list.extend(self.tool_state.tool_data.active_tool().actions());
		list
	}
}
