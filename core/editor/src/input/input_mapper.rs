use crate::message_prelude::*;
use crate::tool::ToolType;

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
}

#[derive(Debug, Clone)]
struct Mapping {
	up: [Vec<MappingEntry>; NUMBER_OF_KEYS],
	down: [Vec<MappingEntry>; NUMBER_OF_KEYS],
	mouse_move: Vec<MappingEntry>,
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
		MappingEntry {modifiers: modifiers!($($($m),*)?), action: $action.into()}
	}};
}
macro_rules! mapping {
	//[$(<action=$action:expr; message=$key:expr; $(modifiers=[$($m:ident),* $(,)?];)?>)*] => {{
	[$($entry:expr),* $(,)?] => {{
		let mut up: [Vec<MappingEntry>; NUMBER_OF_KEYS] = Default::default();
		let mut down: [Vec<MappingEntry>; NUMBER_OF_KEYS] = Default::default();
		let mut mouse_move: Vec<MappingEntry> = Default::default();
		$(
			if let Message::InputMapper(message) = $entry.action {
			let arr = match message {
				InputMapperMessage::KeyDown(key) => &mut down[key as usize],
				InputMapperMessage::KeyUp(key) => &mut up[key as usize],
				InputMapperMessage::MouseMove => &mut mouse_move,
			};
			arr.push($entry);
			}
        );*
		(up, down, mouse_move)
	}};
}

impl Default for Mapping {
	fn default() -> Self {
		use InputMapperMessage::*;
		let (up, down, mouse_move) = mapping![
			entry! {action=RectangleMessage::Center, key_down=KeyAlt},
			entry! {action=RectangleMessage::UnCenter, key_up=KeyAlt},
			entry! {action=RectangleMessage::MouseMove, message=MouseMove},
			entry! {action=RectangleMessage::DragStart, key_down=Lmb},
			entry! {action=RectangleMessage::DragStop, key_up=Lmb},
			entry! {action=RectangleMessage::Abort, key_down=Rmb},
			entry! {action=RectangleMessage::Abort, key_down=KeyEscape},
			entry! {action=RectangleMessage::LockAspectRatio, key_down=KeyAlt},
			entry! {action=RectangleMessage::UnlockAspectRatio, key_up=KeyAlt},
			entry! {action=DocumentMessage::Undo, key_down=KeyZ, modifiers=[KeyControl]},
		];
		Self { up, down, mouse_move }
	}
}

#[derive(Debug, Default)]
pub struct InputMapper {
	mapping: Mapping,
}

impl MessageHandler<InputMapperMessage, (&InputPreprocessor, ActionList)> for InputMapper {
	fn process_action(&mut self, message: InputMapperMessage, data: (&InputPreprocessor, ActionList), responses: &mut VecDeque<Message>) {
		let (input, actions) = data;
		use InputMapperMessage::*;
		let res = match message {
			KeyDown(key) => self.translate_key_down(key, input),
			KeyUp(key) => self.translate_key_up(key, input),
			MouseMove => self.translate_key_down(Key::MouseMove, input),
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
			//KeyEnter => RectangleMessage::Confirm.into(),
			KeyAlt => RectangleMessage::Center.into(),
			KeyShift | KeyCaps => RectangleMessage::LockAspectRatio.into(),
			_ => Message::NoOp,
		}
	}
}
/*
				   Key::KeyV => {
					   editor_state.tool_state.tool_data.active_tool_type = ToolType::Select;
					   self.dispatch_response(ToolResponse::SetActiveTool {
						   tool_name: ToolType::Select.to_string(),
					   });
				   }
				   Key::KeyL => {
					   editor_state.tool_state.tool_data.active_tool_type = ToolType::Line;
					   self.dispatch_response(ToolResponse::SetActiveTool {
						   tool_name: ToolType::Line.to_string(),
					   });
				   }
				   Key::KeyP => {
					   editor_state.tool_state.tool_data.active_tool_type = ToolType::Pen;
					   self.dispatch_response(ToolResponse::SetActiveTool { tool_name: ToolType::Pen.to_string() });
				   }
				   Key::KeyM => {
					   editor_state.tool_state.tool_data.active_tool_type = ToolType::Rectangle;
					   self.dispatch_response(ToolResponse::SetActiveTool {
						   tool_name: ToolType::Rectangle.to_string(),
					   });
				   }
				   Key::KeyY => {
					   editor_state.tool_state.tool_data.active_tool_type = ToolType::Shape;
					   self.dispatch_response(ToolResponse::SetActiveTool {
						   tool_name: ToolType::Shape.to_string(),
					   });
				   }
				   Key::KeyE => {
					   editor_state.tool_state.tool_data.active_tool_type = ToolType::Ellipse;
					   self.dispatch_response(ToolResponse::SetActiveTool {
						   tool_name: ToolType::Ellipse.to_string(),
					   });
				   }
				   Key::KeyX => {
					   editor_state.tool_state.swap_colors();
				   }
				   _ => (),
*/
