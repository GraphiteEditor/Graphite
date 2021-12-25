use glam::DVec2;

use super::{
	keyboard::{Key, KeyStates, NUMBER_OF_KEYS},
	InputPreprocessor,
};
use crate::message_prelude::*;
use crate::tool::ToolType;

use serde::{Deserialize, Serialize};
use std::fmt::Write;

const NUDGE_AMOUNT: f64 = 1.;
const SHIFT_NUDGE_AMOUNT: f64 = 10.;

#[impl_message(Message, InputMapper)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum InputMapperMessage {
	PointerMove,
	MouseScroll,
	#[child]
	KeyUp(Key),
	#[child]
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
	key_up: [KeyMappingEntries; NUMBER_OF_KEYS],
	key_down: [KeyMappingEntries; NUMBER_OF_KEYS],
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
		$(
			for entry in $entry {
				let arr = match entry.trigger {
					InputMapperMessage::KeyDown(key) => &mut key_down[key as usize],
					InputMapperMessage::KeyUp(key) => &mut key_up[key as usize],
					InputMapperMessage::PointerMove => &mut pointer_move,
					InputMapperMessage::MouseScroll => &mut mouse_scroll,
				};
				arr.push(entry.clone());
			}
		)*
		(key_up, key_down, pointer_move, mouse_scroll)
	}};
}

