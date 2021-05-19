pub mod tool_message_handler;
pub mod tool_settings;
pub mod tools;

use crate::message_prelude::*;
use crate::SvgDocument;
use crate::{
	communication::{message::Message, MessageHandler},
	Color,
};
use crate::{input::InputPreprocessor, EditorError};
use std::collections::VecDeque;
use std::{
	collections::HashMap,
	fmt::{self, Debug},
};
pub use tool_message_handler::ToolActionHandler;
use tool_settings::ToolSettings;
pub use tool_settings::*;
use tools::*;

pub mod tool_messages {
	pub use super::tool_message_handler::{ToolMessage, ToolMessageDiscriminant};
	pub use super::tools::rectangle::{RectangleMessage, RectangleMessageDiscriminant};
}

pub type ToolActionHandlerData<'a> = (&'a SvgDocument, &'a DocumentToolData, &'a InputPreprocessor);

pub trait Fsm {
	type ToolData;

	fn transition(self, message: ToolMessage, document: &SvgDocument, tool_data: &DocumentToolData, data: &mut Self::ToolData, input: &InputPreprocessor, messages: &mut VecDeque<Message>) -> Self;
}

#[derive(Debug, Clone)]
pub struct DocumentToolData {
	pub primary_color: Color,
	pub secondary_color: Color,
	tool_settings: HashMap<ToolType, ToolSettings>,
}

pub struct ToolData {
	pub active_tool_type: ToolType,
	pub tools: HashMap<ToolType, Box<dyn for<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>>>>,
}

impl fmt::Debug for ToolData {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("ToolData").field("active_tool_type", &self.active_tool_type).field("tool_settings", &"[…]").finish()
	}
}

impl ToolData {
	pub fn active_tool_mut(&mut self) -> Result<&mut Box<dyn for<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>>>, EditorError> {
		self.tools.get_mut(&self.active_tool_type).ok_or(EditorError::UnknownTool)
	}
	pub fn active_tool(&self) -> Result<&dyn for<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>>, EditorError> {
		self.tools.get(&self.active_tool_type).map(|x| x.as_ref()).ok_or(EditorError::UnknownTool)
	}
}

#[derive(Debug)]
pub struct ToolFsmState {
	pub document_tool_data: DocumentToolData,
	pub tool_data: ToolData,
}

impl Default for ToolFsmState {
	fn default() -> Self {
		ToolFsmState {
			tool_data: ToolData {
				active_tool_type: ToolType::Select,
				tools: gen_tools_hash_map! {
					Rectangle => rectangle::Rectangle,
					/*Select => select::Select,
					Crop => crop::Crop,
					Navigate => navigate::Navigate,
					Eyedropper => eyedropper::Eyedropper,
					Path => path::Path,
					Pen => pen::Pen,
					Line => line::Line,
					Ellipse => ellipse::Ellipse,
					Shape => shape::Shape,*/
				},
			},
			document_tool_data: DocumentToolData {
				primary_color: Color::BLACK,
				secondary_color: Color::WHITE,
				tool_settings: default_tool_settings(),
			},
		}
	}
}

impl ToolFsmState {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn swap_colors(&mut self) {
		std::mem::swap(&mut self.document_tool_data.primary_color, &mut self.document_tool_data.secondary_color);
	}
}

fn default_tool_settings() -> HashMap<ToolType, ToolSettings> {
	let tool_init = |tool: ToolType| (tool, tool.default_settings());
	std::array::IntoIter::new([
		tool_init(ToolType::Select),
		tool_init(ToolType::Ellipse),
		tool_init(ToolType::Shape), // TODO: Add more tool defaults
	])
	.collect()
}

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolType {
	Select,
	Crop,
	Navigate,
	Eyedropper,
	Text,
	Fill,
	Gradient,
	Brush,
	Heal,
	Clone,
	Patch,
	BlurSharpen,
	Relight,
	Path,
	Pen,
	Freehand,
	Spline,
	Line,
	Rectangle,
	Ellipse,
	Shape,
}

impl fmt::Display for ToolType {
	fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		use ToolType::*;

		let name = match_variant_name!(match (self) {
			Select,
			Crop,
			Navigate,
			Eyedropper,
			Text,
			Fill,
			Gradient,
			Brush,
			Heal,
			Clone,
			Patch,
			BlurSharpen,
			Relight,
			Path,
			Pen,
			Freehand,
			Spline,
			Line,
			Rectangle,
			Ellipse,
			Shape
		});

		formatter.write_str(name)
	}
}

impl ToolType {
	fn default_settings(&self) -> ToolSettings {
		match self {
			ToolType::Select => ToolSettings::Select { append_mode: SelectAppendMode::New },
			ToolType::Ellipse => ToolSettings::Ellipse,
			ToolType::Shape => ToolSettings::Shape {
				shape: Shape::Polygon { vertices: 3 },
			},
			_ => todo!(),
		}
	}
}
