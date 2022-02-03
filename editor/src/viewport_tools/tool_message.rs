use super::tool::ToolType;
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
	Select(SelectMessage),
	#[remain::unsorted]
	#[child]
	Crop(CropMessage),
	#[remain::unsorted]
	#[child]
	Navigate(NavigateMessage),
	#[remain::unsorted]
	#[child]
	Eyedropper(EyedropperMessage),
	// #[remain::unsorted]
	// #[child]
	// Text(TextMessage),
	#[remain::unsorted]
	#[child]
	Text(TextMessage),
	#[remain::unsorted]
	#[child]
	Fill(FillMessage),
	// #[remain::unsorted]
	// #[child]
	// Gradient(GradientMessage),
	// #[remain::unsorted]
	// #[child]
	// Brush(BrushMessage),
	// #[remain::unsorted]
	// #[child]
	// Heal(HealMessage),
	// #[remain::unsorted]
	// #[child]
	// Clone(CloneMessage),
	// #[remain::unsorted]
	// #[child]
	// Patch(PatchMessage),
	// #[remain::unsorted]
	// #[child]
	// Detail(DetailMessage),
	// #[remain::unsorted]
	// #[child]
	// Relight(RelightMessage),
	#[remain::unsorted]
	#[child]
	Path(PathMessage),
	#[remain::unsorted]
	#[child]
	Pen(PenMessage),
	#[remain::unsorted]
	#[child]
	Freehand(FreehandMessage),
	// #[remain::unsorted]
	// #[child]
	// Spline(SplineMessage),
	#[remain::unsorted]
	#[child]
	Line(LineMessage),
	#[remain::unsorted]
	#[child]
	Rectangle(RectangleMessage),
	#[remain::unsorted]
	#[child]
	Ellipse(EllipseMessage),
	#[remain::unsorted]
	#[child]
	Shape(ShapeMessage),

	// Messages
	#[remain::unsorted]
	NoOp,
	AbortCurrentTool,
	ActivateTool {
		tool_type: ToolType,
	},
	DocumentIsDirty,
	ResetColors,
	SelectPrimaryColor {
		color: Color,
	},
	SelectSecondaryColor {
		color: Color,
	},
	SwapColors,
	UpdateCursor,
	UpdateHints,
}
