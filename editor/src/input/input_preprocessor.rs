use std::usize;

use super::keyboard::{Key, KeyStates};
use super::mouse::{EditorMouseState, MouseKeys, MouseState, ViewportBounds};
use crate::message_prelude::*;
use bitflags::bitflags;

#[doc(inline)]
pub use graphene::DocumentResponse;

#[impl_message(Message, InputPreprocessor)]
#[derive(PartialEq, Clone, Debug)]
pub enum InputPreprocessorMessage {
	MouseDown(EditorMouseState, ModifierKeys),
	MouseUp(EditorMouseState, ModifierKeys),
	MouseMove(EditorMouseState, ModifierKeys),
	MouseScroll(EditorMouseState, ModifierKeys),
	KeyUp(Key, ModifierKeys),
	KeyDown(Key, ModifierKeys),
	BoundsOfViewports(Vec<ViewportBounds>),
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

#[derive(Debug, Default)]
pub struct InputPreprocessor {
	pub keyboard: KeyStates,
	pub mouse: MouseState,
	pub viewport_bounds: ViewportBounds,
}

enum KeyPosition {
	Pressed,
	Released,
}

impl MessageHandler<InputPreprocessorMessage, ()> for InputPreprocessor {
	fn process_action(&mut self, message: InputPreprocessorMessage, _data: (), responses: &mut VecDeque<Message>) {
		match message {
			InputPreprocessorMessage::MouseMove(editor_mouse_state, modifier_keys) => {
				self.handle_modifier_keys(modifier_keys, responses);

				let mouse_state = editor_mouse_state.to_mouse_state(&self.viewport_bounds);
				self.mouse.position = mouse_state.position;

				responses.push_back(InputMapperMessage::PointerMove.into());
			}
			InputPreprocessorMessage::MouseDown(editor_mouse_state, modifier_keys) => {
				self.handle_modifier_keys(modifier_keys, responses);

				let mouse_state = editor_mouse_state.to_mouse_state(&self.viewport_bounds);
				self.mouse.position = mouse_state.position;

				if let Some(message) = self.translate_mouse_event(mouse_state, KeyPosition::Pressed) {
					responses.push_back(message);
				}
			}
			InputPreprocessorMessage::MouseUp(editor_mouse_state, modifier_keys) => {
				self.handle_modifier_keys(modifier_keys, responses);

				let mouse_state = editor_mouse_state.to_mouse_state(&self.viewport_bounds);
				self.mouse.position = mouse_state.position;

				if let Some(message) = self.translate_mouse_event(mouse_state, KeyPosition::Released) {
					responses.push_back(message);
				}
			}
			InputPreprocessorMessage::MouseScroll(editor_mouse_state, modifier_keys) => {
				self.handle_modifier_keys(modifier_keys, responses);

				let mouse_state = editor_mouse_state.to_mouse_state(&self.viewport_bounds);
				self.mouse.position = mouse_state.position;
				self.mouse.scroll_delta = mouse_state.scroll_delta;

				responses.push_back(InputMapperMessage::MouseScroll.into());
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
			InputPreprocessorMessage::BoundsOfViewports(bounds_of_viewports) => {
				assert_eq!(bounds_of_viewports.len(), 1, "Only one viewport is currently supported");

				for bounds in bounds_of_viewports {
					let new_size = bounds.size();
					let existing_size = self.viewport_bounds.size();

					let translation = (new_size - existing_size) / 2.;

					// TODO: Extend this to multiple viewports instead of setting it to the value of this last loop iteration
					self.viewport_bounds = bounds;

					responses.push_back(
						graphene::Operation::TransformLayer {
							path: vec![],
							transform: glam::DAffine2::from_translation(translation).to_cols_array(),
						}
						.into(),
					);
				}
			}
		};
	}
	// clean user input and if possible reconstruct it
	// store the changes in the keyboard if it is a key event
	// transform canvas coordinates to document coordinates
	advertise_actions!();
}

impl InputPreprocessor {
	fn translate_mouse_event(&mut self, new_state: MouseState, position: KeyPosition) -> Option<Message> {
		// Calculate the difference between the two key states (binary xor)
		let diff = self.mouse.mouse_keys ^ new_state.mouse_keys;
		self.mouse = new_state;
		let key = match diff {
			MouseKeys::LEFT => Key::Lmb,
			MouseKeys::RIGHT => Key::Rmb,
			MouseKeys::MIDDLE => Key::Mmb,
			MouseKeys::NONE => return None, // self.mouse.mouse_keys was invalid, e.g. when a drag began outside the client
			_ => {
				log::warn!("The number of buttons modified at the same time was greater than 1. Modification: {:#010b}", diff);
				Key::UnknownKey
			}
		};
		Some(match position {
			KeyPosition::Pressed => InputMapperMessage::KeyDown(key).into(),
			KeyPosition::Released => InputMapperMessage::KeyUp(key).into(),
		})
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
	use crate::input::mouse::ViewportPosition;

	use super::*;

	#[test]
	fn process_action_mouse_move_handle_modifier_keys() {
		let mut input_preprocessor = InputPreprocessor::default();
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
		let mut input_preprocessor = InputPreprocessor::default();
		let message = InputPreprocessorMessage::MouseDown(EditorMouseState::new(), ModifierKeys::CONTROL);
		let mut responses = VecDeque::new();

		input_preprocessor.process_action(message, (), &mut responses);

		assert!(input_preprocessor.keyboard.get(Key::KeyControl as usize));
		assert_eq!(responses.pop_front(), Some(InputMapperMessage::KeyDown(Key::KeyControl).into()));
	}

	#[test]
	fn process_action_mouse_up_handle_modifier_keys() {
		let mut input_preprocessor = InputPreprocessor::default();
		let message = InputPreprocessorMessage::MouseUp(EditorMouseState::new(), ModifierKeys::SHIFT);
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