impl Default for Mapping {
	fn default() -> Self {
		use Key::*;
		// WARNING!
		// If a new mapping isn't being handled (and perhaps another lower-precedence one is instead), make sure to advertise
		// it as an available action in the respective message handler file (such as the bottom of `document_message_handler.rs`)
		let mappings = mapping![
			// Higher priority than entries in sections below
			entry! {action=DocumentsMessage::Paste, key_down=KeyV, modifiers=[KeyControl]},
			entry! {action=MovementMessage::EnableSnapping, key_down=KeyShift},
			entry! {action=MovementMessage::DisableSnapping, key_up=KeyShift},
			// Transform layers
			entry! {action=TransformLayerMessage::ApplyOperation, key_down=KeyEnter},
			entry! {action=TransformLayerMessage::ApplyOperation, key_down=Lmb},
			entry! {action=TransformLayerMessage::CancelOperation, key_down=KeyEscape},
			entry! {action=TransformLayerMessage::CancelOperation, key_down=Rmb},
			entry! {action=TransformLayerMessage::ConstrainX, key_down=KeyX},
			entry! {action=TransformLayerMessage::ConstrainY, key_down=KeyY},
			entry! {action=TransformLayerMessage::TypeBackspace, key_down=KeyBackspace},
			entry! {action=TransformLayerMessage::TypeNegate, key_down=KeyMinus},
			entry! {action=TransformLayerMessage::TypeDecimalPoint, key_down=KeyComma},
			entry! {action=TransformLayerMessage::TypeDecimalPoint, key_down=KeyPeriod},
			entry! {action=TransformLayerMessage::MouseMove{slow_key: KeyShift, snap_key: KeyControl}, triggers=[KeyShift, KeyControl]},
			// Select
			entry! {action=SelectMessage::MouseMove, message=InputMapperMessage::PointerMove},
			entry! {action=SelectMessage::DragStart{add_to_selection: KeyShift}, key_down=Lmb},
			entry! {action=SelectMessage::DragStop, key_up=Lmb},
			entry! {action=SelectMessage::Abort, key_down=Rmb},
			entry! {action=SelectMessage::Abort, key_down=KeyEscape},
			// Eyedropper
			entry! {action=EyedropperMessage::LeftMouseDown, key_down=Lmb},
			entry! {action=EyedropperMessage::RightMouseDown, key_down=Rmb},
			// Rectangle
			entry! {action=RectangleMessage::DragStart, key_down=Lmb},
			entry! {action=RectangleMessage::DragStop, key_up=Lmb},
			entry! {action=RectangleMessage::Abort, key_down=Rmb},
			entry! {action=RectangleMessage::Abort, key_down=KeyEscape},
			entry! {action=RectangleMessage::Resize{center: KeyAlt, lock_ratio: KeyShift}, triggers=[KeyAlt, KeyShift]},
			// Ellipse
			entry! {action=EllipseMessage::DragStart, key_down=Lmb},
			entry! {action=EllipseMessage::DragStop, key_up=Lmb},
			entry! {action=EllipseMessage::Abort, key_down=Rmb},
			entry! {action=EllipseMessage::Abort, key_down=KeyEscape},
			entry! {action=EllipseMessage::Resize{center: KeyAlt, lock_ratio: KeyShift}, triggers=[KeyAlt, KeyShift]},
			// Shape
			entry! {action=ShapeMessage::DragStart, key_down=Lmb},
			entry! {action=ShapeMessage::DragStop, key_up=Lmb},
			entry! {action=ShapeMessage::Abort, key_down=Rmb},
			entry! {action=ShapeMessage::Abort, key_down=KeyEscape},
			entry! {action=ShapeMessage::Resize{center: KeyAlt, lock_ratio: KeyShift}, triggers=[KeyAlt, KeyShift]},
			// Line
			entry! {action=LineMessage::DragStart, key_down=Lmb},
			entry! {action=LineMessage::DragStop, key_up=Lmb},
			entry! {action=LineMessage::Abort, key_down=Rmb},
			entry! {action=LineMessage::Abort, key_down=KeyEscape},
			entry! {action=LineMessage::Redraw{center: KeyAlt, lock_angle: KeyControl, snap_angle: KeyShift}, triggers=[KeyAlt, KeyShift, KeyControl]},
			// Path
			entry! {action=PathMessage::MouseDown, key_down=Lmb},
			// Pen
			entry! {action=PenMessage::PointerMove, message=InputMapperMessage::PointerMove},
			entry! {action=PenMessage::DragStart, key_down=Lmb},
			entry! {action=PenMessage::DragStop, key_up=Lmb},
			entry! {action=PenMessage::Confirm, key_down=Rmb},
			entry! {action=PenMessage::Confirm, key_down=KeyEscape},
			entry! {action=PenMessage::Confirm, key_down=KeyEnter},
			// Fill
			entry! {action=FillMessage::LeftMouseDown, key_down=Lmb},
			entry! {action=FillMessage::RightMouseDown, key_down=Rmb},
			// Tool Actions
			entry! {action=ToolMessage::ActivateTool(ToolType::Select), key_down=KeyV},
			entry! {action=ToolMessage::ActivateTool(ToolType::Eyedropper), key_down=KeyI},
			entry! {action=ToolMessage::ActivateTool(ToolType::Fill), key_down=KeyF},
			entry! {action=ToolMessage::ActivateTool(ToolType::Path), key_down=KeyA},
			entry! {action=ToolMessage::ActivateTool(ToolType::Pen), key_down=KeyP},
			entry! {action=ToolMessage::ActivateTool(ToolType::Line), key_down=KeyL},
			entry! {action=ToolMessage::ActivateTool(ToolType::Rectangle), key_down=KeyM},
			entry! {action=ToolMessage::ActivateTool(ToolType::Ellipse), key_down=KeyE},
			entry! {action=ToolMessage::ActivateTool(ToolType::Shape), key_down=KeyY},
			// Colors
			entry! {action=ToolMessage::ResetColors, key_down=KeyX, modifiers=[KeyShift, KeyControl]},
			entry! {action=ToolMessage::SwapColors, key_down=KeyX, modifiers=[KeyShift]},
			// Editor Actions
			entry! {action=FrontendMessage::OpenDocumentBrowse, key_down=KeyO, modifiers=[KeyControl]},
			// Document Actions
			entry! {action=DocumentsMessage::Paste, key_down=KeyV, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::Redo, key_down=KeyZ, modifiers=[KeyControl, KeyShift]},
			entry! {action=DocumentMessage::Undo, key_down=KeyZ, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::DeselectAllLayers, key_down=KeyA, modifiers=[KeyControl, KeyAlt]},
			entry! {action=DocumentMessage::SelectAllLayers, key_down=KeyA, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::CreateEmptyFolder(vec![]), key_down=KeyN, modifiers=[KeyControl, KeyShift]},
			entry! {action=DocumentMessage::DeleteSelectedLayers, key_down=KeyDelete},
			entry! {action=DocumentMessage::DeleteSelectedLayers, key_down=KeyX},
			entry! {action=DocumentMessage::DeleteSelectedLayers, key_down=KeyBackspace},
			entry! {action=DocumentMessage::ExportDocument, key_down=KeyE, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::SaveDocument, key_down=KeyS, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::SaveDocument, key_down=KeyS, modifiers=[KeyControl, KeyShift]},
			// Initiate Transform Layers
			entry! {action=TransformLayerMessage::BeginGrab, key_down=KeyG},
			entry! {action=TransformLayerMessage::BeginRotate, key_down=KeyR},
			entry! {action=TransformLayerMessage::BeginScale, key_down=KeyS},
			// Document movement
			entry! {action=MovementMessage::MouseMove, message=InputMapperMessage::PointerMove},
			entry! {action=MovementMessage::RotateCanvasBegin{snap:false}, key_down=Mmb, modifiers=[KeyControl]},
			entry! {action=MovementMessage::RotateCanvasBegin{snap:true}, key_down=Mmb, modifiers=[KeyControl, KeyShift]},
			entry! {action=MovementMessage::ZoomCanvasBegin, key_down=Mmb, modifiers=[KeyShift]},
			entry! {action=MovementMessage::ZoomCanvasToFitAll, key_down=Key0, modifiers=[KeyControl]},
			entry! {action=MovementMessage::TranslateCanvasBegin, key_down=Mmb},
			entry! {action=MovementMessage::TransformCanvasEnd, key_up=Mmb},
			entry! {action=MovementMessage::TranslateCanvasBegin, key_down=Lmb, modifiers=[KeySpace]},
			entry! {action=MovementMessage::TransformCanvasEnd, key_up=Lmb, modifiers=[KeySpace]},
			entry! {action=MovementMessage::IncreaseCanvasZoom, key_down=KeyPlus, modifiers=[KeyControl]},
			entry! {action=MovementMessage::IncreaseCanvasZoom, key_down=KeyEquals, modifiers=[KeyControl]},
			entry! {action=MovementMessage::DecreaseCanvasZoom, key_down=KeyMinus, modifiers=[KeyControl]},
			entry! {action=MovementMessage::SetCanvasZoom(1.), key_down=Key1, modifiers=[KeyControl]},
			entry! {action=MovementMessage::SetCanvasZoom(2.), key_down=Key2, modifiers=[KeyControl]},
			entry! {action=MovementMessage::WheelCanvasZoom, message=InputMapperMessage::MouseScroll, modifiers=[KeyControl]},
			entry! {action=MovementMessage::WheelCanvasTranslate{use_y_as_x: true}, message=InputMapperMessage::MouseScroll, modifiers=[KeyShift]},
			entry! {action=MovementMessage::WheelCanvasTranslate{use_y_as_x: false}, message=InputMapperMessage::MouseScroll},
			entry! {action=MovementMessage::TranslateCanvasByViewportFraction(DVec2::new(1., 0.)), key_down=KeyPageUp, modifiers=[KeyShift]},
			entry! {action=MovementMessage::TranslateCanvasByViewportFraction(DVec2::new(-1., 0.)), key_down=KeyPageDown, modifiers=[KeyShift]},
			entry! {action=MovementMessage::TranslateCanvasByViewportFraction(DVec2::new(0., 1.)), key_down=KeyPageUp},
			entry! {action=MovementMessage::TranslateCanvasByViewportFraction(DVec2::new(0., -1.)), key_down=KeyPageDown},
			// Document actions
			entry! {action=DocumentsMessage::NewDocument, key_down=KeyN, modifiers=[KeyControl]},
			entry! {action=DocumentsMessage::NextDocument, key_down=KeyTab, modifiers=[KeyControl]},
			entry! {action=DocumentsMessage::PrevDocument, key_down=KeyTab, modifiers=[KeyControl, KeyShift]},
			entry! {action=DocumentsMessage::CloseAllDocumentsWithConfirmation, key_down=KeyW, modifiers=[KeyControl, KeyAlt]},
			entry! {action=DocumentsMessage::CloseActiveDocumentWithConfirmation, key_down=KeyW, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::DuplicateSelectedLayers, key_down=KeyD, modifiers=[KeyControl]},
			entry! {action=DocumentsMessage::Copy, key_down=KeyC, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::GroupSelectedLayers, key_down=KeyG, modifiers=[KeyControl]},
			// Nudging
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
			// Reorder Layers
			entry! {action=DocumentMessage::ReorderSelectedLayers(i32::MAX), key_down=KeyRightCurlyBracket, modifiers=[KeyControl]}, // TODO: Use KeyRightBracket with ctrl+shift modifiers once input system is fixed
			entry! {action=DocumentMessage::ReorderSelectedLayers(1), key_down=KeyRightBracket, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::ReorderSelectedLayers(-1), key_down=KeyLeftBracket, modifiers=[KeyControl]},
			entry! {action=DocumentMessage::ReorderSelectedLayers(i32::MIN), key_down=KeyLeftCurlyBracket, modifiers=[KeyControl]}, // TODO: Use KeyLeftBracket with ctrl+shift modifiers once input system is fixed
			// Global Actions
			entry! {action=GlobalMessage::LogInfo, key_down=Key1},
			entry! {action=GlobalMessage::LogDebug, key_down=Key2},
			entry! {action=GlobalMessage::LogTrace, key_down=Key3},
		];

		let (mut key_up, mut key_down, mut pointer_move, mut mouse_scroll) = mappings;
		const NUMBER_KEYS: [Key; 10] = [Key0, Key1, Key2, Key3, Key4, Key5, Key6, Key7, Key8, Key9];
		for (i, key) in NUMBER_KEYS.iter().enumerate() {
			key_down[*key as usize].0.insert(
				0,
				MappingEntry {
					trigger: InputMapperMessage::KeyDown(*key),
					modifiers: modifiers! {},
					action: TransformLayerMessage::TypeNumber(i as u8).into(),
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
		Self {
			key_up,
			key_down,
			pointer_move,
			mouse_scroll,
		}
	}
}

impl Mapping {
	fn match_message(&self, message: InputMapperMessage, keys: &KeyStates, actions: ActionList) -> Option<Message> {
		use InputMapperMessage::*;
		let list = match message {
			KeyDown(key) => &self.key_down[key as usize],
			KeyUp(key) => &self.key_up[key as usize],
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

impl InputMapper {
	pub fn hints(&self, actions: ActionList) -> String {
		let mut output = String::new();
		let mut actions = actions
			.into_iter()
			.flatten()
			.filter(|a| !matches!(*a, MessageDiscriminant::Tool(ToolMessageDiscriminant::ActivateTool) | MessageDiscriminant::Global(_)));
		self.mapping
			.key_down
			.iter()
			.enumerate()
			.filter_map(|(i, m)| {
				let ma = m.0.iter().find_map(|m| actions.find_map(|a| (a == m.action.to_discriminant()).then(|| m.action.to_discriminant())));

				ma.map(|a| unsafe { (std::mem::transmute_copy::<usize, Key>(&i), a) })
			})
			.for_each(|(k, a)| {
				let _ = write!(output, "{}: {}, ", k.to_discriminant().local_name(), a.local_name().split('.').last().unwrap());
			});
		output.replace("Key", "")
	}
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
