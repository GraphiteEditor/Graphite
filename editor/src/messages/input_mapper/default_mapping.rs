use crate::consts::{BIG_NUDGE_AMOUNT, NUDGE_AMOUNT};
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, KeyStates};
use crate::messages::input_mapper::utility_types::macros::*;
use crate::messages::input_mapper::utility_types::misc::MappingEntry;
use crate::messages::input_mapper::utility_types::misc::{KeyMappingEntries, Mapping};
use crate::messages::portfolio::document::utility_types::clipboards::Clipboard;
use crate::messages::prelude::*;

use glam::DVec2;

pub fn default_mapping() -> Mapping {
	use InputMapperMessage::*;
	use Key::*;

	// NOTICE:
	// If a new mapping you added here isn't working (and perhaps another lower-precedence one is instead), make sure to advertise
	// it as an available action in the respective message handler file (such as the bottom of `document_message_handler.rs`).

	let mappings = mapping![
		// HIGHER PRIORITY:
		//
		// NavigationMessage
		entry!(
			PointerMove;
			refresh_keys=[Control],
			action_dispatch=NavigationMessage::PointerMove { snap_angle: Control, wait_for_snap_angle_release: true, snap_zoom: Control, zoom_from_viewport: None },
		),
		// NORMAL PRIORITY:
		//
		// NodeGraphMessage
		entry!(KeyDown(Delete); action_dispatch=NodeGraphMessage::DeleteSelectedNodes),
		entry!(KeyDown(Backspace); action_dispatch=NodeGraphMessage::DeleteSelectedNodes),
		entry!(KeyDown(KeyX); modifiers=[Accel], action_dispatch=NodeGraphMessage::Cut),
		entry!(KeyDown(KeyC); modifiers=[Accel], action_dispatch=NodeGraphMessage::Copy),
		entry!(KeyDown(KeyD); modifiers=[Accel], action_dispatch=NodeGraphMessage::DuplicateSelectedNodes),
		entry!(KeyDown(KeyH); modifiers=[Accel], action_dispatch=NodeGraphMessage::ToggleHidden),
		//
		// TransformLayerMessage
		entry!(KeyDown(Enter); action_dispatch=TransformLayerMessage::ApplyTransformOperation),
		entry!(KeyDown(Lmb); action_dispatch=TransformLayerMessage::ApplyTransformOperation),
		entry!(KeyDown(Escape); action_dispatch=TransformLayerMessage::CancelTransformOperation),
		entry!(KeyDown(Rmb); action_dispatch=TransformLayerMessage::CancelTransformOperation),
		entry!(KeyDown(KeyX); action_dispatch=TransformLayerMessage::ConstrainX),
		entry!(KeyDown(KeyY); action_dispatch=TransformLayerMessage::ConstrainY),
		entry!(KeyDown(Backspace); action_dispatch=TransformLayerMessage::TypeBackspace),
		entry!(KeyDown(Minus); action_dispatch=TransformLayerMessage::TypeNegate),
		entry!(KeyDown(Comma); action_dispatch=TransformLayerMessage::TypeDecimalPoint),
		entry!(KeyDown(Period); action_dispatch=TransformLayerMessage::TypeDecimalPoint),
		entry!(PointerMove; refresh_keys=[Shift, Control], action_dispatch=TransformLayerMessage::PointerMove { slow_key: Shift, snap_key: Control }),
		//
		// SelectToolMessage
		entry!(PointerMove; refresh_keys=[Control, Shift, Alt], action_dispatch=SelectToolMessage::PointerMove { axis_align: Shift, snap_angle: Control, center: Alt, duplicate: Alt }),
		entry!(KeyDown(Lmb); action_dispatch=SelectToolMessage::DragStart { add_to_selection: Shift }),
		entry!(KeyUp(Lmb); action_dispatch=SelectToolMessage::DragStop),
		entry!(KeyDown(Enter); action_dispatch=SelectToolMessage::DragStop),
		entry!(DoubleClick; action_dispatch=SelectToolMessage::EditLayer),
		entry!(KeyDown(Rmb); action_dispatch=SelectToolMessage::Abort),
		entry!(KeyDown(Escape); action_dispatch=SelectToolMessage::Abort),
		//
		// ArtboardToolMessage
		entry!(KeyDown(Lmb); action_dispatch=ArtboardToolMessage::PointerDown),
		entry!(PointerMove; refresh_keys=[Shift, Alt], action_dispatch=ArtboardToolMessage::PointerMove { constrain_axis_or_aspect: Shift, center: Alt }),
		entry!(KeyUp(Lmb); action_dispatch=ArtboardToolMessage::PointerUp),
		entry!(KeyDown(Delete); action_dispatch=ArtboardToolMessage::DeleteSelected),
		entry!(KeyDown(Backspace); action_dispatch=ArtboardToolMessage::DeleteSelected),
		entry!(KeyDown(ArrowUp); modifiers=[Shift, ArrowLeft], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: -BIG_NUDGE_AMOUNT, delta_y: -BIG_NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowUp); modifiers=[Shift, ArrowRight], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: BIG_NUDGE_AMOUNT, delta_y: -BIG_NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowUp); modifiers=[Shift], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: 0., delta_y: -BIG_NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowDown); modifiers=[Shift, ArrowLeft], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: -BIG_NUDGE_AMOUNT, delta_y: BIG_NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowDown); modifiers=[Shift, ArrowRight], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: BIG_NUDGE_AMOUNT, delta_y: BIG_NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowDown); modifiers=[Shift], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: 0., delta_y: BIG_NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowLeft); modifiers=[Shift, ArrowUp], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: -BIG_NUDGE_AMOUNT, delta_y: -BIG_NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowLeft); modifiers=[Shift, ArrowDown], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: -BIG_NUDGE_AMOUNT, delta_y: BIG_NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowLeft); modifiers=[Shift], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: -BIG_NUDGE_AMOUNT, delta_y: 0. }),
		entry!(KeyDown(ArrowRight); modifiers=[Shift, ArrowUp], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: BIG_NUDGE_AMOUNT, delta_y: -BIG_NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowRight); modifiers=[Shift, ArrowDown], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: BIG_NUDGE_AMOUNT, delta_y: BIG_NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowRight); modifiers=[Shift], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: BIG_NUDGE_AMOUNT, delta_y: 0. }),
		entry!(KeyDown(ArrowUp); modifiers=[ArrowLeft], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: -NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowUp); modifiers=[ArrowRight], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowUp); action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: 0., delta_y: -NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowDown); modifiers=[ArrowLeft], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: -NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowDown); modifiers=[ArrowRight], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowDown); action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: 0., delta_y: NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowLeft); modifiers=[ArrowUp], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: -NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowLeft); modifiers=[ArrowDown], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: -NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowLeft); action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: -NUDGE_AMOUNT, delta_y: 0. }),
		entry!(KeyDown(ArrowRight); modifiers=[ArrowUp], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowRight); modifiers=[ArrowDown], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowRight); action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: NUDGE_AMOUNT, delta_y: 0. }),
		//
		// NavigateToolMessage
		entry!(KeyUp(Lmb); modifiers=[Shift], action_dispatch=NavigateToolMessage::ClickZoom { zoom_in: false }),
		entry!(KeyUp(Lmb); action_dispatch=NavigateToolMessage::ClickZoom { zoom_in: true }),
		entry!(PointerMove; refresh_keys=[Control], action_dispatch=NavigateToolMessage::PointerMove { snap_angle: Control, snap_zoom: Control }),
		entry!(KeyDown(Mmb); action_dispatch=NavigateToolMessage::TranslateCanvasBegin),
		entry!(KeyDown(Rmb); action_dispatch=NavigateToolMessage::RotateCanvasBegin),
		entry!(KeyDown(Lmb); action_dispatch=NavigateToolMessage::ZoomCanvasBegin),
		entry!(KeyUp(Rmb); action_dispatch=NavigateToolMessage::TransformCanvasEnd),
		entry!(KeyUp(Lmb); action_dispatch=NavigateToolMessage::TransformCanvasEnd),
		entry!(KeyUp(Mmb); action_dispatch=NavigateToolMessage::TransformCanvasEnd),
		//
		// EyedropperToolMessage
		entry!(PointerMove; action_dispatch=EyedropperToolMessage::PointerMove),
		entry!(KeyDown(Lmb); action_dispatch=EyedropperToolMessage::LeftPointerDown),
		entry!(KeyDown(Rmb); action_dispatch=EyedropperToolMessage::RightPointerDown),
		entry!(KeyUp(Lmb); action_dispatch=EyedropperToolMessage::LeftPointerUp),
		entry!(KeyUp(Rmb); action_dispatch=EyedropperToolMessage::RightPointerUp),
		entry!(KeyDown(Escape); action_dispatch=EyedropperToolMessage::Abort),
		//
		// TextToolMessage
		entry!(KeyUp(Lmb); action_dispatch=TextToolMessage::Interact),
		entry!(KeyDown(Escape); action_dispatch=TextToolMessage::Abort),
		entry!(KeyDown(Enter); modifiers=[Accel], action_dispatch=TextToolMessage::CommitText),
		//
		// GradientToolMessage
		entry!(KeyDown(Lmb); action_dispatch=GradientToolMessage::PointerDown),
		entry!(PointerMove; refresh_keys=[Shift], action_dispatch=GradientToolMessage::PointerMove { constrain_axis: Shift }),
		entry!(KeyUp(Lmb); action_dispatch=GradientToolMessage::PointerUp),
		entry!(DoubleClick; action_dispatch=GradientToolMessage::InsertStop),
		entry!(KeyDown(Delete); action_dispatch=GradientToolMessage::DeleteStop),
		entry!(KeyDown(Backspace); action_dispatch=GradientToolMessage::DeleteStop),
		//
		// RectangleToolMessage
		entry!(KeyDown(Lmb); action_dispatch=RectangleToolMessage::DragStart),
		entry!(KeyUp(Lmb); action_dispatch=RectangleToolMessage::DragStop),
		entry!(KeyDown(Rmb); action_dispatch=RectangleToolMessage::Abort),
		entry!(KeyDown(Escape); action_dispatch=RectangleToolMessage::Abort),
		entry!(PointerMove; refresh_keys=[Alt, Shift], action_dispatch=RectangleToolMessage::Resize { center: Alt, lock_ratio: Shift }),
		//
		// ImaginateToolMessage
		entry!(KeyDown(Lmb); action_dispatch=ImaginateToolMessage::DragStart),
		entry!(KeyUp(Lmb); action_dispatch=ImaginateToolMessage::DragStop),
		entry!(KeyDown(Rmb); action_dispatch=ImaginateToolMessage::Abort),
		entry!(KeyDown(Escape); action_dispatch=ImaginateToolMessage::Abort),
		entry!(PointerMove; refresh_keys=[Alt, Shift], action_dispatch=ImaginateToolMessage::Resize { center: Alt, lock_ratio: Shift }),
		//
		// NodeGraphFrameToolMessage
		entry!(KeyDown(Lmb); action_dispatch=NodeGraphFrameToolMessage::DragStart),
		entry!(KeyUp(Lmb); action_dispatch=NodeGraphFrameToolMessage::DragStop),
		entry!(KeyDown(Rmb); action_dispatch=NodeGraphFrameToolMessage::Abort),
		entry!(KeyDown(Escape); action_dispatch=NodeGraphFrameToolMessage::Abort),
		entry!(PointerMove; refresh_keys=[Alt, Shift], action_dispatch=NodeGraphFrameToolMessage::Resize { center: Alt, lock_ratio: Shift }),
		//
		// EllipseToolMessage
		entry!(KeyDown(Lmb); action_dispatch=EllipseToolMessage::DragStart),
		entry!(KeyUp(Lmb); action_dispatch=EllipseToolMessage::DragStop),
		entry!(KeyDown(Rmb); action_dispatch=EllipseToolMessage::Abort),
		entry!(KeyDown(Escape); action_dispatch=EllipseToolMessage::Abort),
		entry!(PointerMove; refresh_keys=[Alt, Shift], action_dispatch=EllipseToolMessage::Resize { center: Alt, lock_ratio: Shift }),
		//
		// ShapeToolMessage
		entry!(KeyDown(Lmb); action_dispatch=ShapeToolMessage::DragStart),
		entry!(KeyUp(Lmb); action_dispatch=ShapeToolMessage::DragStop),
		entry!(KeyDown(Rmb); action_dispatch=ShapeToolMessage::Abort),
		entry!(KeyDown(Escape); action_dispatch=ShapeToolMessage::Abort),
		entry!(PointerMove; refresh_keys=[Alt, Shift], action_dispatch=ShapeToolMessage::Resize { center: Alt, lock_ratio: Shift }),
		//
		// LineToolMessage
		entry!(KeyDown(Lmb); action_dispatch=LineToolMessage::DragStart),
		entry!(KeyUp(Lmb); action_dispatch=LineToolMessage::DragStop),
		entry!(KeyDown(Rmb); action_dispatch=LineToolMessage::Abort),
		entry!(KeyDown(Escape); action_dispatch=LineToolMessage::Abort),
		entry!(PointerMove; refresh_keys=[Alt, Shift, Control], action_dispatch=LineToolMessage::Redraw { center: Alt, lock_angle: Control, snap_angle: Shift }),
		//
		// PathToolMessage
		entry!(KeyDown(Lmb); action_dispatch=PathToolMessage::DragStart { add_to_selection: Shift }),
		entry!(PointerMove; refresh_keys=[Alt, Shift], action_dispatch=PathToolMessage::PointerMove { alt_mirror_angle: Alt, shift_mirror_distance: Shift }),
		entry!(KeyDown(Delete); action_dispatch=PathToolMessage::Delete),
		entry!(KeyDown(Backspace); action_dispatch=PathToolMessage::Delete),
		entry!(KeyUp(Lmb); action_dispatch=PathToolMessage::DragStop),
		entry!(DoubleClick; action_dispatch=PathToolMessage::InsertPoint),
		//
		// PenToolMessage
		entry!(PointerMove; refresh_keys=[Shift, Control], action_dispatch=PenToolMessage::PointerMove { snap_angle: Control, break_handle: Shift }),
		entry!(KeyDown(Lmb); action_dispatch=PenToolMessage::DragStart),
		entry!(KeyUp(Lmb); action_dispatch=PenToolMessage::DragStop),
		entry!(KeyDown(Rmb); action_dispatch=PenToolMessage::Confirm),
		entry!(KeyDown(Escape); action_dispatch=PenToolMessage::Confirm),
		entry!(KeyDown(Enter); action_dispatch=PenToolMessage::Confirm),
		//
		// FreehandToolMessage
		entry!(PointerMove; action_dispatch=FreehandToolMessage::PointerMove),
		entry!(KeyDown(Lmb); action_dispatch=FreehandToolMessage::DragStart),
		entry!(KeyUp(Lmb); action_dispatch=FreehandToolMessage::DragStop),
		//
		// SplineToolMessage
		entry!(PointerMove; action_dispatch=SplineToolMessage::PointerMove),
		entry!(KeyDown(Lmb); action_dispatch=SplineToolMessage::DragStart),
		entry!(KeyUp(Lmb); action_dispatch=SplineToolMessage::DragStop),
		entry!(KeyDown(Rmb); action_dispatch=SplineToolMessage::Confirm),
		entry!(KeyDown(Escape); action_dispatch=SplineToolMessage::Confirm),
		entry!(KeyDown(Enter); action_dispatch=SplineToolMessage::Confirm),
		//
		// FillToolMessage
		entry!(KeyDown(Lmb); action_dispatch=FillToolMessage::LeftPointerDown),
		entry!(KeyDown(Rmb); action_dispatch=FillToolMessage::RightPointerDown),
		//
		// ToolMessage
		entry!(KeyDown(KeyV); action_dispatch=ToolMessage::ActivateToolSelect),
		entry!(KeyDown(KeyZ); action_dispatch=ToolMessage::ActivateToolNavigate),
		entry!(KeyDown(KeyI); action_dispatch=ToolMessage::ActivateToolEyedropper),
		entry!(KeyDown(KeyT); action_dispatch=ToolMessage::ActivateToolText),
		entry!(KeyDown(KeyF); action_dispatch=ToolMessage::ActivateToolFill),
		entry!(KeyDown(KeyH); action_dispatch=ToolMessage::ActivateToolGradient),
		entry!(KeyDown(KeyA); action_dispatch=ToolMessage::ActivateToolPath),
		entry!(KeyDown(KeyP); action_dispatch=ToolMessage::ActivateToolPen),
		entry!(KeyDown(KeyN); action_dispatch=ToolMessage::ActivateToolFreehand),
		entry!(KeyDown(KeyL); action_dispatch=ToolMessage::ActivateToolLine),
		entry!(KeyDown(KeyM); action_dispatch=ToolMessage::ActivateToolRectangle),
		entry!(KeyDown(KeyE); action_dispatch=ToolMessage::ActivateToolEllipse),
		entry!(KeyDown(KeyY); action_dispatch=ToolMessage::ActivateToolShape),
		entry!(KeyDown(KeyX); modifiers=[Shift, Accel], action_dispatch=ToolMessage::ResetColors),
		entry!(KeyDown(KeyX); modifiers=[Shift], action_dispatch=ToolMessage::SwapColors),
		entry!(KeyDown(KeyC); modifiers=[Alt], action_dispatch=ToolMessage::SelectRandomPrimaryColor),
		//
		// DocumentMessage
		entry!(KeyDown(Delete); action_dispatch=DocumentMessage::DeleteSelectedLayers),
		entry!(KeyDown(Backspace); action_dispatch=DocumentMessage::DeleteSelectedLayers),
		entry!(KeyDown(KeyP); modifiers=[Alt], action_dispatch=DocumentMessage::DebugPrintDocument),
		entry!(KeyDown(KeyZ); modifiers=[Accel, Shift], action_dispatch=DocumentMessage::Redo),
		entry!(KeyDown(KeyZ); modifiers=[Accel], action_dispatch=DocumentMessage::Undo),
		entry!(KeyDown(KeyA); modifiers=[Accel, Shift], action_dispatch=DocumentMessage::DeselectAllLayers),
		entry!(KeyDown(KeyA); modifiers=[Accel], action_dispatch=DocumentMessage::SelectAllLayers),
		entry!(KeyDown(KeyS); modifiers=[Accel], action_dispatch=DocumentMessage::SaveDocument),
		entry!(KeyDown(KeyD); modifiers=[Accel], action_dispatch=DocumentMessage::DuplicateSelectedLayers),
		entry!(KeyDown(KeyG); modifiers=[Accel], action_dispatch=DocumentMessage::GroupSelectedLayers),
		entry!(KeyDown(KeyG); modifiers=[Accel, Shift], action_dispatch=DocumentMessage::UngroupSelectedLayers),
		entry!(KeyDown(KeyN); modifiers=[Accel, Shift], action_dispatch=DocumentMessage::CreateEmptyFolder { container_path: vec![] }),
		entry!(KeyDown(Digit0); modifiers=[Accel], action_dispatch=DocumentMessage::ZoomCanvasToFitAll),
		entry!(KeyDown(Digit1); modifiers=[Accel], action_dispatch=DocumentMessage::ZoomCanvasTo100Percent),
		entry!(KeyDown(Digit2); modifiers=[Accel], action_dispatch=DocumentMessage::ZoomCanvasTo200Percent),
		entry!(KeyDown(BracketLeft); modifiers=[Accel, Shift], action_dispatch=DocumentMessage::SelectedLayersLowerToBack),
		entry!(KeyDown(BracketRight); modifiers=[Accel, Shift], action_dispatch=DocumentMessage::SelectedLayersRaiseToFront),
		entry!(KeyDown(BracketLeft); modifiers=[Accel], action_dispatch=DocumentMessage::SelectedLayersLower),
		entry!(KeyDown(BracketRight); modifiers=[Accel], action_dispatch=DocumentMessage::SelectedLayersRaise),
		entry!(KeyDown(ArrowUp); modifiers=[Shift, ArrowLeft], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -BIG_NUDGE_AMOUNT, delta_y: -BIG_NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowUp); modifiers=[Shift, ArrowRight], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: BIG_NUDGE_AMOUNT, delta_y: -BIG_NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowUp); modifiers=[Shift], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: 0., delta_y: -BIG_NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowDown); modifiers=[Shift, ArrowLeft], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -BIG_NUDGE_AMOUNT, delta_y: BIG_NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowDown); modifiers=[Shift, ArrowRight], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: BIG_NUDGE_AMOUNT, delta_y: BIG_NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowDown); modifiers=[Shift], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: 0., delta_y: BIG_NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowLeft); modifiers=[Shift, ArrowUp], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -BIG_NUDGE_AMOUNT, delta_y: -BIG_NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowLeft); modifiers=[Shift, ArrowDown], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -BIG_NUDGE_AMOUNT, delta_y: BIG_NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowLeft); modifiers=[Shift], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -BIG_NUDGE_AMOUNT, delta_y: 0. }),
		entry!(KeyDown(ArrowRight); modifiers=[Shift, ArrowUp], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: BIG_NUDGE_AMOUNT, delta_y: -BIG_NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowRight); modifiers=[Shift, ArrowDown], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: BIG_NUDGE_AMOUNT, delta_y: BIG_NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowRight); modifiers=[Shift], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: BIG_NUDGE_AMOUNT, delta_y: 0. }),
		entry!(KeyDown(ArrowUp); modifiers=[ArrowLeft], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowUp); modifiers=[ArrowRight], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowUp); action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: 0., delta_y: -NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowDown); modifiers=[ArrowLeft], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowDown); modifiers=[ArrowRight], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowDown); action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: 0., delta_y: NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowLeft); modifiers=[ArrowUp], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowLeft); modifiers=[ArrowDown], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowLeft); action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: 0. }),
		entry!(KeyDown(ArrowRight); modifiers=[ArrowUp], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowRight); modifiers=[ArrowDown], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowRight); action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: 0. }),
		//
		// TransformLayerMessage
		entry!(KeyDown(KeyG); action_dispatch=TransformLayerMessage::BeginGrab),
		entry!(KeyDown(KeyR); action_dispatch=TransformLayerMessage::BeginRotate),
		entry!(KeyDown(KeyS); action_dispatch=TransformLayerMessage::BeginScale),
		//
		// NavigationMessage
		entry!(KeyDown(Mmb); modifiers=[Control], action_dispatch=NavigationMessage::RotateCanvasBegin),
		entry!(KeyDown(Mmb); modifiers=[Shift], action_dispatch=NavigationMessage::ZoomCanvasBegin),
		entry!(KeyDown(Mmb); action_dispatch=NavigationMessage::TranslateCanvasBegin),
		entry!(KeyUp(Mmb); action_dispatch=NavigationMessage::TransformCanvasEnd),
		entry!(KeyDown(Lmb); modifiers=[Space], action_dispatch=NavigationMessage::TranslateCanvasBegin),
		entry!(KeyUp(Lmb); modifiers=[Space], action_dispatch=NavigationMessage::TransformCanvasEnd),
		entry!(KeyDown(NumpadAdd); modifiers=[Accel], action_dispatch=NavigationMessage::IncreaseCanvasZoom { center_on_mouse: false }),
		entry!(KeyDown(Equal); modifiers=[Accel], action_dispatch=NavigationMessage::IncreaseCanvasZoom { center_on_mouse: false }),
		entry!(KeyDown(Minus); modifiers=[Accel], action_dispatch=NavigationMessage::DecreaseCanvasZoom { center_on_mouse: false }),
		entry!(WheelScroll; modifiers=[Control], action_dispatch=NavigationMessage::WheelCanvasZoom),
		entry!(WheelScroll; modifiers=[Shift], action_dispatch=NavigationMessage::WheelCanvasTranslate { use_y_as_x: true }),
		entry!(WheelScroll; action_dispatch=NavigationMessage::WheelCanvasTranslate { use_y_as_x: false }),
		entry!(KeyDown(PageUp); modifiers=[Shift], action_dispatch=NavigationMessage::TranslateCanvasByViewportFraction { delta: DVec2::new(1., 0.) }),
		entry!(KeyDown(PageDown); modifiers=[Shift], action_dispatch=NavigationMessage::TranslateCanvasByViewportFraction { delta: DVec2::new(-1., 0.) }),
		entry!(KeyDown(PageUp); action_dispatch=NavigationMessage::TranslateCanvasByViewportFraction { delta: DVec2::new(0., 1.) }),
		entry!(KeyDown(PageDown); action_dispatch=NavigationMessage::TranslateCanvasByViewportFraction { delta: DVec2::new(0., -1.) }),
		//
		// PortfolioMessage
		entry!(KeyDown(KeyO); modifiers=[Accel], action_dispatch=PortfolioMessage::OpenDocument),
		entry!(KeyDown(KeyI); modifiers=[Accel], action_dispatch=PortfolioMessage::Import),
		entry!(KeyDown(Tab); modifiers=[Control], action_dispatch=PortfolioMessage::NextDocument),
		entry!(KeyDown(Tab); modifiers=[Control, Shift], action_dispatch=PortfolioMessage::PrevDocument),
		entry!(KeyDown(KeyW); modifiers=[Accel], action_dispatch=PortfolioMessage::CloseActiveDocumentWithConfirmation),
		entry!(KeyDown(KeyX); modifiers=[Accel], action_dispatch=PortfolioMessage::Cut { clipboard: Clipboard::Device }),
		entry!(KeyDown(KeyC); modifiers=[Accel], action_dispatch=PortfolioMessage::Copy { clipboard: Clipboard::Device }),
		entry!(KeyDown(KeyV); modifiers=[Accel], action_dispatch=FrontendMessage::TriggerPaste),
		//
		// DialogMessage
		entry!(KeyDown(KeyW); modifiers=[Accel, Alt], action_dispatch=DialogMessage::CloseAllDocumentsWithConfirmation),
		entry!(KeyDown(KeyE); modifiers=[Accel], action_dispatch=DialogMessage::RequestExportDialog),
		entry!(KeyDown(KeyN); modifiers=[Accel], action_dispatch=DialogMessage::RequestNewDocumentDialog),
		entry!(KeyDown(Comma); modifiers=[Accel], action_dispatch=DialogMessage::RequestPreferencesDialog),
		//
		// DebugMessage
		entry!(KeyDown(KeyT); modifiers=[Alt], action_dispatch=DebugMessage::ToggleTraceLogs),
		entry!(KeyDown(Digit0); modifiers=[Alt], action_dispatch=DebugMessage::MessageOff),
		entry!(KeyDown(Digit1); modifiers=[Alt], action_dispatch=DebugMessage::MessageNames),
		entry!(KeyDown(Digit2); modifiers=[Alt], action_dispatch=DebugMessage::MessageContents),
	];
	let (mut key_up, mut key_down, mut double_click, mut wheel_scroll, mut pointer_move) = mappings;

	// TODO: Hardcode these 10 lines into 10 lines of declarations, or make this use a macro to do all 10 in one line
	const NUMBER_KEYS: [Key; 10] = [Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9];
	for (i, key) in NUMBER_KEYS.iter().enumerate() {
		key_down[*key as usize].0.insert(
			0,
			MappingEntry {
				action: TransformLayerMessage::TypeDigit { digit: i as u8 }.into(),
				input: InputMapperMessage::KeyDown(*key),
				modifiers: modifiers!(),
			},
		);
	}

	let sort = |list: &mut KeyMappingEntries| list.0.sort_by(|u, v| v.modifiers.ones().cmp(&u.modifiers.ones()));
	for list in [&mut key_up, &mut key_down] {
		for sublist in list {
			sort(sublist);
		}
	}
	sort(&mut double_click);
	sort(&mut wheel_scroll);
	sort(&mut pointer_move);

	Mapping {
		key_up,
		key_down,
		double_click,
		wheel_scroll,
		pointer_move,
	}
}
