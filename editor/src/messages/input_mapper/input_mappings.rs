use crate::consts::{BIG_NUDGE_AMOUNT, BRUSH_SIZE_CHANGE_KEYBOARD, NUDGE_AMOUNT};
use crate::messages::input_mapper::key_mapping::MappingVariant;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, KeyStates};
use crate::messages::input_mapper::utility_types::input_mouse::MouseButton;
use crate::messages::input_mapper::utility_types::macros::*;
use crate::messages::input_mapper::utility_types::misc::MappingEntry;
use crate::messages::input_mapper::utility_types::misc::{KeyMappingEntries, Mapping};
use crate::messages::portfolio::document::node_graph::utility_types::Direction;
use crate::messages::portfolio::document::utility_types::clipboards::Clipboard;
use crate::messages::portfolio::document::utility_types::misc::GroupFolderType;
use crate::messages::prelude::*;
use crate::messages::tool::tool_messages::brush_tool::BrushToolMessageOptionsUpdate;
use crate::messages::tool::tool_messages::select_tool::SelectToolPointerKeys;
use glam::DVec2;

impl From<MappingVariant> for Mapping {
	fn from(value: MappingVariant) -> Self {
		match value {
			MappingVariant::Default => input_mappings(),
			MappingVariant::ZoomWithScroll => zoom_with_scroll(),
		}
	}
}

