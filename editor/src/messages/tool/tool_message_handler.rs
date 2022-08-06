use super::utility_types::{tool_message_to_tool_type, ToolFsmState};
use crate::application::generate_uuid;
use crate::messages::layout::utility_types::layout_widget::PropertyHolder;
use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::prelude::*;
use crate::messages::tool::utility_types::ToolType;

use graphene::color::Color;
use graphene::layers::text_layer::FontCache;

#[derive(Debug, Default)]
pub struct ToolMessageHandler {
	tool_state: ToolFsmState,
}

impl MessageHandler<ToolMessage, (&DocumentMessageHandler, &InputPreprocessorMessageHandler, &FontCache)> for ToolMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: ToolMessage, data: (&DocumentMessageHandler, &InputPreprocessorMessageHandler, &FontCache), responses: &mut VecDeque<Message>) {
		let (document, input, font_cache) = data;
		#[remain::sorted]
		match message {
			// Messages
			#[remain::unsorted]
			ToolMessage::ActivateToolSelect => responses.push_front(ToolMessage::ActivateTool { tool_type: ToolType::Select }.into()),
			#[remain::unsorted]
			ToolMessage::ActivateToolArtboard => responses.push_front(ToolMessage::ActivateTool { tool_type: ToolType::Artboard }.into()),
			#[remain::unsorted]
			ToolMessage::ActivateToolNavigate => responses.push_front(ToolMessage::ActivateTool { tool_type: ToolType::Navigate }.into()),
			#[remain::unsorted]
			ToolMessage::ActivateToolEyedropper => responses.push_front(ToolMessage::ActivateTool { tool_type: ToolType::Eyedropper }.into()),
			#[remain::unsorted]
			ToolMessage::ActivateToolText => responses.push_front(ToolMessage::ActivateTool { tool_type: ToolType::Text }.into()),
			#[remain::unsorted]
			ToolMessage::ActivateToolFill => responses.push_front(ToolMessage::ActivateTool { tool_type: ToolType::Fill }.into()),
			#[remain::unsorted]
			ToolMessage::ActivateToolGradient => responses.push_front(ToolMessage::ActivateTool { tool_type: ToolType::Gradient }.into()),

			#[remain::unsorted]
			ToolMessage::ActivateToolPath => responses.push_front(ToolMessage::ActivateTool { tool_type: ToolType::Path }.into()),
			#[remain::unsorted]
			ToolMessage::ActivateToolPen => responses.push_front(ToolMessage::ActivateTool { tool_type: ToolType::Pen }.into()),
			#[remain::unsorted]
			ToolMessage::ActivateToolFreehand => responses.push_front(ToolMessage::ActivateTool { tool_type: ToolType::Freehand }.into()),
			#[remain::unsorted]
			ToolMessage::ActivateToolSpline => responses.push_front(ToolMessage::ActivateTool { tool_type: ToolType::Spline }.into()),
			#[remain::unsorted]
			ToolMessage::ActivateToolLine => responses.push_front(ToolMessage::ActivateTool { tool_type: ToolType::Line }.into()),
			#[remain::unsorted]
			ToolMessage::ActivateToolRectangle => responses.push_front(ToolMessage::ActivateTool { tool_type: ToolType::Rectangle }.into()),
			#[remain::unsorted]
			ToolMessage::ActivateToolEllipse => responses.push_front(ToolMessage::ActivateTool { tool_type: ToolType::Ellipse }.into()),
			#[remain::unsorted]
			ToolMessage::ActivateToolShape => responses.push_front(ToolMessage::ActivateTool { tool_type: ToolType::Shape }.into()),

			ToolMessage::ActivateTool { tool_type } => {
				let tool_data = &mut self.tool_state.tool_data;
				let document_data = &self.tool_state.document_tool_data;
				let old_tool = tool_data.active_tool_type;

				// Do nothing if switching to the same tool
				if tool_type == old_tool {
					return;
				}

				// Send the Abort state transition to the tool
				let mut send_abort_to_tool = |tool_type, update_hints_and_cursor: bool| {
					if let Some(tool) = tool_data.tools.get_mut(&tool_type) {
						if let Some(tool_abort_message) = tool.event_to_message_map().tool_abort {
							tool.process_message(tool_abort_message, (document, document_data, input, font_cache), responses);
						}

						if update_hints_and_cursor {
							tool.process_message(ToolMessage::UpdateHints, (document, document_data, input, font_cache), responses);
							tool.process_message(ToolMessage::UpdateCursor, (document, document_data, input, font_cache), responses);
						}
					}
				};

				// Send the old and new tools a transition to their FSM Abort states
				send_abort_to_tool(tool_type, true);
				send_abort_to_tool(old_tool, false);

				// Unsubscribe old tool from the broadcaster
				tool_data.tools.get(&tool_type).unwrap().deactivate(responses);

				// Store the new active tool
				tool_data.active_tool_type = tool_type;

				// Subscribe new tool
				tool_data.tools.get(&tool_type).unwrap().activate(responses);

				// Send the SelectionChanged message to the active tool, this will ensure the selection is updated
				responses.push_back(BroadcastEvent::SelectionChanged.into());

				// Send the DocumentIsDirty message to the active tool's sub-tool message handler
				responses.push_back(BroadcastEvent::DocumentIsDirty.into());

				// Send Properties to the frontend
				tool_data.tools.get(&tool_type).unwrap().register_properties(responses, LayoutTarget::ToolOptions);

				// Notify the frontend about the new active tool to be displayed
				tool_data.register_properties(responses, LayoutTarget::ToolShelf);
			}
			ToolMessage::DeactivateTools => {
				let tool_data = &mut self.tool_state.tool_data;
				tool_data.tools.get(&tool_data.active_tool_type).unwrap().deactivate(responses);
			}
			ToolMessage::InitTools => {
				let tool_data = &mut self.tool_state.tool_data;
				let document_data = &self.tool_state.document_tool_data;
				let active_tool = &tool_data.active_tool_type;

				// Subscribe tool to broadcast messages
				tool_data.tools.get(active_tool).unwrap().activate(responses);

				// Register initial properties
				tool_data.tools.get(active_tool).unwrap().register_properties(responses, LayoutTarget::ToolOptions);

				// Notify the frontend about the initial active tool
				tool_data.register_properties(responses, LayoutTarget::ToolShelf);

				// Notify the frontend about the initial working colors
				document_data.update_working_colors(responses);
				responses.push_back(FrontendMessage::TriggerRefreshBoundsOfViewports.into());

				// Set initial hints and cursor
				tool_data
					.active_tool_mut()
					.process_message(ToolMessage::UpdateHints, (document, document_data, input, font_cache), responses);
				tool_data
					.active_tool_mut()
					.process_message(ToolMessage::UpdateCursor, (document, document_data, input, font_cache), responses);
			}
			ToolMessage::ResetColors => {
				let document_data = &mut self.tool_state.document_tool_data;

				document_data.primary_color = Color::BLACK;
				document_data.secondary_color = Color::WHITE;

				document_data.update_working_colors(responses);
			}
			ToolMessage::SelectPrimaryColor { color } => {
				let document_data = &mut self.tool_state.document_tool_data;
				document_data.primary_color = color;

				self.tool_state.document_tool_data.update_working_colors(responses);
			}
			ToolMessage::SelectRandomPrimaryColor => {
				// Select a random primary color (rgba) based on an UUID
				let document_data = &mut self.tool_state.document_tool_data;

				let random_number = generate_uuid();
				let r = (random_number >> 16) as u8;
				let g = (random_number >> 8) as u8;
				let b = random_number as u8;
				let random_color = Color::from_rgba8(r, g, b, 255);
				document_data.primary_color = random_color;

				document_data.update_working_colors(responses);
			}
			ToolMessage::SelectSecondaryColor { color } => {
				let document_data = &mut self.tool_state.document_tool_data;
				document_data.secondary_color = color;

				document_data.update_working_colors(responses);
			}
			ToolMessage::SwapColors => {
				let document_data = &mut self.tool_state.document_tool_data;

				std::mem::swap(&mut document_data.primary_color, &mut document_data.secondary_color);

				document_data.update_working_colors(responses);
			}

			// Sub-messages
			#[remain::unsorted]
			tool_message => {
				let tool_type = match &tool_message {
					ToolMessage::UpdateCursor | ToolMessage::UpdateHints => self.tool_state.tool_data.active_tool_type,
					tool_message => tool_message_to_tool_type(tool_message),
				};
				let document_data = &self.tool_state.document_tool_data;
				let tool_data = &mut self.tool_state.tool_data;

				if let Some(tool) = tool_data.tools.get_mut(&tool_type) {
					if tool_type == tool_data.active_tool_type {
						tool.process_message(tool_message, (document, document_data, input, font_cache), responses);
					}
				}
			}
		}
	}

	fn actions(&self) -> ActionList {
		let mut list = actions!(ToolMessageDiscriminant;
			ActivateToolSelect,
			ActivateToolArtboard,
			ActivateToolNavigate,
			ActivateToolEyedropper,
			ActivateToolText,
			ActivateToolFill,
			ActivateToolGradient,
			ActivateToolPath,
			ActivateToolPen,
			ActivateToolFreehand,
			ActivateToolSpline,
			ActivateToolLine,
			ActivateToolRectangle,
			ActivateToolEllipse,
			ActivateToolShape,
			SelectRandomPrimaryColor,
			ResetColors,
			SwapColors,
		);
		list.extend(self.tool_state.tool_data.active_tool().actions());

		list
	}
}
