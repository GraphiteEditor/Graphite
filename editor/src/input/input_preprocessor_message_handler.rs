use super::input_preprocessor::{KeyPosition, ModifierKeys};
use super::keyboard::{Key, KeyStates};
use super::mouse::{MouseKeys, MouseState, ViewportBounds};
use crate::message_prelude::*;

#[doc(inline)]
pub use graphene::DocumentResponse;

#[derive(Debug, Default)]
pub struct InputPreprocessorMessageHandler {
	pub keyboard: KeyStates,
	pub mouse: MouseState,
	pub viewport_bounds: ViewportBounds,
}

impl MessageHandler<InputPreprocessorMessage, ()> for InputPreprocessorMessageHandler {
	#[remain::check]
	fn process_action(&mut self, message: InputPreprocessorMessage, _data: (), responses: &mut VecDeque<Message>) {
		#[remain::sorted]
		match message {
			InputPreprocessorMessage::BoundsOfViewports { bounds_of_viewports } => {
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
					responses.push_back(
						DocumentMessage::Artboard(
							graphene::Operation::TransformLayer {
								path: vec![],
								transform: glam::DAffine2::from_translation(translation).to_cols_array(),
							}
							.into(),
						)
						.into(),
					);
				}
			}
			InputPreprocessorMessage::KeyDown { key, modifier_keys } => {
				self.handle_modifier_keys(modifier_keys, responses);
				self.keyboard.set(key as usize);
				responses.push_back(InputMapperMessage::KeyDown(key).into());
			}
			InputPreprocessorMessage::KeyUp { key, modifier_keys } => {
				self.handle_modifier_keys(modifier_keys, responses);
				self.keyboard.unset(key as usize);
				responses.push_back(InputMapperMessage::KeyUp(key).into());
			}
			InputPreprocessorMessage::MouseDown { editor_mouse_state, modifier_keys } => {
				self.handle_modifier_keys(modifier_keys, responses);

				let mouse_state = editor_mouse_state.to_mouse_state(&self.viewport_bounds);
				self.mouse.position = mouse_state.position;

				if let Some(message) = self.translate_mouse_event(mouse_state, KeyPosition::Pressed) {
					responses.push_back(message);
				}
			}
			InputPreprocessorMessage::MouseMove { editor_mouse_state, modifier_keys } => {
				self.handle_modifier_keys(modifier_keys, responses);

				let mouse_state = editor_mouse_state.to_mouse_state(&self.viewport_bounds);
				self.mouse.position = mouse_state.position;

				responses.push_back(InputMapperMessage::PointerMove.into());
			}
			InputPreprocessorMessage::MouseScroll { editor_mouse_state, modifier_keys } => {
				self.handle_modifier_keys(modifier_keys, responses);

				let mouse_state = editor_mouse_state.to_mouse_state(&self.viewport_bounds);
				self.mouse.position = mouse_state.position;
				self.mouse.scroll_delta = mouse_state.scroll_delta;

				responses.push_back(InputMapperMessage::MouseScroll.into());
			}
			InputPreprocessorMessage::MouseUp { editor_mouse_state, modifier_keys } => {
				self.handle_modifier_keys(modifier_keys, responses);

				let mouse_state = editor_mouse_state.to_mouse_state(&self.viewport_bounds);
				self.mouse.position = mouse_state.position;

				if let Some(message) = self.translate_mouse_event(mouse_state, KeyPosition::Released) {
					responses.push_back(message);
				}
			}
		};
	}

	// Clean user input and if possible reconstruct it.
	// Store the changes in the keyboard if it is a key event.
	// Transform canvas coordinates to document coordinates.
	advertise_actions!();
}

impl InputPreprocessorMessageHandler {
	fn translate_mouse_event(&mut self, new_state: MouseState, position: KeyPosition) -> Option<Message> {
		// Calculate the difference between the two key states (binary xor)
		let difference = self.mouse.mouse_keys ^ new_state.mouse_keys;

		self.mouse = new_state;

		let key = match difference {
			MouseKeys::LEFT => Key::Lmb,
			MouseKeys::RIGHT => Key::Rmb,
			MouseKeys::MIDDLE => Key::Mmb,
			MouseKeys::NONE => return None, // self.mouse.mouse_keys was invalid, e.g. when a drag began outside the client
			_ => {
				log::warn!("The number of buttons modified at the same time was greater than 1. Modification: {:#010b}", difference);
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
