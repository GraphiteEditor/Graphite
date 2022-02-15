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
	Select(SelectToolMessage),
	#[remain::unsorted]
	#[child]
	Crop(CropToolMessage),
	#[remain::unsorted]
	#[child]
	Navigate(NavigateToolMessage),
	#[remain::unsorted]
	#[child]
	Eyedropper(EyedropperToolMessage),
	// #[remain::unsorted]
	// #[child]
	// Text(TextMessage),
	#[remain::unsorted]
	#[child]
	Text(TextMessage),
	#[remain::unsorted]
	#[child]
	Fill(FillToolMessage),
	#[remain::unsorted]
	#[child]
	Gradient(GradientToolMessage),
	// #[remain::unsorted]
	// #[child]
	// Brush(BrushToolMessage),
	// #[remain::unsorted]
	// #[child]
	// Heal(HealToolMessage),
	// #[remain::unsorted]
	// #[child]
	// Clone(CloneToolMessage),
	// #[remain::unsorted]
	// #[child]
	// Patch(PatchToolMessage),
	// #[remain::unsorted]
	// #[child]
	// Detail(DetailToolMessage),
	// #[remain::unsorted]
	// #[child]
	// Relight(RelightToolMessage),
	#[remain::unsorted]
	#[child]
	Path(PathToolMessage),
	#[remain::unsorted]
	#[child]
	Pen(PenToolMessage),
	#[remain::unsorted]
	#[child]
	Freehand(FreehandToolMessage),
	#[remain::unsorted]
	#[child]
	Spline(SplineToolMessage),
	#[remain::unsorted]
	#[child]
	Line(LineToolMessage),
	#[remain::unsorted]
	#[child]
	Rectangle(RectangleToolMessage),
	#[remain::unsorted]
	#[child]
	Ellipse(EllipseToolMessage),
	#[remain::unsorted]
	#[child]
	Shape(ShapeToolMessage),

	// Messages
	#[remain::unsorted]
	NoOp,
	AbortCurrentTool,
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
	SwapColors,
	UpdateCursor,
	UpdateHints,
}
