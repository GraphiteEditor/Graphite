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
		const SHIFT              = 0b0000_0001;
		const ALT                = 0b0000_0010;
		const CONTROL            = 0b0000_0100;
		const META_OR_COMMAND    = 0b0000_1000;
	}
}

#[cfg(test)]
mod test {
	use crate::document::utility_types::KeyboardPlatformLayout;
	use crate::input::input_preprocessor::ModifierKeys;
	use crate::input::keyboard::Key;
	use crate::input::mouse::EditorMouseState;
	use crate::input::{InputMapperMessage, InputPreprocessorMessage, InputPreprocessorMessageHandler};
	use crate::message_prelude::MessageHandler;

	use std::collections::VecDeque;

	#[test]
	fn process_action_mouse_move_handle_modifier_keys() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();

		let editor_mouse_state = EditorMouseState::from_editor_position(4., 809.);
		let modifier_keys = ModifierKeys::ALT;
		let message = InputPreprocessorMessage::PointerMove { editor_mouse_state, modifier_keys };

		let mut responses = VecDeque::new();

		input_preprocessor.process_action(message, KeyboardPlatformLayout::Standard, &mut responses);

		assert!(input_preprocessor.keyboard.get(Key::KeyAlt as usize));
		assert_eq!(responses.pop_front(), Some(InputMapperMessage::KeyDown(Key::KeyAlt).into()));
	}

	#[test]
	fn process_action_mouse_down_handle_modifier_keys() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();

		let editor_mouse_state = EditorMouseState::new();
		let modifier_keys = ModifierKeys::CONTROL;
		let message = InputPreprocessorMessage::PointerDown { editor_mouse_state, modifier_keys };

		let mut responses = VecDeque::new();

		input_preprocessor.process_action(message, KeyboardPlatformLayout::Standard, &mut responses);

		assert!(input_preprocessor.keyboard.get(Key::KeyControl as usize));
		assert_eq!(responses.pop_front(), Some(InputMapperMessage::KeyDown(Key::KeyControl).into()));
	}

	#[test]
	fn process_action_mouse_up_handle_modifier_keys() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();

		let editor_mouse_state = EditorMouseState::new();
		let modifier_keys = ModifierKeys::SHIFT;
		let message = InputPreprocessorMessage::PointerUp { editor_mouse_state, modifier_keys };

		let mut responses = VecDeque::new();

		input_preprocessor.process_action(message, KeyboardPlatformLayout::Standard, &mut responses);

		assert!(input_preprocessor.keyboard.get(Key::KeyShift as usize));
		assert_eq!(responses.pop_front(), Some(InputMapperMessage::KeyDown(Key::KeyShift).into()));
	}

	#[test]
	fn process_action_key_down_handle_modifier_keys() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();
		input_preprocessor.keyboard.set(Key::KeyControl as usize);

		let key = Key::KeyA;
		let modifier_keys = ModifierKeys::empty();
		let message = InputPreprocessorMessage::KeyDown { key, modifier_keys };

		let mut responses = VecDeque::new();

		input_preprocessor.process_action(message, KeyboardPlatformLayout::Standard, &mut responses);

		assert!(!input_preprocessor.keyboard.get(Key::KeyControl as usize));
		assert_eq!(responses.pop_front(), Some(InputMapperMessage::KeyUp(Key::KeyControl).into()));
	}

	#[test]
	fn process_action_key_up_handle_modifier_keys() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();

		let key = Key::KeyS;
		let modifier_keys = ModifierKeys::CONTROL | ModifierKeys::SHIFT;
		let message = InputPreprocessorMessage::KeyUp { key, modifier_keys };

		let mut responses = VecDeque::new();

		input_preprocessor.process_action(message, KeyboardPlatformLayout::Standard, &mut responses);

		assert!(input_preprocessor.keyboard.get(Key::KeyControl as usize));
		assert!(input_preprocessor.keyboard.get(Key::KeyShift as usize));
		assert!(responses.contains(&InputMapperMessage::KeyDown(Key::KeyControl).into()));
		assert!(responses.contains(&InputMapperMessage::KeyDown(Key::KeyControl).into()));
	}
}
