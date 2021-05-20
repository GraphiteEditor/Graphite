use crate::message_prelude::*;
use crate::tool::ToolType;

use super::{
	keyboard::{Key, Keyboard, NUMBER_OF_KEYS},
	InputPreprocessor,
};

#[impl_message(Message, InputMapper)]
#[derive(PartialEq, Clone, Debug)]
pub enum InputMapperMessage {
	KeyUp(Key),
	KeyDown(Key),
}

#[derive(PartialEq, Clone, Debug)]
struct MappingEntry {
	modifiers: Keyboard,
	action: Message,
}

struct Mapping {
	up: [Vec<MappingEntry>; NUMBER_OF_KEYS],
	down: [Vec<MappingEntry>; NUMBER_OF_KEYS],
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
macro_rules! mapping {
	[$(<action=$action:expr; key=$key:expr; $(modifiers=[$($m:ident),* $(,)?];)?>)*] => {{
		let mut up: [Vec<MappingEntry>; NUMBER_OF_KEYS] = Default::default();
		let mut down: [Vec<MappingEntry>; NUMBER_OF_KEYS] = Default::default();
		$({
			let  (arr, key) =  match $key {
				InputMapperMessage::KeyDown(key) => (&mut down, key),
				InputMapperMessage::KeyUp(key) => (&mut up, key),
			};
			arr[key as usize].push( MappingEntry {modifiers: modifiers!($($($m),*)?), action: $action.into()});
		})*
		(up, down)
	}};
}

impl Default for Mapping {
	fn default() -> Self {
		use InputMapperMessage::*;
		let (up, down) = mapping![
			<action=DocumentMessage::Undo; key=KeyDown(Key::KeyZ); modifiers=[KeyControl];>
			<action=RectangleMessage::Center; key=KeyDown(Key::KeyAlt);>
			<action=RectangleMessage::UnCenter; key=KeyUp(Key::KeyAlt);>
			<action=RectangleMessage::MouseMove; key=KeyDown(Key::MouseMove);>
			<action=RectangleMessage::DragStart; key=KeyDown(Key::LMB);>
			<action=RectangleMessage::DragStop; key=KeyUp(Key::LMB);>
			<action=RectangleMessage::Abort; key=KeyDown(Key::RMB);>
			<action=RectangleMessage::Abort; key=KeyDown(Key::KeyEscape);>
			<action=RectangleMessage::LockAspectRatio; key=KeyDown(Key::KeyAlt);>
			<action=RectangleMessage::UnlockAspectRatio; key=KeyUp(Key::KeyAlt);>

		];
		Self { up, down }
	}
}

#[derive(Debug, Default)]
pub struct InputMapper {}

impl MessageHandler<InputMapperMessage, (&InputPreprocessor, ActionList)> for InputMapper {
	fn process_action(&mut self, message: InputMapperMessage, data: (&InputPreprocessor, ActionList), responses: &mut VecDeque<Message>) {
		let (input, actions) = data;
		use InputMapperMessage::*;
		let res = match message {
			KeyDown(key) => self.translate_key_down(key, input),
			KeyUp(key) => self.translate_key_up(key, input),
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
