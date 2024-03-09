use super::utility_types::ToolType;
use crate::messages::prelude::*;

use graphene_core::raster::color::Color;

use serde::{Deserialize, Serialize};

#[impl_message(Message, Tool)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum ToolMessage {
	// Sub-messages
	#[child]
	TransformLayer(TransformLayerMessage),

	#[child]
	Select(SelectToolMessage),
	#[child]
	Artboard(ArtboardToolMessage),
	#[child]
	Navigate(NavigateToolMessage),
	#[child]
	Eyedropper(EyedropperToolMessage),
	#[child]
	Fill(FillToolMessage),
	#[child]
	Gradient(GradientToolMessage),

	#[child]
	Path(PathToolMessage),
	#[child]
	Pen(PenToolMessage),
	#[child]
	Freehand(FreehandToolMessage),
	#[child]
	Spline(SplineToolMessage),
	#[child]
	Line(LineToolMessage),
	#[child]
	Rectangle(RectangleToolMessage),
	#[child]
	Ellipse(EllipseToolMessage),
	#[child]
	Polygon(PolygonToolMessage),
	#[child]
	Text(TextToolMessage),

	#[child]
	Brush(BrushToolMessage),
	// 	// #[child]
	// Heal(HealToolMessage),
	// 	// #[child]
	// Clone(CloneToolMessage),
	// 	// #[child]
	// Patch(PatchToolMessage),
	// 	// #[child]
	// Relight(RelightToolMessage),
	// 	// #[child]
	// Detail(DetailToolMessage),
	#[child]
	Imaginate(ImaginateToolMessage),

	// Messages
	ActivateToolSelect,
	ActivateToolArtboard,
	ActivateToolNavigate,
	ActivateToolEyedropper,
	ActivateToolText,
	ActivateToolFill,
	ActivateToolGradient,

	ActivateToolPath,
	ActivateToolPen,
	ActivateToolFreehand,
	ActivateToolSpline,
	ActivateToolLine,
	ActivateToolRectangle,
	ActivateToolEllipse,
	ActivateToolPolygon,

	ActivateToolBrush,
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
