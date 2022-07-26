use super::keyboard::{Key, KeyStates, NUMBER_OF_KEYS};
use crate::document::clipboards::Clipboard;
use crate::message_prelude::*;
use crate::viewport_tools::tool::ToolType;

use glam::DVec2;

const NUDGE_AMOUNT: f64 = 1.;
const SHIFT_NUDGE_AMOUNT: f64 = 10.;

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
		use input_mapper_macros::{entry, entry_for_layout, entry_multiplatform, mac, mapping, modifiers, standard};
		use InputMapperMessage::*;
		use Key::*;

		// WARNING!
		// If a new mapping you added here isn't working (and perhaps another lower-precedence one is instead), make sure to advertise
		// it as an available action in the respective message handler file (such as the bottom of `document_message_handler.rs`).

		let mappings = mapping![
			// HIGHER PRIORITY:
			//
			// MovementMessage
			entry! {
				PointerMove;
				refresh_keys=[KeyControl],
				action_dispatch=MovementMessage::PointerMove { snap_angle: KeyControl, wait_for_snap_angle_release: true, snap_zoom: KeyControl, zoom_from_viewport: None },
			},
			// NORMAL PRIORITY:
			//
			// TransformLayerMessage
			entry! {KeyDown(KeyEnter); action_dispatch=TransformLayerMessage::ApplyTransformOperation},
			entry! {KeyDown(Lmb); action_dispatch=TransformLayerMessage::ApplyTransformOperation},
			entry! {KeyDown(KeyEscape); action_dispatch=TransformLayerMessage::CancelTransformOperation},
			entry! {KeyDown(Rmb); action_dispatch=TransformLayerMessage::CancelTransformOperation},
			entry! {KeyDown(KeyX); action_dispatch=TransformLayerMessage::ConstrainX},
			entry! {KeyDown(KeyY); action_dispatch=TransformLayerMessage::ConstrainY},
			entry! {KeyDown(KeyBackspace); action_dispatch=TransformLayerMessage::TypeBackspace},
			entry! {KeyDown(KeyMinus); action_dispatch=TransformLayerMessage::TypeNegate},
			entry! {KeyDown(KeyComma); action_dispatch=TransformLayerMessage::TypeDecimalPoint},
			entry! {KeyDown(KeyPeriod); action_dispatch=TransformLayerMessage::TypeDecimalPoint},
			entry! {PointerMove; refresh_keys=[KeyShift, KeyControl], action_dispatch=TransformLayerMessage::PointerMove { slow_key: KeyShift, snap_key: KeyControl }},
			// SelectToolMessage
			entry! {PointerMove; refresh_keys=[KeyControl, KeyShift, KeyAlt], action_dispatch=SelectToolMessage::PointerMove { axis_align: KeyShift, snap_angle: KeyControl, center: KeyAlt }},
			entry! {KeyDown(Lmb); action_dispatch=SelectToolMessage::DragStart { add_to_selection: KeyShift }},
			entry! {KeyUp(Lmb); action_dispatch=SelectToolMessage::DragStop},
			entry! {KeyDown(KeyEnter); action_dispatch=SelectToolMessage::DragStop},
			entry! {DoubleClick; action_dispatch=SelectToolMessage::EditLayer},
			entry! {KeyDown(Rmb); action_dispatch=SelectToolMessage::Abort},
			entry! {KeyDown(KeyEscape); action_dispatch=SelectToolMessage::Abort},
			// ArtboardToolMessage
			entry! {KeyDown(Lmb); action_dispatch=ArtboardToolMessage::PointerDown},
			entry! {PointerMove; refresh_keys=[KeyShift, KeyAlt], action_dispatch=ArtboardToolMessage::PointerMove { constrain_axis_or_aspect: KeyShift, center: KeyAlt }},
			entry! {KeyUp(Lmb); action_dispatch=ArtboardToolMessage::PointerUp},
			entry! {KeyDown(KeyDelete); action_dispatch=ArtboardToolMessage::DeleteSelected},
			entry! {KeyDown(KeyBackspace); action_dispatch=ArtboardToolMessage::DeleteSelected},
			// NavigateToolMessage
			entry! {KeyUp(Lmb); modifiers=[KeyShift], action_dispatch=NavigateToolMessage::ClickZoom { zoom_in: false }},
			entry! {KeyUp(Lmb); action_dispatch=NavigateToolMessage::ClickZoom { zoom_in: true }},
			entry! {PointerMove; refresh_keys=[KeyControl], action_dispatch=NavigateToolMessage::PointerMove { snap_angle: KeyControl, snap_zoom: KeyControl }},
			entry! {KeyDown(Mmb); action_dispatch=NavigateToolMessage::TranslateCanvasBegin},
			entry! {KeyDown(Rmb); action_dispatch=NavigateToolMessage::RotateCanvasBegin},
			entry! {KeyDown(Lmb); action_dispatch=NavigateToolMessage::ZoomCanvasBegin},
			entry! {KeyUp(Rmb); action_dispatch=NavigateToolMessage::TransformCanvasEnd},
			entry! {KeyUp(Lmb); action_dispatch=NavigateToolMessage::TransformCanvasEnd},
			entry! {KeyUp(Mmb); action_dispatch=NavigateToolMessage::TransformCanvasEnd},
			// EyedropperToolMessage
			entry! {KeyDown(Lmb); action_dispatch=EyedropperToolMessage::LeftMouseDown},
			entry! {KeyDown(Rmb); action_dispatch=EyedropperToolMessage::RightMouseDown},
			// TextToolMessage
			entry! {KeyUp(Lmb); action_dispatch=TextToolMessage::Interact},
			entry! {KeyDown(KeyEscape); action_dispatch=TextToolMessage::Abort},
			entry_multiplatform! {
				standard! {KeyDown(KeyEnter); modifiers=[KeyControl], action_dispatch=TextToolMessage::CommitText},
				mac!      {KeyDown(KeyEnter); modifiers=[KeyCommand], action_dispatch=TextToolMessage::CommitText},
			},
			// GradientToolMessage
			entry! {KeyDown(Lmb); action_dispatch=GradientToolMessage::PointerDown},
			entry! {PointerMove; refresh_keys=[KeyShift], action_dispatch=GradientToolMessage::PointerMove { constrain_axis: KeyShift }},
			entry! {KeyUp(Lmb); action_dispatch=GradientToolMessage::PointerUp},
			// RectangleToolMessage
			entry! {KeyDown(Lmb); action_dispatch=RectangleToolMessage::DragStart},
			entry! {KeyUp(Lmb); action_dispatch=RectangleToolMessage::DragStop},
			entry! {KeyDown(Rmb); action_dispatch=RectangleToolMessage::Abort},
			entry! {KeyDown(KeyEscape); action_dispatch=RectangleToolMessage::Abort},
			entry! {PointerMove; refresh_keys=[KeyAlt, KeyShift], action_dispatch=RectangleToolMessage::Resize { center: KeyAlt, lock_ratio: KeyShift }},
			// EllipseToolMessage
			entry! {KeyDown(Lmb); action_dispatch=EllipseToolMessage::DragStart},
			entry! {KeyUp(Lmb); action_dispatch=EllipseToolMessage::DragStop},
			entry! {KeyDown(Rmb); action_dispatch=EllipseToolMessage::Abort},
			entry! {KeyDown(KeyEscape); action_dispatch=EllipseToolMessage::Abort},
			entry! {PointerMove; refresh_keys=[KeyAlt, KeyShift], action_dispatch=EllipseToolMessage::Resize { center: KeyAlt, lock_ratio: KeyShift }},
			// ShapeToolMessage
			entry! {KeyDown(Lmb); action_dispatch=ShapeToolMessage::DragStart},
			entry! {KeyUp(Lmb); action_dispatch=ShapeToolMessage::DragStop},
			entry! {KeyDown(Rmb); action_dispatch=ShapeToolMessage::Abort},
			entry! {KeyDown(KeyEscape); action_dispatch=ShapeToolMessage::Abort},
			entry! {PointerMove; refresh_keys=[KeyAlt, KeyShift], action_dispatch=ShapeToolMessage::Resize { center: KeyAlt, lock_ratio: KeyShift }},
			// LineToolMessage
			entry! {KeyDown(Lmb); action_dispatch=LineToolMessage::DragStart},
			entry! {KeyUp(Lmb); action_dispatch=LineToolMessage::DragStop},
			entry! {KeyDown(Rmb); action_dispatch=LineToolMessage::Abort},
			entry! {KeyDown(KeyEscape); action_dispatch=LineToolMessage::Abort},
			entry! {PointerMove; refresh_keys=[KeyAlt, KeyShift, KeyControl], action_dispatch=LineToolMessage::Redraw { center: KeyAlt, lock_angle: KeyControl, snap_angle: KeyShift }},
			// PathToolMessage
			entry! {KeyDown(Lmb); action_dispatch=PathToolMessage::DragStart { add_to_selection: KeyShift }},
			entry! {PointerMove; refresh_keys=[KeyAlt, KeyShift], action_dispatch=PathToolMessage::PointerMove { alt_mirror_angle: KeyAlt, shift_mirror_distance: KeyShift }},
			entry! {KeyDown(KeyDelete); action_dispatch=PathToolMessage::Delete},
			entry! {KeyDown(KeyBackspace); action_dispatch=PathToolMessage::Delete},
			entry! {KeyUp(Lmb); action_dispatch=PathToolMessage::DragStop},
			// PenToolMessage
			entry! {PointerMove; refresh_keys=[KeyShift, KeyControl], action_dispatch=PenToolMessage::PointerMove { snap_angle: KeyControl, break_handle: KeyShift }},
			entry! {KeyDown(Lmb); action_dispatch=PenToolMessage::DragStart},
			entry! {KeyUp(Lmb); action_dispatch=PenToolMessage::DragStop},
			entry! {KeyDown(Rmb); action_dispatch=PenToolMessage::Confirm},
			entry! {KeyDown(KeyEscape); action_dispatch=PenToolMessage::Confirm},
			entry! {KeyDown(KeyEnter); action_dispatch=PenToolMessage::Confirm},
			// FreehandToolMessage
			entry! {PointerMove; action_dispatch=FreehandToolMessage::PointerMove},
			entry! {KeyDown(Lmb); action_dispatch=FreehandToolMessage::DragStart},
			entry! {KeyUp(Lmb); action_dispatch=FreehandToolMessage::DragStop},
			// SplineToolMessage
			entry! {PointerMove; action_dispatch=SplineToolMessage::PointerMove},
			entry! {KeyDown(Lmb); action_dispatch=SplineToolMessage::DragStart},
			entry! {KeyUp(Lmb); action_dispatch=SplineToolMessage::DragStop},
			entry! {KeyDown(Rmb); action_dispatch=SplineToolMessage::Confirm},
			entry! {KeyDown(KeyEscape); action_dispatch=SplineToolMessage::Confirm},
			entry! {KeyDown(KeyEnter); action_dispatch=SplineToolMessage::Confirm},
			// FillToolMessage
			entry! {KeyDown(Lmb); action_dispatch=FillToolMessage::LeftMouseDown},
			entry! {KeyDown(Rmb); action_dispatch=FillToolMessage::RightMouseDown},
			// ToolMessage
			entry! {KeyDown(KeyV); action_dispatch=ToolMessage::ActivateTool { tool_type: ToolType::Select }},
			entry! {KeyDown(KeyZ); action_dispatch=ToolMessage::ActivateTool { tool_type: ToolType::Navigate }},
			entry! {KeyDown(KeyI); action_dispatch=ToolMessage::ActivateTool { tool_type: ToolType::Eyedropper }},
			entry! {KeyDown(KeyT); action_dispatch=ToolMessage::ActivateTool { tool_type: ToolType::Text }},
			entry! {KeyDown(KeyF); action_dispatch=ToolMessage::ActivateTool { tool_type: ToolType::Fill }},
			entry! {KeyDown(KeyH); action_dispatch=ToolMessage::ActivateTool { tool_type: ToolType::Gradient }},
			entry! {KeyDown(KeyA); action_dispatch=ToolMessage::ActivateTool { tool_type: ToolType::Path }},
			entry! {KeyDown(KeyP); action_dispatch=ToolMessage::ActivateTool { tool_type: ToolType::Pen }},
			entry! {KeyDown(KeyN); action_dispatch=ToolMessage::ActivateTool { tool_type: ToolType::Freehand }},
			entry! {KeyDown(KeyL); action_dispatch=ToolMessage::ActivateTool { tool_type: ToolType::Line }},
			entry! {KeyDown(KeyM); action_dispatch=ToolMessage::ActivateTool { tool_type: ToolType::Rectangle }},
			entry! {KeyDown(KeyE); action_dispatch=ToolMessage::ActivateTool { tool_type: ToolType::Ellipse }},
			entry! {KeyDown(KeyY); action_dispatch=ToolMessage::ActivateTool { tool_type: ToolType::Shape }},
			entry_multiplatform! {
				standard! {KeyDown(KeyX); modifiers=[KeyShift, KeyControl], action_dispatch=ToolMessage::ResetColors},
				mac!      {KeyDown(KeyX); modifiers=[KeyShift, KeyCommand], action_dispatch=ToolMessage::ResetColors},
			},
			entry! {KeyDown(KeyX); modifiers=[KeyShift], action_dispatch=ToolMessage::SwapColors},
			entry! {KeyDown(KeyC); modifiers=[KeyAlt], action_dispatch=ToolMessage::SelectRandomPrimaryColor},
			// DocumentMessage
			entry! {KeyDown(KeyDelete); action_dispatch=DocumentMessage::DeleteSelectedLayers},
			entry! {KeyDown(KeyBackspace); action_dispatch=DocumentMessage::DeleteSelectedLayers},
			entry! {KeyDown(KeyP); modifiers=[KeyAlt], action_dispatch=DocumentMessage::DebugPrintDocument},
			entry_multiplatform! {
				standard! {KeyDown(KeyZ); modifiers=[KeyControl, KeyShift], action_dispatch=DocumentMessage::Redo},
				mac!      {KeyDown(KeyZ); modifiers=[KeyCommand, KeyShift], action_dispatch=DocumentMessage::Redo},
			},
			entry_multiplatform! {
				standard! {KeyDown(KeyZ); modifiers=[KeyControl], action_dispatch=DocumentMessage::Undo},
				mac!      {KeyDown(KeyZ); modifiers=[KeyCommand], action_dispatch=DocumentMessage::Undo},
			},
			entry_multiplatform! {
				standard! {KeyDown(KeyA); modifiers=[KeyControl, KeyAlt], action_dispatch=DocumentMessage::DeselectAllLayers},
				mac!      {KeyDown(KeyA); modifiers=[KeyCommand, KeyAlt], action_dispatch=DocumentMessage::DeselectAllLayers},
			},
			entry_multiplatform! {
				standard! {KeyDown(KeyA); modifiers=[KeyControl], action_dispatch=DocumentMessage::SelectAllLayers},
				mac!      {KeyDown(KeyA); modifiers=[KeyCommand], action_dispatch=DocumentMessage::SelectAllLayers},
			},
			entry_multiplatform! {
				standard! {KeyDown(KeyS); modifiers=[KeyControl], action_dispatch=DocumentMessage::SaveDocument},
				mac!      {KeyDown(KeyS); modifiers=[KeyCommand], action_dispatch=DocumentMessage::SaveDocument},
			},
			entry_multiplatform! {
				standard! {KeyDown(KeyS); modifiers=[KeyControl, KeyShift], action_dispatch=DocumentMessage::SaveDocument},
				mac!      {KeyDown(KeyS); modifiers=[KeyCommand, KeyShift], action_dispatch=DocumentMessage::SaveDocument},
			},
			entry_multiplatform! {
				standard! {KeyDown(Key0); modifiers=[KeyControl], action_dispatch=DocumentMessage::ZoomCanvasToFitAll},
				mac!      {KeyDown(Key0); modifiers=[KeyCommand], action_dispatch=DocumentMessage::ZoomCanvasToFitAll},
			},
			entry_multiplatform! {
				standard! {KeyDown(KeyD); modifiers=[KeyControl], action_dispatch=DocumentMessage::DuplicateSelectedLayers},
				mac!      {KeyDown(KeyD); modifiers=[KeyCommand], action_dispatch=DocumentMessage::DuplicateSelectedLayers},
			},
			entry_multiplatform! {
				standard! {KeyDown(KeyG); modifiers=[KeyControl], action_dispatch=DocumentMessage::GroupSelectedLayers},
				mac!      {KeyDown(KeyG); modifiers=[KeyCommand], action_dispatch=DocumentMessage::GroupSelectedLayers},
			},
			entry_multiplatform! {
				standard! {KeyDown(KeyG); modifiers=[KeyControl, KeyShift], action_dispatch=DocumentMessage::UngroupSelectedLayers},
				mac!      {KeyDown(KeyG); modifiers=[KeyCommand, KeyShift], action_dispatch=DocumentMessage::UngroupSelectedLayers},
			},
			entry_multiplatform! {
				standard! {KeyDown(KeyN); modifiers=[KeyControl, KeyShift], action_dispatch=DocumentMessage::CreateEmptyFolder { container_path: vec![] }},
				mac!      {KeyDown(KeyN); modifiers=[KeyCommand, KeyShift], action_dispatch=DocumentMessage::CreateEmptyFolder { container_path: vec![] }},
			},
			entry_multiplatform! {
				// TODO: Use KeyLeftBracket, the non-shifted version of the key, when the input system can distinguish between the non-shifted and shifted keys (important for other language keyboards)
				standard! {KeyDown(KeyLeftCurlyBracket); modifiers=[KeyControl, KeyShift], action_dispatch=DocumentMessage::ReorderSelectedLayers { relative_index_offset: isize::MIN }},
				mac!      {KeyDown(KeyLeftCurlyBracket); modifiers=[KeyCommand, KeyShift], action_dispatch=DocumentMessage::ReorderSelectedLayers { relative_index_offset: isize::MIN }},
			},
			entry_multiplatform! {
				// TODO: Use KeyRightBracket, the non-shifted version of the key, when the input system can distinguish between the non-shifted and shifted keys (important for other language keyboards)
				standard! {KeyDown(KeyRightCurlyBracket); modifiers=[KeyControl, KeyShift], action_dispatch=DocumentMessage::ReorderSelectedLayers { relative_index_offset: isize::MAX }},
				mac!      {KeyDown(KeyRightCurlyBracket); modifiers=[KeyCommand, KeyShift], action_dispatch=DocumentMessage::ReorderSelectedLayers { relative_index_offset: isize::MAX }},
			},
			entry_multiplatform! {
				standard! {KeyDown(KeyLeftBracket); modifiers=[KeyControl], action_dispatch=DocumentMessage::ReorderSelectedLayers { relative_index_offset: -1 }},
				mac!      {KeyDown(KeyLeftBracket); modifiers=[KeyCommand], action_dispatch=DocumentMessage::ReorderSelectedLayers { relative_index_offset: -1 }},
			},
			entry_multiplatform! {
				standard! {KeyDown(KeyRightBracket); modifiers=[KeyControl], action_dispatch=DocumentMessage::ReorderSelectedLayers { relative_index_offset: 1 }},
				mac!      {KeyDown(KeyRightBracket); modifiers=[KeyCommand], action_dispatch=DocumentMessage::ReorderSelectedLayers { relative_index_offset: 1 }},
			},
			entry! {KeyDown(KeyArrowUp); modifiers=[KeyShift, KeyArrowLeft], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -SHIFT_NUDGE_AMOUNT, delta_y: -SHIFT_NUDGE_AMOUNT }},
			entry! {KeyDown(KeyArrowUp); modifiers=[KeyShift, KeyArrowRight], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: SHIFT_NUDGE_AMOUNT, delta_y: -SHIFT_NUDGE_AMOUNT }},
			entry! {KeyDown(KeyArrowUp); modifiers=[KeyShift], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: 0., delta_y: -SHIFT_NUDGE_AMOUNT }},
			entry! {KeyDown(KeyArrowDown); modifiers=[KeyShift, KeyArrowLeft], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -SHIFT_NUDGE_AMOUNT, delta_y: SHIFT_NUDGE_AMOUNT }},
			entry! {KeyDown(KeyArrowDown); modifiers=[KeyShift, KeyArrowRight], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: SHIFT_NUDGE_AMOUNT, delta_y: SHIFT_NUDGE_AMOUNT }},
			entry! {KeyDown(KeyArrowDown); modifiers=[KeyShift], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: 0., delta_y: SHIFT_NUDGE_AMOUNT }},
			entry! {KeyDown(KeyArrowLeft); modifiers=[KeyShift, KeyArrowUp], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -SHIFT_NUDGE_AMOUNT, delta_y: -SHIFT_NUDGE_AMOUNT }},
			entry! {KeyDown(KeyArrowLeft); modifiers=[KeyShift, KeyArrowDown], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -SHIFT_NUDGE_AMOUNT, delta_y: SHIFT_NUDGE_AMOUNT }},
			entry! {KeyDown(KeyArrowLeft); modifiers=[KeyShift], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -SHIFT_NUDGE_AMOUNT, delta_y: 0. }},
			entry! {KeyDown(KeyArrowRight); modifiers=[KeyShift, KeyArrowUp], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: SHIFT_NUDGE_AMOUNT, delta_y: -SHIFT_NUDGE_AMOUNT }},
			entry! {KeyDown(KeyArrowRight); modifiers=[KeyShift, KeyArrowDown], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: SHIFT_NUDGE_AMOUNT, delta_y: SHIFT_NUDGE_AMOUNT }},
			entry! {KeyDown(KeyArrowRight); modifiers=[KeyShift], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: SHIFT_NUDGE_AMOUNT, delta_y: 0. }},
			entry! {KeyDown(KeyArrowUp); modifiers=[KeyArrowLeft], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT }},
			entry! {KeyDown(KeyArrowUp); modifiers=[KeyArrowRight], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT }},
			entry! {KeyDown(KeyArrowUp); action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: 0., delta_y: -NUDGE_AMOUNT }},
			entry! {KeyDown(KeyArrowDown); modifiers=[KeyArrowLeft], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT }},
			entry! {KeyDown(KeyArrowDown); modifiers=[KeyArrowRight], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT }},
			entry! {KeyDown(KeyArrowDown); action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: 0., delta_y: NUDGE_AMOUNT }},
			entry! {KeyDown(KeyArrowLeft); modifiers=[KeyArrowUp], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT }},
			entry! {KeyDown(KeyArrowLeft); modifiers=[KeyArrowDown], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT }},
			entry! {KeyDown(KeyArrowLeft); action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: 0. }},
			entry! {KeyDown(KeyArrowRight); modifiers=[KeyArrowUp], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT }},
			entry! {KeyDown(KeyArrowRight); modifiers=[KeyArrowDown], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT }},
			entry! {KeyDown(KeyArrowRight); action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: 0. }},
			// TransformLayerMessage
			entry! {KeyDown(KeyG); action_dispatch=TransformLayerMessage::BeginGrab},
			entry! {KeyDown(KeyR); action_dispatch=TransformLayerMessage::BeginRotate},
			entry! {KeyDown(KeyS); action_dispatch=TransformLayerMessage::BeginScale},
			// MovementMessage
			entry! {KeyDown(Mmb); modifiers=[KeyControl], action_dispatch=MovementMessage::RotateCanvasBegin},
			entry! {KeyDown(Mmb); modifiers=[KeyShift], action_dispatch=MovementMessage::ZoomCanvasBegin},
			entry! {KeyDown(Mmb); action_dispatch=MovementMessage::TranslateCanvasBegin},
			entry! {KeyUp(Mmb); action_dispatch=MovementMessage::TransformCanvasEnd},
			entry! {KeyDown(Lmb); modifiers=[KeySpace], action_dispatch=MovementMessage::TranslateCanvasBegin},
			entry! {KeyUp(Lmb); modifiers=[KeySpace], action_dispatch=MovementMessage::TransformCanvasEnd},
			entry_multiplatform! {
				standard! {KeyDown(KeyPlus); modifiers=[KeyControl], action_dispatch=MovementMessage::IncreaseCanvasZoom { center_on_mouse: false }},
				mac!      {KeyDown(KeyPlus); modifiers=[KeyCommand], action_dispatch=MovementMessage::IncreaseCanvasZoom { center_on_mouse: false }},
			},
			entry_multiplatform! {
				standard! {KeyDown(KeyEquals); modifiers=[KeyControl], action_dispatch=MovementMessage::IncreaseCanvasZoom { center_on_mouse: false }},
				mac!      {KeyDown(KeyEquals); modifiers=[KeyCommand], action_dispatch=MovementMessage::IncreaseCanvasZoom { center_on_mouse: false }},
			},
			entry_multiplatform! {
				standard! {KeyDown(KeyMinus); modifiers=[KeyControl], action_dispatch=MovementMessage::DecreaseCanvasZoom { center_on_mouse: false }},
				mac!      {KeyDown(KeyMinus); modifiers=[KeyCommand], action_dispatch=MovementMessage::DecreaseCanvasZoom { center_on_mouse: false }},
			},
			entry_multiplatform! {
				standard! {KeyDown(Key1); modifiers=[KeyControl], action_dispatch=MovementMessage::SetCanvasZoom { zoom_factor: 1. }},
				mac!      {KeyDown(Key1); modifiers=[KeyCommand], action_dispatch=MovementMessage::SetCanvasZoom { zoom_factor: 1. }},
			},
			entry_multiplatform! {
				standard! {KeyDown(Key2); modifiers=[KeyControl], action_dispatch=MovementMessage::SetCanvasZoom { zoom_factor: 2. }},
				mac!      {KeyDown(Key2); modifiers=[KeyCommand], action_dispatch=MovementMessage::SetCanvasZoom { zoom_factor: 2. }},
			},
			entry! {WheelScroll; modifiers=[KeyControl], action_dispatch=MovementMessage::WheelCanvasZoom},
			entry! {WheelScroll; modifiers=[KeyShift], action_dispatch=MovementMessage::WheelCanvasTranslate { use_y_as_x: true }},
			entry! {WheelScroll; action_dispatch=MovementMessage::WheelCanvasTranslate { use_y_as_x: false }},
			entry! {KeyDown(KeyPageUp); modifiers=[KeyShift], action_dispatch=MovementMessage::TranslateCanvasByViewportFraction { delta: DVec2::new(1., 0.) }},
			entry! {KeyDown(KeyPageDown); modifiers=[KeyShift], action_dispatch=MovementMessage::TranslateCanvasByViewportFraction { delta: DVec2::new(-1., 0.) }},
			entry! {KeyDown(KeyPageUp); action_dispatch=MovementMessage::TranslateCanvasByViewportFraction { delta: DVec2::new(0., 1.) }},
			entry! {KeyDown(KeyPageDown); action_dispatch=MovementMessage::TranslateCanvasByViewportFraction { delta: DVec2::new(0., -1.) }},
			// PortfolioMessage
			entry_multiplatform! {
				standard! {KeyDown(KeyO); modifiers=[KeyControl], action_dispatch=PortfolioMessage::OpenDocument},
				mac!      {KeyDown(KeyO); modifiers=[KeyCommand], action_dispatch=PortfolioMessage::OpenDocument},
			},
			entry_multiplatform! {
				standard! {KeyDown(KeyI); modifiers=[KeyControl], action_dispatch=PortfolioMessage::Import},
				mac!      {KeyDown(KeyI); modifiers=[KeyCommand], action_dispatch=PortfolioMessage::Import},
			},
			entry! {KeyDown(KeyTab); modifiers=[KeyControl], action_dispatch=PortfolioMessage::NextDocument},
			entry! {KeyDown(KeyTab); modifiers=[KeyControl, KeyShift], action_dispatch=PortfolioMessage::PrevDocument},
			entry_multiplatform! {
				standard! {KeyDown(KeyW); modifiers=[KeyControl], action_dispatch=PortfolioMessage::CloseActiveDocumentWithConfirmation},
				mac!      {KeyDown(KeyW); modifiers=[KeyCommand], action_dispatch=PortfolioMessage::CloseActiveDocumentWithConfirmation},
			},
			entry_multiplatform! {
				standard! {KeyDown(KeyC); modifiers=[KeyControl], action_dispatch=PortfolioMessage::Copy { clipboard: Clipboard::Device }},
				mac!      {KeyDown(KeyC); modifiers=[KeyCommand], action_dispatch=PortfolioMessage::Copy { clipboard: Clipboard::Device }},
			},
			entry_multiplatform! {
				standard! {KeyDown(KeyX); modifiers=[KeyControl], action_dispatch=PortfolioMessage::Cut { clipboard: Clipboard::Device }},
				mac!      {KeyDown(KeyX); modifiers=[KeyCommand], action_dispatch=PortfolioMessage::Cut { clipboard: Clipboard::Device }},
			},
			// DialogMessage
			entry_multiplatform! {
				standard! {KeyDown(KeyN); modifiers=[KeyControl], action_dispatch=DialogMessage::RequestNewDocumentDialog},
				mac!      {KeyDown(KeyN); modifiers=[KeyCommand], action_dispatch=DialogMessage::RequestNewDocumentDialog},
			},
			entry_multiplatform! {
				standard! {KeyDown(KeyW); modifiers=[KeyControl, KeyAlt], action_dispatch=DialogMessage::CloseAllDocumentsWithConfirmation},
				mac!      {KeyDown(KeyW); modifiers=[KeyCommand, KeyAlt], action_dispatch=DialogMessage::CloseAllDocumentsWithConfirmation},
			},
			entry_multiplatform! {
				standard! {KeyDown(KeyE); modifiers=[KeyControl], action_dispatch=DialogMessage::RequestExportDialog},
				mac!      {KeyDown(KeyE); modifiers=[KeyCommand], action_dispatch=DialogMessage::RequestExportDialog},
			},
			// DebugMessage
			entry! {KeyDown(KeyT); modifiers=[KeyAlt], action_dispatch=DebugMessage::ToggleTraceLogs},
			entry! {KeyDown(Key0); modifiers=[KeyAlt], action_dispatch=DebugMessage::MessageOff},
			entry! {KeyDown(Key1); modifiers=[KeyAlt], action_dispatch=DebugMessage::MessageNames},
			entry! {KeyDown(Key2); modifiers=[KeyAlt], action_dispatch=DebugMessage::MessageContents},
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
					platform_layout: KeyboardPlatformLayout::Agnostic,
					modifiers: modifiers! {},
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
	pub fn match_input_message(&self, message: InputMapperMessage, keyboard_state: &KeyStates, actions: ActionList) -> Option<Message> {
		let list = match message {
			InputMapperMessage::KeyDown(key) => &self.key_down[key as usize],
			InputMapperMessage::KeyUp(key) => &self.key_up[key as usize],
			InputMapperMessage::DoubleClick => &self.double_click,
			InputMapperMessage::WheelScroll => &self.wheel_scroll,
			InputMapperMessage::PointerMove => &self.pointer_move,
		};
		list.match_mapping(keyboard_state, actions)
	}
}

