use super::tools::*;
use crate::communication::message_handler::MessageHandler;
use crate::document::DocumentMessageHandler;
use crate::input::InputPreprocessorMessageHandler;
use crate::layout::widgets::PropertyHolder;
use crate::message_prelude::*;

use graphene::color::Color;

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fmt::{self, Debug};

pub type ToolActionHandlerData<'a> = (&'a DocumentMessageHandler, &'a DocumentToolData, &'a InputPreprocessorMessageHandler);

pub trait Fsm {
	type ToolData;
	type ToolOptions;

	#[must_use]
	fn transition(
		self,
		message: ToolMessage,
		document: &DocumentMessageHandler,
		tool_data: &DocumentToolData,
		data: &mut Self::ToolData,
		options: &Self::ToolOptions,
		input: &InputPreprocessorMessageHandler,
		messages: &mut VecDeque<Message>,
	) -> Self;

	fn update_hints(&self, responses: &mut VecDeque<Message>);
	fn update_cursor(&self, responses: &mut VecDeque<Message>);
}

#[derive(Debug, Clone)]
pub struct DocumentToolData {
	pub primary_color: Color,
	pub secondary_color: Color,
}

pub trait ToolCommon: for<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> + PropertyHolder {}
impl<T> ToolCommon for T where T: for<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> + PropertyHolder {}

type Tool = dyn ToolCommon;

pub struct ToolData {
	pub active_tool_type: ToolType,
	pub tools: HashMap<ToolType, Box<Tool>>,
}

impl fmt::Debug for ToolData {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("ToolData").field("active_tool_type", &self.active_tool_type).field("tool_options", &"[…]").finish()
	}
}

impl ToolData {
	pub fn active_tool_mut(&mut self) -> &mut Box<Tool> {
		self.tools.get_mut(&self.active_tool_type).expect("The active tool is not initialized")
	}

