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
		use input_mapper_macros::{entry, mapping, modifiers};
		use Key::*;

		// WARNING!
		// If a new mapping you added here isn't working (and perhaps another lower-precedence one is instead), make sure to advertise
		// it as an available action in the respective message handler file (such as the bottom of `document_message_handler.rs`).

		let mappings = mapping![
			// Higher priority than entries in sections below
			entry! {action=PortfolioMessage::Paste { clipboard: Clipboard::User }, key_down=KeyV, modifiers=[KeyControl]},
			entry! {action=MovementMessage::PointerMove { snap_angle: KeyControl, wait_for_snap_angle_release: true, snap_zoom: KeyControl, zoom_from_viewport: None }, message=InputMapperMessage::PointerMove},
			// Transform layers
			entry! {action=TransformLayerMessage::ApplyTransformOperation, key_down=KeyEnter},
			entry! {action=TransformLayerMessage::ApplyTransformOperation, key_down=Lmb},
			entry! {action=TransformLayerMessage::CancelTransformOperation, key_down=KeyEscape},
			entry! {action=TransformLayerMessage::CancelTransformOperation, key_down=Rmb},
			entry! {action=TransformLayerMessage::ConstrainX, key_down=KeyX},
			entry! {action=TransformLayerMessage::ConstrainY, key_down=KeyY},
			entry! {action=TransformLayerMessage::TypeBackspace, key_down=KeyBackspace},
			entry! {action=TransformLayerMessage::TypeNegate, key_down=KeyMinus},
			entry! {action=TransformLayerMessage::TypeDecimalPoint, key_down=KeyComma},
			entry! {action=TransformLayerMessage::TypeDecimalPoint, key_down=KeyPeriod},
			entry! {action=TransformLayerMessage::PointerMove { slow_key: KeyShift, snap_key: KeyControl }, triggers=[KeyShift, KeyControl]},
			// Select
			entry! {action=SelectMessage::PointerMove { axis_align: KeyShift, snap_angle: KeyControl, center: KeyAlt }, message=InputMapperMessage::PointerMove},
			entry! {action=SelectMessage::DragStart { add_to_selection: KeyShift }, key_down=Lmb},
			entry! {action=SelectMessage::DragStop, key_up=Lmb},
			entry! {action=SelectMessage::EditLayer, message=InputMapperMessage::DoubleClick},
			entry! {action=SelectMessage::Abort, key_down=Rmb},
			entry! {action=SelectMessage::Abort, key_down=KeyEscape},
			// Crop
			entry! {action=CropMessage::PointerDown, key_down=Lmb},
			entry! {action=CropMessage::PointerMove { constrain_axis_or_aspect: KeyShift, center: KeyAlt }, message=InputMapperMessage::PointerMove},
			entry! {action=CropMessage::PointerUp, key_up=Lmb},
			entry! {action=CropMessage::DeleteSelected, key_down=KeyDelete},
			entry! {action=CropMessage::DeleteSelected, key_down=KeyBackspace},
			// Navigate
			entry! {action=NavigateMessage::ClickZoom { zoom_in: false }, key_up=Lmb, modifiers=[KeyShift]},
			entry! {action=NavigateMessage::ClickZoom { zoom_in: true }, key_up=Lmb},
			entry! {action=NavigateMessage::PointerMove { snap_angle: KeyControl, snap_zoom: KeyControl }, message=InputMapperMessage::PointerMove},
			entry! {action=NavigateMessage::TranslateCanvasBegin, key_down=Mmb},
			entry! {action=NavigateMessage::RotateCanvasBegin, key_down=Rmb},
			entry! {action=NavigateMessage::ZoomCanvasBegin, key_down=Lmb},
			entry! {action=NavigateMessage::TransformCanvasEnd, key_up=Rmb},
			entry! {action=NavigateMessage::TransformCanvasEnd, key_up=Lmb},
			entry! {action=NavigateMessage::TransformCanvasEnd, key_up=Mmb},
			// Eyedropper
			entry! {action=EyedropperMessage::LeftMouseDown, key_down=Lmb},
			entry! {action=EyedropperMessage::RightMouseDown, key_down=Rmb},
			// Text
			entry! {action=TextMessage::Interact, key_up=Lmb},
			entry! {action=TextMessage::Abort, key_down=KeyEscape},
			entry! {action=TextMessage::CommitText, key_down=KeyEnter, modifiers=[KeyControl]},
			// Rectangle
			entry! {action=RectangleMessage::DragStart, key_down=Lmb},
			entry! {action=RectangleMessage::DragStop, key_up=Lmb},
			entry! {action=RectangleMessage::Abort, key_down=Rmb},
			entry! {action=RectangleMessage::Abort, key_down=KeyEscape},
			entry! {action=RectangleMessage::Resize { center: KeyAlt, lock_ratio: KeyShift }, triggers=[KeyAlt, KeyShift]},
			// Ellipse
			entry! {action=EllipseMessage::DragStart, key_down=Lmb},
			entry! {action=EllipseMessage::DragStop, key_up=Lmb},
			entry! {action=EllipseMessage::Abort, key_down=Rmb},
			entry! {action=EllipseMessage::Abort, key_down=KeyEscape},
			entry! {action=EllipseMessage::Resize { center: KeyAlt, lock_ratio: KeyShift }, triggers=[KeyAlt, KeyShift]},
			// Shape
			entry! {action=ShapeMessage::DragStart, key_down=Lmb},
			entry! {action=ShapeMessage::DragStop, key_up=Lmb},
			entry! {action=ShapeMessage::Abort, key_down=Rmb},
			entry! {action=ShapeMessage::Abort, key_down=KeyEscape},
			entry! {action=ShapeMessage::Resize { center: KeyAlt, lock_ratio: KeyShift }, triggers=[KeyAlt, KeyShift]},
			// Line
			entry! {action=LineMessage::DragStart, key_down=Lmb},
			entry! {action=LineMessage::DragStop, key_up=Lmb},
			entry! {action=LineMessage::Abort, key_down=Rmb},
			entry! {action=LineMessage::Abort, key_down=KeyEscape},
			entry! {action=LineMessage::Redraw { center: KeyAlt, lock_angle: KeyControl, snap_angle: KeyShift }, triggers=[KeyAlt, KeyShift, KeyControl]},
			// Path
			entry! {action=PathMessage::DragStart { add_to_selection: KeyShift }, key_down=Lmb},
			entry! {action=PathMessage::PointerMove { alt_mirror_angle: KeyAlt, shift_mirror_distance: KeyShift }, message=InputMapperMessage::PointerMove},
			entry! {action=PathMessage::DragStop, key_up=Lmb},
			// Pen
			entry! {action=PenMessage::PointerMove, message=InputMapperMessage::PointerMove},
			entry! {action=PenMessage::DragStart, key_down=Lmb},
			entry! {action=PenMessage::DragStop, key_up=Lmb},
			entry! {action=PenMessage::Confirm, key_down=Rmb},
			entry! {action=PenMessage::Confirm, key_down=KeyEscape},
			entry! {action=PenMessage::Confirm, key_down=KeyEnter},
			// Freehand
			entry! {action=FreehandMessage::PointerMove, message=InputMapperMessage::PointerMove},
			entry! {action=FreehandMessage::DragStart, key_down=Lmb},
			entry! {action=FreehandMessage::DragStop, key_up=Lmb},
			// Spline
			entry! {action=SplineMessage::PointerMove, message=InputMapperMessage::PointerMove},
			entry! {action=SplineMessage::DragStart, key_down=Lmb},
			entry! {action=SplineMessage::DragStop, key_up=Lmb},
			entry! {action=SplineMessage::Confirm, key_down=Rmb},
			entry! {action=SplineMessage::Confirm, key_down=KeyEscape},
			entry! {action=SplineMessage::Confirm, key_down=KeyEnter},
			// Fill
			entry! {action=FillMessage::LeftMouseDown, key_down=Lmb},
			entry! {action=FillMessage::RightMouseDown, key_down=Rmb},
			// Tool Actions
			entry! {action=ToolMessage::ActivateTool { tool_type: ToolType::Select }, key_down=KeyV},
			entry! {action=ToolMessage::ActivateTool { tool_type: ToolType::Navigate }, key_down=KeyZ},
			entry! {action=ToolMessage::ActivateTool { tool_type: ToolType::Eyedropper }, key_down=KeyI},
			entry! {action=ToolMessage::ActivateTool { tool_type: ToolType::Text }, key_down=KeyT},
			entry! {action=ToolMessage::ActivateTool { tool_type: ToolType::Fill }, key_down=KeyF},
			entry! {action=ToolMessage::ActivateTool { tool_type: ToolType::Path }, key_down=KeyA},
			entry! {action=ToolMessage::ActivateTool { tool_type: ToolType::Pen }, key_down=KeyP},
			entry! {action=ToolMessage::ActivateTool { tool_type: ToolType::Freehand }, key_down=KeyN},
			entry! {action=ToolMessage::ActivateTool { tool_type: ToolType::Line }, key_down=KeyL},
			entry! {action=ToolMessage::ActivateTool { tool_type: ToolType::Rectangle }, key_down=KeyM},
			entry! {action=ToolMessage::ActivateTool { tool_type: ToolType::Ellipse }, key_down=KeyE},
			entry! {action=ToolMessage::ActivateTool { tool_type: ToolType::Shape }, key_down=KeyY},
			// Colors
			entry! {action=ToolMessage::ResetColors, key_down=KeyX, modifiers=[KeyShift, KeyControl]},
			entry! {action=ToolMessage::SwapColors, key_down=KeyX, modifiers=[KeyShift]},
			// Editor Actions
			entry! {action=FrontendMessage::TriggerFileUpload, key_down=KeyO, modifiers=[KeyControl]},
			// Document Actions
			entry! {action=PortfolioMessage::Paste { clipboard: Clipboard::User }, key_down=KeyV, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::Redo, key_down=KeyZ, modifiers=[KeyControl, KeyShift]},
			entry! {action=DocumentMessage::Undo, key_down=KeyZ, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::DeselectAllLayers, key_down=KeyA, modifiers=[KeyControl, KeyAlt]},
			entry! {action=DocumentMessage::SelectAllLayers, key_down=KeyA, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::CreateEmptyFolder { container_path: vec![] }, key_down=KeyN, modifiers=[KeyControl, KeyShift]},
			entry! {action=DocumentMessage::DeleteSelectedLayers, key_down=KeyDelete},
			entry! {action=DocumentMessage::DeleteSelectedLayers, key_down=KeyBackspace},
			entry! {action=DocumentMessage::ExportDocument, key_down=KeyE, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::SaveDocument, key_down=KeyS, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::SaveDocument, key_down=KeyS, modifiers=[KeyControl, KeyShift]},
			entry! {action=DocumentMessage::DebugPrintDocument, key_down=Key9},
			entry! {action=DocumentMessage::ZoomCanvasToFitAll, key_down=Key0, modifiers=[KeyControl]},
			// Initiate Transform Layers
			entry! {action=TransformLayerMessage::BeginGrab, key_down=KeyG},
			entry! {action=TransformLayerMessage::BeginRotate, key_down=KeyR},
			entry! {action=TransformLayerMessage::BeginScale, key_down=KeyS},
			// Document movement
			entry! {action=MovementMessage::RotateCanvasBegin, key_down=Mmb, modifiers=[KeyControl]},
			entry! {action=MovementMessage::ZoomCanvasBegin, key_down=Mmb, modifiers=[KeyShift]},
			entry! {action=MovementMessage::TranslateCanvasBegin, key_down=Mmb},
			entry! {action=MovementMessage::TransformCanvasEnd, key_up=Mmb},
			entry! {action=MovementMessage::TranslateCanvasBegin, key_down=Lmb, modifiers=[KeySpace]},
			entry! {action=MovementMessage::TransformCanvasEnd, key_up=Lmb, modifiers=[KeySpace]},
			entry! {action=MovementMessage::IncreaseCanvasZoom { center_on_mouse: false }, key_down=KeyPlus, modifiers=[KeyControl]},
			entry! {action=MovementMessage::IncreaseCanvasZoom { center_on_mouse: false }, key_down=KeyEquals, modifiers=[KeyControl]},
			entry! {action=MovementMessage::DecreaseCanvasZoom { center_on_mouse: false }, key_down=KeyMinus, modifiers=[KeyControl]},
			entry! {action=MovementMessage::SetCanvasZoom { zoom_factor: 1. }, key_down=Key1, modifiers=[KeyControl]},
			entry! {action=MovementMessage::SetCanvasZoom { zoom_factor: 2. }, key_down=Key2, modifiers=[KeyControl]},
			entry! {action=MovementMessage::WheelCanvasZoom, message=InputMapperMessage::MouseScroll, modifiers=[KeyControl]},
			entry! {action=MovementMessage::WheelCanvasTranslate { use_y_as_x: true }, message=InputMapperMessage::MouseScroll, modifiers=[KeyShift]},
			entry! {action=MovementMessage::WheelCanvasTranslate { use_y_as_x: false }, message=InputMapperMessage::MouseScroll},
			entry! {action=MovementMessage::TranslateCanvasByViewportFraction { delta: DVec2::new(1., 0.) }, key_down=KeyPageUp, modifiers=[KeyShift]},
			entry! {action=MovementMessage::TranslateCanvasByViewportFraction { delta: DVec2::new(-1., 0.) }, key_down=KeyPageDown, modifiers=[KeyShift]},
			entry! {action=MovementMessage::TranslateCanvasByViewportFraction { delta: DVec2::new(0., 1.) }, key_down=KeyPageUp},
			entry! {action=MovementMessage::TranslateCanvasByViewportFraction { delta: DVec2::new(0., -1.) }, key_down=KeyPageDown},
			// Document actions
			entry! {action=PortfolioMessage::NewDocument, key_down=KeyN, modifiers=[KeyControl]},
			entry! {action=PortfolioMessage::NextDocument, key_down=KeyTab, modifiers=[KeyControl]},
			entry! {action=PortfolioMessage::PrevDocument, key_down=KeyTab, modifiers=[KeyControl, KeyShift]},
			entry! {action=PortfolioMessage::CloseAllDocumentsWithConfirmation, key_down=KeyW, modifiers=[KeyControl, KeyAlt]},
			entry! {action=PortfolioMessage::CloseActiveDocumentWithConfirmation, key_down=KeyW, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::DuplicateSelectedLayers, key_down=KeyD, modifiers=[KeyControl]},
			entry! {action=PortfolioMessage::Copy { clipboard: Clipboard::User }, key_down=KeyC, modifiers=[KeyControl]},
			entry! {action=PortfolioMessage::Cut { clipboard: Clipboard::User }, key_down=KeyX, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::GroupSelectedLayers, key_down=KeyG, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::UngroupSelectedLayers, key_down=KeyG, modifiers=[KeyControl, KeyShift]},
			// Nudging
			entry! {action=DocumentMessage::NudgeSelectedLayers { delta_x: -SHIFT_NUDGE_AMOUNT, delta_y: -SHIFT_NUDGE_AMOUNT }, key_down=KeyArrowUp, modifiers=[KeyShift, KeyArrowLeft]},
			entry! {action=DocumentMessage::NudgeSelectedLayers { delta_x: SHIFT_NUDGE_AMOUNT, delta_y: -SHIFT_NUDGE_AMOUNT }, key_down=KeyArrowUp, modifiers=[KeyShift, KeyArrowRight]},
			entry! {action=DocumentMessage::NudgeSelectedLayers { delta_x: 0., delta_y: -SHIFT_NUDGE_AMOUNT }, key_down=KeyArrowUp, modifiers=[KeyShift]},
			entry! {action=DocumentMessage::NudgeSelectedLayers { delta_x: -SHIFT_NUDGE_AMOUNT, delta_y: SHIFT_NUDGE_AMOUNT }, key_down=KeyArrowDown, modifiers=[KeyShift, KeyArrowLeft]},
			entry! {action=DocumentMessage::NudgeSelectedLayers { delta_x: SHIFT_NUDGE_AMOUNT, delta_y: SHIFT_NUDGE_AMOUNT }, key_down=KeyArrowDown, modifiers=[KeyShift, KeyArrowRight]},
			entry! {action=DocumentMessage::NudgeSelectedLayers { delta_x: 0., delta_y: SHIFT_NUDGE_AMOUNT }, key_down=KeyArrowDown, modifiers=[KeyShift]},
			entry! {action=DocumentMessage::NudgeSelectedLayers { delta_x: -SHIFT_NUDGE_AMOUNT, delta_y: -SHIFT_NUDGE_AMOUNT }, key_down=KeyArrowLeft, modifiers=[KeyShift, KeyArrowUp]},
			entry! {action=DocumentMessage::NudgeSelectedLayers { delta_x: -SHIFT_NUDGE_AMOUNT, delta_y: SHIFT_NUDGE_AMOUNT }, key_down=KeyArrowLeft, modifiers=[KeyShift, KeyArrowDown]},
			entry! {action=DocumentMessage::NudgeSelectedLayers { delta_x: -SHIFT_NUDGE_AMOUNT, delta_y: 0. }, key_down=KeyArrowLeft, modifiers=[KeyShift]},
			entry! {action=DocumentMessage::NudgeSelectedLayers { delta_x: SHIFT_NUDGE_AMOUNT, delta_y: -SHIFT_NUDGE_AMOUNT }, key_down=KeyArrowRight, modifiers=[KeyShift, KeyArrowUp]},
			entry! {action=DocumentMessage::NudgeSelectedLayers { delta_x: SHIFT_NUDGE_AMOUNT, delta_y: SHIFT_NUDGE_AMOUNT }, key_down=KeyArrowRight, modifiers=[KeyShift, KeyArrowDown]},
			entry! {action=DocumentMessage::NudgeSelectedLayers { delta_x: SHIFT_NUDGE_AMOUNT, delta_y: 0. }, key_down=KeyArrowRight, modifiers=[KeyShift]},
			entry! {action=DocumentMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT }, key_down=KeyArrowUp, modifiers=[KeyArrowLeft]},
			entry! {action=DocumentMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT }, key_down=KeyArrowUp, modifiers=[KeyArrowRight]},
			entry! {action=DocumentMessage::NudgeSelectedLayers { delta_x: 0., delta_y: -NUDGE_AMOUNT }, key_down=KeyArrowUp},
			entry! {action=DocumentMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT }, key_down=KeyArrowDown, modifiers=[KeyArrowLeft]},
			entry! {action=DocumentMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT }, key_down=KeyArrowDown, modifiers=[KeyArrowRight]},
			entry! {action=DocumentMessage::NudgeSelectedLayers { delta_x: 0., delta_y: NUDGE_AMOUNT }, key_down=KeyArrowDown},
			entry! {action=DocumentMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT }, key_down=KeyArrowLeft, modifiers=[KeyArrowUp]},
			entry! {action=DocumentMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT }, key_down=KeyArrowLeft, modifiers=[KeyArrowDown]},
			entry! {action=DocumentMessage::NudgeSelectedLayers { delta_x: -NUDGE_AMOUNT, delta_y: 0. }, key_down=KeyArrowLeft},
			entry! {action=DocumentMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: -NUDGE_AMOUNT }, key_down=KeyArrowRight, modifiers=[KeyArrowUp]},
			entry! {action=DocumentMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: NUDGE_AMOUNT }, key_down=KeyArrowRight, modifiers=[KeyArrowDown]},
			entry! {action=DocumentMessage::NudgeSelectedLayers { delta_x: NUDGE_AMOUNT, delta_y: 0. }, key_down=KeyArrowRight},
			// Reorder Layers
			entry! {action=DocumentMessage::ReorderSelectedLayers { relative_index_offset: isize::MAX }, key_down=KeyRightCurlyBracket, modifiers=[KeyControl]}, // TODO: Use KeyRightBracket with ctrl+shift modifiers once input system is fixed
			entry! {action=DocumentMessage::ReorderSelectedLayers { relative_index_offset: 1 }, key_down=KeyRightBracket, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::ReorderSelectedLayers { relative_index_offset: -1 }, key_down=KeyLeftBracket, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::ReorderSelectedLayers { relative_index_offset: isize::MIN }, key_down=KeyLeftCurlyBracket, modifiers=[KeyControl]}, // TODO: Use KeyLeftBracket with ctrl+shift modifiers once input system is fixed
			// Global Actions
			entry! {action=GlobalMessage::LogInfo, key_down=Key1},
			entry! {action=GlobalMessage::LogDebug, key_down=Key2},
			entry! {action=GlobalMessage::LogTrace, key_down=Key3},
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
	pub fn match_message(&self, message: InputMapperMessage, keys: &KeyStates, actions: ActionList) -> Option<Message> {
		use InputMapperMessage::*;

		let list = match message {
			KeyDown(key) => &self.key_down[key as usize],
			KeyUp(key) => &self.key_up[key as usize],
			DoubleClick => &self.double_click,
			MouseScroll => &self.mouse_scroll,
			PointerMove => &self.pointer_move,
		};
		list.match_mapping(keys, actions)
	}
}

