use crate::consts::{MINUS_KEY_ZOOM_RATE, PLUS_KEY_ZOOM_RATE};
use crate::message_prelude::*;
use crate::tool::ToolType;

use super::{
	keyboard::{Key, KeyStates, NUMBER_OF_KEYS},
	InputPreprocessor,
};

const NUDGE_AMOUNT: f64 = 1.;
const SHIFT_NUDGE_AMOUNT: f64 = 10.;

#[impl_message(Message, InputMapper)]
#[derive(PartialEq, Clone, Debug)]
pub enum InputMapperMessage {
	PointerMove,
	MouseScroll,
	KeyUp(Key),
	KeyDown(Key),
}

#[derive(PartialEq, Clone, Debug)]
struct MappingEntry {
	trigger: InputMapperMessage,
	modifiers: KeyStates,
	action: Message,
}

#[derive(Debug, Clone)]
struct KeyMappingEntries(Vec<MappingEntry>);

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

#[derive(Debug, Clone)]
struct Mapping {
	up: [KeyMappingEntries; NUMBER_OF_KEYS],
	down: [KeyMappingEntries; NUMBER_OF_KEYS],
	pointer_move: KeyMappingEntries,
	mouse_scroll: KeyMappingEntries,
}

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
		MappingEntry {trigger: $message, modifiers: modifiers!($($($m),*)?), action: $action.into()}
	}};
}
macro_rules! mapping {
	//[$(<action=$action:expr; message=$key:expr; $(modifiers=[$($m:ident),* $(,)?];)?>)*] => {{
	[$($entry:expr),* $(,)?] => {{
		let mut up =  KeyMappingEntries::key_array();
		let mut down = KeyMappingEntries::key_array();
		let mut pointer_move: KeyMappingEntries = Default::default();
		let mut mouse_scroll: KeyMappingEntries = Default::default();
		$(
			let arr = match $entry.trigger {
				InputMapperMessage::KeyDown(key) => &mut down[key as usize],
				InputMapperMessage::KeyUp(key) => &mut up[key as usize],
				InputMapperMessage::PointerMove => &mut pointer_move,
				InputMapperMessage::MouseScroll => &mut mouse_scroll,
			};
			arr.push($entry);
		)*
		(up, down, pointer_move, mouse_scroll)
	}};
}