	pub fn active_tool(&self) -> &Tool {
		self.tools.get(&self.active_tool_type).map(|x| x.as_ref()).expect("The active tool is not initialized")
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
					Select => select::Select,
					Crop => crop::Crop,
					Navigate => navigate::Navigate,
					Eyedropper => eyedropper::Eyedropper,
					Text => text::Text,
					Fill => fill::Fill,
					// Gradient => gradient::Gradient,
					// Brush => brush::Brush,
					// Heal => heal::Heal,
					// Clone => clone::Clone,
					// Patch => patch::Patch,
					// BlurSharpen => blursharpen::BlurSharpen,
					// Relight => relight::Relight,
					Path => path::Path,
					Pen => pen::Pen,
					Freehand => freehand::Freehand,
					// Spline => spline::Spline,
					Line => line::Line,
					Rectangle => rectangle::Rectangle,
					Ellipse => ellipse::Ellipse,
					Shape => shape::Shape,
				},
			},
			document_tool_data: DocumentToolData {
				primary_color: Color::BLACK,
				secondary_color: Color::WHITE,
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

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

pub enum StandardToolMessageType {
	Abort,
	DocumentIsDirty,
}

// TODO: Find a nicer way in Rust to make this generic so we don't have to manually map to enum variants
pub fn standard_tool_message(tool: ToolType, message_type: StandardToolMessageType) -> Option<ToolMessage> {
	match message_type {
		StandardToolMessageType::DocumentIsDirty => match tool {
			ToolType::Select => Some(SelectMessage::DocumentIsDirty.into()),
			ToolType::Crop => Some(CropMessage::DocumentIsDirty.into()),
			ToolType::Navigate => None,   // Some(NavigateMessage::DocumentIsDirty.into()),
			ToolType::Eyedropper => None, // Some(EyedropperMessage::DocumentIsDirty.into()),
			ToolType::Text => Some(TextMessage::DocumentIsDirty.into()),
			ToolType::Fill => None,        // Some(FillMessage::DocumentIsDirty.into()),
			ToolType::Gradient => None,    // Some(GradientMessage::DocumentIsDirty.into()),
			ToolType::Brush => None,       // Some(BrushMessage::DocumentIsDirty.into()),
			ToolType::Heal => None,        // Some(HealMessage::DocumentIsDirty.into()),
			ToolType::Clone => None,       // Some(CloneMessage::DocumentIsDirty.into()),
			ToolType::Patch => None,       // Some(PatchMessage::DocumentIsDirty.into()),
			ToolType::BlurSharpen => None, // Some(BlurSharpenMessage::DocumentIsDirty.into()),
			ToolType::Relight => None,     // Some(RelightMessage::DocumentIsDirty.into()),
			ToolType::Path => Some(PathMessage::DocumentIsDirty.into()),
			ToolType::Pen => None,       // Some(PenMessage::DocumentIsDirty.into()),
			ToolType::Freehand => None,  // Some(FreehandMessage::DocumentIsDirty.into()),
			ToolType::Spline => None,    // Some(SplineMessage::DocumentIsDirty.into()),
			ToolType::Line => None,      // Some(LineMessage::DocumentIsDirty.into()),
			ToolType::Rectangle => None, // Some(RectangleMessage::DocumentIsDirty.into()),
			ToolType::Ellipse => None,   // Some(EllipseMessage::DocumentIsDirty.into()),
			ToolType::Shape => None,     // Some(ShapeMessage::DocumentIsDirty.into()),
		},
		StandardToolMessageType::Abort => match tool {
			ToolType::Select => Some(SelectMessage::Abort.into()),
			ToolType::Crop => Some(CropMessage::Abort.into()),
			ToolType::Navigate => Some(NavigateMessage::Abort.into()),
			ToolType::Eyedropper => Some(EyedropperMessage::Abort.into()),
			ToolType::Text => Some(TextMessage::Abort.into()),
			ToolType::Fill => Some(FillMessage::Abort.into()),
			// ToolType::Gradient => Some(GradientMessage::Abort.into()),
			// ToolType::Brush => Some(BrushMessage::Abort.into()),
			// ToolType::Heal => Some(HealMessage::Abort.into()),
			// ToolType::Clone => Some(CloneMessage::Abort.into()),
			// ToolType::Patch => Some(PatchMessage::Abort.into()),
			// ToolType::BlurSharpen => Some(BlurSharpenMessage::Abort.into()),
			// ToolType::Relight => Some(RelightMessage::Abort.into()),
			ToolType::Path => Some(PathMessage::Abort.into()),
			ToolType::Pen => Some(PenMessage::Abort.into()),
			ToolType::Freehand => Some(FreehandMessage::Abort.into()),
			// ToolType::Spline => Some(SplineMessage::Abort.into()),
			ToolType::Line => Some(LineMessage::Abort.into()),
			ToolType::Rectangle => Some(RectangleMessage::Abort.into()),
			ToolType::Ellipse => Some(EllipseMessage::Abort.into()),
			ToolType::Shape => Some(ShapeMessage::Abort.into()),
			_ => None,
		},
	}
}

pub fn message_to_tool_type(message: &ToolMessage) -> ToolType {
	use ToolMessage::*;

	match message {
		Select(_) => ToolType::Select,
		Crop(_) => ToolType::Crop,
		Navigate(_) => ToolType::Navigate,
		Eyedropper(_) => ToolType::Eyedropper,
		Text(_) => ToolType::Text,
		Fill(_) => ToolType::Fill,
		// Gradient(_) => ToolType::Gradient,
		// Brush(_) => ToolType::Brush,
		// Heal(_) => ToolType::Heal,
		// Clone(_) => ToolType::Clone,
		// Patch(_) => ToolType::Patch,
		// BlurSharpen(_) => ToolType::BlurSharpen,
		// Relight(_) => ToolType::Relight,
		Path(_) => ToolType::Path,
		Pen(_) => ToolType::Pen,
		Freehand(_) => ToolType::Freehand,
		// Spline(_) => ToolType::Spline,
		Line(_) => ToolType::Line,
		Rectangle(_) => ToolType::Rectangle,
		Ellipse(_) => ToolType::Ellipse,
		Shape(_) => ToolType::Shape,
		_ => panic!("Conversion from message to tool type impossible because the given ToolMessage does not belong to a tool"),
	}
}

pub fn update_working_colors(document_data: &DocumentToolData, responses: &mut VecDeque<Message>) {
	responses.push_back(
		FrontendMessage::UpdateWorkingColors {
			primary: document_data.primary_color,
			secondary: document_data.secondary_color,
		}
		.into(),
	);
}
