use super::input_mapper_macros::*;
use super::keyboard::{Key, KeyStates, NUMBER_OF_KEYS};
use crate::consts::{BIG_NUDGE_AMOUNT, NUDGE_AMOUNT};
use crate::document::clipboards::Clipboard;
use crate::document::utility_types::KeyboardPlatformLayout;
use crate::message_prelude::*;

use glam::DVec2;
use serde::{Serialize, Serializer};

#[derive(Debug, Clone)]
pub struct Mapping {
	pub key_up: [KeyMappingEntries; NUMBER_OF_KEYS],
	pub key_down: [KeyMappingEntries; NUMBER_OF_KEYS],
	pub double_click: KeyMappingEntries,
	pub wheel_scroll: KeyMappingEntries,
	pub pointer_move: KeyMappingEntries,
}

impl Default for Mapping {
	fn default() -> Self {
		use InputMapperMessage::*;
		use Key::*;

		// WARNING!
		// If a new mapping you added here isn't working (and perhaps another lower-precedence one is instead), make sure to advertise
		// it as an available action in the respective message handler file (such as the bottom of `document_message_handler.rs`).

		let mappings = mapping![
			// HIGHER PRIORITY:
			//
			// MovementMessage
			entry!(
				PointerMove;
				refresh_keys=[KeyControl],
				action_dispatch=MovementMessage::PointerMove { snap_angle: KeyControl, wait_for_snap_angle_release: true, snap_zoom: KeyControl, zoom_from_viewport: None },
			),
			// NORMAL PRIORITY:
			//
			// TransformLayerMessage
			entry!(KeyDown(KeyEnter); action_dispatch=TransformLayerMessage::ApplyTransformOperation),
			entry!(KeyDown(Lmb); action_dispatch=TransformLayerMessage::ApplyTransformOperation),
			entry!(KeyDown(KeyEscape); action_dispatch=TransformLayerMessage::CancelTransformOperation),
			entry!(KeyDown(Rmb); action_dispatch=TransformLayerMessage::CancelTransformOperation),
			entry!(KeyDown(KeyX); action_dispatch=TransformLayerMessage::ConstrainX),
			entry!(KeyDown(KeyY); action_dispatch=TransformLayerMessage::ConstrainY),
			entry!(KeyDown(KeyBackspace); action_dispatch=TransformLayerMessage::TypeBackspace),
			entry!(KeyDown(KeyMinus); action_dispatch=TransformLayerMessage::TypeNegate),
			entry!(KeyDown(KeyComma); action_dispatch=TransformLayerMessage::TypeDecimalPoint),
			entry!(KeyDown(KeyPeriod); action_dispatch=TransformLayerMessage::TypeDecimalPoint),
			entry!(PointerMove; refresh_keys=[KeyShift, KeyControl], action_dispatch=TransformLayerMessage::PointerMove { slow_key: KeyShift, snap_key: KeyControl }),
			// SelectToolMessage
			entry!(PointerMove; refresh_keys=[KeyControl, KeyShift, KeyAlt], action_dispatch=SelectToolMessage::PointerMove { axis_align: KeyShift, snap_angle: KeyControl, center: KeyAlt }),
			entry!(KeyDown(Lmb); action_dispatch=SelectToolMessage::DragStart { add_to_selection: KeyShift }),
			entry!(KeyUp(Lmb); action_dispatch=SelectToolMessage::DragStop),
			entry!(KeyDown(KeyEnter); action_dispatch=SelectToolMessage::DragStop),
			entry!(DoubleClick; action_dispatch=SelectToolMessage::EditLayer),
			entry!(KeyDown(Rmb); action_dispatch=SelectToolMessage::Abort),
			entry!(KeyDown(KeyEscape); action_dispatch=SelectToolMessage::Abort),
			// ArtboardToolMessage
			entry!(KeyDown(Lmb); action_dispatch=ArtboardToolMessage::PointerDown),
			entry!(PointerMove; refresh_keys=[KeyShift, KeyAlt], action_dispatch=ArtboardToolMessage::PointerMove { constrain_axis_or_aspect: KeyShift, center: KeyAlt }),
			entry!(KeyUp(Lmb); action_dispatch=ArtboardToolMessage::PointerUp),
			entry!(KeyDown(KeyDelete); action_dispatch=ArtboardToolMessage::DeleteSelected),
			entry!(KeyDown(KeyBackspace); action_dispatch=ArtboardToolMessage::DeleteSelected),
			// NavigateToolMessage
			entry!(KeyUp(Lmb); modifiers=[KeyShift], action_dispatch=NavigateToolMessage::ClickZoom { zoom_in: false }),
			entry!(KeyUp(Lmb); action_dispatch=NavigateToolMessage::ClickZoom { zoom_in: true }),
			entry!(PointerMove; refresh_keys=[KeyControl], action_dispatch=NavigateToolMessage::PointerMove { snap_angle: KeyControl, snap_zoom: KeyControl }),
			entry!(KeyDown(Mmb); action_dispatch=NavigateToolMessage::TranslateCanvasBegin),
			entry!(KeyDown(Rmb); action_dispatch=NavigateToolMessage::RotateCanvasBegin),
			entry!(KeyDown(Lmb); action_dispatch=NavigateToolMessage::ZoomCanvasBegin),
			entry!(KeyUp(Rmb); action_dispatch=NavigateToolMessage::TransformCanvasEnd),
			entry!(KeyUp(Lmb); action_dispatch=NavigateToolMessage::TransformCanvasEnd),
			entry!(KeyUp(Mmb); action_dispatch=NavigateToolMessage::TransformCanvasEnd),
			// EyedropperToolMessage
			entry!(KeyDown(Lmb); action_dispatch=EyedropperToolMessage::LeftMouseDown),
			entry!(KeyDown(Rmb); action_dispatch=EyedropperToolMessage::RightMouseDown),
			// TextToolMessage
			entry!(KeyUp(Lmb); action_dispatch=TextToolMessage::Interact),
			entry!(KeyDown(KeyEscape); action_dispatch=TextToolMessage::Abort),
			entry_multiplatform!(
				standard!(KeyDown(KeyEnter); modifiers=[KeyControl], action_dispatch=TextToolMessage::CommitText),
				mac_only!(KeyDown(KeyEnter); modifiers=[KeyCommand], action_dispatch=TextToolMessage::CommitText),
			),
			// GradientToolMessage
			entry!(KeyDown(Lmb); action_dispatch=GradientToolMessage::PointerDown),
			entry!(PointerMove; refresh_keys=[KeyShift], action_dispatch=GradientToolMessage::PointerMove { constrain_axis: KeyShift }),
			entry!(KeyUp(Lmb); action_dispatch=GradientToolMessage::PointerUp),
			// RectangleToolMessage
			entry!(KeyDown(Lmb); action_dispatch=RectangleToolMessage::DragStart),
			entry!(KeyUp(Lmb); action_dispatch=RectangleToolMessage::DragStop),
			entry!(KeyDown(Rmb); action_dispatch=RectangleToolMessage::Abort),
			entry!(KeyDown(KeyEscape); action_dispatch=RectangleToolMessage::Abort),
			entry!(PointerMove; refresh_keys=[KeyAlt, KeyShift], action_dispatch=RectangleToolMessage::Resize { center: KeyAlt, lock_ratio: KeyShift }),
			// EllipseToolMessage
			entry!(KeyDown(Lmb); action_dispatch=EllipseToolMessage::DragStart),
			entry!(KeyUp(Lmb); action_dispatch=EllipseToolMessage::DragStop),
			entry!(KeyDown(Rmb); action_dispatch=EllipseToolMessage::Abort),
			entry!(KeyDown(KeyEscape); action_dispatch=EllipseToolMessage::Abort),
			entry!(PointerMove; refresh_keys=[KeyAlt, KeyShift], action_dispatch=EllipseToolMessage::Resize { center: KeyAlt, lock_ratio: KeyShift }),
			// ShapeToolMessage
			entry!(KeyDown(Lmb); action_dispatch=ShapeToolMessage::DragStart),
			entry!(KeyUp(Lmb); action_dispatch=ShapeToolMessage::DragStop),
			entry!(KeyDown(Rmb); action_dispatch=ShapeToolMessage::Abort),
			entry!(KeyDown(KeyEscape); action_dispatch=ShapeToolMessage::Abort),
			entry!(PointerMove; refresh_keys=[KeyAlt, KeyShift], action_dispatch=ShapeToolMessage::Resize { center: KeyAlt, lock_ratio: KeyShift }),
			// LineToolMessage
			entry!(KeyDown(Lmb); action_dispatch=LineToolMessage::DragStart),
			entry!(KeyUp(Lmb); action_dispatch=LineToolMessage::DragStop),
			entry!(KeyDown(Rmb); action_dispatch=LineToolMessage::Abort),
			entry!(KeyDown(KeyEscape); action_dispatch=LineToolMessage::Abort),
			entry!(PointerMove; refresh_keys=[KeyAlt, KeyShift, KeyControl], action_dispatch=LineToolMessage::Redraw { center: KeyAlt, lock_angle: KeyControl, snap_angle: KeyShift }),
			// PathToolMessage
			entry!(KeyDown(Lmb); action_dispatch=PathToolMessage::DragStart { add_to_selection: KeyShift }),
			entry!(PointerMove; refresh_keys=[KeyAlt, KeyShift], action_dispatch=PathToolMessage::PointerMove { alt_mirror_angle: KeyAlt, shift_mirror_distance: KeyShift }),
			entry!(KeyDown(KeyDelete); action_dispatch=PathToolMessage::Delete),
			entry!(KeyDown(KeyBackspace); action_dispatch=PathToolMessage::Delete),
			entry!(KeyUp(Lmb); action_dispatch=PathToolMessage::DragStop),
			// PenToolMessage
			entry!(PointerMove; refresh_keys=[KeyShift, KeyControl], action_dispatch=PenToolMessage::PointerMove { snap_angle: KeyControl, break_handle: KeyShift }),
			entry!(KeyDown(Lmb); action_dispatch=PenToolMessage::DragStart),
			entry!(KeyUp(Lmb); action_dispatch=PenToolMessage::DragStop),
			entry!(KeyDown(Rmb); action_dispatch=PenToolMessage::Confirm),
			entry!(KeyDown(KeyEscape); action_dispatch=PenToolMessage::Confirm),
			entry!(KeyDown(KeyEnter); action_dispatch=PenToolMessage::Confirm),
			// FreehandToolMessage
			entry!(PointerMove; action_dispatch=FreehandToolMessage::PointerMove),
			entry!(KeyDown(Lmb); action_dispatch=FreehandToolMessage::DragStart),
			entry!(KeyUp(Lmb); action_dispatch=FreehandToolMessage::DragStop),
			// SplineToolMessage
			entry!(PointerMove; action_dispatch=SplineToolMessage::PointerMove),
			entry!(KeyDown(Lmb); action_dispatch=SplineToolMessage::DragStart),
			entry!(KeyUp(Lmb); action_dispatch=SplineToolMessage::DragStop),
			entry!(KeyDown(Rmb); action_dispatch=SplineToolMessage::Confirm),
			entry!(KeyDown(KeyEscape); action_dispatch=SplineToolMessage::Confirm),
			entry!(KeyDown(KeyEnter); action_dispatch=SplineToolMessage::Confirm),
			// FillToolMessage
			entry!(KeyDown(Lmb); action_dispatch=FillToolMessage::LeftMouseDown),
			entry!(KeyDown(Rmb); action_dispatch=FillToolMessage::RightMouseDown),
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
			entry_multiplatform!(
				standard!(KeyDown(KeyX); modifiers=[KeyShift, KeyControl], action_dispatch=ToolMessage::ResetColors),
				mac_only!(KeyDown(KeyX); modifiers=[KeyShift, KeyCommand], action_dispatch=ToolMessage::ResetColors),
			),
			entry!(KeyDown(KeyX); modifiers=[KeyShift], action_dispatch=ToolMessage::SwapColors),
			entry!(KeyDown(KeyC); modifiers=[KeyAlt], action_dispatch=ToolMessage::SelectRandomPrimaryColor),
			// DocumentMessage
			entry!(KeyDown(KeyDelete); action_dispatch=DocumentMessage::DeleteSelectedLayers),
			entry!(KeyDown(KeyBackspace); action_dispatch=DocumentMessage::DeleteSelectedLayers),
			entry!(KeyDown(KeyP); modifiers=[KeyAlt], action_dispatch=DocumentMessage::DebugPrintDocument),
			entry_multiplatform!(
				standard!(KeyDown(KeyZ); modifiers=[KeyControl, KeyShift], action_dispatch=DocumentMessage::Redo),
				mac_only!(KeyDown(KeyZ); modifiers=[KeyCommand, KeyShift], action_dispatch=DocumentMessage::Redo),
			),
			entry_multiplatform!(
				standard!(KeyDown(KeyZ); modifiers=[KeyControl], action_dispatch=DocumentMessage::Undo),
				mac_only!(KeyDown(KeyZ); modifiers=[KeyCommand], action_dispatch=DocumentMessage::Undo),
			),
			entry_multiplatform!(
				standard!(KeyDown(KeyA); modifiers=[KeyControl, KeyAlt], action_dispatch=DocumentMessage::DeselectAllLayers),
				mac_only!(KeyDown(KeyA); modifiers=[KeyCommand, KeyAlt], action_dispatch=DocumentMessage::DeselectAllLayers),
			),
			entry_multiplatform!(
				standard!(KeyDown(KeyA); modifiers=[KeyControl], action_dispatch=DocumentMessage::SelectAllLayers),
				mac_only!(KeyDown(KeyA); modifiers=[KeyCommand], action_dispatch=DocumentMessage::SelectAllLayers),
			),
			entry_multiplatform!(
				standard!(KeyDown(KeyS); modifiers=[KeyControl], action_dispatch=DocumentMessage::SaveDocument),
				mac_only!(KeyDown(KeyS); modifiers=[KeyCommand], action_dispatch=DocumentMessage::SaveDocument),
			),
			entry_multiplatform!(
				standard!(KeyDown(Key0); modifiers=[KeyControl], action_dispatch=DocumentMessage::ZoomCanvasToFitAll),
				mac_only!(KeyDown(Key0); modifiers=[KeyCommand], action_dispatch=DocumentMessage::ZoomCanvasToFitAll),
			),
			entry_multiplatform!(
				standard!(KeyDown(KeyD); modifiers=[KeyControl], action_dispatch=DocumentMessage::DuplicateSelectedLayers),
				mac_only!(KeyDown(KeyD); modifiers=[KeyCommand], action_dispatch=DocumentMessage::DuplicateSelectedLayers),
			),
			entry_multiplatform!(
				standard!(KeyDown(KeyG); modifiers=[KeyControl], action_dispatch=DocumentMessage::GroupSelectedLayers),
				mac_only!(KeyDown(KeyG); modifiers=[KeyCommand], action_dispatch=DocumentMessage::GroupSelectedLayers),
			),
			entry_multiplatform!(
				standard!(KeyDown(KeyG); modifiers=[KeyControl, KeyShift], action_dispatch=DocumentMessage::UngroupSelectedLayers),
				mac_only!(KeyDown(KeyG); modifiers=[KeyCommand, KeyShift], action_dispatch=DocumentMessage::UngroupSelectedLayers),
			),
			entry_multiplatform!(
				standard!(KeyDown(KeyN); modifiers=[KeyControl, KeyShift], action_dispatch=DocumentMessage::CreateEmptyFolder { container_path: vec![] }),
				mac_only!(KeyDown(KeyN); modifiers=[KeyCommand, KeyShift], action_dispatch=DocumentMessage::CreateEmptyFolder { container_path: vec![] }),
			),
			entry_multiplatform!(
				standard!(KeyDown(KeyLeftBracket); modifiers=[KeyControl, KeyShift], action_dispatch=DocumentMessage::SelectedLayersLowerToBack),
				mac_only!(KeyDown(KeyLeftBracket); modifiers=[KeyCommand, KeyShift], action_dispatch=DocumentMessage::SelectedLayersLowerToBack),
			),
			entry_multiplatform!(
				// TODO: Delete this in favor of the KeyLeftBracket (non-shifted version of this key) mapping above once the input system can distinguish between the non-shifted and shifted keys (important for other language keyboards)
				standard!(KeyDown(KeyLeftCurlyBracket); modifiers=[KeyControl, KeyShift], action_dispatch=DocumentMessage::SelectedLayersLowerToBack),
				mac_only!(KeyDown(KeyLeftCurlyBracket); modifiers=[KeyCommand, KeyShift], action_dispatch=DocumentMessage::SelectedLayersLowerToBack),
			),
			entry_multiplatform!(
				standard!(KeyDown(KeyRightBracket); modifiers=[KeyControl, KeyShift], action_dispatch=DocumentMessage::SelectedLayersRaiseToFront),
				mac_only!(KeyDown(KeyRightBracket); modifiers=[KeyCommand, KeyShift], action_dispatch=DocumentMessage::SelectedLayersRaiseToFront),
			),
			entry_multiplatform!(
				// TODO: Delete this in favor of the KeyRightBracket (non-shifted version of this key) mapping above once the input system can distinguish between the non-shifted and shifted keys (important for other language keyboards)
				standard!(KeyDown(KeyRightCurlyBracket); modifiers=[KeyControl, KeyShift], action_dispatch=DocumentMessage::SelectedLayersRaiseToFront),
				mac_only!(KeyDown(KeyRightCurlyBracket); modifiers=[KeyCommand, KeyShift], action_dispatch=DocumentMessage::SelectedLayersRaiseToFront),
			),
			entry_multiplatform!(
				standard!(KeyDown(KeyLeftBracket); modifiers=[KeyControl], action_dispatch=DocumentMessage::SelectedLayersLower),
				mac_only!(KeyDown(KeyLeftBracket); modifiers=[KeyCommand], action_dispatch=DocumentMessage::SelectedLayersLower),
			),
			entry_multiplatform!(
				standard!(KeyDown(KeyRightBracket); modifiers=[KeyControl], action_dispatch=DocumentMessage::SelectedLayersRaise),
				mac_only!(KeyDown(KeyRightBracket); modifiers=[KeyCommand], action_dispatch=DocumentMessage::SelectedLayersRaise),
			),
			entry!(KeyDown(KeyArrowUp); modifiers=[KeyShift, KeyArrowLeft], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -BIG_NUDGE_AMOUNT, delta_y: -BIG_NUDGE_AMOUNT }),
			entry!(KeyDown(KeyArrowUp); modifiers=[KeyShift, KeyArrowRight], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: BIG_NUDGE_AMOUNT, delta_y: -BIG_NUDGE_AMOUNT }),
			entry!(KeyDown(KeyArrowUp); modifiers=[KeyShift], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: 0., delta_y: -BIG_NUDGE_AMOUNT }),
			entry!(KeyDown(KeyArrowDown); modifiers=[KeyShift, KeyArrowLeft], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -BIG_NUDGE_AMOUNT, delta_y: BIG_NUDGE_AMOUNT }),
			entry!(KeyDown(KeyArrowDown); modifiers=[KeyShift, KeyArrowRight], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: BIG_NUDGE_AMOUNT, delta_y: BIG_NUDGE_AMOUNT }),
			entry!(KeyDown(KeyArrowDown); modifiers=[KeyShift], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: 0., delta_y: BIG_NUDGE_AMOUNT }),
			entry!(KeyDown(KeyArrowLeft); modifiers=[KeyShift, KeyArrowUp], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -BIG_NUDGE_AMOUNT, delta_y: -BIG_NUDGE_AMOUNT }),
			entry!(KeyDown(KeyArrowLeft); modifiers=[KeyShift, KeyArrowDown], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -BIG_NUDGE_AMOUNT, delta_y: BIG_NUDGE_AMOUNT }),
			entry!(KeyDown(KeyArrowLeft); modifiers=[KeyShift], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -BIG_NUDGE_AMOUNT, delta_y: 0. }),
			entry!(KeyDown(KeyArrowRight); modifiers=[KeyShift, KeyArrowUp], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: BIG_NUDGE_AMOUNT, delta_y: -BIG_NUDGE_AMOUNT }),
			entry!(KeyDown(KeyArrowRight); modifiers=[KeyShift, KeyArrowDown], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: BIG_NUDGE_AMOUNT, delta_y: BIG_NUDGE_AMOUNT }),
			entry!(KeyDown(KeyArrowRight); modifiers=[KeyShift], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: BIG_NUDGE_AMOUNT, delta_y: 0. }),
			entry!(KeyDown(KeyArrowUp); modifiers=[KeyArrowLeft], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT }),
			entry!(KeyDown(KeyArrowUp); modifiers=[KeyArrowRight], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT }),
			entry!(KeyDown(KeyArrowUp); action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: 0., delta_y: -NUDGE_AMOUNT }),
			entry!(KeyDown(KeyArrowDown); modifiers=[KeyArrowLeft], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT }),
			entry!(KeyDown(KeyArrowDown); modifiers=[KeyArrowRight], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT }),
			entry!(KeyDown(KeyArrowDown); action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: 0., delta_y: NUDGE_AMOUNT }),
			entry!(KeyDown(KeyArrowLeft); modifiers=[KeyArrowUp], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT }),
			entry!(KeyDown(KeyArrowLeft); modifiers=[KeyArrowDown], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT }),
			entry!(KeyDown(KeyArrowLeft); action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: 0. }),
			entry!(KeyDown(KeyArrowRight); modifiers=[KeyArrowUp], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT }),
			entry!(KeyDown(KeyArrowRight); modifiers=[KeyArrowDown], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT }),
			entry!(KeyDown(KeyArrowRight); action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: 0. }),
			// TransformLayerMessage
			entry!(KeyDown(KeyG); action_dispatch=TransformLayerMessage::BeginGrab),
			entry!(KeyDown(KeyR); action_dispatch=TransformLayerMessage::BeginRotate),
			entry!(KeyDown(KeyS); action_dispatch=TransformLayerMessage::BeginScale),
			// MovementMessage
			entry!(KeyDown(Mmb); modifiers=[KeyControl], action_dispatch=MovementMessage::RotateCanvasBegin),
			entry!(KeyDown(Mmb); modifiers=[KeyShift], action_dispatch=MovementMessage::ZoomCanvasBegin),
			entry!(KeyDown(Mmb); action_dispatch=MovementMessage::TranslateCanvasBegin),
			entry!(KeyUp(Mmb); action_dispatch=MovementMessage::TransformCanvasEnd),
			entry!(KeyDown(Lmb); modifiers=[KeySpace], action_dispatch=MovementMessage::TranslateCanvasBegin),
			entry!(KeyUp(Lmb); modifiers=[KeySpace], action_dispatch=MovementMessage::TransformCanvasEnd),
			entry_multiplatform!(
				standard!(KeyDown(KeyPlus); modifiers=[KeyControl], action_dispatch=MovementMessage::IncreaseCanvasZoom { center_on_mouse: false }),
				mac_only!(KeyDown(KeyPlus); modifiers=[KeyCommand], action_dispatch=MovementMessage::IncreaseCanvasZoom { center_on_mouse: false }),
			),
			entry_multiplatform!(
				standard!(KeyDown(KeyEquals); modifiers=[KeyControl], action_dispatch=MovementMessage::IncreaseCanvasZoom { center_on_mouse: false }),
				mac_only!(KeyDown(KeyEquals); modifiers=[KeyCommand], action_dispatch=MovementMessage::IncreaseCanvasZoom { center_on_mouse: false }),
			),
			entry_multiplatform!(
				standard!(KeyDown(KeyMinus); modifiers=[KeyControl], action_dispatch=MovementMessage::DecreaseCanvasZoom { center_on_mouse: false }),
				mac_only!(KeyDown(KeyMinus); modifiers=[KeyCommand], action_dispatch=MovementMessage::DecreaseCanvasZoom { center_on_mouse: false }),
			),
			entry_multiplatform!(
				standard!(KeyDown(Key1); modifiers=[KeyControl], action_dispatch=MovementMessage::SetCanvasZoom { zoom_factor: 1. }),
				mac_only!(KeyDown(Key1); modifiers=[KeyCommand], action_dispatch=MovementMessage::SetCanvasZoom { zoom_factor: 1. }),
			),
			entry_multiplatform!(
				standard!(KeyDown(Key2); modifiers=[KeyControl], action_dispatch=MovementMessage::SetCanvasZoom { zoom_factor: 2. }),
				mac_only!(KeyDown(Key2); modifiers=[KeyCommand], action_dispatch=MovementMessage::SetCanvasZoom { zoom_factor: 2. }),
			),
			entry!(WheelScroll; modifiers=[KeyControl], action_dispatch=MovementMessage::WheelCanvasZoom),
			entry!(WheelScroll; modifiers=[KeyShift], action_dispatch=MovementMessage::WheelCanvasTranslate { use_y_as_x: true }),
			entry!(WheelScroll; action_dispatch=MovementMessage::WheelCanvasTranslate { use_y_as_x: false }),
			entry!(KeyDown(KeyPageUp); modifiers=[KeyShift], action_dispatch=MovementMessage::TranslateCanvasByViewportFraction { delta: DVec2::new(1., 0.) }),
			entry!(KeyDown(KeyPageDown); modifiers=[KeyShift], action_dispatch=MovementMessage::TranslateCanvasByViewportFraction { delta: DVec2::new(-1., 0.) }),
			entry!(KeyDown(KeyPageUp); action_dispatch=MovementMessage::TranslateCanvasByViewportFraction { delta: DVec2::new(0., 1.) }),
			entry!(KeyDown(KeyPageDown); action_dispatch=MovementMessage::TranslateCanvasByViewportFraction { delta: DVec2::new(0., -1.) }),
			// PortfolioMessage
			entry_multiplatform!(
				standard!(KeyDown(KeyO); modifiers=[KeyControl], action_dispatch=PortfolioMessage::OpenDocument),
				mac_only!(KeyDown(KeyO); modifiers=[KeyCommand], action_dispatch=PortfolioMessage::OpenDocument),
			),
			entry_multiplatform!(
				standard!(KeyDown(KeyI); modifiers=[KeyControl], action_dispatch=PortfolioMessage::Import),
				mac_only!(KeyDown(KeyI); modifiers=[KeyCommand], action_dispatch=PortfolioMessage::Import),
			),
			entry!(KeyDown(KeyTab); modifiers=[KeyControl], action_dispatch=PortfolioMessage::NextDocument),
			entry!(KeyDown(KeyTab); modifiers=[KeyControl, KeyShift], action_dispatch=PortfolioMessage::PrevDocument),
			entry_multiplatform!(
				standard!(KeyDown(KeyW); modifiers=[KeyControl], action_dispatch=PortfolioMessage::CloseActiveDocumentWithConfirmation),
				mac_only!(KeyDown(KeyW); modifiers=[KeyCommand], action_dispatch=PortfolioMessage::CloseActiveDocumentWithConfirmation),
			),
			entry_multiplatform!(
				standard!(KeyDown(KeyX); modifiers=[KeyControl], action_dispatch=PortfolioMessage::Cut { clipboard: Clipboard::Device }),
				mac_only!(KeyDown(KeyX); modifiers=[KeyCommand], action_dispatch=PortfolioMessage::Cut { clipboard: Clipboard::Device }),
			),
			entry_multiplatform!(
				standard!(KeyDown(KeyC); modifiers=[KeyControl], action_dispatch=PortfolioMessage::Copy { clipboard: Clipboard::Device }),
				mac_only!(KeyDown(KeyC); modifiers=[KeyCommand], action_dispatch=PortfolioMessage::Copy { clipboard: Clipboard::Device }),
			),
			entry_multiplatform!(
				// This shortcut is intercepted in the frontend; it exists here only as a shortcut mapping source
				standard!(KeyDown(KeyV); modifiers=[KeyControl], action_dispatch=FrontendMessage::TriggerPaste),
				mac_only!(KeyDown(KeyV); modifiers=[KeyCommand], action_dispatch=FrontendMessage::TriggerPaste),
			),
			// DialogMessage
			entry_multiplatform!(
				standard!(KeyDown(KeyN); modifiers=[KeyControl], action_dispatch=DialogMessage::RequestNewDocumentDialog),
				mac_only!(KeyDown(KeyN); modifiers=[KeyCommand], action_dispatch=DialogMessage::RequestNewDocumentDialog),
			),
			entry_multiplatform!(
				standard!(KeyDown(KeyW); modifiers=[KeyControl, KeyAlt], action_dispatch=DialogMessage::CloseAllDocumentsWithConfirmation),
				mac_only!(KeyDown(KeyW); modifiers=[KeyCommand, KeyAlt], action_dispatch=DialogMessage::CloseAllDocumentsWithConfirmation),
			),
			entry_multiplatform!(
				standard!(KeyDown(KeyE); modifiers=[KeyControl], action_dispatch=DialogMessage::RequestExportDialog),
				mac_only!(KeyDown(KeyE); modifiers=[KeyCommand], action_dispatch=DialogMessage::RequestExportDialog),
			),
			// DebugMessage
			entry!(KeyDown(KeyT); modifiers=[KeyAlt], action_dispatch=DebugMessage::ToggleTraceLogs),
			entry!(KeyDown(Key0); modifiers=[KeyAlt], action_dispatch=DebugMessage::MessageOff),
			entry!(KeyDown(Key1); modifiers=[KeyAlt], action_dispatch=DebugMessage::MessageNames),
			entry!(KeyDown(Key2); modifiers=[KeyAlt], action_dispatch=DebugMessage::MessageContents),
		];
		let (mut key_up, mut key_down, mut double_click, mut wheel_scroll, mut pointer_move) = mappings;

		// TODO: Hardcode these 10 lines into 10 lines of declarations, or make this use a macro to do all 10 in one line
		const NUMBER_KEYS: [Key; 10] = [Key0, Key1, Key2, Key3, Key4, Key5, Key6, Key7, Key8, Key9];
		for (i, key) in NUMBER_KEYS.iter().enumerate() {
			key_down[*key as usize].0.insert(
				0,
				MappingEntry {
					action: TransformLayerMessage::TypeDigit { digit: i as u8 }.into(),
					input: InputMapperMessage::KeyDown(*key),
					platform_layout: None,
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

		Self {
			key_up,
			key_down,
			double_click,
			wheel_scroll,
			pointer_move,
		}
	}
}

impl Mapping {
	pub fn match_input_message(&self, message: InputMapperMessage, keyboard_state: &KeyStates, actions: ActionList, keyboard_platform: KeyboardPlatformLayout) -> Option<Message> {
		let list = match message {
			InputMapperMessage::KeyDown(key) => &self.key_down[key as usize],
			InputMapperMessage::KeyUp(key) => &self.key_up[key as usize],
			InputMapperMessage::DoubleClick => &self.double_click,
			InputMapperMessage::WheelScroll => &self.wheel_scroll,
			InputMapperMessage::PointerMove => &self.pointer_move,
		};
		list.match_mapping(keyboard_state, actions, keyboard_platform)
	}
}

#[derive(PartialEq, Clone, Debug)]
pub struct MappingEntry {
	/// Serves two purposes:
	/// - This is the message that gets dispatched when the hotkey is matched
	/// - This message's discriminant is the action; it must be a currently active action to be considered as a shortcut
	pub action: Message,
	/// The user input event from an input device which this input mapping matches on
	pub input: InputMapperMessage,
	/// Any additional keys that must be also pressed for this input mapping to match
	pub modifiers: KeyStates,
	/// The keyboard platform layout which this mapping is exclusive to, or `None` if it's platform-agnostic
	pub platform_layout: Option<KeyboardPlatformLayout>,
}

#[derive(Debug, Clone)]
pub struct KeyMappingEntries(pub Vec<MappingEntry>);

impl KeyMappingEntries {
	fn match_mapping(&self, keyboard_state: &KeyStates, actions: ActionList, keyboard_platform: KeyboardPlatformLayout) -> Option<Message> {
		for entry in self.0.iter() {
			// Skip this entry if it is platform-specific, and for a layout that does not match the user's keyboard platform layout
			if let Some(entry_platform_layout) = entry.platform_layout {
				if entry_platform_layout != keyboard_platform {
					continue;
				}
			}

			// Find which currently pressed keys are also the modifiers in this hotkey entry, then compare those against the required modifiers to see if there are zero missing
			let pressed_modifiers = *keyboard_state & entry.modifiers;
			let all_modifiers_without_pressed_modifiers = entry.modifiers ^ pressed_modifiers;
			let all_required_modifiers_pressed = all_modifiers_without_pressed_modifiers.is_empty();
			// Skip this entry if any of the required modifiers are missing
			if !all_required_modifiers_pressed {
				continue;
			}

			if actions.iter().flatten().any(|action| entry.action.to_discriminant() == *action) {
				return Some(entry.action.clone());
			}
		}
		None
	}

	fn push(&mut self, entry: MappingEntry) {
		self.0.push(entry)
	}

	const fn new() -> Self {
		Self(Vec::new())
	}

	fn key_array() -> [Self; NUMBER_OF_KEYS] {
		const DEFAULT: KeyMappingEntries = KeyMappingEntries::new();
		[DEFAULT; NUMBER_OF_KEYS]
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct FutureKeyMapping {
	action: MessageDiscriminant,
	realized: Option<Vec<Key>>,
}

impl FutureKeyMapping {
	pub fn new(action: MessageDiscriminant) -> Self {
		Self { action, realized: None }
	}

	pub fn realize(&mut self, action_input_mapping: &impl Fn(&MessageDiscriminant) -> Vec<Vec<Key>>) {
		// FutureKeyMapping is immutable, so realizing it more than once is not respected
		if self.realized.is_some() {
			return;
		}

		if let Some(keys) = action_input_mapping(&self.action).get_mut(0) {
			let mut taken_keys = Vec::new();
			std::mem::swap(keys, &mut taken_keys);

			self.realized = Some(taken_keys);
		}
	}

	pub fn iter(&self) -> impl Iterator<Item = Key> + '_ {
		self.realized.iter().flatten().copied()
	}

	pub fn text_shortcut(&self, keyboard_platform: KeyboardPlatformLayout) -> String {
		const JOINER_MARK: &str = "+";

		let mut joined = self
			.iter()
			.map(|key| {
				let key_string = key.to_string();

				if keyboard_platform == KeyboardPlatformLayout::Mac {
					match key_string.as_str() {
						"Command" => "⌘".to_string(),
						"Control" => "⌃".to_string(),
						"Alt" => "⌥".to_string(),
						"Shift" => "⇧".to_string(),
						_ => key_string + JOINER_MARK,
					}
				} else {
					key_string + JOINER_MARK
				}
			})
			.collect::<String>();

		// Truncate to cut the joining character off the end if it's present
		if joined.ends_with(JOINER_MARK) {
			joined.truncate(joined.len() - JOINER_MARK.len());
		}

		joined
	}
}

impl Serialize for FutureKeyMapping {
	fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		serializer.collect_seq(self.iter())
	}
}

pub mod future_key_mapping {
	macro_rules! action_shortcut {
		($action:expr) => {
			Some(crate::input::input_mapper::FutureKeyMapping::new($action.into()))
		};
	}

	pub(crate) use action_shortcut;
}
