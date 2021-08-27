use crate::message_prelude::*;
use graphene::color::Color;

use crate::input::InputPreprocessor;
use crate::{
	document::DocumentMessageHandler,
	tool::{tool_options::ToolOptions, DocumentToolData, ToolFsmState, ToolType},
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
			SelectTool(tool) => {
				let tool_data = &mut self.tool_state.tool_data;
				let document_data = &self.tool_state.document_tool_data;
				let old_tool = tool_data.active_tool_type;

				// Prepare to reset the old and new tools by obtaining their FSM Abort state, which will be sent to the tool
				let reset = |tool| match tool {
					ToolType::Ellipse => EllipseMessage::Abort.into(),
					ToolType::Rectangle => RectangleMessage::Abort.into(),
					ToolType::Shape => ShapeMessage::Abort.into(),
					ToolType::Line => LineMessage::Abort.into(),
					ToolType::Pen => PenMessage::Abort.into(),
					ToolType::Select => SelectMessage::Abort.into(),
					_ => ToolMessage::NoOp,
				};
				let new = reset(tool);
				let old = reset(old_tool);

				// Send the old and new tools a transition to the FSM Abort state
				let mut send_to_tool = |tool_type, message: ToolMessage| {
					if let Some(tool) = tool_data.tools.get_mut(&tool_type) {
						tool.process_action(message, (document, document_data, input), responses);
					}
				};
				send_to_tool(tool, new);
				send_to_tool(old_tool, old);

				// Special cases for specific tools
				if tool == ToolType::Select {
					responses.push_back(SelectMessage::UpdateSelectionBoundingBox.into());
				}
				self.tool_state.tool_data.active_tool_type = tool;

				// Notify the frontend about the new active tool to be displayed
				responses.push_back(FrontendMessage::SetActiveTool { tool_name: tool.to_string() }.into());
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
		let mut list = actions!(ToolMessageDiscriminant; ResetColors, SwapColors, SelectTool, SetToolOptions);
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
