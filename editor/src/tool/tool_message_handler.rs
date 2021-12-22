use crate::message_prelude::*;
use graphene::color::Color;

use crate::input::InputPreprocessor;
use crate::{
	document::DocumentMessageHandler,
	tool::{tool_options::ToolOptions, DocumentToolData, ToolFsmState, ToolType},
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[impl_message(Message, Tool)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum ToolMessage {
	UpdateHints,
	ActivateTool(ToolType),
	SelectPrimaryColor(Color),
	SelectSecondaryColor(Color),
	SelectedLayersChanged,
	SwapColors,
	ResetColors,
	NoOp,
	SetToolOptions(ToolType, ToolOptions),
	#[child]
	Fill(FillMessage),
	#[child]
	Rectangle(RectangleMessage),
	#[child]
	Ellipse(EllipseMessage),
	#[child]
	Select(SelectMessage),
	#[child]
	Line(LineMessage),
	#[child]
	Crop(CropMessage),
	#[child]
	Eyedropper(EyedropperMessage),
	#[child]
	Navigate(NavigateMessage),
	#[child]
	Path(PathMessage),
	#[child]
	Pen(PenMessage),
	#[child]
	Shape(ShapeMessage),
}

#[derive(Debug, Default)]
pub struct ToolMessageHandler {
	tool_state: ToolFsmState,
}
impl MessageHandler<ToolMessage, (&DocumentMessageHandler, &InputPreprocessor)> for ToolMessageHandler {
	fn process_action(&mut self, message: ToolMessage, data: (&DocumentMessageHandler, &InputPreprocessor), responses: &mut VecDeque<Message>) {
		let (document, input) = data;
		use ToolMessage::*;
		match message {
			SelectPrimaryColor(color) => {
				let document_data = &mut self.tool_state.document_tool_data;
				document_data.primary_color = color;

				update_working_colors(&self.tool_state.document_tool_data, responses);
			}
			SelectSecondaryColor(color) => {
				let document_data = &mut self.tool_state.document_tool_data;
				document_data.secondary_color = color;

				update_working_colors(document_data, responses);
			}
			ActivateTool(new_tool) => {
				let tool_data = &mut self.tool_state.tool_data;
				let document_data = &self.tool_state.document_tool_data;
				let old_tool = tool_data.active_tool_type;

				// Do nothing if switching to the same tool
				if new_tool == old_tool {
					return;
				}

				// Get the Abort state of a tool's FSM
				let reset_message = |tool| match tool {
					ToolType::Select => Some(SelectMessage::Abort.into()),
					ToolType::Path => Some(PathMessage::Abort.into()),
					ToolType::Pen => Some(PenMessage::Abort.into()),
					ToolType::Line => Some(LineMessage::Abort.into()),
					ToolType::Rectangle => Some(RectangleMessage::Abort.into()),
					ToolType::Ellipse => Some(EllipseMessage::Abort.into()),
					ToolType::Shape => Some(ShapeMessage::Abort.into()),
					_ => None,
				};

				// Send the Abort state transition to the tool
				let mut send_message_to_tool = |tool_type, message: ToolMessage, update_hints: bool| {
					if let Some(tool) = tool_data.tools.get_mut(&tool_type) {
						tool.process_action(message, (document, document_data, input), responses);

						if update_hints {
							tool.process_action(ToolMessage::UpdateHints, (document, document_data, input), responses);
						}
					}
				};

				// Send the old and new tools a transition to their FSM Abort states
				if let Some(tool_message) = reset_message(new_tool) {
					send_message_to_tool(new_tool, tool_message, true);
				}
				if let Some(tool_message) = reset_message(old_tool) {
					send_message_to_tool(old_tool, tool_message, false);
				}

				// Special cases for specific tools
				// TODO: Refactor to avoid doing this here
				if new_tool == ToolType::Select || new_tool == ToolType::Path {
					responses.push_back(ToolMessage::SelectedLayersChanged.into());
				}

				// Store the new active tool
				tool_data.active_tool_type = new_tool;

				// Notify the frontend about the new active tool to be displayed
				let tool_name = new_tool.to_string();
				let tool_options = self.tool_state.document_tool_data.tool_options.get(&new_tool).copied();
				responses.push_back(FrontendMessage::SetActiveTool { tool_name, tool_options }.into());
			}
			SelectedLayersChanged => {
				match self.tool_state.tool_data.active_tool_type {
					ToolType::Select => responses.push_back(SelectMessage::UpdateSelectionBoundingBox.into()),
					ToolType::Path => responses.push_back(PathMessage::RedrawOverlay.into()),
					_ => (),
				};
			}
			SwapColors => {
				let document_data = &mut self.tool_state.document_tool_data;

				std::mem::swap(&mut document_data.primary_color, &mut document_data.secondary_color);

				update_working_colors(document_data, responses);
			}
			ResetColors => {
				let document_data = &mut self.tool_state.document_tool_data;

				document_data.primary_color = Color::BLACK;
				document_data.secondary_color = Color::WHITE;

				update_working_colors(document_data, responses);
			}
			SetToolOptions(tool_type, tool_options) => {
				let document_data = &mut self.tool_state.document_tool_data;

				document_data.tool_options.insert(tool_type, tool_options);
			}
			message => {
				let tool_type = message_to_tool_type(&message);
				let document_data = &self.tool_state.document_tool_data;
				let tool_data = &mut self.tool_state.tool_data;

				if let Some(tool) = tool_data.tools.get_mut(&tool_type) {
					if tool_type == tool_data.active_tool_type {
						tool.process_action(message, (document, document_data, input), responses);
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

fn message_to_tool_type(message: &ToolMessage) -> ToolType {
	use ToolMessage::*;
	let tool_type = match message {
		Fill(_) => ToolType::Fill,
		Rectangle(_) => ToolType::Rectangle,
		Ellipse(_) => ToolType::Ellipse,
		Shape(_) => ToolType::Shape,
		Line(_) => ToolType::Line,
		Pen(_) => ToolType::Pen,
		Select(_) => ToolType::Select,
		Crop(_) => ToolType::Crop,
		Eyedropper(_) => ToolType::Eyedropper,
		Navigate(_) => ToolType::Navigate,
		Path(_) => ToolType::Path,
		_ => unreachable!(),
	};

	tool_type
}

fn update_working_colors(document_data: &DocumentToolData, responses: &mut VecDeque<Message>) {
	responses.push_back(
		FrontendMessage::UpdateWorkingColors {
			primary: document_data.primary_color,
			secondary: document_data.secondary_color,
		}
		.into(),
	);
}
