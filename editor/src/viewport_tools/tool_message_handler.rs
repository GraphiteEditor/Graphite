use super::tool::{message_to_tool_type, standard_tool_message, update_working_colors, StandardToolMessageType, ToolFsmState};
use super::tool_message::ToolMessage;
use crate::document::DocumentMessageHandler;
use crate::input::InputPreprocessorMessageHandler;
use crate::message_prelude::*;

use graphene::color::Color;

use std::collections::VecDeque;

#[derive(Debug, Default)]
pub struct ToolMessageHandler {
	tool_state: ToolFsmState,
}

impl MessageHandler<ToolMessage, (&DocumentMessageHandler, &InputPreprocessorMessageHandler)> for ToolMessageHandler {
	#[remain::check]
	fn process_action(&mut self, message: ToolMessage, data: (&DocumentMessageHandler, &InputPreprocessorMessageHandler), responses: &mut VecDeque<Message>) {
		use ToolMessage::*;

		let (document, input) = data;
		#[remain::sorted]
		match message {
			ActivateTool { tool_type } => {
				let tool_data = &mut self.tool_state.tool_data;
				let document_data = &self.tool_state.document_tool_data;
				let old_tool = tool_data.active_tool_type;

				// Do nothing if switching to the same tool
				if tool_type == old_tool {
					return;
				}

				// Send the Abort state transition to the tool
				let mut send_abort_to_tool = |tool_type, message: ToolMessage, update_hints: bool| {
					if let Some(tool) = tool_data.tools.get_mut(&tool_type) {
						tool.process_action(message, (document, document_data, input), responses);

						if update_hints {
							tool.process_action(ToolMessage::UpdateHints, (document, document_data, input), responses);
						}
					}
				};
				// Send the old and new tools a transition to their FSM Abort states
				if let Some(tool_message) = standard_tool_message(tool_type, StandardToolMessageType::Abort) {
					send_abort_to_tool(tool_type, tool_message, true);
				}
				if let Some(tool_message) = standard_tool_message(old_tool, StandardToolMessageType::Abort) {
					send_abort_to_tool(old_tool, tool_message, false);
				}

				// Send the DocumentIsDirty message to the active tool's sub-tool message handler
				if let Some(message) = standard_tool_message(tool_type, StandardToolMessageType::DocumentIsDirty) {
					responses.push_back(message.into());
				}

				// Store the new active tool
				tool_data.active_tool_type = tool_type;

				// Notify the frontend about the new active tool to be displayed
				let tool_name = tool_type.to_string();
				let tool_options = self.tool_state.document_tool_data.tool_options.get(&tool_type).copied();
				responses.push_back(FrontendMessage::UpdateActiveTool { tool_name, tool_options }.into());
			}
			DocumentIsDirty => {
				// Send the DocumentIsDirty message to the active tool's sub-tool message handler
				let active_tool = self.tool_state.tool_data.active_tool_type;
				if let Some(message) = standard_tool_message(active_tool, StandardToolMessageType::DocumentIsDirty) {
					responses.push_back(message.into());
				}
			}
			ResetColors => {
				let document_data = &mut self.tool_state.document_tool_data;

				document_data.primary_color = Color::BLACK;
				document_data.secondary_color = Color::WHITE;

				update_working_colors(document_data, responses);
			}
			SelectPrimaryColor { color } => {
				let document_data = &mut self.tool_state.document_tool_data;
				document_data.primary_color = color;

				update_working_colors(&self.tool_state.document_tool_data, responses);
			}
			SelectSecondaryColor { color } => {
				let document_data = &mut self.tool_state.document_tool_data;
				document_data.secondary_color = color;

				update_working_colors(document_data, responses);
			}
			SetToolOptions { tool_type, tool_options } => {
				let document_data = &mut self.tool_state.document_tool_data;

				document_data.tool_options.insert(tool_type, tool_options);
			}
			SwapColors => {
				let document_data = &mut self.tool_state.document_tool_data;

				std::mem::swap(&mut document_data.primary_color, &mut document_data.secondary_color);

				update_working_colors(document_data, responses);
			}
			tool_message => {
				let tool_type = message_to_tool_type(&tool_message);
				let document_data = &self.tool_state.document_tool_data;
				let tool_data = &mut self.tool_state.tool_data;

				if let Some(tool) = tool_data.tools.get_mut(&tool_type) {
					if tool_type == tool_data.active_tool_type {
						tool.process_action(tool_message, (document, document_data, input), responses);
					}
				}
			}
		}
	}

	fn actions(&self) -> ActionList {
		let mut list = actions!(ToolMessageDiscriminant; ResetColors, SwapColors, ActivateTool, SetToolOptions);
		list.extend(self.tool_state.tool_data.active_tool().actions());

		list
	}
}