#[derive(PartialEq, Clone, Debug, Default)]
pub enum KeyboardPlatformLayout {
	/// Keyboard mapping which is the same on standard and Mac layouts
	#[default]
	Agnostic,
	/// Standard keyboard mapping used by Windows and Linux
	Standard,
	/// Keyboard mapping used by Macs
	Mac,
}

#[derive(PartialEq, Clone, Debug)]
pub struct MappingEntry {
	pub action: Message,
	pub input: InputMapperMessage,
	pub modifiers: KeyStates,
	pub platform_layout: KeyboardPlatformLayout,
}

#[derive(Debug, Clone)]
pub struct KeyMappingEntries(pub Vec<MappingEntry>);

impl KeyMappingEntries {
	fn match_mapping(&self, keyboard_state: &KeyStates, actions: ActionList) -> Option<Message> {
		for entry in self.0.iter() {
			// Find which currently pressed keys are also the modifiers in this hotkey entry, then compare those against the required modifiers to see if there are zero missing.
			let pressed_modifiers = *keyboard_state & entry.modifiers;
			let all_modifiers_without_pressed_modifiers = entry.modifiers ^ pressed_modifiers;
			let all_required_modifiers_pressed = all_modifiers_without_pressed_modifiers.is_empty();
			if all_required_modifiers_pressed && actions.iter().flatten().any(|action| entry.action.to_discriminant() == *action) {
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

mod input_mapper_macros {
	/// Constructs a `KeyStates` bit vector and sets the bit flags for all the given modifier `Key`s.
	macro_rules! modifiers {
		($($m:ident),*) => {{
			#[allow(unused_mut)]
			let mut state = KeyStates::new();
			$(
			state.set(Key::$m as usize);
			)*
			state
		}};
	}

	/// Builds a slice of `MappingEntry` struct(s) that are used to:
	/// - ...dispatch the given `action_dispatch` as an output `Message` if its discriminant is a currently available action
	/// - ...when the `InputMapperMessage` enum variant, as specified at the start and followed by a semicolon, is received
	/// - ...while any further conditions are met, like the optional `modifiers` being pressed or `layout` matching the OS.
	///
	/// Syntax:
	/// ```rs
	/// entry_for_layout! {Key; modifiers?: Key[], refresh_keys?: Key[], action_dispatch: Message, layout: KeyboardPlatformLayout}
	/// ```
	///
	/// To avoid having to specify the final `layout` argument, instead use the wrapper macros: [entry], [standard], and [mac].
	///
	/// The actions system controls which actions are currently available. Those are provided by the different message handlers based on the current application state and context.
	/// Each handler adds or removes actions in the form of message discriminants. Here, we tie an input condition (such as a hotkey) to an action's full message.
	/// When an action is currently available, and the user enters that input, the action's message is dispatched on the message bus.
	macro_rules! entry_for_layout {
		{$input:expr; $(modifiers=[$($modifier:ident),*],)? $(refresh_keys=[$($refresh:ident),* $(,)?],)? action_dispatch=$action_dispatch:expr,$(,)? layout=$layout:ident} => {{
			&[
				// Cause the `action_dispatch` message to be sent when the specified input occurs.
				MappingEntry {
					action: $action_dispatch.into(),
					input: $input,
					modifiers: modifiers!($($($modifier),*)?),
					platform_layout: KeyboardPlatformLayout::$layout,
				},

				// Also cause the `action_dispatch` message to be sent when any of the specified refresh keys change.
				//
				// For example, a snapping state bound to the Shift key may change if the user presses or releases that key.
				// In that case, we want to dispatch the action's message even though the pointer didn't necessarily move so
				// the input handler can update the snapping state without making the user move the mouse to see the change.
				$(
				$(
				MappingEntry {
					action: $action_dispatch.into(),
					input: InputMapperMessage::KeyDown(Key::$refresh),
					modifiers: modifiers!(),
					platform_layout: KeyboardPlatformLayout::$layout,
				},
				MappingEntry {
					action: $action_dispatch.into(),
					input: InputMapperMessage::KeyUp(Key::$refresh),
					modifiers: modifiers!(),
					platform_layout: KeyboardPlatformLayout::$layout,
				},
				)*
				)*
			]
		}};
	}

	/// Wraps [entry_for_layout]! and calls it with an `Agnostic` keyboard platform `layout` to avoid having to specify that argument.
	///
	/// Syntax:
	/// ```rs
	/// entry! {Key; modifiers?: Key[], refresh_keys?: Key[], action_dispatch: Message}
	/// ```
	macro_rules! entry {
		{$($arg:tt)*} => {{
			&[entry_for_layout! {$($arg)*, layout=Agnostic}]
		}};
	}

	/// Wraps [entry_for_layout]! and calls it with a `Standard` keyboard platform `layout` to avoid having to specify that argument.
	///
	/// Syntax:
	/// ```rs
	/// standard! {Key; modifiers?: Key[], refresh_keys?: Key[], action_dispatch: Message}
	/// ```
	macro_rules! standard {
		{$($arg:tt)*} => {{
			entry_for_layout! {$($arg)*, layout=Standard}
		}};
	}

	/// Wraps [entry_for_layout]! and calls it with a `Mac` keyboard platform `layout` to avoid having to specify that argument.
	///
	/// Syntax:
	/// ```rs
	/// mac! {Key; modifiers?: Key[], refresh_keys?: Key[], action_dispatch: Message}
	/// ```
	macro_rules! mac {
		{$($arg:tt)*} => {{
			entry_for_layout! {$($arg)*, layout=Mac}
		}};
	}

	/// Groups multiple related entries for different platforms.
	/// When a keyboard shortcut is not platform-agnostic, this should be used to contain a [mac]! and/or [standard]! entry.
	///
	/// Syntax:
	///
	/// ```rs
	/// entry_multiplatform! {
	///     standard! {Key; modifiers?: Key[], refresh_keys?: Key[], action_dispatch: Message},
	///     mac!      {Key; modifiers?: Key[], refresh_keys?: Key[], action_dispatch: Message},
	/// }
	/// ```
	macro_rules! entry_multiplatform {
		{$($arg:expr),*,} => {{
			&[$($arg ),*]
		}};
	}

	/// Constructs a `KeyMappingEntries` list for each input type and inserts every given entry into the list corresponding to its input type.
	/// Returns a tuple of `KeyMappingEntries` in the order:
	/// ```rs
	/// (key_up, key_down, double_click, wheel_scroll, pointer_move)
	/// ```
	macro_rules! mapping {
		[$($entry:expr),* $(,)?] => {{
			let mut key_up = KeyMappingEntries::key_array();
			let mut key_down = KeyMappingEntries::key_array();
			let mut double_click = KeyMappingEntries::new();
			let mut wheel_scroll = KeyMappingEntries::new();
			let mut pointer_move = KeyMappingEntries::new();

			$(
			// Each of the many entry slices, one specified per action
			for entry_slice in $entry {
				// Each entry in the slice (usually just one, except when `refresh_keys` adds additional key entries)
				for entry in entry_slice.into_iter() {
					let corresponding_list = match entry.input {
						InputMapperMessage::KeyDown(key) => &mut key_down[key as usize],
						InputMapperMessage::KeyUp(key) => &mut key_up[key as usize],
						InputMapperMessage::DoubleClick => &mut double_click,
						InputMapperMessage::WheelScroll => &mut wheel_scroll,
						InputMapperMessage::PointerMove => &mut pointer_move,
					};
					// Push each entry to the corresponding `KeyMappingEntries` list for its input type
					corresponding_list.push(entry.clone());
				}
			}
			)*

			(key_up, key_down, double_click, wheel_scroll, pointer_move)
		}};
	}

	pub(crate) use entry;
	pub(crate) use entry_for_layout;
	pub(crate) use entry_multiplatform;
	pub(crate) use mac;
	pub(crate) use mapping;
	pub(crate) use modifiers;
	pub(crate) use standard;
}
