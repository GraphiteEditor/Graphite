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
	pub mouse_scroll: KeyMappingEntries,
	pub pointer_move: KeyMappingEntries,
}

impl Default for Mapping {
	fn default() -> Self {
		use input_mapper_macros::{entry, entry_for_layout, entry_multiplatform, mac, mapping, modifiers, standard};
		use Key::*;

		// WARNING!
		// If a new mapping you added here isn't working (and perhaps another lower-precedence one is instead), make sure to advertise
		// it as an available action in the respective message handler file (such as the bottom of `document_message_handler.rs`).

		let mappings = mapping![
			// HIGHER PRIORITY:
			//
			// MovementMessage
			entry! {
				action_dispatch=MovementMessage::PointerMove { snap_angle: KeyControl, wait_for_snap_angle_release: true, snap_zoom: KeyControl, zoom_from_viewport: None },
				pointer_move,
				refresh_keys=[KeyControl]
			},
			// NORMAL PRIORITY:
			//
			// TransformLayerMessage
			entry! {action_dispatch=TransformLayerMessage::ApplyTransformOperation, key_down=KeyEnter},
			entry! {action_dispatch=TransformLayerMessage::ApplyTransformOperation, key_down=Lmb},
			entry! {action_dispatch=TransformLayerMessage::CancelTransformOperation, key_down=KeyEscape},
			entry! {action_dispatch=TransformLayerMessage::CancelTransformOperation, key_down=Rmb},
			entry! {action_dispatch=TransformLayerMessage::ConstrainX, key_down=KeyX},
			entry! {action_dispatch=TransformLayerMessage::ConstrainY, key_down=KeyY},
			entry! {action_dispatch=TransformLayerMessage::TypeBackspace, key_down=KeyBackspace},
			entry! {action_dispatch=TransformLayerMessage::TypeNegate, key_down=KeyMinus},
			entry! {action_dispatch=TransformLayerMessage::TypeDecimalPoint, key_down=KeyComma},
			entry! {action_dispatch=TransformLayerMessage::TypeDecimalPoint, key_down=KeyPeriod},
			entry! {action_dispatch=TransformLayerMessage::PointerMove { slow_key: KeyShift, snap_key: KeyControl }, pointer_move, refresh_keys=[KeyShift, KeyControl]},
			// SelectToolMessage
			entry! {action_dispatch=SelectToolMessage::PointerMove { axis_align: KeyShift, snap_angle: KeyControl, center: KeyAlt }, pointer_move, refresh_keys=[KeyControl, KeyShift, KeyAlt]},
			entry! {action_dispatch=SelectToolMessage::DragStart { add_to_selection: KeyShift }, key_down=Lmb},
			entry! {action_dispatch=SelectToolMessage::DragStop, key_up=Lmb},
			entry! {action_dispatch=SelectToolMessage::DragStop, key_down=KeyEnter},
			entry! {action_dispatch=SelectToolMessage::EditLayer, double_click},
			entry! {action_dispatch=SelectToolMessage::Abort, key_down=Rmb},
			entry! {action_dispatch=SelectToolMessage::Abort, key_down=KeyEscape},
			// ArtboardToolMessage
			entry! {action_dispatch=ArtboardToolMessage::PointerDown, key_down=Lmb},
			entry! {action_dispatch=ArtboardToolMessage::PointerMove { constrain_axis_or_aspect: KeyShift, center: KeyAlt }, pointer_move, refresh_keys=[KeyShift, KeyAlt]},
			entry! {action_dispatch=ArtboardToolMessage::PointerUp, key_up=Lmb},
			entry! {action_dispatch=ArtboardToolMessage::DeleteSelected, key_down=KeyDelete},
			entry! {action_dispatch=ArtboardToolMessage::DeleteSelected, key_down=KeyBackspace},
			// NavigateToolMessage
			entry! {action_dispatch=NavigateToolMessage::ClickZoom { zoom_in: false }, key_up=Lmb, modifiers=[KeyShift]},
			entry! {action_dispatch=NavigateToolMessage::ClickZoom { zoom_in: true }, key_up=Lmb},
			entry! {action_dispatch=NavigateToolMessage::PointerMove { snap_angle: KeyControl, snap_zoom: KeyControl }, pointer_move, refresh_keys=[KeyControl]},
			entry! {action_dispatch=NavigateToolMessage::TranslateCanvasBegin, key_down=Mmb},
			entry! {action_dispatch=NavigateToolMessage::RotateCanvasBegin, key_down=Rmb},
			entry! {action_dispatch=NavigateToolMessage::ZoomCanvasBegin, key_down=Lmb},
			entry! {action_dispatch=NavigateToolMessage::TransformCanvasEnd, key_up=Rmb},
			entry! {action_dispatch=NavigateToolMessage::TransformCanvasEnd, key_up=Lmb},
			entry! {action_dispatch=NavigateToolMessage::TransformCanvasEnd, key_up=Mmb},
			// EyedropperToolMessage
			entry! {action_dispatch=EyedropperToolMessage::LeftMouseDown, key_down=Lmb},
			entry! {action_dispatch=EyedropperToolMessage::RightMouseDown, key_down=Rmb},
			// TextToolMessage
			entry! {action_dispatch=TextToolMessage::Interact, key_up=Lmb},
			entry! {action_dispatch=TextToolMessage::Abort, key_down=KeyEscape},
			entry_multiplatform! {
				standard! {action_dispatch=TextToolMessage::CommitText, key_down=KeyEnter, modifiers=[KeyControl]},
				mac!      {action_dispatch=TextToolMessage::CommitText, key_down=KeyEnter, modifiers=[KeyCommand]},
			},
			// GradientToolMessage
			entry! {action_dispatch=GradientToolMessage::PointerDown, key_down=Lmb},
			entry! {action_dispatch=GradientToolMessage::PointerMove { constrain_axis: KeyShift }, pointer_move, refresh_keys=[KeyShift]},
			entry! {action_dispatch=GradientToolMessage::PointerUp, key_up=Lmb},
			// RectangleToolMessage
			entry! {action_dispatch=RectangleToolMessage::DragStart, key_down=Lmb},
			entry! {action_dispatch=RectangleToolMessage::DragStop, key_up=Lmb},
			entry! {action_dispatch=RectangleToolMessage::Abort, key_down=Rmb},
			entry! {action_dispatch=RectangleToolMessage::Abort, key_down=KeyEscape},
			entry! {action_dispatch=RectangleToolMessage::Resize { center: KeyAlt, lock_ratio: KeyShift }, pointer_move, refresh_keys=[KeyAlt, KeyShift]},
			// EllipseToolMessage
			entry! {action_dispatch=EllipseToolMessage::DragStart, key_down=Lmb},
			entry! {action_dispatch=EllipseToolMessage::DragStop, key_up=Lmb},
			entry! {action_dispatch=EllipseToolMessage::Abort, key_down=Rmb},
			entry! {action_dispatch=EllipseToolMessage::Abort, key_down=KeyEscape},
			entry! {action_dispatch=EllipseToolMessage::Resize { center: KeyAlt, lock_ratio: KeyShift }, pointer_move, refresh_keys=[KeyAlt, KeyShift]},
			// ShapeToolMessage
			entry! {action_dispatch=ShapeToolMessage::DragStart, key_down=Lmb},
			entry! {action_dispatch=ShapeToolMessage::DragStop, key_up=Lmb},
			entry! {action_dispatch=ShapeToolMessage::Abort, key_down=Rmb},
			entry! {action_dispatch=ShapeToolMessage::Abort, key_down=KeyEscape},
			entry! {action_dispatch=ShapeToolMessage::Resize { center: KeyAlt, lock_ratio: KeyShift }, pointer_move, refresh_keys=[KeyAlt, KeyShift]},
			// LineToolMessage
			entry! {action_dispatch=LineToolMessage::DragStart, key_down=Lmb},
			entry! {action_dispatch=LineToolMessage::DragStop, key_up=Lmb},
			entry! {action_dispatch=LineToolMessage::Abort, key_down=Rmb},
			entry! {action_dispatch=LineToolMessage::Abort, key_down=KeyEscape},
			entry! {action_dispatch=LineToolMessage::Redraw { center: KeyAlt, lock_angle: KeyControl, snap_angle: KeyShift }, pointer_move, refresh_keys=[KeyAlt, KeyShift, KeyControl]},
			// PathToolMessage
			entry! {action_dispatch=PathToolMessage::DragStart { add_to_selection: KeyShift }, key_down=Lmb},
			entry! {action_dispatch=PathToolMessage::PointerMove { alt_mirror_angle: KeyAlt, shift_mirror_distance: KeyShift }, pointer_move, refresh_keys=[KeyAlt, KeyShift]},
			entry! {action_dispatch=PathToolMessage::Delete, key_down=KeyDelete},
			entry! {action_dispatch=PathToolMessage::Delete, key_down=KeyBackspace},
			entry! {action_dispatch=PathToolMessage::DragStop, key_up=Lmb},
			// PenToolMessage
			entry! {action_dispatch=PenToolMessage::PointerMove { snap_angle: KeyControl, break_handle: KeyShift }, pointer_move, refresh_keys=[KeyShift, KeyControl]},
			entry! {action_dispatch=PenToolMessage::DragStart, key_down=Lmb},
			entry! {action_dispatch=PenToolMessage::DragStop, key_up=Lmb},
			entry! {action_dispatch=PenToolMessage::Confirm, key_down=Rmb},
			entry! {action_dispatch=PenToolMessage::Confirm, key_down=KeyEscape},
			entry! {action_dispatch=PenToolMessage::Confirm, key_down=KeyEnter},
			// FreehandToolMessage
			entry! {action_dispatch=FreehandToolMessage::PointerMove, pointer_move, refresh_keys=[]},
			entry! {action_dispatch=FreehandToolMessage::DragStart, key_down=Lmb},
			entry! {action_dispatch=FreehandToolMessage::DragStop, key_up=Lmb},
			// SplineToolMessage
			entry! {action_dispatch=SplineToolMessage::PointerMove, pointer_move, refresh_keys=[]},
			entry! {action_dispatch=SplineToolMessage::DragStart, key_down=Lmb},
			entry! {action_dispatch=SplineToolMessage::DragStop, key_up=Lmb},
			entry! {action_dispatch=SplineToolMessage::Confirm, key_down=Rmb},
			entry! {action_dispatch=SplineToolMessage::Confirm, key_down=KeyEscape},
			entry! {action_dispatch=SplineToolMessage::Confirm, key_down=KeyEnter},
			// FillToolMessage
			entry! {action_dispatch=FillToolMessage::LeftMouseDown, key_down=Lmb},
			entry! {action_dispatch=FillToolMessage::RightMouseDown, key_down=Rmb},
			// ToolMessage
			entry! {action_dispatch=ToolMessage::ActivateTool { tool_type: ToolType::Select }, key_down=KeyV},
			entry! {action_dispatch=ToolMessage::ActivateTool { tool_type: ToolType::Navigate }, key_down=KeyZ},
			entry! {action_dispatch=ToolMessage::ActivateTool { tool_type: ToolType::Eyedropper }, key_down=KeyI},
			entry! {action_dispatch=ToolMessage::ActivateTool { tool_type: ToolType::Text }, key_down=KeyT},
			entry! {action_dispatch=ToolMessage::ActivateTool { tool_type: ToolType::Fill }, key_down=KeyF},
			entry! {action_dispatch=ToolMessage::ActivateTool { tool_type: ToolType::Gradient }, key_down=KeyH},
			entry! {action_dispatch=ToolMessage::ActivateTool { tool_type: ToolType::Path }, key_down=KeyA},
			entry! {action_dispatch=ToolMessage::ActivateTool { tool_type: ToolType::Pen }, key_down=KeyP},
			entry! {action_dispatch=ToolMessage::ActivateTool { tool_type: ToolType::Freehand }, key_down=KeyN},
			entry! {action_dispatch=ToolMessage::ActivateTool { tool_type: ToolType::Line }, key_down=KeyL},
			entry! {action_dispatch=ToolMessage::ActivateTool { tool_type: ToolType::Rectangle }, key_down=KeyM},
			entry! {action_dispatch=ToolMessage::ActivateTool { tool_type: ToolType::Ellipse }, key_down=KeyE},
			entry! {action_dispatch=ToolMessage::ActivateTool { tool_type: ToolType::Shape }, key_down=KeyY},
			entry_multiplatform! {
				standard! {action_dispatch=ToolMessage::ResetColors, key_down=KeyX, modifiers=[KeyShift, KeyControl]},
				mac!      {action_dispatch=ToolMessage::ResetColors, key_down=KeyX, modifiers=[KeyShift, KeyCommand]},
			},
			entry! {action_dispatch=ToolMessage::SwapColors, key_down=KeyX, modifiers=[KeyShift]},
			entry! {action_dispatch=ToolMessage::SelectRandomPrimaryColor, key_down=KeyC, modifiers=[KeyAlt]},
			// DocumentMessage
			entry! {action_dispatch=DocumentMessage::DeleteSelectedLayers, key_down=KeyDelete},
			entry! {action_dispatch=DocumentMessage::DeleteSelectedLayers, key_down=KeyBackspace},
			entry! {action_dispatch=DocumentMessage::DebugPrintDocument, key_down=KeyP, modifiers=[KeyAlt]},
			entry_multiplatform! {
				standard! {action_dispatch=DocumentMessage::Redo, key_down=KeyZ, modifiers=[KeyControl, KeyShift]},
				mac!      {action_dispatch=DocumentMessage::Redo, key_down=KeyZ, modifiers=[KeyCommand, KeyShift]},
			},
			entry_multiplatform! {
				standard! {action_dispatch=DocumentMessage::Undo, key_down=KeyZ, modifiers=[KeyControl]},
				mac!      {action_dispatch=DocumentMessage::Undo, key_down=KeyZ, modifiers=[KeyCommand]},
			},
			entry_multiplatform! {
				standard! {action_dispatch=DocumentMessage::DeselectAllLayers, key_down=KeyA, modifiers=[KeyControl, KeyAlt]},
				mac!      {action_dispatch=DocumentMessage::DeselectAllLayers, key_down=KeyA, modifiers=[KeyCommand, KeyAlt]},
			},
			entry_multiplatform! {
				standard! {action_dispatch=DocumentMessage::SelectAllLayers, key_down=KeyA, modifiers=[KeyControl]},
				mac!      {action_dispatch=DocumentMessage::SelectAllLayers, key_down=KeyA, modifiers=[KeyCommand]},
			},
			entry_multiplatform! {
				standard! {action_dispatch=DocumentMessage::SaveDocument, key_down=KeyS, modifiers=[KeyControl]},
				mac!      {action_dispatch=DocumentMessage::SaveDocument, key_down=KeyS, modifiers=[KeyCommand]},
			},
			entry_multiplatform! {
				standard! {action_dispatch=DocumentMessage::SaveDocument, key_down=KeyS, modifiers=[KeyControl, KeyShift]},
				mac!      {action_dispatch=DocumentMessage::SaveDocument, key_down=KeyS, modifiers=[KeyCommand, KeyShift]},
			},
			entry_multiplatform! {
				standard! {action_dispatch=DocumentMessage::ZoomCanvasToFitAll, key_down=Key0, modifiers=[KeyControl]},
				mac!      {action_dispatch=DocumentMessage::ZoomCanvasToFitAll, key_down=Key0, modifiers=[KeyCommand]},
			},
			entry_multiplatform! {
				standard! {action_dispatch=DocumentMessage::DuplicateSelectedLayers, key_down=KeyD, modifiers=[KeyControl]},
				mac!      {action_dispatch=DocumentMessage::DuplicateSelectedLayers, key_down=KeyD, modifiers=[KeyCommand]},
			},
			entry_multiplatform! {
				standard! {action_dispatch=DocumentMessage::GroupSelectedLayers, key_down=KeyG, modifiers=[KeyControl]},
				mac!      {action_dispatch=DocumentMessage::GroupSelectedLayers, key_down=KeyG, modifiers=[KeyCommand]},
			},
			entry_multiplatform! {
				standard! {action_dispatch=DocumentMessage::UngroupSelectedLayers, key_down=KeyG, modifiers=[KeyControl, KeyShift]},
				mac!      {action_dispatch=DocumentMessage::UngroupSelectedLayers, key_down=KeyG, modifiers=[KeyCommand, KeyShift]},
			},
			entry_multiplatform! {
				standard! {action_dispatch=DocumentMessage::CreateEmptyFolder { container_path: vec![] }, key_down=KeyN, modifiers=[KeyControl, KeyShift]},
				mac!      {action_dispatch=DocumentMessage::CreateEmptyFolder { container_path: vec![] }, key_down=KeyN, modifiers=[KeyCommand, KeyShift]},
			},
			entry_multiplatform! {
				// TODO: Use KeyLeftBracket, the non-shifted version of the key, when the input system can distinguish between the non-shifted and shifted keys (important for other language keyboards)
				standard! {action_dispatch=DocumentMessage::ReorderSelectedLayers { relative_index_offset: isize::MIN }, key_down=KeyLeftCurlyBracket, modifiers=[KeyControl, KeyShift]},
				mac!      {action_dispatch=DocumentMessage::ReorderSelectedLayers { relative_index_offset: isize::MIN }, key_down=KeyLeftCurlyBracket, modifiers=[KeyCommand, KeyShift]},
			},
			entry_multiplatform! {
				// TODO: Use KeyRightBracket, the non-shifted version of the key, when the input system can distinguish between the non-shifted and shifted keys (important for other language keyboards)
				standard! {action_dispatch=DocumentMessage::ReorderSelectedLayers { relative_index_offset: isize::MAX }, key_down=KeyRightCurlyBracket, modifiers=[KeyControl, KeyShift]},
				mac!      {action_dispatch=DocumentMessage::ReorderSelectedLayers { relative_index_offset: isize::MAX }, key_down=KeyRightCurlyBracket, modifiers=[KeyCommand, KeyShift]},
			},
			entry_multiplatform! {
				standard! {action_dispatch=DocumentMessage::ReorderSelectedLayers { relative_index_offset: -1 }, key_down=KeyLeftBracket, modifiers=[KeyControl]},
				mac!      {action_dispatch=DocumentMessage::ReorderSelectedLayers { relative_index_offset: -1 }, key_down=KeyLeftBracket, modifiers=[KeyCommand]},
			},
			entry_multiplatform! {
				standard! {action_dispatch=DocumentMessage::ReorderSelectedLayers { relative_index_offset: 1 }, key_down=KeyRightBracket, modifiers=[KeyControl]},
				mac!      {action_dispatch=DocumentMessage::ReorderSelectedLayers { relative_index_offset: 1 }, key_down=KeyRightBracket, modifiers=[KeyCommand]},
			},
			entry! {action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -SHIFT_NUDGE_AMOUNT, delta_y: -SHIFT_NUDGE_AMOUNT }, key_down=KeyArrowUp, modifiers=[KeyShift, KeyArrowLeft]},
			entry! {action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: SHIFT_NUDGE_AMOUNT, delta_y: -SHIFT_NUDGE_AMOUNT }, key_down=KeyArrowUp, modifiers=[KeyShift, KeyArrowRight]},
			entry! {action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: 0., delta_y: -SHIFT_NUDGE_AMOUNT }, key_down=KeyArrowUp, modifiers=[KeyShift]},
			entry! {action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -SHIFT_NUDGE_AMOUNT, delta_y: SHIFT_NUDGE_AMOUNT }, key_down=KeyArrowDown, modifiers=[KeyShift, KeyArrowLeft]},
			entry! {action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: SHIFT_NUDGE_AMOUNT, delta_y: SHIFT_NUDGE_AMOUNT }, key_down=KeyArrowDown, modifiers=[KeyShift, KeyArrowRight]},
			entry! {action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: 0., delta_y: SHIFT_NUDGE_AMOUNT }, key_down=KeyArrowDown, modifiers=[KeyShift]},
			entry! {action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -SHIFT_NUDGE_AMOUNT, delta_y: -SHIFT_NUDGE_AMOUNT }, key_down=KeyArrowLeft, modifiers=[KeyShift, KeyArrowUp]},
			entry! {action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -SHIFT_NUDGE_AMOUNT, delta_y: SHIFT_NUDGE_AMOUNT }, key_down=KeyArrowLeft, modifiers=[KeyShift, KeyArrowDown]},
			entry! {action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -SHIFT_NUDGE_AMOUNT, delta_y: 0. }, key_down=KeyArrowLeft, modifiers=[KeyShift]},
			entry! {action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: SHIFT_NUDGE_AMOUNT, delta_y: -SHIFT_NUDGE_AMOUNT }, key_down=KeyArrowRight, modifiers=[KeyShift, KeyArrowUp]},
			entry! {action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: SHIFT_NUDGE_AMOUNT, delta_y: SHIFT_NUDGE_AMOUNT }, key_down=KeyArrowRight, modifiers=[KeyShift, KeyArrowDown]},
			entry! {action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: SHIFT_NUDGE_AMOUNT, delta_y: 0. }, key_down=KeyArrowRight, modifiers=[KeyShift]},
			entry! {action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT }, key_down=KeyArrowUp, modifiers=[KeyArrowLeft]},
			entry! {action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT }, key_down=KeyArrowUp, modifiers=[KeyArrowRight]},
			entry! {action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: 0., delta_y: -NUDGE_AMOUNT }, key_down=KeyArrowUp},
			entry! {action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT }, key_down=KeyArrowDown, modifiers=[KeyArrowLeft]},
			entry! {action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT }, key_down=KeyArrowDown, modifiers=[KeyArrowRight]},
			entry! {action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: 0., delta_y: NUDGE_AMOUNT }, key_down=KeyArrowDown},
			entry! {action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT }, key_down=KeyArrowLeft, modifiers=[KeyArrowUp]},
			entry! {action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT }, key_down=KeyArrowLeft, modifiers=[KeyArrowDown]},
			entry! {action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: 0. }, key_down=KeyArrowLeft},
			entry! {action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT }, key_down=KeyArrowRight, modifiers=[KeyArrowUp]},
			entry! {action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT }, key_down=KeyArrowRight, modifiers=[KeyArrowDown]},
			entry! {action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: 0. }, key_down=KeyArrowRight},
			// TransformLayerMessage
			entry! {action_dispatch=TransformLayerMessage::BeginGrab, key_down=KeyG},
			entry! {action_dispatch=TransformLayerMessage::BeginRotate, key_down=KeyR},
			entry! {action_dispatch=TransformLayerMessage::BeginScale, key_down=KeyS},
			// MovementMessage
			entry! {action_dispatch=MovementMessage::RotateCanvasBegin, key_down=Mmb, modifiers=[KeyControl]},
			entry! {action_dispatch=MovementMessage::ZoomCanvasBegin, key_down=Mmb, modifiers=[KeyShift]},
			entry! {action_dispatch=MovementMessage::TranslateCanvasBegin, key_down=Mmb},
			entry! {action_dispatch=MovementMessage::TransformCanvasEnd, key_up=Mmb},
			entry! {action_dispatch=MovementMessage::TranslateCanvasBegin, key_down=Lmb, modifiers=[KeySpace]},
			entry! {action_dispatch=MovementMessage::TransformCanvasEnd, key_up=Lmb, modifiers=[KeySpace]},
			entry_multiplatform! {
				standard! {action_dispatch=MovementMessage::IncreaseCanvasZoom { center_on_mouse: false }, key_down=KeyPlus, modifiers=[KeyControl]},
				mac!      {action_dispatch=MovementMessage::IncreaseCanvasZoom { center_on_mouse: false }, key_down=KeyPlus, modifiers=[KeyCommand]},
			},
			entry_multiplatform! {
				standard! {action_dispatch=MovementMessage::IncreaseCanvasZoom { center_on_mouse: false }, key_down=KeyEquals, modifiers=[KeyControl]},
				mac!      {action_dispatch=MovementMessage::IncreaseCanvasZoom { center_on_mouse: false }, key_down=KeyEquals, modifiers=[KeyCommand]},
			},
			entry_multiplatform! {
				standard! {action_dispatch=MovementMessage::DecreaseCanvasZoom { center_on_mouse: false }, key_down=KeyMinus, modifiers=[KeyControl]},
				mac!      {action_dispatch=MovementMessage::DecreaseCanvasZoom { center_on_mouse: false }, key_down=KeyMinus, modifiers=[KeyCommand]},
			},
			entry_multiplatform! {
				standard! {action_dispatch=MovementMessage::SetCanvasZoom { zoom_factor: 1. }, key_down=Key1, modifiers=[KeyControl]},
				mac!      {action_dispatch=MovementMessage::SetCanvasZoom { zoom_factor: 1. }, key_down=Key1, modifiers=[KeyCommand]},
			},
			entry_multiplatform! {
				standard! {action_dispatch=MovementMessage::SetCanvasZoom { zoom_factor: 2. }, key_down=Key2, modifiers=[KeyControl]},
				mac!      {action_dispatch=MovementMessage::SetCanvasZoom { zoom_factor: 2. }, key_down=Key2, modifiers=[KeyCommand]},
			},
			entry! {action_dispatch=MovementMessage::WheelCanvasZoom, mouse_scroll, modifiers=[KeyControl]},
			entry! {action_dispatch=MovementMessage::WheelCanvasTranslate { use_y_as_x: true }, mouse_scroll, modifiers=[KeyShift]},
			entry! {action_dispatch=MovementMessage::WheelCanvasTranslate { use_y_as_x: false }, mouse_scroll},
			entry! {action_dispatch=MovementMessage::TranslateCanvasByViewportFraction { delta: DVec2::new(1., 0.) }, key_down=KeyPageUp, modifiers=[KeyShift]},
			entry! {action_dispatch=MovementMessage::TranslateCanvasByViewportFraction { delta: DVec2::new(-1., 0.) }, key_down=KeyPageDown, modifiers=[KeyShift]},
			entry! {action_dispatch=MovementMessage::TranslateCanvasByViewportFraction { delta: DVec2::new(0., 1.) }, key_down=KeyPageUp},
			entry! {action_dispatch=MovementMessage::TranslateCanvasByViewportFraction { delta: DVec2::new(0., -1.) }, key_down=KeyPageDown},
			// PortfolioMessage
			entry_multiplatform! {
				standard! {action_dispatch=PortfolioMessage::OpenDocument, key_down=KeyO, modifiers=[KeyControl]},
				mac!      {action_dispatch=PortfolioMessage::OpenDocument, key_down=KeyO, modifiers=[KeyCommand]},
			},
			entry_multiplatform! {
				standard! {action_dispatch=PortfolioMessage::Import, key_down=KeyI, modifiers=[KeyControl]},
				mac!      {action_dispatch=PortfolioMessage::Import, key_down=KeyI, modifiers=[KeyCommand]},
			},
			entry! {action_dispatch=PortfolioMessage::NextDocument, key_down=KeyTab, modifiers=[KeyControl]},
			entry! {action_dispatch=PortfolioMessage::PrevDocument, key_down=KeyTab, modifiers=[KeyControl, KeyShift]},
			entry_multiplatform! {
				standard! {action_dispatch=PortfolioMessage::CloseActiveDocumentWithConfirmation, key_down=KeyW, modifiers=[KeyControl]},
				mac!      {action_dispatch=PortfolioMessage::CloseActiveDocumentWithConfirmation, key_down=KeyW, modifiers=[KeyCommand]},
			},
			entry_multiplatform! {
				standard! {action_dispatch=PortfolioMessage::Copy { clipboard: Clipboard::Device }, key_down=KeyC, modifiers=[KeyControl]},
				mac!      {action_dispatch=PortfolioMessage::Copy { clipboard: Clipboard::Device }, key_down=KeyC, modifiers=[KeyCommand]},
			},
			entry_multiplatform! {
				standard! {action_dispatch=PortfolioMessage::Cut { clipboard: Clipboard::Device }, key_down=KeyX, modifiers=[KeyControl]},
				mac!      {action_dispatch=PortfolioMessage::Cut { clipboard: Clipboard::Device }, key_down=KeyX, modifiers=[KeyCommand]},
			},
			// DialogMessage
			entry_multiplatform! {
				standard! {action_dispatch=DialogMessage::RequestNewDocumentDialog, key_down=KeyN, modifiers=[KeyControl]},
				mac!      {action_dispatch=DialogMessage::RequestNewDocumentDialog, key_down=KeyN, modifiers=[KeyCommand]},
			},
			entry_multiplatform! {
				standard! {action_dispatch=DialogMessage::CloseAllDocumentsWithConfirmation, key_down=KeyW, modifiers=[KeyControl, KeyAlt]},
				mac!      {action_dispatch=DialogMessage::CloseAllDocumentsWithConfirmation, key_down=KeyW, modifiers=[KeyCommand, KeyAlt]},
			},
			entry_multiplatform! {
				standard! {action_dispatch=DialogMessage::RequestExportDialog, key_down=KeyE, modifiers=[KeyControl]},
				mac!      {action_dispatch=DialogMessage::RequestExportDialog, key_down=KeyE, modifiers=[KeyCommand]},
			},
			// DebugMessage
			entry! {action_dispatch=DebugMessage::ToggleTraceLogs, key_down=KeyT, modifiers=[KeyAlt]},
			entry! {action_dispatch=DebugMessage::MessageOff, key_down=Key0, modifiers=[KeyAlt]},
			entry! {action_dispatch=DebugMessage::MessageNames, key_down=Key1, modifiers=[KeyAlt]},
			entry! {action_dispatch=DebugMessage::MessageContents, key_down=Key2, modifiers=[KeyAlt]},
		];
		let (mut key_up, mut key_down, mut double_click, mut mouse_scroll, mut pointer_move) = mappings;

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
		sort(&mut mouse_scroll);
		sort(&mut pointer_move);

		Self {
			key_up,
			key_down,
			double_click,
			mouse_scroll,
			pointer_move,
		}
	}
}

