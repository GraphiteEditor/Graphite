use super::keyboard::{Key, KeyState};
use super::mouse::{MouseKeys, MouseState, ViewportPosition};
use crate::message_prelude::*;

#[doc(inline)]
pub use document_core::DocumentResponse;

use std::collections::HashMap;

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
	keyboard: HashMap<Key, KeyState>,
	//key_translation: HashMap<Key, VirtualInputToolMessage>,
	pub mouse_state: MouseState,
}

impl MessageHandler<InputPreprocessorMessage, ()> for InputPreprocessor {
	fn process_action(&mut self, message: InputPreprocessorMessage, _data: (), responses: &mut VecDeque<Message>) {
		let response = match message {
			InputPreprocessorMessage::MouseMove(pos) => {
				self.mouse_state.position = pos;
				InputMapperMessage::MouseMove.into()
			}
			InputPreprocessorMessage::MouseDown(state) => self.translate_mouse_event(state, true),
			InputPreprocessorMessage::MouseUp(state) => self.translate_mouse_event(state, false),
			InputPreprocessorMessage::KeyDown(key) => InputMapperMessage::KeyDown(key).into(),
			InputPreprocessorMessage::KeyUp(key) => InputMapperMessage::KeyUp(key).into(),
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
		match (down, diff) {
			(true, MouseKeys::LEFT) => InputMapperMessage::LmbDown.into(),
			(true, MouseKeys::RIGHT) => InputMapperMessage::RmbDown.into(),
			(true, MouseKeys::MIDDLE) => InputMapperMessage::MmbDown.into(),
			(false, MouseKeys::LEFT) => InputMapperMessage::LmbUp.into(),
			(false, MouseKeys::RIGHT) => InputMapperMessage::RmbUp.into(),
			(false, MouseKeys::MIDDLE) => InputMapperMessage::MmbUp.into(),
			(_, _) => {
				log::warn!("The number of buttons modified at the same time was not equal to 1. Modification: {:#010b}", diff);
				Message::NoOp
			}
		}
	}
}
