use crate::message_prelude::*;
use document_core::color::Color;

use crate::input::InputPreprocessor;
use crate::{
	document::Document,
	tool::{tool_settings::ToolSettings, ToolFsmState, ToolType},
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
	SetToolSettings(ToolType, ToolSettings),
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
impl MessageHandler<ToolMessage, (&Document, &InputPreprocessor)> for ToolMessageHandler {
	fn process_action(&mut self, message: ToolMessage, data: (&Document, &InputPreprocessor), responses: &mut VecDeque<Message>) {
		let (document, input) = data;
		use ToolMessage::*;
		match message {
			SelectPrimaryColor(c) => self.tool_state.document_tool_data.primary_color = c,
			SelectSecondaryColor(c) => self.tool_state.document_tool_data.secondary_color = c,
			SelectTool(tool) => {
				let mut reset = |tool| match tool {
					ToolType::Ellipse => responses.push_back(EllipseMessage::Abort.into()),
					ToolType::Rectangle => responses.push_back(RectangleMessage::Abort.into()),
					ToolType::Shape => responses.push_back(ShapeMessage::Abort.into()),
					ToolType::Line => responses.push_back(LineMessage::Abort.into()),
					ToolType::Pen => responses.push_back(PenMessage::Abort.into()),
					_ => (),
				};
				reset(tool);
				reset(self.tool_state.tool_data.active_tool_type);
				self.tool_state.tool_data.active_tool_type = tool;

				responses.push_back(FrontendMessage::SetActiveTool { tool_name: tool.to_string() }.into())
			}
			SwapColors => {
				let doc_data = &mut self.tool_state.document_tool_data;
				std::mem::swap(&mut doc_data.primary_color, &mut doc_data.secondary_color);
				responses.push_back(
					FrontendMessage::UpdateWorkingColors {
						primary: doc_data.primary_color,
						secondary: doc_data.secondary_color,
					}
					.into(),
				)
			}
			ResetColors => {
				let doc_data = &mut self.tool_state.document_tool_data;
				doc_data.primary_color = Color::BLACK;
				doc_data.secondary_color = Color::WHITE;
				responses.push_back(
					FrontendMessage::UpdateWorkingColors {
						primary: doc_data.primary_color,
						secondary: doc_data.secondary_color,
					}
					.into(),
				)
			}
			SetToolSettings(tool_type, tool_settings) => {
				self.tool_state.document_tool_data.tool_settings.insert(tool_type, tool_settings);
			}
			message => {
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
				if let Some(tool) = self.tool_state.tool_data.tools.get_mut(&tool_type) {
					tool.process_action(message, (&document, &self.tool_state.document_tool_data, input), responses);
				}
			}
		}
	}
	fn actions(&self) -> ActionList {
		let mut list = actions!(ToolMessageDiscriminant; ResetColors, SwapColors, SelectTool, SetToolSettings);
		list.extend(self.tool_state.tool_data.active_tool().actions());
		list
	}
}
