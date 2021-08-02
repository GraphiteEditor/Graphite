use std::usize;

use super::keyboard::{Key, KeyStates};
use super::mouse::{MouseKeys, MouseState, ScrollDelta, ViewportPosition};
use crate::message_prelude::*;
use bitflags::bitflags;

#[doc(inline)]
pub use document_core::DocumentResponse;
use glam::DVec2;

#[impl_message(Message, InputPreprocessor)]
#[derive(PartialEq, Clone, Debug)]
pub enum InputPreprocessorMessage {
	MouseDown(MouseState, ModifierKeys),
	MouseUp(MouseState, ModifierKeys),
	MouseMove(ViewportPosition, ModifierKeys),
	MouseScroll(ScrollDelta, ModifierKeys),
	KeyUp(Key, ModifierKeys),
	KeyDown(Key, ModifierKeys),
	ViewportResize(ViewportPosition),
}

bitflags! {
	#[derive(Default)]
	#[repr(transparent)]
	pub struct ModifierKeys: u8 {
		const CONTROL = 0b0000_0001;
		const SHIFT   = 0b0000_0010;
		const ALT     = 0b0000_0100;
	}
}

#[derive(Debug, Default, Hash)]
pub struct InputPreprocessor {
	pub keyboard: KeyStates,
	pub mouse: MouseState,
	pub viewport_size: ViewportPosition,
}

enum KeyPosition {
	Pressed,
	Released,
}

impl MessageHandler<InputPreprocessorMessage, ()> for InputPreprocessor {
	fn process_action(&mut self, message: InputPreprocessorMessage, _data: (), responses: &mut VecDeque<Message>) {
		match message {
			InputPreprocessorMessage::MouseMove(pos, modifier_keys) => {
				self.handle_modifier_keys(modifier_keys, responses);
				self.mouse.position = pos;
				responses.push_back(InputMapperMessage::PointerMove.into());
			}
			InputPreprocessorMessage::MouseScroll(delta, modifier_keys) => {
				self.handle_modifier_keys(modifier_keys, responses);
				self.mouse.scroll_delta = delta;
				responses.push_back(InputMapperMessage::MouseScroll.into());
			}
			InputPreprocessorMessage::MouseDown(state, modifier_keys) => {
				self.handle_modifier_keys(modifier_keys, responses);
				responses.push_back(self.translate_mouse_event(state, KeyPosition::Pressed));
			}
			InputPreprocessorMessage::MouseUp(state, modifier_keys) => {
				self.handle_modifier_keys(modifier_keys, responses);
				responses.push_back(self.translate_mouse_event(state, KeyPosition::Released));
			}
			InputPreprocessorMessage::KeyDown(key, modifier_keys) => {
				self.handle_modifier_keys(modifier_keys, responses);
				self.keyboard.set(key as usize);
				responses.push_back(InputMapperMessage::KeyDown(key).into());
			}
			InputPreprocessorMessage::KeyUp(key, modifier_keys) => {
				self.handle_modifier_keys(modifier_keys, responses);
				self.keyboard.unset(key as usize);
				responses.push_back(InputMapperMessage::KeyUp(key).into());
			}
			InputPreprocessorMessage::ViewportResize(size) => {
				responses.push_back(
					document_core::Operation::TransformLayer {
						path: vec![],
						transform: glam::DAffine2::from_translation(DVec2::new((size.x as f64 - self.viewport_size.x as f64) / 2., (size.y as f64 - self.viewport_size.y as f64) / 2.)).to_cols_array(),
					}
					.into(),
				);
				self.viewport_size = size;
			}
		};
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

	fn handle_modifier_keys(&mut self, modifier_keys: ModifierKeys, responses: &mut VecDeque<Message>) {
		self.handle_modifier_key(Key::KeyControl, modifier_keys.contains(ModifierKeys::CONTROL), responses);
		self.handle_modifier_key(Key::KeyShift, modifier_keys.contains(ModifierKeys::SHIFT), responses);
		self.handle_modifier_key(Key::KeyAlt, modifier_keys.contains(ModifierKeys::ALT), responses);
	}

	fn handle_modifier_key(&mut self, key: Key, key_is_down: bool, responses: &mut VecDeque<Message>) {
		let key_was_down = self.keyboard.get(key as usize);
		if key_was_down && !key_is_down {
			self.keyboard.unset(key as usize);
			responses.push_back(InputMapperMessage::KeyUp(key).into());
		} else if !key_was_down && key_is_down {
			self.keyboard.set(key as usize);
			responses.push_back(InputMapperMessage::KeyDown(key).into());
		}
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn process_action_mouse_move_handle_modifier_keys() {
		let mut input_preprocessor = InputPreprocessor::default();
		let message = InputPreprocessorMessage::MouseMove((4, 809).into(), ModifierKeys::ALT);
		let mut responses = VecDeque::new();

		input_preprocessor.process_action(message, (), &mut responses);

		assert!(input_preprocessor.keyboard.get(Key::KeyAlt as usize));
		assert_eq!(responses.pop_front(), Some(InputMapperMessage::KeyDown(Key::KeyAlt).into()));
	}

	#[test]
	fn process_action_mouse_down_handle_modifier_keys() {
		let mut input_preprocessor = InputPreprocessor::default();
		let message = InputPreprocessorMessage::MouseDown(MouseState::new(), ModifierKeys::CONTROL);
		let mut responses = VecDeque::new();

		input_preprocessor.process_action(message, (), &mut responses);

		assert!(input_preprocessor.keyboard.get(Key::KeyControl as usize));
		assert_eq!(responses.pop_front(), Some(InputMapperMessage::KeyDown(Key::KeyControl).into()));
	}

	#[test]
	fn process_action_mouse_up_handle_modifier_keys() {
		let mut input_preprocessor = InputPreprocessor::default();
		let message = InputPreprocessorMessage::MouseUp(MouseState::new(), ModifierKeys::SHIFT);
		let mut responses = VecDeque::new();

		input_preprocessor.process_action(message, (), &mut responses);

		assert!(input_preprocessor.keyboard.get(Key::KeyShift as usize));
		assert_eq!(responses.pop_front(), Some(InputMapperMessage::KeyDown(Key::KeyShift).into()));
	}

	#[test]
	fn process_action_key_down_handle_modifier_keys() {
		let mut input_preprocessor = InputPreprocessor::default();
		input_preprocessor.keyboard.set(Key::KeyControl as usize);
		let message = InputPreprocessorMessage::KeyDown(Key::KeyA, ModifierKeys::empty());
		let mut responses = VecDeque::new();

		input_preprocessor.process_action(message, (), &mut responses);

		assert!(!input_preprocessor.keyboard.get(Key::KeyControl as usize));
		assert_eq!(responses.pop_front(), Some(InputMapperMessage::KeyUp(Key::KeyControl).into()));
	}

	#[test]
	fn process_action_key_up_handle_modifier_keys() {
		let mut input_preprocessor = InputPreprocessor::default();
		let message = InputPreprocessorMessage::KeyUp(Key::KeyS, ModifierKeys::CONTROL | ModifierKeys::SHIFT);
		let mut responses = VecDeque::new();

		input_preprocessor.process_action(message, (), &mut responses);

		assert!(input_preprocessor.keyboard.get(Key::KeyControl as usize));
		assert!(input_preprocessor.keyboard.get(Key::KeyShift as usize));
		assert!(responses.contains(&InputMapperMessage::KeyDown(Key::KeyControl).into()));
		assert!(responses.contains(&InputMapperMessage::KeyDown(Key::KeyControl).into()));
	}
}
