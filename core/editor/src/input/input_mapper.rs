use crate::message_prelude::*;
use crate::tool::ToolType;

use super::{keyboard::Key, InputPreprocessor};

#[impl_message(Message, InputMapper)]
#[derive(PartialEq, Clone, Debug)]
pub enum InputMapperMessage {
	LmbDown,
	RmbDown,
	MmbDown,
	LmbUp,
	RmbUp,
	MmbUp,
	MouseMove,
	KeyUp(Key),
	KeyDown(Key),
}

/*
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

const _DEFAULT_MAPPING: &[(&str, &str, Message, &[Key])] = &[
	key!("Undo", KeyZ, (KeyControl)),
	key!("*", "Redo", KeyZ, (KeyControl, KeyShift)),
	key!("Redo", KeyZ, (KeyControl, KeyCaps)),
	key!("Center", KeyAlt),
];
*/
#[derive(Debug, Default)]
pub struct InputMapper {}

impl MessageHandler<InputMapperMessage, (&InputPreprocessor, ActionList)> for InputMapper {
	fn process_action(&mut self, message: InputMapperMessage, data: (&InputPreprocessor, ActionList), responses: &mut VecDeque<Message>) {
		let (input, actions) = data;
		use InputMapperMessage::*;
		let res = match message {
			MouseMove => RectangleMessage::MouseMove.into(),
			LmbDown => RectangleMessage::DragStart.into(),
			LmbUp => RectangleMessage::DragStop.into(),
			RmbDown => RectangleMessage::Abort.into(),
			KeyDown(key) => self.translate_key_down(key, input),
			KeyUp(key) => self.translate_key_up(key, input),
			e => todo!("Unhandled event: {:?}", e),
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
