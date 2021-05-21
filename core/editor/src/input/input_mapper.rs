use crate::message_prelude::*;

use super::{
	keyboard::{Key, Keyboard, NUMBER_OF_KEYS},
	InputPreprocessor,
};

#[impl_message(Message, InputMapper)]
#[derive(PartialEq, Clone, Debug)]
pub enum InputMapperMessage {
	MouseMove,
	KeyUp(Key),
	KeyDown(Key),
}

#[derive(PartialEq, Clone, Debug)]
struct MappingEntry {
	modifiers: Keyboard,
	action: Message,
	cause: InputMapperMessage,
}

#[derive(Debug, Clone, Default)]
struct MappingList(Vec<MappingEntry>);

impl MappingList {
	fn match_mapping(&self, keys: &Keyboard, actions: ActionList) -> Option<Message> {
		for entry in self.0.iter() {
			if (*keys & entry.modifiers ^ *keys).is_empty() && actions.iter().flatten().any(|action| entry.action.to_discriminant() == *action) {
				return Some(entry.action.clone());
			}
		}
		None
	}
	fn push(&mut self, entry: MappingEntry) {
		self.0.push(entry)
	}
}

#[derive(Debug, Clone)]
struct Mappings {
	up: [MappingList; NUMBER_OF_KEYS],
	down: [MappingList; NUMBER_OF_KEYS],
	mouse_move: MappingList,
}

macro_rules! modifiers {
	($($m:ident),*) => {{
		#[allow(unused_mut)]
		let mut state = Keyboard::new();
		$(
			state.set(Key::$m as usize);
		),*
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
		MappingEntry {cause: $message, modifiers: modifiers!($($($m),*)?), action: $action.into()}
	}};
}
macro_rules! mapping {
	//[$(<action=$action:expr; message=$key:expr; $(modifiers=[$($m:ident),* $(,)?];)?>)*] => {{
	[$($entry:expr),* $(,)?] => {{
		let mut up: [MappingList; NUMBER_OF_KEYS] = Default::default();
		let mut down: [MappingList; NUMBER_OF_KEYS] = Default::default();
		let mut mouse_move: MappingList = Default::default();
		$(
			let arr = match $entry.cause {
				InputMapperMessage::KeyDown(key) => &mut down[key as usize],
				InputMapperMessage::KeyUp(key) => &mut up[key as usize],
				InputMapperMessage::MouseMove => &mut mouse_move,
			};
			arr.push($entry);
        )*
		(up, down, mouse_move)
	}};
}

impl Default for Mappings {
	fn default() -> Self {
		let (up, down, mouse_move) = mapping![
			// Rectangle
			entry! {action=RectangleMessage::Center, key_down=KeyAlt},
			entry! {action=RectangleMessage::UnCenter, key_up=KeyAlt},
			entry! {action=RectangleMessage::MouseMove, message=InputMapperMessage::MouseMove},
			entry! {action=RectangleMessage::DragStart, key_down=Lmb},
			entry! {action=RectangleMessage::DragStop, key_up=Lmb},
			entry! {action=RectangleMessage::Abort, key_down=Rmb},
			entry! {action=RectangleMessage::Abort, key_down=KeyEscape},
			entry! {action=RectangleMessage::LockAspectRatio, key_down=KeyAlt},
			entry! {action=RectangleMessage::UnlockAspectRatio, key_up=KeyAlt},
			// Ellipse
			entry! {action=EllipseMessage::Center, key_down=KeyAlt},
			entry! {action=EllipseMessage::UnCenter, key_up=KeyAlt},
			entry! {action=EllipseMessage::MouseMove, message=InputMapperMessage::MouseMove},
			entry! {action=EllipseMessage::DragStart, key_down=Lmb},
			entry! {action=EllipseMessage::DragStop, key_up=Lmb},
			entry! {action=EllipseMessage::Abort, key_down=Rmb},
			entry! {action=EllipseMessage::Abort, key_down=KeyEscape},
			entry! {action=EllipseMessage::LockAspectRatio, key_down=KeyAlt},
			entry! {action=EllipseMessage::UnlockAspectRatio, key_up=KeyAlt},
			// Document Actions
			entry! {action=DocumentMessage::Undo, key_down=KeyZ, modifiers=[KeyControl]},
			// Global Actions
			entry! {action=GlobalMessage::LogInfo, key_down=Key1},
			entry! {action=GlobalMessage::LogDebug, key_down=Key2},
			entry! {action=GlobalMessage::LogTrace, key_down=Key3},
		];
		Self { up, down, mouse_move }
	}
}

impl Mappings {
	fn match_message(&self, message: InputMapperMessage, keys: &Keyboard, actions: ActionList) -> Option<Message> {
		use InputMapperMessage::*;
		let list = match message {
			KeyDown(key) => &self.down[key as usize],
			KeyUp(key) => &self.up[key as usize],
			MouseMove => &self.mouse_move,
		};
		list.match_mapping(keys, actions)
	}
}

#[derive(Debug, Default)]
pub struct InputMapper {
	mapping: Mappings,
}

impl MessageHandler<InputMapperMessage, (&InputPreprocessor, ActionList)> for InputMapper {
	fn process_action(&mut self, message: InputMapperMessage, data: (&InputPreprocessor, ActionList), responses: &mut VecDeque<Message>) {
		let (input, actions) = data;
		if let Some(message) = self.mapping.match_message(message, &input.keyboard, actions) {
			responses.push_back(message);
		}
	}
	actions_fn!();
}