impl Mapping {
	pub fn match_input_message(&self, message: InputMapperMessage, keys: &KeyStates, actions: ActionList) -> Option<Message> {
		let list = match message {
			InputMapperMessage::KeyDown(key) => &self.key_down[key as usize],
			InputMapperMessage::KeyUp(key) => &self.key_up[key as usize],
			InputMapperMessage::DoubleClick => &self.double_click,
			InputMapperMessage::MouseScroll => &self.mouse_scroll,
			InputMapperMessage::PointerMove => &self.pointer_move,
		};
		list.match_mapping(keys, actions)
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
	fn match_mapping(&self, keys: &KeyStates, actions: ActionList) -> Option<Message> {
		for entry in self.0.iter() {
			let all_required_modifiers_pressed = ((*keys & entry.modifiers) ^ entry.modifiers).is_empty();
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
	/// - ...dispatch the given `action_dispatch` as an output message if its discriminant is a currently available action
	/// - ...when an `InputMapperMessage` is received that matches the input specified by `key_down=...`, `key_up=...`, `double_click`, `mouse_scroll`, or `pointer_move`.
	///
	/// The actions system controls which actions are currently available. Those are provided by the different message handlers based on the current application state and context.
	/// Each handler adds or removes actions in the form of message discriminants. Here, we tie an input condition (such as a hotkey) to an action's full message.
	/// When an action is currently available, and the user enters that input, the action's message is dispatched on the message bus.
	///
	/// # Syntax variants:
	///
	/// 1. Syntax that matches on a `KeyDown` message input:
	///    ```rs
	///    entry_for_layout! {action_dispatch: Message, key_down: Key, modifiers?: Key[], layout: KeyboardPlatformLayout}
	///    ```
	/// 2. Syntax that matches on a `KeyUp` message input:
	///    ```rs
	///    entry_for_layout! {action_dispatch: Message, key_up: Key, modifiers?: Key[], layout: KeyboardPlatformLayout}
	///    ```
	/// 3. Syntax that matches on a `DoubleClick` message input:
	///    ```rs
	///    entry_for_layout! {action_dispatch: Message, double_click, modifiers?: Key[], layout: KeyboardPlatformLayout}
	///    ```
	/// 4. Syntax that matches on a `MouseScroll` message input:
	///    ```rs
	///    entry_for_layout! {action_dispatch: Message, mouse_scroll, modifiers?: Key[], layout: KeyboardPlatformLayout}
	///    ```
	/// 5. Syntax that matches on a `PointerMove` message input and also on `KeyDown` and `KeyUp` input messages for specified refresh keys:
	///    ```rs
	///    entry_for_layout! {action_dispatch: Message, pointer_move, refresh_keys: Key[], layout: KeyboardPlatformLayout}
	///    ```
	macro_rules! entry_for_layout {
		// 1. KeyDown input
		{action_dispatch=$action_dispatch:expr, key_down=$key:ident $(, modifiers=[$($modifier:ident),* $(,)?])?, layout=$layout:ident} => {{
			&[
				MappingEntry {
					action: $action_dispatch.into(),
					input: InputMapperMessage::KeyDown(Key::$key),
					modifiers: modifiers!($($($modifier),*)?),
					platform_layout: KeyboardPlatformLayout::$layout,
				},
			]
		}};

		// 2. KeyUp input
		{action_dispatch=$action_dispatch:expr, key_up=$key:ident $(, modifiers=[$($modifier:ident),* $(,)?])?, layout=$layout:ident} => {{
			&[
				MappingEntry {
					action: $action_dispatch.into(),
					input: InputMapperMessage::KeyUp(Key::$key),
					modifiers: modifiers!($($($modifier),*)?),
					platform_layout: KeyboardPlatformLayout::$layout,
				},
			]
		}};

		// 3. DoubleClick input
		{action_dispatch=$action_dispatch:expr, double_click $(, modifiers=[$($modifier:ident),* $(,)?])?, layout=$layout:ident} => {{
			&[
				MappingEntry {
					action: $action_dispatch.into(),
					input: InputMapperMessage::DoubleClick,
					modifiers: modifiers!($($($modifier),*)?),
					platform_layout: KeyboardPlatformLayout::$layout,
				},
			]
		}};

		// 4. MouseScroll input
		{action_dispatch=$action_dispatch:expr, mouse_scroll $(, modifiers=[$($modifier:ident),* $(,)?])?, layout=$layout:ident} => {{
			&[
				MappingEntry {
					action: $action_dispatch.into(),
					input: InputMapperMessage::MouseScroll,
					modifiers: modifiers!($($($modifier),*)?),
					platform_layout: KeyboardPlatformLayout::$layout,
				},
			]
		}};

		// 5. PointerMove input with optional refresh on KeyDown/KeyUp
		{action_dispatch=$action_dispatch:expr, pointer_move $(, refresh_keys=[$($refresh:ident),* $(,)?])?, layout=$layout:ident} => {{
			&[
				// Cause the `action_dispatch` message to be sent when the mouse moves.
				MappingEntry {
					action: $action_dispatch.into(),
					input: InputMapperMessage::PointerMove,
					platform_layout: KeyboardPlatformLayout::$layout,
					modifiers: modifiers!(),
				},

				// Also cause the `action_dispatch` message to be sent when the mouse doesn't move, but any of the triggered keys change.
				//
				// For example, a snapping state bound to the Shift key may change if the user presses or releases that key.
				// In that case, we want to dispatch the action's message even though the pointer didn't necessarily move so
				// the input handler can update the snapping state without making the user move the mouse to see the change.
				$(
				$(
				MappingEntry {
					action: $action_dispatch.into(),
					input: InputMapperMessage::KeyDown(Key::$refresh),
					platform_layout: KeyboardPlatformLayout::$layout,
					modifiers: modifiers!(),
				},
				MappingEntry {
					action: $action_dispatch.into(),
					input: InputMapperMessage::KeyUp(Key::$refresh),
					platform_layout: KeyboardPlatformLayout::$layout,
					modifiers: modifiers!(),
				},
				)*
				)*
			]
		}};
	}

	/// Wraps [entry_for_layout]! and calls it with an `Agnostic` keyboard platform layout to avoid having to specify that.
	macro_rules! entry {
		{$($arg:tt)*} => {{
			&[entry_for_layout! {$($arg)*, layout=Agnostic}]
		}};
	}
	/// Wraps [entry_for_layout]! and calls it with a `Mac` keyboard platform layout to avoid having to specify that.
	macro_rules! mac {
		{$($arg:tt)*} => {{
			entry_for_layout! {$($arg)*, layout=Mac}
		}};
	}
	/// Wraps [entry_for_layout]! and calls it with a `Standard` keyboard platform layout to avoid having to specify that.
	macro_rules! standard {
		{$($arg:tt)*} => {{
			entry_for_layout! {$($arg)*, layout=Standard}
		}};
	}

	/// Groups multiple related entries for different platforms.
	/// When a keyboard shortcut is not platform-agnostic, this should be used to contain a [mac]! and/or [standard]! entry.
	///
	/// # Examples
	///
	/// ```rs
	/// entry_multiplatform! {
	///     standard! {...},
	///     mac!      {...},
	/// },
	/// ```
	macro_rules! entry_multiplatform {
		{$($arg:expr),*,} => {{
			&[$($arg ),*]
		}};
	}

	/// Constructs a `KeyMappingEntries` list for each input type and inserts every given entry into the list corresponding to its input type.
	/// Returns a tuple of `KeyMappingEntries` in the order:
	/// ```rs
	/// (key_up, key_down, double_click, mouse_scroll, pointer_move)
	/// ```
	macro_rules! mapping {
		[$($entry:expr),* $(,)?] => {{
			let mut key_up = KeyMappingEntries::key_array();
			let mut key_down = KeyMappingEntries::key_array();
			let mut double_click = KeyMappingEntries::new();
			let mut mouse_scroll = KeyMappingEntries::new();
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
						InputMapperMessage::MouseScroll => &mut mouse_scroll,
						InputMapperMessage::PointerMove => &mut pointer_move,
					};
					// Push each entry to the corresponding `KeyMappingEntries` list for its input type
					corresponding_list.push(entry.clone());
				}
			}
			)*

			(key_up, key_down, double_click, mouse_scroll, pointer_move)
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
