use super::tool::ToolType;
use super::tool_options::ToolOptions;
use crate::message_prelude::*;

use graphene::color::Color;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, Tool)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum ToolMessage {
	ActivateTool(ToolType),
	#[child]
	Crop(CropMessage),
	DocumentIsDirty,
	#[child]
	Ellipse(EllipseMessage),
	#[child]
	Eyedropper(EyedropperMessage),
	#[child]
	Fill(FillMessage),
	#[child]
	Line(LineMessage),
	#[child]
	Navigate(NavigateMessage),
	NoOp,
	#[child]
	Path(PathMessage),
	#[child]
	Pen(PenMessage),
	#[child]
	Rectangle(RectangleMessage),
	ResetColors,
	#[child]
	Select(SelectMessage),
	SelectPrimaryColor(Color),
	SelectSecondaryColor(Color),
	SetToolOptions(ToolType, ToolOptions),
	#[child]
	Shape(ShapeMessage),
	SwapColors,
	UpdateHints,
}
