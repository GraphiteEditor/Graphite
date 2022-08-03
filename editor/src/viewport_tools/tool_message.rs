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
	Artboard(ArtboardToolMessage),
	#[remain::unsorted]
	#[child]
	Navigate(NavigateToolMessage),
	#[remain::unsorted]
	#[child]
	Eyedropper(EyedropperToolMessage),
	#[remain::unsorted]
	#[child]
	Fill(FillToolMessage),
	#[remain::unsorted]
	#[child]
	Gradient(GradientToolMessage),

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
	#[remain::unsorted]
	#[child]
	Text(TextToolMessage),

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
	// Relight(RelightToolMessage),
	// #[remain::unsorted]
	// #[child]
	// Detail(DetailToolMessage),

	// Messages
	#[remain::unsorted]
	ActivateToolSelect,
	#[remain::unsorted]
	ActivateToolArtboard,
	#[remain::unsorted]
	ActivateToolNavigate,
	#[remain::unsorted]
	ActivateToolEyedropper,
	#[remain::unsorted]
	ActivateToolText,
	#[remain::unsorted]
	ActivateToolFill,
	#[remain::unsorted]
	ActivateToolGradient,

	#[remain::unsorted]
	ActivateToolPath,
	#[remain::unsorted]
	ActivateToolPen,
	#[remain::unsorted]
	ActivateToolFreehand,
	#[remain::unsorted]
	ActivateToolSpline,
	#[remain::unsorted]
	ActivateToolLine,
	#[remain::unsorted]
	ActivateToolRectangle,
	#[remain::unsorted]
	ActivateToolEllipse,
	#[remain::unsorted]
	ActivateToolShape,

	ActivateTool {
		tool_type: ToolType,
	},
	DeactivateTools,
	InitTools,
	ResetColors,
	SelectPrimaryColor {
		color: Color,
	},
	SelectRandomPrimaryColor,
	SelectSecondaryColor {
		color: Color,
	},
	SwapColors,
	UpdateCursor,
	UpdateHints,
}