#[derive(PartialEq, Clone, Debug)]
pub struct MappingEntry {
	pub trigger: InputMapperMessage,
	pub modifiers: KeyStates,
	pub action: Message,
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

	macro_rules! entry {
		{action=$action:expr, key_down=$key:ident $(, modifiers=[$($m:ident),* $(,)?])?} => {{
			entry!{action=$action, message=InputMapperMessage::KeyDown(Key::$key) $(, modifiers=[$($m),*])?}
		}};
		{action=$action:expr, key_up=$key:ident $(, modifiers=[$($m:ident),* $(,)?])?} => {{
			entry!{action=$action, message=InputMapperMessage::KeyUp(Key::$key) $(, modifiers=[$($m),* ])?}
		}};
		{action=$action:expr, message=$message:expr $(, modifiers=[$($m:ident),* $(,)?])?} => {{
			&[MappingEntry {trigger: $message, modifiers: modifiers!($($($m),*)?), action: $action.into()}]
		}};
		{action=$action:expr, triggers=[$($m:ident),* $(,)?]} => {{
			&[
				MappingEntry {trigger:InputMapperMessage::PointerMove, action: $action.into(), modifiers: modifiers!()},
				$(
				MappingEntry {trigger:InputMapperMessage::KeyDown(Key::$m), action: $action.into(), modifiers: modifiers!()},
				MappingEntry {trigger:InputMapperMessage::KeyUp(Key::$m), action: $action.into(), modifiers: modifiers!()},
				)*
			]
		}};
	}

	macro_rules! mapping {
		//[$(<action=$action:expr; message=$key:expr; $(modifiers=[$($m:ident),* $(,)?];)?>)*] => {{
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
	pub(crate) use mapping;
	pub(crate) use modifiers;
}
