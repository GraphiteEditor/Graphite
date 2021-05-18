use crate::tools::ToolType;
use graphite_proc_macros::*;

use super::{
	events::{Event, Key, MouseState},
	message::prelude::*,
	MessageHandler,
};
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Default)]
pub struct KeyState {
	depressed: bool,
	// time of last press
	// mod keys held down while pressing
	// …
}

#[derive(Debug, Default)]
pub struct InputPreprocessor {
	mouse_keys: MouseState,
	keyboard: HashMap<Key, KeyState>,
	//key_translation: HashMap<Key, VirtualInputToolMessage>,
	pub mouse_state: MouseState,
}

#[impl_message(Message, InputPreprocessor)]
#[derive(PartialEq, Clone, Debug)]
pub enum InputPreprocessorMessage {
	Event(Event),
}

#[impl_message(Message, InputMapper)]
#[derive(PartialEq, Clone, Debug)]
pub enum InputMapperMessage {
	Event(Event),
}

impl MessageHandler<InputPreprocessorMessage, ()> for InputPreprocessor {
	fn process_action(&mut self, message: InputPreprocessorMessage, _data: (), responses: &mut VecDeque<Message>) {
		match message {
			InputPreprocessorMessage::Event(Event::MouseMove(pos)) => {
				self.mouse_state.position = pos;
				responses.push_back(InputMapperMessage::Event(Event::MouseMove(pos)).into())
			}
			InputPreprocessorMessage::Event(e) => responses.push_back(InputMapperMessage::Event(e).into()),
		}
	}
	// clean user input and if possible reconstruct it
	// store the changes in the keyboard if it is a key event
	// translate the key events to VirtualKeyToolMessages and return them
	// transform canvas coordinates to document coordinates
	// Last pressed key
	actions_fn!();
}

macro_rules! key {
	($path:expr, $action:expr, $k:ident, ($($s:ident),*)) => {
		($path, $action, Event::KeyDown(Key::$k), &[$(Key::$s,)*])
	};
	($action:expr, $k:ident, ($($s:ident),*)) => {
		key!("*", $action, $k, ($($s),*))
	};
	($path:expr, $action:expr, $k:ident) => {
		key!($path, $action, $k, ())
	};
	($action:expr, $k:ident) => {
		key!("*", $action, $k, ())
	};
}

const _DEFAULT_MAPPING: &[(&str, &str, Event, &[Key])] = &[
	key!("Undo", KeyZ, (KeyControl)),
	key!("*", "Redo", KeyZ, (KeyControl, KeyShift)),
	key!("Redo", KeyZ, (KeyControl, KeyCaps)),
	key!("Center", KeyAlt),
];

#[derive(Debug, Default)]
pub struct InputMapper {}

impl MessageHandler<InputMapperMessage, &InputPreprocessor> for InputMapper {
	fn process_action(&mut self, message: InputMapperMessage, input: &InputPreprocessor, responses: &mut VecDeque<Message>) {
		let res = match message {
			InputMapperMessage::Event(e) => match e {
				Event::SelectTool(tool_name) => ToolMessage::SelectTool(tool_name).into(),
				Event::SelectPrimaryColor(color) => ToolMessage::SelectPrimaryColor(color).into(),
				Event::SelectSecondaryColor(color) => ToolMessage::SelectSecondaryColor(color).into(),
				Event::SwapColors => ToolMessage::SwapColors.into(),
				Event::ResetColors => ToolMessage::ResetColors.into(),
				Event::MouseMove(_) => RectangleMessage::MouseMove.into(),
				Event::ToggleLayerVisibility(path) => DocumentMessage::ToggleLayerVisibility(path).into(),
				Event::LmbDown(_) => RectangleMessage::DragStart.into(),
				Event::LmbUp(_) => RectangleMessage::DragStop.into(),
				Event::RmbDown(_) => RectangleMessage::Abort.into(),
				Event::KeyDown(key) => self.translate_key_down(key, input),
				Event::KeyUp(key) => self.translate_key_up(key, input),
				e => todo!("Unhandled event: {:?}", e),
			},
		};
		responses.push_back(res);
	}
	actions_fn!();
}
impl InputMapper {
	fn translate_key_up(&self, key: Key, _input: &InputPreprocessor) -> Message {
		use Key::*;
		match key {
			KeyAlt => RectangleMessage::UnCenter.into(),
			KeyShift | KeyCaps => RectangleMessage::UnlockAspectRatio.into(),
			_ => Message::NoOp,
		}
	}
	fn translate_key_down(&self, key: Key, _input: &InputPreprocessor) -> Message {
		use Key::*;
		match key {
			Key1 => GlobalMessage::LogInfo.into(),
			Key2 => GlobalMessage::LogDebug.into(),
			Key3 => GlobalMessage::LogTrace.into(),
			KeyV => ToolMessage::SelectTool(ToolType::Select).into(),
			KeyL => ToolMessage::SelectTool(ToolType::Line).into(),
			KeyP => ToolMessage::SelectTool(ToolType::Pen).into(),
			KeyM => ToolMessage::SelectTool(ToolType::Rectangle).into(),
			KeyY => ToolMessage::SelectTool(ToolType::Shape).into(),
			KeyE => ToolMessage::SelectTool(ToolType::Ellipse).into(),
			KeyX => ToolMessage::SwapColors.into(),
			KeyZ => DocumentMessage::Undo.into(),
			KeyEnter => RectangleMessage::Confirm.into(),
			KeyAlt => RectangleMessage::Center.into(),
			KeyShift | KeyCaps => RectangleMessage::LockAspectRatio.into(),
			_ => Message::NoOp,
		}
	}
}
