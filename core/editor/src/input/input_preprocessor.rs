use super::keyboard::{Key, Keyboard};
use super::mouse::{MouseKeys, MouseState, ViewportPosition};
use crate::message_prelude::*;

#[doc(inline)]
pub use document_core::DocumentResponse;

#[impl_message(Message, InputPreprocessor)]
#[derive(PartialEq, Clone, Debug)]
pub enum InputPreprocessorMessage {
	MouseDown(MouseState),
	MouseUp(MouseState),
	MouseMove(ViewportPosition),
	KeyUp(Key),
	KeyDown(Key),
}

#[derive(Debug, Default)]
pub struct InputPreprocessor {
	pub keyboard: Keyboard,
	pub mouse_state: MouseState,
}

impl MessageHandler<InputPreprocessorMessage, ()> for InputPreprocessor {
	fn process_action(&mut self, message: InputPreprocessorMessage, _data: (), responses: &mut VecDeque<Message>) {
		let response = match message {
			InputPreprocessorMessage::MouseMove(pos) => {
				self.mouse_state.position = pos;
				InputMapperMessage::PointerMove.into()
			}
			InputPreprocessorMessage::MouseDown(state) => self.translate_mouse_event(state, true),
			InputPreprocessorMessage::MouseUp(state) => self.translate_mouse_event(state, false),
			InputPreprocessorMessage::KeyDown(key) => {
				self.keyboard.set(key as usize);
				InputMapperMessage::KeyDown(key).into()
			}
			InputPreprocessorMessage::KeyUp(key) => {
				self.keyboard.unset(key as usize);
				InputMapperMessage::KeyUp(key).into()
			}
		};
		responses.push_back(response)
	}
	// clean user input and if possible reconstruct it
	// store the changes in the keyboard if it is a key event
	// translate the key events to VirtualKeyToolMessages and return them
	// transform canvas coordinates to document coordinates
	// Last pressed key
	actions_fn!();
}

impl InputPreprocessor {
	fn translate_mouse_event(&mut self, new_state: MouseState, down: bool) -> Message {
		let diff = self.mouse_state.mouse_keys ^ new_state.mouse_keys;
		self.mouse_state = new_state;
		let key = match diff {
			MouseKeys::LEFT => Key::Lmb,
			MouseKeys::RIGHT => Key::Rmb,
			MouseKeys::MIDDLE => Key::Mmb,
			_ => {
				log::warn!("The number of buttons modified at the same time was not equal to 1. Modification: {:#010b}", diff);
				Key::UnknownKey
			}
		};
		match down {
			true => InputMapperMessage::KeyDown(key).into(),
			false => InputMapperMessage::KeyUp(key).into(),
		}
	}
}