pub fn input_mappings() -> Mapping {
	use InputMapperMessage::*;
	use Key::*;

	// NOTICE:
	// If a new mapping you added here isn't working (and perhaps another lower-precedence one is instead), make sure to advertise
	// it as an available action in the respective message handler file (such as the bottom of `document_message_handler.rs`).

	let mappings = mapping![
		// ===============
		// HIGHER PRIORITY
		// ===============
		//
		// NavigationMessage
		entry!(PointerMove; refresh_keys=[Shift], action_dispatch=NavigationMessage::PointerMove { snap: Shift }),
		entry!(KeyUp(MouseLeft); action_dispatch=NavigationMessage::EndCanvasPTZ { abort_transform: false }),
		entry!(KeyUp(MouseMiddle); action_dispatch=NavigationMessage::EndCanvasPTZ { abort_transform: false }),
		entry!(KeyUp(MouseRight); action_dispatch=NavigationMessage::EndCanvasPTZ { abort_transform: false }),
		entry!(KeyDown(MouseRight); action_dispatch=NavigationMessage::EndCanvasPTZ { abort_transform: true }),
		entry!(KeyDown(Escape); action_dispatch=NavigationMessage::EndCanvasPTZ { abort_transform: true }),
		entry!(KeyDown(MouseLeft); action_dispatch=NavigationMessage::EndCanvasPTZWithClick { commit_key: MouseLeft }),
		entry!(KeyDown(MouseMiddle); action_dispatch=NavigationMessage::EndCanvasPTZWithClick { commit_key: MouseMiddle }),
		entry!(KeyDown(MouseRight); action_dispatch=NavigationMessage::EndCanvasPTZWithClick { commit_key: MouseRight }),
		//
		// ===============
		// NORMAL PRIORITY
		// ===============
		//
		// Hack to prevent Left Click + Accel + Z combo (this effectively blocks you from making a double undo with AbortTransaction)
		entry!(KeyDown(KeyZ); modifiers=[Accel, MouseLeft], action_dispatch=DocumentMessage::Noop),
		//
		// NodeGraphMessage
		entry!(KeyDown(MouseLeft); action_dispatch=NodeGraphMessage::PointerDown { shift_click: false, control_click: false, alt_click: false, right_click: false }),
		entry!(KeyDown(MouseLeft); modifiers=[Shift], action_dispatch=NodeGraphMessage::PointerDown { shift_click: true, control_click: false, alt_click: false, right_click: false }),
		entry!(KeyDown(MouseLeft); modifiers=[Accel], action_dispatch=NodeGraphMessage::PointerDown { shift_click: false, control_click: true, alt_click: false, right_click: false }),
		entry!(KeyDown(MouseLeft); modifiers=[Shift, Accel], action_dispatch=NodeGraphMessage::PointerDown { shift_click: true, control_click: true, alt_click: false, right_click: false }),
		entry!(KeyDown(MouseLeft); modifiers=[Alt], action_dispatch=NodeGraphMessage::PointerDown { shift_click: false, control_click: false, alt_click: true, right_click: false }),
		entry!(KeyDown(MouseRight); action_dispatch=NodeGraphMessage::PointerDown { shift_click: false, control_click: false, alt_click: false, right_click: true }),
		entry!(DoubleClick(MouseButton::Left); action_dispatch=NodeGraphMessage::EnterNestedNetwork),
		entry!(PointerMove; refresh_keys=[Shift], action_dispatch=NodeGraphMessage::PointerMove { shift: Shift }),
		entry!(PointerShake; action_dispatch=NodeGraphMessage::ShakeNode),
		entry!(KeyUp(MouseLeft); action_dispatch=NodeGraphMessage::PointerUp),
		entry!(KeyDown(Delete); modifiers=[Accel], action_dispatch=NodeGraphMessage::DeleteSelectedNodes { delete_children: false }),
		entry!(KeyDown(Backspace); modifiers=[Accel], action_dispatch=NodeGraphMessage::DeleteSelectedNodes { delete_children: false }),
		entry!(KeyDown(Delete); action_dispatch=NodeGraphMessage::DeleteSelectedNodes { delete_children: true }),
		entry!(KeyDown(Backspace); action_dispatch=NodeGraphMessage::DeleteSelectedNodes { delete_children: true }),
		entry!(KeyDown(KeyX); modifiers=[Accel], action_dispatch=NodeGraphMessage::Cut),
		entry!(KeyDown(KeyC); modifiers=[Accel], action_dispatch=NodeGraphMessage::Copy),
		entry!(KeyDown(KeyD); modifiers=[Accel], action_dispatch=NodeGraphMessage::DuplicateSelectedNodes),
		entry!(KeyDown(KeyH); modifiers=[Accel], action_dispatch=NodeGraphMessage::ToggleSelectedVisibility),
		entry!(KeyDown(KeyL); modifiers=[Accel], action_dispatch=NodeGraphMessage::ToggleSelectedLocked),
		entry!(KeyDown(KeyL); modifiers=[Alt], action_dispatch=NodeGraphMessage::ToggleSelectedAsLayersOrNodes),
		entry!(KeyDown(KeyC); modifiers=[Alt], action_dispatch=NodeGraphMessage::SendClickTargets),
		entry!(KeyDown(KeyM); modifiers=[Accel], action_dispatch=NodeGraphMessage::MergeSelectedNodes),
		entry!(KeyUp(KeyC); action_dispatch=NodeGraphMessage::EndSendClickTargets),
		entry!(KeyDown(ArrowUp); action_dispatch=NodeGraphMessage::ShiftSelectedNodes { direction: Direction::Up, rubber_band: false }),
		entry!(KeyDown(ArrowRight); action_dispatch=NodeGraphMessage::ShiftSelectedNodes { direction: Direction::Right, rubber_band: false }),
		entry!(KeyDown(ArrowDown); action_dispatch=NodeGraphMessage::ShiftSelectedNodes { direction: Direction::Down, rubber_band: false }),
		entry!(KeyDown(ArrowLeft); action_dispatch=NodeGraphMessage::ShiftSelectedNodes { direction: Direction::Left, rubber_band: false }),
		//
		// TransformLayerMessage
		entry!(KeyDown(Enter); action_dispatch=TransformLayerMessage::ApplyTransformOperation { final_transform: true }),
		entry!(KeyDown(MouseLeft); action_dispatch=TransformLayerMessage::ApplyTransformOperation { final_transform: true }),
		entry!(KeyDown(MouseRight); action_dispatch=TransformLayerMessage::CancelTransformOperation),
		entry!(KeyDown(Escape); action_dispatch=TransformLayerMessage::CancelTransformOperation),
		entry!(KeyDown(KeyX); action_dispatch=TransformLayerMessage::ConstrainX),
		entry!(KeyDown(KeyY); action_dispatch=TransformLayerMessage::ConstrainY),
		entry!(KeyDown(Backspace); action_dispatch=TransformLayerMessage::TypeBackspace),
		entry!(KeyDown(Minus); action_dispatch=TransformLayerMessage::TypeNegate),
		entry!(KeyDown(Comma); action_dispatch=TransformLayerMessage::TypeDecimalPoint),
		entry!(KeyDown(Period); action_dispatch=TransformLayerMessage::TypeDecimalPoint),
		entry!(PointerMove; refresh_keys=[Control, Shift], action_dispatch=TransformLayerMessage::PointerMove { slow_key: Shift, increments_key: Control }),
		//
		// SelectToolMessage
		entry!(PointerMove; refresh_keys=[Control, Alt, Shift], action_dispatch=SelectToolMessage::PointerMove(SelectToolPointerKeys { axis_align: Shift, snap_angle: Shift, center: Alt, duplicate: Alt })),
		entry!(KeyDown(MouseLeft); action_dispatch=SelectToolMessage::DragStart { extend_selection: Shift, remove_from_selection: Alt, select_deepest: Accel, lasso_select: Control, skew: Control }),
		entry!(KeyUp(MouseLeft); action_dispatch=SelectToolMessage::DragStop { remove_from_selection: Alt }),
		entry!(KeyDown(Enter); action_dispatch=SelectToolMessage::Enter),
		entry!(DoubleClick(MouseButton::Left); action_dispatch=SelectToolMessage::EditLayer),
		entry!(KeyDown(MouseRight); action_dispatch=SelectToolMessage::Abort),
		entry!(KeyDown(Escape); action_dispatch=SelectToolMessage::Abort),
		//
		// ArtboardToolMessage
		entry!(KeyDown(MouseLeft); action_dispatch=ArtboardToolMessage::PointerDown),
		entry!(PointerMove; refresh_keys=[Shift, Alt], action_dispatch=ArtboardToolMessage::PointerMove { constrain_axis_or_aspect: Shift, center: Alt }),
		entry!(KeyUp(MouseLeft); action_dispatch=ArtboardToolMessage::PointerUp),
		entry!(KeyDown(Delete); action_dispatch=ArtboardToolMessage::DeleteSelected),
		entry!(KeyDown(Backspace); action_dispatch=ArtboardToolMessage::DeleteSelected),
		entry!(KeyDown(ArrowUp); modifiers=[Shift, ArrowLeft], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: -BIG_NUDGE_AMOUNT, delta_y: -BIG_NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowUp); modifiers=[Shift, ArrowRight], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: BIG_NUDGE_AMOUNT, delta_y: -BIG_NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowUp); modifiers=[Shift], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: 0., delta_y: -BIG_NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowDown); modifiers=[Shift, ArrowLeft], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: -BIG_NUDGE_AMOUNT, delta_y: BIG_NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowDown); modifiers=[Shift, ArrowRight], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: BIG_NUDGE_AMOUNT, delta_y: BIG_NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowDown); modifiers=[Shift], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: 0., delta_y: BIG_NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowLeft); modifiers=[Shift, ArrowUp], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: -BIG_NUDGE_AMOUNT, delta_y: -BIG_NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowLeft); modifiers=[Shift, ArrowDown], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: -BIG_NUDGE_AMOUNT, delta_y: BIG_NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowLeft); modifiers=[Shift], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: -BIG_NUDGE_AMOUNT, delta_y: 0., resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowRight); modifiers=[Shift, ArrowUp], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: BIG_NUDGE_AMOUNT, delta_y: -BIG_NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowRight); modifiers=[Shift, ArrowDown], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: BIG_NUDGE_AMOUNT, delta_y: BIG_NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowRight); modifiers=[Shift], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: BIG_NUDGE_AMOUNT, delta_y: 0., resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowUp); modifiers=[ArrowLeft], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: -NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowUp); modifiers=[ArrowRight], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowUp); action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: 0., delta_y: -NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowDown); modifiers=[ArrowLeft], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: -NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowDown); modifiers=[ArrowRight], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowDown); action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: 0., delta_y: NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowLeft); modifiers=[ArrowUp], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: -NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowLeft); modifiers=[ArrowDown], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: -NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowLeft); action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: -NUDGE_AMOUNT, delta_y: 0., resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowRight); modifiers=[ArrowUp], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowRight); modifiers=[ArrowDown], action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowRight); action_dispatch=ArtboardToolMessage::NudgeSelected { delta_x: NUDGE_AMOUNT, delta_y: 0., resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(MouseRight); action_dispatch=ArtboardToolMessage::Abort),
		entry!(KeyDown(Escape); action_dispatch=ArtboardToolMessage::Abort),
		//
		// NavigateToolMessage
		entry!(KeyDown(MouseLeft); action_dispatch=NavigateToolMessage::ZoomCanvasBegin),
		entry!(KeyDown(MouseLeft); modifiers=[Alt], action_dispatch=NavigateToolMessage::TiltCanvasBegin),
		entry!(PointerMove; refresh_keys=[Shift], action_dispatch=NavigateToolMessage::PointerMove { snap: Shift }),
		entry!(KeyUp(MouseLeft); action_dispatch=NavigateToolMessage::PointerUp { zoom_in: true }),
		entry!(KeyUp(MouseLeft); modifiers=[Shift], action_dispatch=NavigateToolMessage::PointerUp { zoom_in: false }),
		//
		// EyedropperToolMessage
		entry!(KeyDown(MouseLeft); action_dispatch=EyedropperToolMessage::SamplePrimaryColorBegin),
		entry!(KeyDown(MouseLeft); modifiers=[Shift], action_dispatch=EyedropperToolMessage::SampleSecondaryColorBegin),
		entry!(KeyUp(MouseLeft); action_dispatch=EyedropperToolMessage::SamplePrimaryColorEnd),
		entry!(KeyUp(MouseLeft); modifiers=[Shift], action_dispatch=EyedropperToolMessage::SampleSecondaryColorEnd),
		entry!(PointerMove; action_dispatch=EyedropperToolMessage::PointerMove),
		entry!(KeyDown(MouseRight); action_dispatch=EyedropperToolMessage::Abort),
		entry!(KeyDown(Escape); action_dispatch=EyedropperToolMessage::Abort),
		//
		// TextToolMessage
		entry!(PointerMove; refresh_keys=[Alt, Shift], action_dispatch=TextToolMessage::PointerMove { center: Alt, lock_ratio: Shift }),
		entry!(KeyDown(MouseLeft); action_dispatch=TextToolMessage::DragStart),
		entry!(KeyUp(MouseLeft); action_dispatch=TextToolMessage::DragStop),
		entry!(KeyDown(MouseRight); action_dispatch=TextToolMessage::Abort),
		entry!(KeyDown(Escape); action_dispatch=TextToolMessage::Abort),
		entry!(KeyDown(Enter); modifiers=[Accel], action_dispatch=TextToolMessage::Abort),
		//
		// GradientToolMessage
		entry!(KeyDown(MouseLeft); action_dispatch=GradientToolMessage::PointerDown),
		entry!(PointerMove; refresh_keys=[Shift], action_dispatch=GradientToolMessage::PointerMove { constrain_axis: Shift }),
		entry!(KeyUp(MouseLeft); action_dispatch=GradientToolMessage::PointerUp),
		entry!(DoubleClick(MouseButton::Left); action_dispatch=GradientToolMessage::InsertStop),
		entry!(KeyDown(Delete); action_dispatch=GradientToolMessage::DeleteStop),
		entry!(KeyDown(Backspace); action_dispatch=GradientToolMessage::DeleteStop),
		entry!(KeyDown(MouseRight); action_dispatch=GradientToolMessage::Abort),
		entry!(KeyDown(Escape); action_dispatch=GradientToolMessage::Abort),
		//
		// ShapeToolMessage
		entry!(KeyDown(MouseLeft); action_dispatch=ShapeToolMessage::DragStart),
		entry!(KeyUp(MouseLeft); action_dispatch=ShapeToolMessage::DragStop),
		entry!(KeyDown(MouseRight); action_dispatch=ShapeToolMessage::Abort),
		entry!(KeyDown(Escape); action_dispatch=ShapeToolMessage::Abort),
		entry!(KeyDown(BracketLeft); action_dispatch=ShapeToolMessage::DecreaseSides),
		entry!(KeyDown(BracketRight); action_dispatch=ShapeToolMessage::IncreaseSides),
		entry!(PointerMove; refresh_keys=[Alt, Shift, Control], action_dispatch=ShapeToolMessage::PointerMove([Alt, Shift, Control])),
		entry!(KeyDown(ArrowUp); modifiers=[Shift, ArrowLeft], action_dispatch=ShapeToolMessage::NudgeSelectedLayers { delta_x: -BIG_NUDGE_AMOUNT, delta_y: -BIG_NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowUp); modifiers=[Shift, ArrowRight], action_dispatch=ShapeToolMessage::NudgeSelectedLayers { delta_x: BIG_NUDGE_AMOUNT, delta_y: -BIG_NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowUp); modifiers=[Shift], action_dispatch=ShapeToolMessage::NudgeSelectedLayers { delta_x: 0., delta_y: -BIG_NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowDown); modifiers=[Shift, ArrowLeft], action_dispatch=ShapeToolMessage::NudgeSelectedLayers { delta_x: -BIG_NUDGE_AMOUNT, delta_y: BIG_NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowDown); modifiers=[Shift, ArrowRight], action_dispatch=ShapeToolMessage::NudgeSelectedLayers { delta_x: BIG_NUDGE_AMOUNT, delta_y: BIG_NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowDown); modifiers=[Shift], action_dispatch=ShapeToolMessage::NudgeSelectedLayers { delta_x: 0., delta_y: BIG_NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowLeft); modifiers=[Shift, ArrowUp], action_dispatch=ShapeToolMessage::NudgeSelectedLayers { delta_x: -BIG_NUDGE_AMOUNT, delta_y: -BIG_NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowLeft); modifiers=[Shift, ArrowDown], action_dispatch=ShapeToolMessage::NudgeSelectedLayers { delta_x: -BIG_NUDGE_AMOUNT, delta_y: BIG_NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowLeft); modifiers=[Shift], action_dispatch=ShapeToolMessage::NudgeSelectedLayers { delta_x: -BIG_NUDGE_AMOUNT, delta_y: 0., resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowRight); modifiers=[Shift, ArrowUp], action_dispatch=ShapeToolMessage::NudgeSelectedLayers { delta_x: BIG_NUDGE_AMOUNT, delta_y: -BIG_NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowRight); modifiers=[Shift, ArrowDown], action_dispatch=ShapeToolMessage::NudgeSelectedLayers { delta_x: BIG_NUDGE_AMOUNT, delta_y: BIG_NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowRight); modifiers=[Shift], action_dispatch=ShapeToolMessage::NudgeSelectedLayers { delta_x: BIG_NUDGE_AMOUNT, delta_y: 0., resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowUp); modifiers=[ArrowLeft], action_dispatch=ShapeToolMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowUp); modifiers=[ArrowRight], action_dispatch=ShapeToolMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowUp); action_dispatch=ShapeToolMessage::NudgeSelectedLayers { delta_x: 0., delta_y: -NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowDown); modifiers=[ArrowLeft], action_dispatch=ShapeToolMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowDown); modifiers=[ArrowRight], action_dispatch=ShapeToolMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowDown); action_dispatch=ShapeToolMessage::NudgeSelectedLayers { delta_x: 0., delta_y: NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowLeft); modifiers=[ArrowUp], action_dispatch=ShapeToolMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowLeft); modifiers=[ArrowDown], action_dispatch=ShapeToolMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowLeft); action_dispatch=ShapeToolMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: 0., resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowRight); modifiers=[ArrowUp], action_dispatch=ShapeToolMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowRight); modifiers=[ArrowDown], action_dispatch=ShapeToolMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowRight); action_dispatch=ShapeToolMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: 0., resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowUp); action_dispatch=ShapeToolMessage::IncreaseSides),
		entry!(KeyDown(ArrowDown); action_dispatch=ShapeToolMessage::DecreaseSides),
		//
		// PathToolMessage
		entry!(KeyDown(Delete); modifiers=[Accel], action_dispatch=PathToolMessage::DeleteAndBreakPath),
		entry!(KeyDown(Backspace); modifiers=[Accel], action_dispatch=PathToolMessage::DeleteAndBreakPath),
		entry!(KeyDown(Delete); modifiers=[Shift], action_dispatch=PathToolMessage::BreakPath),
		entry!(KeyDown(Backspace); modifiers=[Shift], action_dispatch=PathToolMessage::BreakPath),
		entry!(KeyDown(KeyX); modifiers=[Accel], action_dispatch=PathToolMessage::Cut { clipboard: Clipboard::Device }),
		entry!(KeyDown(KeyC); modifiers=[Accel], action_dispatch=PathToolMessage::Copy { clipboard: Clipboard::Device }),
		entry!(KeyDown(KeyD); modifiers=[Accel], action_dispatch=PathToolMessage::Duplicate),
		entry!(KeyDownNoRepeat(Tab); action_dispatch=PathToolMessage::SwapSelectedHandles),
		entry!(KeyDown(MouseLeft); action_dispatch=PathToolMessage::MouseDown { extend_selection: Shift, lasso_select: Control, handle_drag_from_anchor: Alt, drag_restore_handle: Control, segment_editing_modifier: Control }),
		entry!(KeyDown(MouseRight); action_dispatch=PathToolMessage::RightClick),
		entry!(KeyDown(Escape); action_dispatch=PathToolMessage::Escape),
		entry!(KeyDown(KeyG); action_dispatch=PathToolMessage::GRS { key: KeyG }),
		entry!(KeyDown(KeyR); action_dispatch=PathToolMessage::GRS { key: KeyR }),
		entry!(KeyDown(KeyS); action_dispatch=PathToolMessage::GRS { key: KeyS }),
		entry!(PointerMove; refresh_keys=[KeyC, Space, Control, Shift, Alt], action_dispatch=PathToolMessage::PointerMove { toggle_colinear: KeyC, equidistant: Alt, move_anchor_with_handles: Space, snap_angle: Shift, lock_angle: Control, delete_segment: Alt, break_colinear_molding: Alt, segment_editing_modifier: Control }),
		entry!(KeyDown(Delete); action_dispatch=PathToolMessage::Delete),
		entry!(KeyDown(KeyA); modifiers=[Accel], action_dispatch=PathToolMessage::SelectAllAnchors),
		entry!(KeyDown(KeyA); modifiers=[Accel, Shift], canonical, action_dispatch=PathToolMessage::DeselectAllPoints),
		entry!(KeyDown(KeyA); modifiers=[Alt], action_dispatch=PathToolMessage::DeselectAllPoints),
		entry!(KeyDown(Backspace); action_dispatch=PathToolMessage::Delete),
		entry!(KeyUp(MouseLeft); action_dispatch=PathToolMessage::DragStop { extend_selection: Shift, shrink_selection: Alt }),
		entry!(KeyDown(Enter); action_dispatch=PathToolMessage::Enter { extend_selection: Shift, shrink_selection: Alt }),
		entry!(DoubleClick(MouseButton::Left); action_dispatch=PathToolMessage::DoubleClick { extend_selection: Shift, shrink_selection: Alt }),
		entry!(KeyDown(ArrowRight); action_dispatch=PathToolMessage::NudgeSelectedPoints { delta_x: NUDGE_AMOUNT, delta_y: 0. }),
		entry!(KeyDown(ArrowRight); modifiers=[Shift], action_dispatch=PathToolMessage::NudgeSelectedPoints { delta_x: BIG_NUDGE_AMOUNT, delta_y: 0. }),
		entry!(KeyDown(ArrowRight); modifiers=[ArrowUp], action_dispatch=PathToolMessage::NudgeSelectedPoints { delta_x: NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowRight); modifiers=[ArrowDown], action_dispatch=PathToolMessage::NudgeSelectedPoints { delta_x: NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowRight); modifiers=[Shift, ArrowUp], action_dispatch=PathToolMessage::NudgeSelectedPoints { delta_x: BIG_NUDGE_AMOUNT, delta_y: -BIG_NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowRight); modifiers=[Shift, ArrowDown], action_dispatch=PathToolMessage::NudgeSelectedPoints { delta_x: BIG_NUDGE_AMOUNT, delta_y: BIG_NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowUp); action_dispatch=PathToolMessage::NudgeSelectedPoints { delta_x: 0., delta_y: -NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowUp); modifiers=[Shift], action_dispatch=PathToolMessage::NudgeSelectedPoints { delta_x: 0., delta_y: -BIG_NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowUp); modifiers=[ArrowLeft], action_dispatch=PathToolMessage::NudgeSelectedPoints { delta_x: -NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowUp); modifiers=[ArrowRight], action_dispatch=PathToolMessage::NudgeSelectedPoints { delta_x: NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowUp); modifiers=[Shift, ArrowLeft], action_dispatch=PathToolMessage::NudgeSelectedPoints { delta_x: -BIG_NUDGE_AMOUNT, delta_y: -BIG_NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowUp); modifiers=[Shift, ArrowRight], action_dispatch=PathToolMessage::NudgeSelectedPoints { delta_x: BIG_NUDGE_AMOUNT, delta_y: -BIG_NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowLeft); action_dispatch=PathToolMessage::NudgeSelectedPoints { delta_x: -NUDGE_AMOUNT, delta_y: 0. }),
		entry!(KeyDown(ArrowLeft); modifiers=[Shift], action_dispatch=PathToolMessage::NudgeSelectedPoints { delta_x: -BIG_NUDGE_AMOUNT, delta_y: 0. }),
		entry!(KeyDown(ArrowLeft); modifiers=[ArrowUp], action_dispatch=PathToolMessage::NudgeSelectedPoints { delta_x: -NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowLeft); modifiers=[ArrowDown], action_dispatch=PathToolMessage::NudgeSelectedPoints { delta_x: -NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowLeft); modifiers=[Shift, ArrowUp], action_dispatch=PathToolMessage::NudgeSelectedPoints { delta_x: -BIG_NUDGE_AMOUNT, delta_y: -BIG_NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowLeft); modifiers=[Shift, ArrowDown], action_dispatch=PathToolMessage::NudgeSelectedPoints { delta_x: -BIG_NUDGE_AMOUNT, delta_y: BIG_NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowDown); action_dispatch=PathToolMessage::NudgeSelectedPoints { delta_x: 0., delta_y: NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowDown); modifiers=[Shift], action_dispatch=PathToolMessage::NudgeSelectedPoints { delta_x: 0., delta_y: BIG_NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowDown); modifiers=[ArrowLeft], action_dispatch=PathToolMessage::NudgeSelectedPoints { delta_x: -NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowDown); modifiers=[ArrowRight], action_dispatch=PathToolMessage::NudgeSelectedPoints { delta_x: NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowDown); modifiers=[Shift, ArrowLeft], action_dispatch=PathToolMessage::NudgeSelectedPoints { delta_x: -BIG_NUDGE_AMOUNT, delta_y: BIG_NUDGE_AMOUNT }),
		entry!(KeyDown(ArrowDown); modifiers=[Shift, ArrowRight], action_dispatch=PathToolMessage::NudgeSelectedPoints { delta_x: BIG_NUDGE_AMOUNT, delta_y: BIG_NUDGE_AMOUNT }),
		entry!(KeyDown(KeyJ); modifiers=[Accel], action_dispatch=ToolMessage::Path(PathToolMessage::ClosePath)),
		//
		// PenToolMessage
		entry!(PointerMove; refresh_keys=[Control, Alt, Shift, KeyC], action_dispatch=PenToolMessage::PointerMove { snap_angle: Shift, break_handle: Alt, lock_angle: Control, colinear: KeyC, move_anchor_with_handles: Space }),
		entry!(KeyDownNoRepeat(Tab); action_dispatch=PenToolMessage::SwapHandles),
		entry!(KeyDown(MouseLeft); action_dispatch=PenToolMessage::DragStart { append_to_selected: Shift }),
		entry!(KeyUp(MouseLeft); action_dispatch=PenToolMessage::DragStop),
		entry!(KeyDown(MouseRight); action_dispatch=PenToolMessage::Abort),
		entry!(KeyDown(Escape); action_dispatch=PenToolMessage::Abort),
		entry!(KeyDown(Enter); action_dispatch=PenToolMessage::Confirm),
		entry!(KeyDown(Delete); action_dispatch=PenToolMessage::RemovePreviousHandle),
		entry!(KeyDown(Backspace); action_dispatch=PenToolMessage::RemovePreviousHandle),
		entry!(KeyDown(KeyG); action_dispatch=PenToolMessage::GRS { grab: KeyG, rotate: KeyR, scale: KeyS }),
		entry!(KeyDown(KeyR); action_dispatch=PenToolMessage::GRS { grab: KeyG, rotate: KeyR, scale: KeyS }),
		entry!(KeyDown(KeyS); action_dispatch=PenToolMessage::GRS { grab: KeyG, rotate: KeyR, scale: KeyS }),
		//
		// FreehandToolMessage
		entry!(PointerMove; action_dispatch=FreehandToolMessage::PointerMove),
		entry!(KeyDown(MouseLeft); action_dispatch=FreehandToolMessage::DragStart { append_to_selected: Shift }),
		entry!(KeyUp(MouseLeft); action_dispatch=FreehandToolMessage::DragStop),
		entry!(KeyDown(MouseRight); action_dispatch=FreehandToolMessage::Abort),
		entry!(KeyDown(Escape); action_dispatch=FreehandToolMessage::Abort),
		//
		// SplineToolMessage
		entry!(PointerMove; action_dispatch=SplineToolMessage::PointerMove),
		entry!(KeyDown(MouseLeft); action_dispatch=SplineToolMessage::DragStart { append_to_selected: Shift }),
		entry!(KeyUp(MouseLeft); action_dispatch=SplineToolMessage::DragStop),
		entry!(KeyDown(MouseRight); action_dispatch=SplineToolMessage::Confirm),
		entry!(KeyDown(Escape); action_dispatch=SplineToolMessage::Confirm),
		entry!(KeyDown(Enter); action_dispatch=SplineToolMessage::Confirm),
		//
		// FillToolMessage
		entry!(PointerMove; refresh_keys=[Shift], action_dispatch=FillToolMessage::PointerMove),
		entry!(KeyDown(MouseLeft); action_dispatch=FillToolMessage::FillPrimaryColor),
		entry!(KeyDown(MouseLeft); modifiers=[Shift], action_dispatch=FillToolMessage::FillSecondaryColor),
		entry!(KeyUp(MouseLeft); action_dispatch=FillToolMessage::PointerUp),
		entry!(KeyDown(MouseRight); action_dispatch=FillToolMessage::Abort),
		entry!(KeyDown(Escape); action_dispatch=FillToolMessage::Abort),
		//
		// BrushToolMessage
		entry!(PointerMove; action_dispatch=BrushToolMessage::PointerMove),
		entry!(KeyDown(MouseLeft); action_dispatch=BrushToolMessage::DragStart),
		entry!(KeyUp(MouseLeft); action_dispatch=BrushToolMessage::DragStop),
		entry!(KeyDown(BracketLeft); action_dispatch=BrushToolMessage::UpdateOptions(BrushToolMessageOptionsUpdate::ChangeDiameter(-BRUSH_SIZE_CHANGE_KEYBOARD))),
		entry!(KeyDown(BracketRight); action_dispatch=BrushToolMessage::UpdateOptions(BrushToolMessageOptionsUpdate::ChangeDiameter(BRUSH_SIZE_CHANGE_KEYBOARD))),
		entry!(KeyDown(MouseRight); action_dispatch=BrushToolMessage::Abort),
		entry!(KeyDown(Escape); action_dispatch=BrushToolMessage::Abort),
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
		entry!(KeyDown(KeyL); action_dispatch=ToolMessage::ActivateToolShapeLine),
		entry!(KeyDown(KeyM); action_dispatch=ToolMessage::ActivateToolShapeRectangle),
		entry!(KeyDown(KeyE); action_dispatch=ToolMessage::ActivateToolShapeEllipse),
		entry!(KeyDown(KeyY); action_dispatch=ToolMessage::ActivateToolShape),
		entry!(KeyDown(KeyB); action_dispatch=ToolMessage::ActivateToolBrush),
		entry!(KeyDown(KeyD); action_dispatch=ToolMessage::ResetColors),
		entry!(KeyDown(KeyX); modifiers=[Shift], action_dispatch=ToolMessage::SwapColors),
		entry!(KeyDown(KeyC); modifiers=[Alt], action_dispatch=ToolMessage::SelectRandomWorkingColor { primary: true }),
		entry!(KeyDown(KeyC); modifiers=[Alt, Shift], action_dispatch=ToolMessage::SelectRandomWorkingColor { primary: false }),
		entry!(KeyDownNoRepeat(Tab); action_dispatch=ToolMessage::ToggleSelectVsPath),
		//
		// DocumentMessage
		entry!(KeyDown(Space); modifiers=[Control], action_dispatch=DocumentMessage::GraphViewOverlayToggle),
		entry!(KeyUp(Escape); action_dispatch=DocumentMessage::Escape),
		entry!(KeyDown(Delete); action_dispatch=DocumentMessage::DeleteSelectedLayers),
		entry!(KeyDown(Backspace); action_dispatch=DocumentMessage::DeleteSelectedLayers),
		entry!(KeyDown(KeyO); modifiers=[Alt], action_dispatch=DocumentMessage::ToggleOverlaysVisibility),
		entry!(KeyDown(KeyS); modifiers=[Alt], action_dispatch=DocumentMessage::ToggleSnapping),
		entry!(KeyDown(KeyH); modifiers=[Accel], action_dispatch=DocumentMessage::ToggleSelectedVisibility),
		entry!(KeyDown(KeyL); modifiers=[Accel], action_dispatch=DocumentMessage::ToggleSelectedLocked),
		entry!(KeyDown(KeyG); modifiers=[Alt], action_dispatch=DocumentMessage::ToggleGridVisibility),
		entry!(KeyDown(KeyZ); modifiers=[Accel, Shift], canonical, action_dispatch=DocumentMessage::Redo),
		entry!(KeyDown(KeyY); modifiers=[Accel], action_dispatch=DocumentMessage::Redo),
		entry!(KeyDown(KeyZ); modifiers=[Accel], action_dispatch=DocumentMessage::Undo),
		entry!(KeyDown(KeyA); modifiers=[Accel], action_dispatch=DocumentMessage::SelectAllLayers),
		entry!(KeyDown(KeyA); modifiers=[Accel, Shift], canonical, action_dispatch=DocumentMessage::DeselectAllLayers),
		entry!(KeyDown(KeyA); modifiers=[Alt], action_dispatch=DocumentMessage::DeselectAllLayers),
		entry!(KeyDown(KeyS); modifiers=[Accel], action_dispatch=DocumentMessage::SaveDocument),
		entry!(KeyDown(KeyD); modifiers=[Accel], canonical, action_dispatch=DocumentMessage::DuplicateSelectedLayers),
		entry!(KeyDown(KeyJ); modifiers=[Accel], action_dispatch=DocumentMessage::DuplicateSelectedLayers),
		entry!(KeyDown(KeyG); modifiers=[Accel], action_dispatch=DocumentMessage::GroupSelectedLayers { group_folder_type: GroupFolderType::Layer }),
		entry!(KeyDown(KeyG); modifiers=[Accel, Shift], action_dispatch=DocumentMessage::UngroupSelectedLayers),
		entry!(KeyDown(KeyN); modifiers=[Accel, Shift], action_dispatch=DocumentMessage::CreateEmptyFolder),
		entry!(KeyDown(Escape); modifiers=[Shift], action_dispatch=DocumentMessage::SelectParentLayer),
		entry!(KeyDown(BracketLeft); modifiers=[Alt], canonical, action_dispatch=DocumentMessage::SelectionStepBack),
		entry!(KeyDown(BracketRight); modifiers=[Alt], canonical, action_dispatch=DocumentMessage::SelectionStepForward),
		entry!(KeyDown(MouseBack); action_dispatch=DocumentMessage::SelectionStepBack),
		entry!(KeyDown(MouseForward); action_dispatch=DocumentMessage::SelectionStepForward),
		entry!(KeyDown(Digit0); modifiers=[Accel], action_dispatch=DocumentMessage::ZoomCanvasToFitAll),
		entry!(KeyDown(Digit1); modifiers=[Accel], action_dispatch=DocumentMessage::ZoomCanvasTo100Percent),
		entry!(KeyDown(Digit2); modifiers=[Accel], action_dispatch=DocumentMessage::ZoomCanvasTo200Percent),
		entry!(KeyDown(BracketLeft); modifiers=[Accel, Shift], action_dispatch=DocumentMessage::SelectedLayersLowerToBack),
		entry!(KeyDown(BracketRight); modifiers=[Accel, Shift], action_dispatch=DocumentMessage::SelectedLayersRaiseToFront),
		entry!(KeyDown(BracketLeft); modifiers=[Accel], action_dispatch=DocumentMessage::SelectedLayersLower),
		entry!(KeyDown(BracketRight); modifiers=[Accel], action_dispatch=DocumentMessage::SelectedLayersRaise),
		entry!(KeyDown(ArrowUp); modifiers=[Shift, ArrowLeft], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -BIG_NUDGE_AMOUNT, delta_y: -BIG_NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowUp); modifiers=[Shift, ArrowRight], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: BIG_NUDGE_AMOUNT, delta_y: -BIG_NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowUp); modifiers=[Shift], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: 0., delta_y: -BIG_NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowDown); modifiers=[Shift, ArrowLeft], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -BIG_NUDGE_AMOUNT, delta_y: BIG_NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowDown); modifiers=[Shift, ArrowRight], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: BIG_NUDGE_AMOUNT, delta_y: BIG_NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowDown); modifiers=[Shift], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: 0., delta_y: BIG_NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowLeft); modifiers=[Shift, ArrowUp], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -BIG_NUDGE_AMOUNT, delta_y: -BIG_NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowLeft); modifiers=[Shift, ArrowDown], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -BIG_NUDGE_AMOUNT, delta_y: BIG_NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowLeft); modifiers=[Shift], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -BIG_NUDGE_AMOUNT, delta_y: 0., resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowRight); modifiers=[Shift, ArrowUp], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: BIG_NUDGE_AMOUNT, delta_y: -BIG_NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowRight); modifiers=[Shift, ArrowDown], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: BIG_NUDGE_AMOUNT, delta_y: BIG_NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowRight); modifiers=[Shift], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: BIG_NUDGE_AMOUNT, delta_y: 0., resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowUp); modifiers=[ArrowLeft], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowUp); modifiers=[ArrowRight], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowUp); action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: 0., delta_y: -NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowDown); modifiers=[ArrowLeft], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowDown); modifiers=[ArrowRight], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowDown); action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: 0., delta_y: NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowLeft); modifiers=[ArrowUp], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowLeft); modifiers=[ArrowDown], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowLeft); action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: 0., resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowRight); modifiers=[ArrowUp], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowRight); modifiers=[ArrowDown], action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT, resize: Alt, resize_opposite_corner: Control }),
		entry!(KeyDown(ArrowRight); action_dispatch=DocumentMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: 0., resize: Alt, resize_opposite_corner: Control }),
		//
		// TransformLayerMessage
		entry!(KeyDown(KeyG); action_dispatch=TransformLayerMessage::BeginGrab),
		entry!(KeyDown(KeyR); action_dispatch=TransformLayerMessage::BeginRotate),
		entry!(KeyDown(KeyS); action_dispatch=TransformLayerMessage::BeginScale),
		entry!(KeyDown(Digit0); action_dispatch=TransformLayerMessage::TypeDigit { digit: 0 }),
		entry!(KeyDown(Digit1); action_dispatch=TransformLayerMessage::TypeDigit { digit: 1 }),
		entry!(KeyDown(Digit2); action_dispatch=TransformLayerMessage::TypeDigit { digit: 2 }),
		entry!(KeyDown(Digit3); action_dispatch=TransformLayerMessage::TypeDigit { digit: 3 }),
		entry!(KeyDown(Digit4); action_dispatch=TransformLayerMessage::TypeDigit { digit: 4 }),
		entry!(KeyDown(Digit5); action_dispatch=TransformLayerMessage::TypeDigit { digit: 5 }),
		entry!(KeyDown(Digit6); action_dispatch=TransformLayerMessage::TypeDigit { digit: 6 }),
		entry!(KeyDown(Digit7); action_dispatch=TransformLayerMessage::TypeDigit { digit: 7 }),
		entry!(KeyDown(Digit8); action_dispatch=TransformLayerMessage::TypeDigit { digit: 8 }),
		entry!(KeyDown(Digit9); action_dispatch=TransformLayerMessage::TypeDigit { digit: 9 }),
		//
		// NavigationMessage
		entry!(KeyDown(MouseMiddle); modifiers=[Alt], action_dispatch=NavigationMessage::BeginCanvasTilt { was_dispatched_from_menu: false }),
		entry!(KeyDown(MouseLeft); modifiers=[Alt, Space], action_dispatch=NavigationMessage::BeginCanvasTilt { was_dispatched_from_menu: false }),
		entry!(KeyDown(MouseMiddle); modifiers=[Control], action_dispatch=NavigationMessage::BeginCanvasZoom),
		entry!(KeyDown(MouseLeft); modifiers=[Control, Space], action_dispatch=NavigationMessage::BeginCanvasZoom),
		entry!(KeyDown(MouseMiddle); action_dispatch=NavigationMessage::BeginCanvasPan),
		entry!(KeyDown(MouseLeft); modifiers=[Space], action_dispatch=NavigationMessage::BeginCanvasPan),
		entry!(KeyDown(NumpadAdd); modifiers=[Accel], action_dispatch=NavigationMessage::CanvasZoomIncrease { center_on_mouse: false }),
		entry!(KeyDown(Equal); modifiers=[Accel], action_dispatch=NavigationMessage::CanvasZoomIncrease { center_on_mouse: false }),
		entry!(KeyDown(Minus); modifiers=[Accel], action_dispatch=NavigationMessage::CanvasZoomDecrease { center_on_mouse: false }),
		entry!(KeyDown(KeyF); modifiers=[Alt], action_dispatch=NavigationMessage::CanvasFlip),
		entry!(WheelScroll; modifiers=[Control], action_dispatch=NavigationMessage::CanvasZoomMouseWheel),
		entry!(WheelScroll; modifiers=[Shift], action_dispatch=NavigationMessage::CanvasPanMouseWheel { use_y_as_x: true }),
		entry!(WheelScroll; action_dispatch=NavigationMessage::CanvasPanMouseWheel { use_y_as_x: false }),
		entry!(KeyDown(PageUp); modifiers=[Shift], action_dispatch=NavigationMessage::CanvasPanByViewportFraction { delta: DVec2::new(1., 0.) }),
		entry!(KeyDown(PageDown); modifiers=[Shift], action_dispatch=NavigationMessage::CanvasPanByViewportFraction { delta: DVec2::new(-1., 0.) }),
		entry!(KeyDown(PageUp); action_dispatch=NavigationMessage::CanvasPanByViewportFraction { delta: DVec2::new(0., 1.) }),
		entry!(KeyDown(PageDown); action_dispatch=NavigationMessage::CanvasPanByViewportFraction { delta: DVec2::new(0., -1.) }),
		entry!(KeyDown(Period); action_dispatch=NavigationMessage::FitViewportToSelection),
		//
		// PortfolioMessage
		entry!(KeyDown(Tab); modifiers=[Control], action_dispatch=PortfolioMessage::NextDocument),
		entry!(KeyDown(Tab); modifiers=[Control, Shift], action_dispatch=PortfolioMessage::PrevDocument),
		entry!(KeyDown(KeyW); modifiers=[Accel], action_dispatch=PortfolioMessage::CloseActiveDocumentWithConfirmation),
		entry!(KeyDown(KeyW); modifiers=[Accel, Alt], action_dispatch=PortfolioMessage::CloseAllDocumentsWithConfirmation),
		entry!(KeyDown(KeyO); modifiers=[Accel], action_dispatch=PortfolioMessage::OpenDocument),
		entry!(KeyDown(KeyI); modifiers=[Accel], action_dispatch=PortfolioMessage::Import),
		entry!(KeyDown(KeyX); modifiers=[Accel], action_dispatch=PortfolioMessage::Cut { clipboard: Clipboard::Device }),
		entry!(KeyDown(KeyC); modifiers=[Accel], action_dispatch=PortfolioMessage::Copy { clipboard: Clipboard::Device }),
		entry!(KeyDown(KeyR); modifiers=[Alt], action_dispatch=PortfolioMessage::ToggleRulers),
		//
		// FrontendMessage
		entry!(KeyDown(KeyV); modifiers=[Accel], action_dispatch=FrontendMessage::TriggerPaste),
		//
		// DialogMessage
		entry!(KeyDown(KeyE); modifiers=[Accel], action_dispatch=DialogMessage::RequestExportDialog),
		entry!(KeyDown(KeyN); modifiers=[Accel], action_dispatch=DialogMessage::RequestNewDocumentDialog),
		entry!(KeyDown(Comma); modifiers=[Accel], action_dispatch=DialogMessage::RequestPreferencesDialog),
		//
		// DebugMessage
		entry!(KeyDown(Digit0); modifiers=[Alt], action_dispatch=DebugMessage::MessageOff),
		entry!(KeyDown(Digit1); modifiers=[Alt], action_dispatch=DebugMessage::MessageNames),
		entry!(KeyDown(Digit2); modifiers=[Alt], action_dispatch=DebugMessage::MessageContents),
		// AnimationMessage
		entry!(KeyDown(Space); modifiers=[Shift], action_dispatch=AnimationMessage::ToggleLivePreview),
		entry!(KeyDown(Home); modifiers=[Shift], action_dispatch=AnimationMessage::RestartAnimation),
	];
	let (mut key_up, mut key_down, mut key_up_no_repeat, mut key_down_no_repeat, mut double_click, mut wheel_scroll, mut pointer_move, mut pointer_shake) = mappings;

	let sort = |list: &mut KeyMappingEntries| list.0.sort_by(|a, b| b.modifiers.count_ones().cmp(&a.modifiers.count_ones()));
	// Sort the sublists of `key_up`, `key_down`, `key_up_no_repeat`, and `key_down_no_repeat`
	for list in [&mut key_up, &mut key_down, &mut key_up_no_repeat, &mut key_down_no_repeat] {
		for sublist in list {
			sort(sublist);
		}
	}
	// Sort the sublists of `double_click`
	for sublist in &mut double_click {
		sort(sublist)
	}
	// Sort `wheel_scroll`
	sort(&mut wheel_scroll);
	// Sort `pointer_move`
	sort(&mut pointer_move);
	// Sort `pointer_shake`
	sort(&mut pointer_shake);

	Mapping {
		key_up,
		key_down,
		key_up_no_repeat,
		key_down_no_repeat,
		double_click,
		wheel_scroll,
		pointer_move,
		pointer_shake,
	}
}

