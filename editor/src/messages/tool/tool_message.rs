use super::utility_types::ToolType;
use crate::messages::prelude::*;

use graphene_core::raster::color::Color;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, Tool)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum ToolMessage {
	// Sub-messages
	#[remain::unsorted]
	#[child]
	TransformLayer(TransformLayerMessage),

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
	Polygon(PolygonToolMessage),
	#[remain::unsorted]
	#[child]
	Text(TextToolMessage),

	#[remain::unsorted]
	#[child]
	Brush(BrushToolMessage),
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
	#[remain::unsorted]
	#[child]
	Imaginate(ImaginateToolMessage),

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
	ActivateToolPolygon,

	#[remain::unsorted]
	ActivateToolBrush,
	#[remain::unsorted]
	ActivateToolImaginate,

	ActivateTool {
		tool_type: ToolType,
	},
	DeactivateTools,
	InitTools,
	PreUndo,
	Redo,
	RefreshToolOptions,
	ResetColors,
	SelectPrimaryColor {
		color: Color,
	},
	SelectRandomPrimaryColor,
	SelectSecondaryColor {
		color: Color,
	},
	SwapColors,
	Undo,
	UpdateCursor,
	UpdateHints,
}
