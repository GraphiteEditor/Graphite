#[doc(inline)]
pub use graphene::DocumentResponse;

use bitflags::bitflags;
use serde::{Deserialize, Serialize};

pub enum KeyPosition {
	Pressed,
	Released,
}

bitflags! {
	#[derive(Default, Serialize, Deserialize)]
	#[repr(transparent)]
	pub struct ModifierKeys: u8 {
		const CONTROL = 0b0000_0001;
		const SHIFT   = 0b0000_0010;
		const ALT     = 0b0000_0100;
	}
}

#[cfg(test)]
mod test {
	use crate::input::input_preprocessor::ModifierKeys;
	use crate::input::keyboard::Key;
	use crate::input::mouse::{EditorMouseState, ViewportPosition};
	use crate::input::InputPreprocessorMessageHandler;
	use crate::message_prelude::MessageHandler;
	use crate::message_prelude::*;

	use std::collections::VecDeque;

	#[test]
	fn process_action_mouse_move_handle_modifier_keys() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();
		let mut editor_mouse_state = EditorMouseState::new();
		editor_mouse_state.editor_position = ViewportPosition::new(4., 809.);
		let message = InputPreprocessorMessage::MouseMove(editor_mouse_state, ModifierKeys::ALT);
		let mut responses = VecDeque::new();

		input_preprocessor.process_action(message, (), &mut responses);

		assert!(input_preprocessor.keyboard.get(Key::KeyAlt as usize));
		assert_eq!(responses.pop_front(), Some(InputMapperMessage::KeyDown(Key::KeyAlt).into()));
	}

	#[test]
	fn process_action_mouse_down_handle_modifier_keys() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();
		let message = InputPreprocessorMessage::MouseDown(EditorMouseState::new(), ModifierKeys::CONTROL);
		let mut responses = VecDeque::new();

		input_preprocessor.process_action(message, (), &mut responses);

		assert!(input_preprocessor.keyboard.get(Key::KeyControl as usize));
		assert_eq!(responses.pop_front(), Some(InputMapperMessage::KeyDown(Key::KeyControl).into()));
	}

	#[test]
	fn process_action_mouse_up_handle_modifier_keys() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();
		let message = InputPreprocessorMessage::MouseUp(EditorMouseState::new(), ModifierKeys::SHIFT);
		let mut responses = VecDeque::new();

		input_preprocessor.process_action(message, (), &mut responses);

		assert!(input_preprocessor.keyboard.get(Key::KeyShift as usize));
		assert_eq!(responses.pop_front(), Some(InputMapperMessage::KeyDown(Key::KeyShift).into()));
	}

	#[test]
	fn process_action_key_down_handle_modifier_keys() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();
		input_preprocessor.keyboard.set(Key::KeyControl as usize);
		let message = InputPreprocessorMessage::KeyDown(Key::KeyA, ModifierKeys::empty());
		let mut responses = VecDeque::new();

		input_preprocessor.process_action(message, (), &mut responses);

		assert!(!input_preprocessor.keyboard.get(Key::KeyControl as usize));
		assert_eq!(responses.pop_front(), Some(InputMapperMessage::KeyUp(Key::KeyControl).into()));
	}

	#[test]
	fn process_action_key_up_handle_modifier_keys() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();
		let message = InputPreprocessorMessage::KeyUp(Key::KeyS, ModifierKeys::CONTROL | ModifierKeys::SHIFT);
		let mut responses = VecDeque::new();

		input_preprocessor.process_action(message, (), &mut responses);

		assert!(input_preprocessor.keyboard.get(Key::KeyControl as usize));
		assert!(input_preprocessor.keyboard.get(Key::KeyShift as usize));
		assert!(responses.contains(&InputMapperMessage::KeyDown(Key::KeyControl).into()));
		assert!(responses.contains(&InputMapperMessage::KeyDown(Key::KeyControl).into()));
	}
}