/// Default mappings except that scrolling without modifier keys held down is bound to zooming instead of vertical panning
pub fn zoom_with_scroll() -> Mapping {
	use InputMapperMessage::*;

	let mut mapping = input_mappings();

	let remove = [
		entry!(WheelScroll; modifiers=[Control], action_dispatch=NavigationMessage::CanvasZoomMouseWheel),
		entry!(WheelScroll; modifiers=[Shift], action_dispatch=NavigationMessage::CanvasPanMouseWheel { use_y_as_x: true }),
		entry!(WheelScroll; action_dispatch=NavigationMessage::CanvasPanMouseWheel { use_y_as_x: false }),
	];
	let add = [
		entry!(WheelScroll; modifiers=[Control], action_dispatch=NavigationMessage::CanvasPanMouseWheel { use_y_as_x: true }),
		entry!(WheelScroll; modifiers=[Shift], action_dispatch=NavigationMessage::CanvasPanMouseWheel { use_y_as_x: false }),
		entry!(WheelScroll; action_dispatch=NavigationMessage::CanvasZoomMouseWheel),
	];

	apply_mapping_patch(&mut mapping, remove, add);

	mapping
}

fn apply_mapping_patch<'a, const N: usize, const M: usize, const X: usize, const Y: usize>(
	mapping: &mut Mapping,
	remove: impl IntoIterator<Item = &'a [&'a [MappingEntry; N]; M]>,
	add: impl IntoIterator<Item = &'a [&'a [MappingEntry; X]; Y]>,
) {
	for entry in remove.into_iter().flat_map(|inner| inner.iter()).flat_map(|inner| inner.iter()) {
		mapping.remove(entry);
	}

	for entry in add.into_iter().flat_map(|inner| inner.iter()).flat_map(|inner| inner.iter()) {
		mapping.add(entry.clone());
	}
}
