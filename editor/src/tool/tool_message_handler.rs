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
	ActivateTool(ToolType),
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
			SelectPrimaryColor(c) => {
				self.tool_state.document_tool_data.primary_color = c;
				update_working_colors(&self.tool_state.document_tool_data, responses);
			}
			SelectSecondaryColor(c) => {
				self.tool_state.document_tool_data.secondary_color = c;
				update_working_colors(&self.tool_state.document_tool_data, responses);
			}
			ActivateTool(tool) => {
				let old_tool = self.tool_state.tool_data.active_tool_type;
				let (new, old) = (reset_tool(tool), reset_tool(old_tool));
				let mut send_to_tool = |tool_type, message: ToolMessage| {
					if let Some(tool) = self.tool_state.tool_data.tools.get_mut(&tool_type) {
						tool.process_action(message, (document, &self.tool_state.document_tool_data, input), responses);
					}
				};
				send_to_tool(tool, new);
				send_to_tool(old_tool, old);
				// TODO: Refactor to avoid tool specific cases
				if tool == ToolType::Select {
					responses.push_back(SelectMessage::SelectedLayersChanged.into());
				}
				self.tool_state.tool_data.active_tool_type = tool;

				responses.push_back(FrontendMessage::SetActiveTool { tool_name: tool.to_string() }.into())
			}
			SwapColors => {
				let doc_data = &mut self.tool_state.document_tool_data;
				std::mem::swap(&mut doc_data.primary_color, &mut doc_data.secondary_color);
				update_working_colors(doc_data, responses);
			}
			ResetColors => {
				let doc_data = &mut self.tool_state.document_tool_data;
				doc_data.primary_color = Color::BLACK;
				doc_data.secondary_color = Color::WHITE;
				update_working_colors(doc_data, responses);
			}
			SetToolOptions(tool_type, tool_options) => {
				self.tool_state.document_tool_data.tool_options.insert(tool_type, tool_options);
			}
			message => {
				let tool_type = message_to_tool_type(&message);
				if let Some(tool) = self.tool_state.tool_data.tools.get_mut(&tool_type) {
					if tool_type == self.tool_state.tool_data.active_tool_type {
						tool.process_action(message, (document, &self.tool_state.document_tool_data, input), responses);
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

fn reset_tool(tool: ToolType) -> ToolMessage {
	match tool {
		ToolType::Select => SelectMessage::Abort.into(),
		ToolType::Path => PathMessage::Abort.into(),
		ToolType::Pen => PenMessage::Abort.into(),
		ToolType::Line => LineMessage::Abort.into(),
		ToolType::Rectangle => RectangleMessage::Abort.into(),
		ToolType::Ellipse => EllipseMessage::Abort.into(),
		ToolType::Shape => ShapeMessage::Abort.into(),
		_ => ToolMessage::NoOp,
	}
}

fn message_to_tool_type(message: &ToolMessage) -> ToolType {
	use ToolMessage::*;
	match message {
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
	}
}

fn update_working_colors(doc_data: &DocumentToolData, responses: &mut VecDeque<Message>) {
	responses.push_back(
		FrontendMessage::UpdateWorkingColors {
			primary: doc_data.primary_color,
			secondary: doc_data.secondary_color,
		}
		.into(),
	);
}
