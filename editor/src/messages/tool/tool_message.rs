use super::utility_types::ToolType;
use crate::messages::preferences::SelectionMode;
use crate::messages::prelude::*;
use graphene_std::raster::color::Color;

#[impl_message(Message, Tool)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
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
	Shape(ShapeToolMessage),
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

	// Messages

	// General tools
	ActivateToolSelect,
	ActivateToolArtboard,
	ActivateToolNavigate,
	ActivateToolEyedropper,
	ActivateToolFill,
	ActivateToolGradient,
	// Vector tools
	ActivateToolPath,
	ActivateToolPen,
	ActivateToolFreehand,
	ActivateToolSpline,
	ActivateToolShapeLine,
	ActivateToolShapeRectangle,
	ActivateToolShapeEllipse,
	ActivateToolShape,
	ActivateToolText,
	// Raster tools
	ActivateToolBrush,

	ActivateTool {
		tool_type: ToolType,
	},
	DeactivateTools,
	InitTools,
	PreUndo,
	Redo,
	RefreshToolOptions,
	ResetColors,
	SelectWorkingColor {
		color: Color,
		primary: bool,
	},
	SelectRandomWorkingColor {
		primary: bool,
	},
	ToggleSelectVsPath,
	SwapColors,
	Undo,
	UpdateCursor,
	UpdateHints,
	UpdateSelectionMode {
		selection_mode: SelectionMode,
	},
}
