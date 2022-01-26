use super::tool::ToolType;
use super::tool_options::ToolOptions;
use crate::message_prelude::*;

use graphene::color::Color;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, Tool)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum ToolMessage {
	// Sub-messages
	#[remain::unsorted]
	#[child]
	Crop(CropMessage),
	#[remain::unsorted]
	#[child]
	Ellipse(EllipseMessage),
	#[remain::unsorted]
	#[child]
	Eyedropper(EyedropperMessage),
	#[remain::unsorted]
	#[child]
	Fill(FillMessage),
	#[remain::unsorted]
	#[child]
	Line(LineMessage),
	#[remain::unsorted]
	#[child]
	Navigate(NavigateMessage),
	#[remain::unsorted]
	#[child]
	Path(PathMessage),
	#[remain::unsorted]
	#[child]
	Pen(PenMessage),
	#[remain::unsorted]
	#[child]
	Rectangle(RectangleMessage),
	#[remain::unsorted]
	#[child]
	Select(SelectMessage),
	#[remain::unsorted]
	#[child]
	Shape(ShapeMessage),

	// Messages
	#[remain::unsorted]
	NoOp,
	ActivateTool {
		tool_type: ToolType,
	},
	DocumentIsDirty,
	ResetColors,
	SelectionChanged,
	SelectPrimaryColor {
		color: Color,
	},
	SelectSecondaryColor {
		color: Color,
	},
	SetToolOptions {
		tool_type: ToolType,
		tool_options: ToolOptions,
	},
	SwapColors,
	UpdateCursor,
	UpdateHints,
}
