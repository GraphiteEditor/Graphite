use super::mouse::{MouseKeys, MouseState, ViewportPosition};
use super::{
	keyboard::{Key, KeyStates},
	mouse::DocumentTransform,
};
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
	pub keyboard: KeyStates,
	pub mouse: MouseState,
	pub document_transform: DocumentTransform,
}

enum KeyPosition {
	Pressed,
	Released,
}

impl MessageHandler<InputPreprocessorMessage, ()> for InputPreprocessor {
	fn process_action(&mut self, message: InputPreprocessorMessage, _data: (), responses: &mut VecDeque<Message>) {
		let response = match message {
			InputPreprocessorMessage::MouseMove(pos) => {
				self.mouse.position = pos;
				InputMapperMessage::PointerMove.into()
			}
			InputPreprocessorMessage::MouseDown(state) => self.translate_mouse_event(state, KeyPosition::Pressed),
			InputPreprocessorMessage::MouseUp(state) => self.translate_mouse_event(state, KeyPosition::Released),
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
	// transform canvas coordinates to document coordinates
	advertise_actions!();
}

impl InputPreprocessor {
	fn translate_mouse_event(&mut self, new_state: MouseState, position: KeyPosition) -> Message {
		// Calculate the difference between the two key states (binary xor)
		let diff = self.mouse.mouse_keys ^ new_state.mouse_keys;
		self.mouse = new_state;
		let key = match diff {
			MouseKeys::LEFT => Key::Lmb,
			MouseKeys::RIGHT => Key::Rmb,
			MouseKeys::MIDDLE => Key::Mmb,
			_ => {
				log::warn!("The number of buttons modified at the same time was not equal to 1. Modification: {:#010b}", diff);
				Key::UnknownKey
			}
		};
		match position {
			KeyPosition::Pressed => InputMapperMessage::KeyDown(key).into(),
			KeyPosition::Released => InputMapperMessage::KeyUp(key).into(),
		}
	}
}