impl Default for Mapping {
	fn default() -> Self {
		let mappings = mapping![
			entry! {action=DocumentMessage::PasteLayers, key_down=KeyV, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::EnableSnapping, key_down=KeyShift},
			entry! {action=DocumentMessage::DisableSnapping, key_up=KeyShift},
			// Select
			entry! {action=SelectMessage::MouseMove, message=InputMapperMessage::PointerMove},
			entry! {action=SelectMessage::DragStart, key_down=Lmb},
			entry! {action=SelectMessage::DragStop, key_up=Lmb},
			entry! {action=SelectMessage::Abort, key_down=Rmb},
			entry! {action=SelectMessage::Abort, key_down=KeyEscape},
			// Rectangle
			entry! {action=RectangleMessage::Center, key_down=KeyAlt},
			entry! {action=RectangleMessage::UnCenter, key_up=KeyAlt},
			entry! {action=RectangleMessage::MouseMove, message=InputMapperMessage::PointerMove},
			entry! {action=RectangleMessage::DragStart, key_down=Lmb},
			entry! {action=RectangleMessage::DragStop, key_up=Lmb},
			entry! {action=RectangleMessage::Abort, key_down=Rmb},
			entry! {action=RectangleMessage::Abort, key_down=KeyEscape},
			entry! {action=RectangleMessage::LockAspectRatio, key_down=KeyShift},
			entry! {action=RectangleMessage::UnlockAspectRatio, key_up=KeyShift},
			// Ellipse
			entry! {action=EllipseMessage::Center, key_down=KeyAlt},
			entry! {action=EllipseMessage::UnCenter, key_up=KeyAlt},
			entry! {action=EllipseMessage::MouseMove, message=InputMapperMessage::PointerMove},
			entry! {action=EllipseMessage::DragStart, key_down=Lmb},
			entry! {action=EllipseMessage::DragStop, key_up=Lmb},
			entry! {action=EllipseMessage::Abort, key_down=Rmb},
			entry! {action=EllipseMessage::Abort, key_down=KeyEscape},
			entry! {action=EllipseMessage::LockAspectRatio, key_down=KeyShift},
			entry! {action=EllipseMessage::UnlockAspectRatio, key_up=KeyShift},
			// Shape
			entry! {action=ShapeMessage::Center, key_down=KeyAlt},
			entry! {action=ShapeMessage::UnCenter, key_up=KeyAlt},
			entry! {action=ShapeMessage::MouseMove, message=InputMapperMessage::PointerMove},
			entry! {action=ShapeMessage::DragStart, key_down=Lmb},
			entry! {action=ShapeMessage::DragStop, key_up=Lmb},
			entry! {action=ShapeMessage::Abort, key_down=Rmb},
			entry! {action=ShapeMessage::Abort, key_down=KeyEscape},
			entry! {action=ShapeMessage::LockAspectRatio, key_down=KeyShift},
			entry! {action=ShapeMessage::UnlockAspectRatio, key_up=KeyShift},
			// Line
			entry! {action=LineMessage::Center, key_down=KeyAlt},
			entry! {action=LineMessage::UnCenter, key_up=KeyAlt},
			entry! {action=LineMessage::MouseMove, message=InputMapperMessage::PointerMove},
			entry! {action=LineMessage::DragStart, key_down=Lmb},
			entry! {action=LineMessage::DragStop, key_up=Lmb},
			entry! {action=LineMessage::Abort, key_down=Rmb},
			entry! {action=LineMessage::Abort, key_down=KeyEscape},
			entry! {action=LineMessage::LockAngle, key_down=KeyControl},
			entry! {action=LineMessage::UnlockAngle, key_up=KeyControl},
			entry! {action=LineMessage::SnapToAngle, key_down=KeyShift},
			entry! {action=LineMessage::UnSnapToAngle, key_up=KeyShift},
			// Pen
			entry! {action=PenMessage::MouseMove, message=InputMapperMessage::PointerMove},
			entry! {action=PenMessage::DragStart, key_down=Lmb},
			entry! {action=PenMessage::DragStop, key_up=Lmb},
			entry! {action=PenMessage::Confirm, key_down=Rmb},
			entry! {action=PenMessage::Confirm, key_down=KeyEscape},
			entry! {action=PenMessage::Confirm, key_down=KeyEnter},
			// Fill
			entry! {action=FillMessage::MouseDown, key_down=Lmb},
			// Tool Actions
			entry! {action=ToolMessage::SelectTool(ToolType::Fill), key_down=KeyF},
			entry! {action=ToolMessage::SelectTool(ToolType::Rectangle), key_down=KeyM},
			entry! {action=ToolMessage::SelectTool(ToolType::Ellipse), key_down=KeyE},
			entry! {action=ToolMessage::SelectTool(ToolType::Select), key_down=KeyV},
			entry! {action=ToolMessage::SelectTool(ToolType::Line), key_down=KeyL},
			entry! {action=ToolMessage::SelectTool(ToolType::Pen), key_down=KeyP},
			entry! {action=ToolMessage::SelectTool(ToolType::Shape), key_down=KeyY},
			entry! {action=ToolMessage::ResetColors, key_down=KeyX, modifiers=[KeyShift, KeyControl]},
			entry! {action=ToolMessage::SwapColors, key_down=KeyX, modifiers=[KeyShift]},
			// Document Actions
			entry! {action=DocumentMessage::Undo, key_down=KeyZ, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::DeselectAllLayers, key_down=KeyA, modifiers=[KeyControl, KeyAlt]},
			entry! {action=DocumentMessage::SelectAllLayers, key_down=KeyA, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::DeleteSelectedLayers, key_down=KeyDelete},
			entry! {action=DocumentMessage::DeleteSelectedLayers, key_down=KeyX},
			entry! {action=DocumentMessage::DeleteSelectedLayers, key_down=KeyBackspace},
			entry! {action=DocumentMessage::ExportDocument, key_down=KeyS, modifiers=[KeyControl, KeyShift]},
			entry! {action=DocumentMessage::ExportDocument, key_down=KeyE, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::MouseMove, message=InputMapperMessage::PointerMove},
			entry! {action=DocumentMessage::RotateCanvasBegin{snap:false}, key_down=Mmb, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::RotateCanvasBegin{snap:true}, key_down=Mmb, modifiers=[KeyControl, KeyShift]},
			entry! {action=DocumentMessage::ZoomCanvasBegin, key_down=Mmb, modifiers=[KeyShift]},
			entry! {action=DocumentMessage::TranslateCanvasBegin, key_down=Mmb},
			entry! {action=DocumentMessage::TranslateCanvasEnd, key_up=Mmb},
			entry! {action=DocumentMessage::MultiplyCanvasZoom(PLUS_KEY_ZOOM_RATE), key_down=KeyPlus, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::MultiplyCanvasZoom(PLUS_KEY_ZOOM_RATE), key_down=KeyEquals, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::MultiplyCanvasZoom(MINUS_KEY_ZOOM_RATE), key_down=KeyMinus, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::SetCanvasZoom(1.), key_down=Key1, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::SetCanvasZoom(2.), key_down=Key2, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::WheelCanvasZoom, message=InputMapperMessage::MouseScroll, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::WheelCanvasTranslate{use_y_as_x: true}, message=InputMapperMessage::MouseScroll, modifiers=[KeyShift]},
			entry! {action=DocumentMessage::WheelCanvasTranslate{use_y_as_x: false}, message=InputMapperMessage::MouseScroll},
			entry! {action=DocumentMessage::NewDocument, key_down=KeyN, modifiers=[KeyShift]},
			entry! {action=DocumentMessage::NextDocument, key_down=KeyTab, modifiers=[KeyShift]},
			entry! {action=DocumentMessage::CloseActiveDocument, key_down=KeyW, modifiers=[KeyShift]},
			entry! {action=DocumentMessage::DuplicateSelectedLayers, key_down=KeyD, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::CopySelectedLayers, key_down=KeyC, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::NudgeSelectedLayers(-SHIFT_NUDGE_AMOUNT, -SHIFT_NUDGE_AMOUNT), key_down=KeyArrowUp, modifiers=[KeyShift, KeyArrowLeft]},
			entry! {action=DocumentMessage::NudgeSelectedLayers(SHIFT_NUDGE_AMOUNT, -SHIFT_NUDGE_AMOUNT), key_down=KeyArrowUp, modifiers=[KeyShift, KeyArrowRight]},
			entry! {action=DocumentMessage::NudgeSelectedLayers(0., -SHIFT_NUDGE_AMOUNT), key_down=KeyArrowUp, modifiers=[KeyShift]},
			entry! {action=DocumentMessage::NudgeSelectedLayers(-SHIFT_NUDGE_AMOUNT, SHIFT_NUDGE_AMOUNT), key_down=KeyArrowDown, modifiers=[KeyShift, KeyArrowLeft]},
			entry! {action=DocumentMessage::NudgeSelectedLayers(SHIFT_NUDGE_AMOUNT, SHIFT_NUDGE_AMOUNT), key_down=KeyArrowDown, modifiers=[KeyShift, KeyArrowRight]},
			entry! {action=DocumentMessage::NudgeSelectedLayers(0., SHIFT_NUDGE_AMOUNT), key_down=KeyArrowDown, modifiers=[KeyShift]},
			entry! {action=DocumentMessage::NudgeSelectedLayers(-SHIFT_NUDGE_AMOUNT, -SHIFT_NUDGE_AMOUNT), key_down=KeyArrowLeft, modifiers=[KeyShift, KeyArrowUp]},
			entry! {action=DocumentMessage::NudgeSelectedLayers(-SHIFT_NUDGE_AMOUNT, SHIFT_NUDGE_AMOUNT), key_down=KeyArrowLeft, modifiers=[KeyShift, KeyArrowDown]},
			entry! {action=DocumentMessage::NudgeSelectedLayers(-SHIFT_NUDGE_AMOUNT, 0.), key_down=KeyArrowLeft, modifiers=[KeyShift]},
			entry! {action=DocumentMessage::NudgeSelectedLayers(SHIFT_NUDGE_AMOUNT, -SHIFT_NUDGE_AMOUNT), key_down=KeyArrowRight, modifiers=[KeyShift, KeyArrowUp]},
			entry! {action=DocumentMessage::NudgeSelectedLayers(SHIFT_NUDGE_AMOUNT, SHIFT_NUDGE_AMOUNT), key_down=KeyArrowRight, modifiers=[KeyShift, KeyArrowDown]},
			entry! {action=DocumentMessage::NudgeSelectedLayers(SHIFT_NUDGE_AMOUNT, 0.), key_down=KeyArrowRight, modifiers=[KeyShift]},
			entry! {action=DocumentMessage::NudgeSelectedLayers(-NUDGE_AMOUNT, -NUDGE_AMOUNT), key_down=KeyArrowUp, modifiers=[KeyArrowLeft]},
			entry! {action=DocumentMessage::NudgeSelectedLayers(NUDGE_AMOUNT, -NUDGE_AMOUNT), key_down=KeyArrowUp, modifiers=[KeyArrowRight]},
			entry! {action=DocumentMessage::NudgeSelectedLayers(0., -NUDGE_AMOUNT), key_down=KeyArrowUp},
			entry! {action=DocumentMessage::NudgeSelectedLayers(-NUDGE_AMOUNT, NUDGE_AMOUNT), key_down=KeyArrowDown, modifiers=[KeyArrowLeft]},
			entry! {action=DocumentMessage::NudgeSelectedLayers(NUDGE_AMOUNT, NUDGE_AMOUNT), key_down=KeyArrowDown, modifiers=[KeyArrowRight]},
			entry! {action=DocumentMessage::NudgeSelectedLayers(0., NUDGE_AMOUNT), key_down=KeyArrowDown},
			entry! {action=DocumentMessage::NudgeSelectedLayers(-NUDGE_AMOUNT, -NUDGE_AMOUNT), key_down=KeyArrowLeft, modifiers=[KeyArrowUp]},
			entry! {action=DocumentMessage::NudgeSelectedLayers(-NUDGE_AMOUNT, NUDGE_AMOUNT), key_down=KeyArrowLeft, modifiers=[KeyArrowDown]},
			entry! {action=DocumentMessage::NudgeSelectedLayers(-NUDGE_AMOUNT, 0.), key_down=KeyArrowLeft},
			entry! {action=DocumentMessage::NudgeSelectedLayers(NUDGE_AMOUNT, -NUDGE_AMOUNT), key_down=KeyArrowRight, modifiers=[KeyArrowUp]},
			entry! {action=DocumentMessage::NudgeSelectedLayers(NUDGE_AMOUNT, NUDGE_AMOUNT), key_down=KeyArrowRight, modifiers=[KeyArrowDown]},
			entry! {action=DocumentMessage::NudgeSelectedLayers(NUDGE_AMOUNT, 0.), key_down=KeyArrowRight},
			// Global Actions
			entry! {action=GlobalMessage::LogInfo, key_down=Key1},
			entry! {action=GlobalMessage::LogDebug, key_down=Key2},
			entry! {action=GlobalMessage::LogTrace, key_down=Key3},
		];

		let (mut up, mut down, mut pointer_move, mut mouse_scroll) = mappings;
		let sort = |list: &mut KeyMappingEntries| list.0.sort_by(|u, v| v.modifiers.ones().cmp(&u.modifiers.ones()));
		for list in [&mut up, &mut down] {
			for sublist in list {
				sort(sublist);
			}
		}
		sort(&mut pointer_move);
		sort(&mut mouse_scroll);
		Self { up, down, pointer_move, mouse_scroll }
	}
}

impl Mapping {
	fn match_message(&self, message: InputMapperMessage, keys: &KeyStates, actions: ActionList) -> Option<Message> {
		use InputMapperMessage::*;
		let list = match message {
			KeyDown(key) => &self.down[key as usize],
			KeyUp(key) => &self.up[key as usize],
			PointerMove => &self.pointer_move,
			MouseScroll => &self.mouse_scroll,
		};
		list.match_mapping(keys, actions)
	}
}

#[derive(Debug, Default)]
pub struct InputMapper {
	mapping: Mapping,
}

impl MessageHandler<InputMapperMessage, (&InputPreprocessor, ActionList)> for InputMapper {
	fn process_action(&mut self, message: InputMapperMessage, data: (&InputPreprocessor, ActionList), responses: &mut VecDeque<Message>) {
		let (input, actions) = data;
		if let Some(message) = self.mapping.match_message(message, &input.keyboard, actions) {
			responses.push_back(message);
		}
	}
	advertise_actions!();
}
