use super::tool::{message_to_tool_type, DocumentToolData, ToolFsmState};
use crate::document::DocumentMessageHandler;
use crate::input::InputPreprocessorMessageHandler;
use crate::layout::layout_message::LayoutTarget;
use crate::layout::widgets::{IconButton, Layout, LayoutGroup, PropertyHolder, SwatchPairInput, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::message_prelude::*;

use graphene::color::Color;
use graphene::layers::text_layer::FontCache;

use std::collections::VecDeque;

#[derive(Debug, Default)]
pub struct ToolMessageHandler {
	tool_state: ToolFsmState,
}

impl MessageHandler<ToolMessage, (&DocumentMessageHandler, &InputPreprocessorMessageHandler, &FontCache)> for ToolMessageHandler {
	#[remain::check]
	fn process_action(&mut self, message: ToolMessage, data: (&DocumentMessageHandler, &InputPreprocessorMessageHandler, &FontCache), responses: &mut VecDeque<Message>) {
		use ToolMessage::*;

		let (document, input, font_cache) = data;
		#[remain::sorted]
		match message {
			// Messages
			ActivateTool { tool_type } => {
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
						if let Some(tool_abort_message) = tool.signal_to_message_map().tool_abort {
							tool.process_action(tool_abort_message, (document, document_data, input, font_cache), responses);
						}

						if update_hints_and_cursor {
							tool.process_action(ToolMessage::UpdateHints, (document, document_data, input, font_cache), responses);
							tool.process_action(ToolMessage::UpdateCursor, (document, document_data, input, font_cache), responses);
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
				responses.push_back(BroadcastSignal::SelectionChanged.into());

				// Send the DocumentIsDirty message to the active tool's sub-tool message handler
				responses.push_back(BroadcastSignal::DocumentIsDirty.into());

				// Send Properties to the frontend
				tool_data.tools.get(&tool_type).unwrap().register_properties(responses, LayoutTarget::ToolOptions);

				// Notify the frontend about the new active tool to be displayed
				tool_data.register_properties(responses, LayoutTarget::ToolShelf);
			}
			DeactivateTools => {
				let tool_data = &mut self.tool_state.tool_data;
				tool_data.tools.get(&tool_data.active_tool_type).unwrap().deactivate(responses);
			}
			InitTools => {
				let tool_data = &mut self.tool_state.tool_data;
				let document_data = &self.tool_state.document_tool_data;
				let active_tool = &tool_data.active_tool_type;

				// subscribe tool to broadcast messages
				tool_data.tools.get(active_tool).unwrap().activate(responses);

				// Register initial properties
				tool_data.tools.get(active_tool).unwrap().register_properties(responses, LayoutTarget::ToolOptions);

				// Notify the frontend about the initial active tool
				tool_data.register_properties(responses, LayoutTarget::ToolShelf);

				// Set initial hints and cursor
				tool_data
					.active_tool_mut()
					.process_action(ToolMessage::UpdateHints, (document, document_data, input, font_cache), responses);
				tool_data
					.active_tool_mut()
					.process_action(ToolMessage::UpdateCursor, (document, document_data, input, font_cache), responses);
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
			SelectRandomPrimaryColor => {
				// Select a random primary color (rgba) based on an UUID
				let document_data = &mut self.tool_state.document_tool_data;

				let random_number = generate_uuid();
				let r = (random_number >> 16) as u8;
				let g = (random_number >> 8) as u8;
				let b = random_number as u8;
				let random_color = Color::from_rgba8(r, g, b, 255);
				document_data.primary_color = random_color;

				update_working_colors(document_data, responses);
			}
			SelectSecondaryColor { color } => {
				let document_data = &mut self.tool_state.document_tool_data;
				document_data.secondary_color = color;

				update_working_colors(document_data, responses);
			}
			SwapColors => {
				let document_data = &mut self.tool_state.document_tool_data;

				std::mem::swap(&mut document_data.primary_color, &mut document_data.secondary_color);

				update_working_colors(document_data, responses);
			}

			// Sub-messages
			#[remain::unsorted]
			tool_message => {
				let tool_type = match &tool_message {
					UpdateCursor | UpdateHints => self.tool_state.tool_data.active_tool_type,
					tool_message => message_to_tool_type(tool_message),
				};
				let document_data = &self.tool_state.document_tool_data;
				let tool_data = &mut self.tool_state.tool_data;

				if let Some(tool) = tool_data.tools.get_mut(&tool_type) {
					if tool_type == tool_data.active_tool_type {
						tool.process_action(tool_message, (document, document_data, input, font_cache), responses);
					}
				}
			}
		}
	}

	fn actions(&self) -> ActionList {
		let mut list = actions!(ToolMessageDiscriminant;
			ActivateTool,
			SelectRandomPrimaryColor,
			ResetColors,
			SwapColors,
		);
		list.extend(self.tool_state.tool_data.active_tool().actions());

		list
	}
}

fn update_working_colors(document_data: &DocumentToolData, responses: &mut VecDeque<Message>) {
	let layout = WidgetLayout::new(vec![
		LayoutGroup::Row {
			widgets: vec![WidgetHolder::new(Widget::SwatchPairInput(SwatchPairInput))],
		},
		LayoutGroup::Row {
			widgets: vec![
				WidgetHolder::new(Widget::IconButton(IconButton {
					size: 16,
					icon: "Swap".into(),
					tooltip: "Swap (Shift+X)".into(), // TODO: Customize this tooltip for the Mac version of the keyboard shortcut
					on_update: WidgetCallback::new(|_| ToolMessage::SwapColors.into()),
					..Default::default()
				})),
				WidgetHolder::new(Widget::IconButton(IconButton {
					size: 16,
					icon: "ResetColors".into(), // TODO: Customize this tooltip for the Mac version of the keyboard shortcut
					tooltip: "Reset (Ctrl+Shift+X)".into(),
					on_update: WidgetCallback::new(|_| ToolMessage::ResetColors.into()),
					..Default::default()
				})),
			],
		},
	]);

	responses.push_back(
		LayoutMessage::SendLayout {
			layout: Layout::WidgetLayout(layout),
			layout_target: LayoutTarget::WorkingColors,
		}
		.into(),
	);

	responses.push_back(
		FrontendMessage::UpdateWorkingColors {
			primary: document_data.primary_color,
			secondary: document_data.secondary_color,
		}
		.into(),
	);
}
