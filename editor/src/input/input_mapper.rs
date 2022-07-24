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
	pub pointer_move: KeyMappingEntries,
	pub mouse_scroll: KeyMappingEntries,
	pub double_click: KeyMappingEntries,
}

impl Default for Mapping {
	fn default() -> Self {
		use input_mapper_macros::{entry, entry_raw, mapping, modifiers};
		use Key::*;

		// WARNING!
		// If a new mapping you added here isn't working (and perhaps another lower-precedence one is instead), make sure to advertise
		// it as an available action in the respective message handler file (such as the bottom of `document_message_handler.rs`).

		let mappings = mapping![
			// Higher priority than entries in sections below
			entry! {action_dispatch=MovementMessage::PointerMove { snap_angle: KeyControl, wait_for_snap_angle_release: true, snap_zoom: KeyControl, zoom_from_viewport: None }, refresh_on=[KeyControl]},
			// Transform layers
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
			entry! {action_dispatch=TransformLayerMessage::PointerMove { slow_key: KeyShift, snap_key: KeyControl }, refresh_on=[KeyShift, KeyControl]},
			// Select
			entry! {action_dispatch=SelectToolMessage::PointerMove { axis_align: KeyShift, snap_angle: KeyControl, center: KeyAlt }, refresh_on=[KeyControl, KeyShift, KeyAlt]},
			entry! {action_dispatch=SelectToolMessage::DragStart { add_to_selection: KeyShift }, key_down=Lmb},
			entry! {action_dispatch=SelectToolMessage::DragStop, key_up=Lmb},
			entry! {action_dispatch=SelectToolMessage::DragStop, key_down=KeyEnter},
			entry! {action_dispatch=SelectToolMessage::EditLayer, on_message=InputMapperMessage::DoubleClick},
			entry! {action_dispatch=SelectToolMessage::Abort, key_down=Rmb},
			entry! {action_dispatch=SelectToolMessage::Abort, key_down=KeyEscape},
			// Artboard
			entry! {action_dispatch=ArtboardToolMessage::PointerDown, key_down=Lmb},
			entry! {action_dispatch=ArtboardToolMessage::PointerMove { constrain_axis_or_aspect: KeyShift, center: KeyAlt }, refresh_on=[KeyShift, KeyAlt]},
			entry! {action_dispatch=ArtboardToolMessage::PointerUp, key_up=Lmb},
			entry! {action_dispatch=ArtboardToolMessage::DeleteSelected, key_down=KeyDelete},
			entry! {action_dispatch=ArtboardToolMessage::DeleteSelected, key_down=KeyBackspace},
			// Navigate
			entry! {action_dispatch=NavigateToolMessage::ClickZoom { zoom_in: false }, key_up=Lmb, modifiers=[KeyShift]},
			entry! {action_dispatch=NavigateToolMessage::ClickZoom { zoom_in: true }, key_up=Lmb},
			entry! {action_dispatch=NavigateToolMessage::PointerMove { snap_angle: KeyControl, snap_zoom: KeyControl }, refresh_on=[KeyControl]},
			entry! {action_dispatch=NavigateToolMessage::TranslateCanvasBegin, key_down=Mmb},
			entry! {action_dispatch=NavigateToolMessage::RotateCanvasBegin, key_down=Rmb},
			entry! {action_dispatch=NavigateToolMessage::ZoomCanvasBegin, key_down=Lmb},
			entry! {action_dispatch=NavigateToolMessage::TransformCanvasEnd, key_up=Rmb},
			entry! {action_dispatch=NavigateToolMessage::TransformCanvasEnd, key_up=Lmb},
			entry! {action_dispatch=NavigateToolMessage::TransformCanvasEnd, key_up=Mmb},
			// Eyedropper
			entry! {action_dispatch=EyedropperToolMessage::LeftMouseDown, key_down=Lmb},
			entry! {action_dispatch=EyedropperToolMessage::RightMouseDown, key_down=Rmb},
			// Text
			entry! {action_dispatch=TextMessage::Interact, key_up=Lmb},
			entry! {action_dispatch=TextMessage::Abort, key_down=KeyEscape},
			entry! {action_dispatch=TextMessage::CommitText, key_down=KeyEnter, modifiers=[KeyControl]},
			// Gradient
			entry! {action_dispatch=GradientToolMessage::PointerDown, key_down=Lmb},
			entry! {action_dispatch=GradientToolMessage::PointerMove { constrain_axis: KeyShift }, refresh_on=[KeyShift]},
			entry! {action_dispatch=GradientToolMessage::PointerUp, key_up=Lmb},
			// Rectangle
			entry! {action_dispatch=RectangleToolMessage::DragStart, key_down=Lmb},
			entry! {action_dispatch=RectangleToolMessage::DragStop, key_up=Lmb},
			entry! {action_dispatch=RectangleToolMessage::Abort, key_down=Rmb},
			entry! {action_dispatch=RectangleToolMessage::Abort, key_down=KeyEscape},
			entry! {action_dispatch=RectangleToolMessage::Resize { center: KeyAlt, lock_ratio: KeyShift }, refresh_on=[KeyAlt, KeyShift]},
			// Ellipse
			entry! {action_dispatch=EllipseToolMessage::DragStart, key_down=Lmb},
			entry! {action_dispatch=EllipseToolMessage::DragStop, key_up=Lmb},
			entry! {action_dispatch=EllipseToolMessage::Abort, key_down=Rmb},
			entry! {action_dispatch=EllipseToolMessage::Abort, key_down=KeyEscape},
			entry! {action_dispatch=EllipseToolMessage::Resize { center: KeyAlt, lock_ratio: KeyShift }, refresh_on=[KeyAlt, KeyShift]},
			// Shape
			entry! {action_dispatch=ShapeToolMessage::DragStart, key_down=Lmb},
			entry! {action_dispatch=ShapeToolMessage::DragStop, key_up=Lmb},
			entry! {action_dispatch=ShapeToolMessage::Abort, key_down=Rmb},
			entry! {action_dispatch=ShapeToolMessage::Abort, key_down=KeyEscape},
			entry! {action_dispatch=ShapeToolMessage::Resize { center: KeyAlt, lock_ratio: KeyShift }, refresh_on=[KeyAlt, KeyShift]},
			// Line
			entry! {action_dispatch=LineToolMessage::DragStart, key_down=Lmb},
			entry! {action_dispatch=LineToolMessage::DragStop, key_up=Lmb},
			entry! {action_dispatch=LineToolMessage::Abort, key_down=Rmb},
			entry! {action_dispatch=LineToolMessage::Abort, key_down=KeyEscape},
			entry! {action_dispatch=LineToolMessage::Redraw { center: KeyAlt, lock_angle: KeyControl, snap_angle: KeyShift }, refresh_on=[KeyAlt, KeyShift, KeyControl]},
			// Path
			entry! {action_dispatch=PathToolMessage::DragStart { add_to_selection: KeyShift }, key_down=Lmb},
			entry! {action_dispatch=PathToolMessage::PointerMove { alt_mirror_angle: KeyAlt, shift_mirror_distance: KeyShift }, refresh_on=[KeyAlt, KeyShift]},
			entry! {action_dispatch=PathToolMessage::Delete, key_down=KeyDelete},
			entry! {action_dispatch=PathToolMessage::Delete, key_down=KeyBackspace},
			entry! {action_dispatch=PathToolMessage::DragStop, key_up=Lmb},
			// Pen
			entry! {action_dispatch=PenToolMessage::PointerMove { snap_angle: KeyControl, break_handle: KeyShift }, refresh_on=[KeyShift, KeyControl]},
			entry! {action_dispatch=PenToolMessage::DragStart, key_down=Lmb},
			entry! {action_dispatch=PenToolMessage::DragStop, key_up=Lmb},
			entry! {action_dispatch=PenToolMessage::Confirm, key_down=Rmb},
			entry! {action_dispatch=PenToolMessage::Confirm, key_down=KeyEscape},
			entry! {action_dispatch=PenToolMessage::Confirm, key_down=KeyEnter},
			// Freehand
			entry! {action_dispatch=FreehandToolMessage::PointerMove, refresh_on=[]},
			entry! {action_dispatch=FreehandToolMessage::DragStart, key_down=Lmb},
			entry! {action_dispatch=FreehandToolMessage::DragStop, key_up=Lmb},
			// Spline
			entry! {action_dispatch=SplineToolMessage::PointerMove, refresh_on=[]},
			entry! {action_dispatch=SplineToolMessage::DragStart, key_down=Lmb},
			entry! {action_dispatch=SplineToolMessage::DragStop, key_up=Lmb},
			entry! {action_dispatch=SplineToolMessage::Confirm, key_down=Rmb},
			entry! {action_dispatch=SplineToolMessage::Confirm, key_down=KeyEscape},
			entry! {action_dispatch=SplineToolMessage::Confirm, key_down=KeyEnter},
			// Fill
			entry! {action_dispatch=FillToolMessage::LeftMouseDown, key_down=Lmb},
			entry! {action_dispatch=FillToolMessage::RightMouseDown, key_down=Rmb},
			// Tool Actions
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
			// Colors
			entry! {action_dispatch=ToolMessage::ResetColors, key_down=KeyX, modifiers=[KeyShift, KeyControl]},
			entry! {action_dispatch=ToolMessage::SwapColors, key_down=KeyX, modifiers=[KeyShift]},
			entry! {action_dispatch=ToolMessage::SelectRandomPrimaryColor, key_down=KeyC, modifiers=[KeyAlt]},
			// Document actions
			entry! {action_dispatch=DocumentMessage::Redo, key_down=KeyZ, modifiers=[KeyControl, KeyShift]},
			entry! {action_dispatch=DocumentMessage::Undo, key_down=KeyZ, modifiers=[KeyControl]},
			entry! {action_dispatch=DocumentMessage::DeselectAllLayers, key_down=KeyA, modifiers=[KeyControl, KeyAlt]},
			entry! {action_dispatch=DocumentMessage::SelectAllLayers, key_down=KeyA, modifiers=[KeyControl]},
			entry! {action_dispatch=DocumentMessage::DeleteSelectedLayers, key_down=KeyDelete},
			entry! {action_dispatch=DocumentMessage::DeleteSelectedLayers, key_down=KeyBackspace},
			entry! {action_dispatch=DialogMessage::RequestExportDialog, key_down=KeyE, modifiers=[KeyControl]},
			entry! {action_dispatch=DocumentMessage::SaveDocument, key_down=KeyS, modifiers=[KeyControl]},
			entry! {action_dispatch=DocumentMessage::SaveDocument, key_down=KeyS, modifiers=[KeyControl, KeyShift]},
			entry! {action_dispatch=DocumentMessage::DebugPrintDocument, key_down=KeyP, modifiers=[KeyAlt]},
			entry! {action_dispatch=DocumentMessage::ZoomCanvasToFitAll, key_down=Key0, modifiers=[KeyControl]},
			entry! {action_dispatch=DocumentMessage::DuplicateSelectedLayers, key_down=KeyD, modifiers=[KeyControl]},
			entry! {action_dispatch=DocumentMessage::GroupSelectedLayers, key_down=KeyG, modifiers=[KeyControl]},
			entry! {action_dispatch=DocumentMessage::UngroupSelectedLayers, key_down=KeyG, modifiers=[KeyControl, KeyShift]},
			entry! {action_dispatch=DocumentMessage::CreateEmptyFolder { container_path: vec![] }, key_down=KeyN, modifiers=[KeyControl, KeyShift]},
			// Layer transformation
			entry! {action_dispatch=TransformLayerMessage::BeginGrab, key_down=KeyG},
			entry! {action_dispatch=TransformLayerMessage::BeginRotate, key_down=KeyR},
			entry! {action_dispatch=TransformLayerMessage::BeginScale, key_down=KeyS},
			// Movement actions
			// entry_multiplatform! {
			// 	nonmac! {action_dispatch=MovementMessage::RotateCanvasBegin, key_down=Mmb, modifiers=[KeyControl]}},
			// 	mac!    {action_dispatch=MovementMessage::RotateCanvasBegin, key_down=Mmb, modifiers=[KeyCommand]}},
			// }
			entry! {action_dispatch=MovementMessage::ZoomCanvasBegin, key_down=Mmb, modifiers=[KeyShift]},
			entry! {action_dispatch=MovementMessage::TranslateCanvasBegin, key_down=Mmb},
			entry! {action_dispatch=MovementMessage::TransformCanvasEnd, key_up=Mmb},
			entry! {action_dispatch=MovementMessage::TranslateCanvasBegin, key_down=Lmb, modifiers=[KeySpace]},
			entry! {action_dispatch=MovementMessage::TransformCanvasEnd, key_up=Lmb, modifiers=[KeySpace]},
			entry! {action_dispatch=MovementMessage::IncreaseCanvasZoom { center_on_mouse: false }, key_down=KeyPlus, modifiers=[KeyControl]},
			entry! {action_dispatch=MovementMessage::IncreaseCanvasZoom { center_on_mouse: false }, key_down=KeyEquals, modifiers=[KeyControl]},
			entry! {action_dispatch=MovementMessage::DecreaseCanvasZoom { center_on_mouse: false }, key_down=KeyMinus, modifiers=[KeyControl]},
			entry! {action_dispatch=MovementMessage::SetCanvasZoom { zoom_factor: 1. }, key_down=Key1, modifiers=[KeyControl]},
			entry! {action_dispatch=MovementMessage::SetCanvasZoom { zoom_factor: 2. }, key_down=Key2, modifiers=[KeyControl]},
			entry! {action_dispatch=MovementMessage::WheelCanvasZoom, on_message=InputMapperMessage::MouseScroll, modifiers=[KeyControl]},
			entry! {action_dispatch=MovementMessage::WheelCanvasTranslate { use_y_as_x: true }, on_message=InputMapperMessage::MouseScroll, modifiers=[KeyShift]},
			entry! {action_dispatch=MovementMessage::WheelCanvasTranslate { use_y_as_x: false }, on_message=InputMapperMessage::MouseScroll},
			entry! {action_dispatch=MovementMessage::TranslateCanvasByViewportFraction { delta: DVec2::new(1., 0.) }, key_down=KeyPageUp, modifiers=[KeyShift]},
			entry! {action_dispatch=MovementMessage::TranslateCanvasByViewportFraction { delta: DVec2::new(-1., 0.) }, key_down=KeyPageDown, modifiers=[KeyShift]},
			entry! {action_dispatch=MovementMessage::TranslateCanvasByViewportFraction { delta: DVec2::new(0., 1.) }, key_down=KeyPageUp},
			entry! {action_dispatch=MovementMessage::TranslateCanvasByViewportFraction { delta: DVec2::new(0., -1.) }, key_down=KeyPageDown},
			// Portfolio actions
			entry! {action_dispatch=PortfolioMessage::OpenDocument, key_down=KeyO, modifiers=[KeyControl]},
			entry! {action_dispatch=PortfolioMessage::Import, key_down=KeyI, modifiers=[KeyControl]},
			entry! {action_dispatch=DialogMessage::RequestNewDocumentDialog, key_down=KeyN, modifiers=[KeyControl]},
			entry! {action_dispatch=PortfolioMessage::NextDocument, key_down=KeyTab, modifiers=[KeyControl]},
			entry! {action_dispatch=PortfolioMessage::PrevDocument, key_down=KeyTab, modifiers=[KeyControl, KeyShift]},
			entry! {action_dispatch=DialogMessage::CloseAllDocumentsWithConfirmation, key_down=KeyW, modifiers=[KeyControl, KeyAlt]},
			entry! {action_dispatch=PortfolioMessage::CloseActiveDocumentWithConfirmation, key_down=KeyW, modifiers=[KeyControl]},
			entry! {action_dispatch=PortfolioMessage::Copy { clipboard: Clipboard::Device }, key_down=KeyC, modifiers=[KeyControl]},
			entry! {action_dispatch=PortfolioMessage::Cut { clipboard: Clipboard::Device }, key_down=KeyX, modifiers=[KeyControl]},
			// Nudging
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
			// Reorder Layers
			entry! {action_dispatch=DocumentMessage::ReorderSelectedLayers { relative_index_offset: isize::MAX }, key_down=KeyRightCurlyBracket, modifiers=[KeyControl]}, // TODO: Use KeyRightBracket with Ctrl+Shift modifiers once input system is fixed
			entry! {action_dispatch=DocumentMessage::ReorderSelectedLayers { relative_index_offset: 1 }, key_down=KeyRightBracket, modifiers=[KeyControl]},
			entry! {action_dispatch=DocumentMessage::ReorderSelectedLayers { relative_index_offset: -1 }, key_down=KeyLeftBracket, modifiers=[KeyControl]},
			entry! {action_dispatch=DocumentMessage::ReorderSelectedLayers { relative_index_offset: isize::MIN }, key_down=KeyLeftCurlyBracket, modifiers=[KeyControl]}, // TODO: Use KeyLeftBracket with Ctrl+Shift modifiers once input system is fixed
			// Debug Actions
			entry! {action_dispatch=DebugMessage::ToggleTraceLogs, key_down=KeyT, modifiers=[KeyAlt]},
			entry! {action_dispatch=DebugMessage::MessageOff, key_down=Key0, modifiers=[KeyAlt]},
			entry! {action_dispatch=DebugMessage::MessageNames, key_down=Key1, modifiers=[KeyAlt]},
			entry! {action_dispatch=DebugMessage::MessageContents, key_down=Key2, modifiers=[KeyAlt]},
		];
		let (mut key_up, mut key_down, mut pointer_move, mut mouse_scroll, mut double_click) = mappings;

		// TODO: Hardcode these 10 lines into 10 lines of declarations, or make this use a macro to do all 10 in one line
		const NUMBER_KEYS: [Key; 10] = [Key0, Key1, Key2, Key3, Key4, Key5, Key6, Key7, Key8, Key9];
		for (i, key) in NUMBER_KEYS.iter().enumerate() {
			key_down[*key as usize].0.insert(
				0,
				MappingEntry {
					action: TransformLayerMessage::TypeDigit { digit: i as u8 }.into(),
					trigger: InputMapperMessage::KeyDown(*key),
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
		sort(&mut pointer_move);
		sort(&mut mouse_scroll);
		sort(&mut double_click);

		Self {
			key_up,
			key_down,
			pointer_move,
			mouse_scroll,
			double_click,
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
	#[default]
	Agnostic,
	NonMac,
	Mac,
}

#[derive(PartialEq, Clone, Debug)]
pub struct MappingEntry {
	pub trigger: InputMapperMessage,
	pub modifiers: KeyStates,
	pub action: Message,
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

impl Default for KeyMappingEntries {
	fn default() -> Self {
		Self::new()
	}
}

mod input_mapper_macros {
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

	/// When this `action_dispatch` action is available and the input `on_message` is received, dispatch the `action_dispatch` as an output message
	macro_rules! entry_raw {
		// Syntax that matches on a KeyDown or KeyUp message input
		{action_dispatch=$action_dispatch:expr, key_down=$key:ident $(, modifiers=[$($m:ident),* $(,)?])?, layout=$layout:ident} => {{
			entry_raw! {action_dispatch=$action_dispatch, on_message=InputMapperMessage::KeyDown(Key::$key) $(, modifiers=[$($m),*])?, layout=$layout}
		}};
		{action_dispatch=$action_dispatch:expr, key_up=$key:ident $(, modifiers=[$($m:ident),* $(,)?])?, layout=$layout:ident} => {{
			entry_raw! {action_dispatch=$action_dispatch, on_message=InputMapperMessage::KeyUp(Key::$key) $(, modifiers=[$($m),* ])?, layout=$layout}
		}};
		// Syntax that matches on a custom message input
		{action_dispatch=$action_dispatch:expr, on_message=$on_message:expr $(, modifiers=[$($m:ident),* $(,)?])?, layout=$layout:ident} => {{
			&[MappingEntry { trigger: $on_message, modifiers: modifiers!($($($m),*)?), action: $action_dispatch.into(), platform_layout: KeyboardPlatformLayout::$layout }]
		}};
		// Syntax that matches on a PointerMove input and also on KeyDown and KeyUp presses for specified refresh keys
		{action_dispatch=$action_dispatch:expr, refresh_on=[$($m:ident),* $(,)?], layout=$layout:ident} => {{
			&[
				// Normal case for the message
				MappingEntry { trigger: InputMapperMessage::PointerMove, action: $action_dispatch.into(), platform_layout: KeyboardPlatformLayout::$layout, modifiers: modifiers!() },

				// Also cause the message to be sent when the mouse doesn't move, but any of the triggered keys change
				$(
				MappingEntry { trigger: InputMapperMessage::KeyDown(Key::$m), action: $action_dispatch.into(), platform_layout: KeyboardPlatformLayout::$layout, modifiers: modifiers!() },
				MappingEntry { trigger: InputMapperMessage::KeyUp(Key::$m), action: $action_dispatch.into(), platform_layout: KeyboardPlatformLayout::$layout, modifiers: modifiers!() },
				)*
			]
		}};
	}

	macro_rules! entry {
		// // Syntax that matches on a KeyDown or KeyUp message input
		// {action_dispatch=$action_dispatch:expr, key_down=$key:ident modifiers=$modifiers:tt} => {{
		// 	entry_raw! {action_dispatch=$action_dispatch, key_down=$key_down, modifiers=$modifiers, layout=Agnostic}
		// }};
		// {action_dispatch=$action_dispatch:expr, key_up=$key:ident modifiers=$modifiers:tt} => {{
		// 	entry_raw! {action_dispatch=$action_dispatch, key_up=$key_up, modifiers=$modifiers, layout=Agnostic}
		// }};
		// // Syntax that matches on a custom message input
		// {action_dispatch=$action_dispatch:expr, on_message=$on_message:expr modifiers=$modifiers:tt} => {{
		// 	entry_raw! {action_dispatch=$action_dispatch, on_message=$on_message, modifiers=$modifiers, layout=Agnostic}
		// }};
		// // Syntax that matches on a PointerMove input and also on KeyDown and KeyUp presses for specified refresh keys
		// {action_dispatch=$action_dispatch:expr, refresh_on=$refresh_on:tt} => {{
		// 	entry_raw! {action_dispatch=$action_dispatch, refresh_on=$refresh_on, layout=Agnostic}
		// }};
		{$($arg:expr)*} => {{
			entry! {$($arg)* , layout=Agnostic}
		}}
	}

	macro_rules! mapping {
		//[$(<action_dispatch=$action_dispatch:expr; message=$key:expr; $(modifiers=[$($m:ident),* $(,)?];)?>)*] => {{
		[$($entry:expr),* $(,)?] => {{
			let mut key_up = KeyMappingEntries::key_array();
			let mut key_down = KeyMappingEntries::key_array();
			let mut pointer_move: KeyMappingEntries = Default::default();
			let mut mouse_scroll: KeyMappingEntries = Default::default();
			let mut double_click: KeyMappingEntries = Default::default();
			$(
				for entry in $entry {
					let arr = match entry.trigger {
						InputMapperMessage::KeyDown(key) => &mut key_down[key as usize],
						InputMapperMessage::KeyUp(key) => &mut key_up[key as usize],
						InputMapperMessage::MouseScroll => &mut mouse_scroll,
						InputMapperMessage::PointerMove => &mut pointer_move,
						InputMapperMessage::DoubleClick => &mut double_click,
					};
					arr.push(entry.clone());
				}
			)*
			(key_up, key_down, pointer_move, mouse_scroll, double_click)
		}};
	}

	pub(crate) use entry;
	pub(crate) use entry_raw;
	pub(crate) use mapping;
	pub(crate) use modifiers;
}
