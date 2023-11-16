use crate::messages::input_mapper::utility_types::input_keyboard::{Key, KeyStates, ModifierKeys};
use crate::messages::input_mapper::utility_types::input_mouse::{MouseButton, MouseKeys, MouseState, ViewportBounds};
use crate::messages::portfolio::utility_types::KeyboardPlatformLayout;
use crate::messages::prelude::*;

use glam::DVec2;

#[derive(Debug, Default)]
pub struct InputPreprocessorMessageHandler {
	pub keyboard: KeyStates,
	pub mouse: MouseState,
	pub viewport_bounds: ViewportBounds,
}

impl MessageHandler<InputPreprocessorMessage, KeyboardPlatformLayout> for InputPreprocessorMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: InputPreprocessorMessage, responses: &mut VecDeque<Message>, keyboard_platform: KeyboardPlatformLayout) {
		#[remain::sorted]
		match message {
			InputPreprocessorMessage::BoundsOfViewports { bounds_of_viewports } => {
				assert_eq!(bounds_of_viewports.len(), 1, "Only one viewport is currently supported");

				for bounds in bounds_of_viewports {
					// TODO: Extend this to multiple viewports instead of setting it to the value of this last loop iteration
					self.viewport_bounds = bounds;

					responses.add(NavigationMessage::TranslateCanvas { delta: DVec2::ZERO });
					responses.add(FrontendMessage::TriggerViewportResize);
				}
			}
			InputPreprocessorMessage::DoubleClick { editor_mouse_state, modifier_keys } => {
				self.update_states_of_modifier_keys(modifier_keys, keyboard_platform, responses);

				let mouse_state = editor_mouse_state.to_mouse_state(&self.viewport_bounds);
				self.mouse.position = mouse_state.position;

				for key in mouse_state.mouse_keys {
					responses.add(InputMapperMessage::DoubleClick(match key {
						MouseKeys::LEFT => MouseButton::Left,
						MouseKeys::RIGHT => MouseButton::Right,
						MouseKeys::MIDDLE => MouseButton::Middle,
						_ => unimplemented!(),
					}));
				}
			}
			InputPreprocessorMessage::KeyDown { key, key_repeat, modifier_keys } => {
				self.update_states_of_modifier_keys(modifier_keys, keyboard_platform, responses);
				self.keyboard.set(key as usize);
				if !key_repeat {
					responses.add(InputMapperMessage::KeyDownNoRepeat(key));
				}
				responses.add(InputMapperMessage::KeyDown(key));
			}
			InputPreprocessorMessage::KeyUp { key, key_repeat, modifier_keys } => {
				self.update_states_of_modifier_keys(modifier_keys, keyboard_platform, responses);
				self.keyboard.unset(key as usize);
				if !key_repeat {
					responses.add(InputMapperMessage::KeyUpNoRepeat(key));
				}
				responses.add(InputMapperMessage::KeyUp(key));
			}
			InputPreprocessorMessage::PointerDown { editor_mouse_state, modifier_keys } => {
				self.update_states_of_modifier_keys(modifier_keys, keyboard_platform, responses);

				let mouse_state = editor_mouse_state.to_mouse_state(&self.viewport_bounds);
				self.mouse.position = mouse_state.position;

				self.translate_mouse_event(mouse_state, true, responses);
			}
			InputPreprocessorMessage::PointerMove { editor_mouse_state, modifier_keys } => {
				self.update_states_of_modifier_keys(modifier_keys, keyboard_platform, responses);

				let mouse_state = editor_mouse_state.to_mouse_state(&self.viewport_bounds);
				self.mouse.position = mouse_state.position;

				responses.add(InputMapperMessage::PointerMove);

				// While any pointer button is already down, additional button down events are not reported, but they are sent as `pointermove` events
				self.translate_mouse_event(mouse_state, false, responses);
			}
			InputPreprocessorMessage::PointerUp { editor_mouse_state, modifier_keys } => {
				self.update_states_of_modifier_keys(modifier_keys, keyboard_platform, responses);

				let mouse_state = editor_mouse_state.to_mouse_state(&self.viewport_bounds);
				self.mouse.position = mouse_state.position;

				self.translate_mouse_event(mouse_state, false, responses);
			}
			InputPreprocessorMessage::WheelScroll { editor_mouse_state, modifier_keys } => {
				self.update_states_of_modifier_keys(modifier_keys, keyboard_platform, responses);

				let mouse_state = editor_mouse_state.to_mouse_state(&self.viewport_bounds);
				self.mouse.position = mouse_state.position;
				self.mouse.scroll_delta = mouse_state.scroll_delta;

				responses.add(InputMapperMessage::WheelScroll);
			}
		};
	}

	// Clean user input and if possible reconstruct it.
	// Store the changes in the keyboard if it is a key event.
	// Transform canvas coordinates to document coordinates.
	advertise_actions!();
}

impl InputPreprocessorMessageHandler {
	fn translate_mouse_event(&mut self, mut new_state: MouseState, allow_first_button_down: bool, responses: &mut VecDeque<Message>) {
		for (bit_flag, key) in [(MouseKeys::LEFT, Key::Lmb), (MouseKeys::RIGHT, Key::Rmb), (MouseKeys::MIDDLE, Key::Mmb)] {
			// Calculate the intersection between the two key states
			let old_down = self.mouse.mouse_keys & bit_flag == bit_flag;
			let new_down = new_state.mouse_keys & bit_flag == bit_flag;
			if !old_down && new_down {
				if allow_first_button_down || self.mouse.mouse_keys != MouseKeys::empty() {
					responses.add(InputMapperMessage::KeyDown(key));
				} else {
					// Required to stop a keyup being emitted for a keydown outside canvas
					new_state.mouse_keys ^= bit_flag;
				}
			}
			if old_down && !new_down {
				responses.add(InputMapperMessage::KeyUp(key));
			}
		}

		self.mouse = new_state;
	}

	fn update_states_of_modifier_keys(&mut self, pressed_modifier_keys: ModifierKeys, keyboard_platform: KeyboardPlatformLayout, responses: &mut VecDeque<Message>) {
		let is_key_pressed = |key_to_check: ModifierKeys| pressed_modifier_keys.contains(key_to_check);

		// Update the state of the concrete modifier keys based on the source state
		self.update_modifier_key(Key::Shift, is_key_pressed(ModifierKeys::SHIFT), responses);
		self.update_modifier_key(Key::Alt, is_key_pressed(ModifierKeys::ALT), responses);
		self.update_modifier_key(Key::Control, is_key_pressed(ModifierKeys::CONTROL), responses);

		// Update the state of either the concrete Meta or the Command keys based on which one is applicable for this platform
		let meta_or_command = match keyboard_platform {
			KeyboardPlatformLayout::Mac => Key::Command,
			KeyboardPlatformLayout::Standard => Key::Meta,
		};
		self.update_modifier_key(meta_or_command, is_key_pressed(ModifierKeys::META_OR_COMMAND), responses);

		// Update the state of the virtual Accel key (the primary accelerator key) based on the source state of the Control or Command key, whichever is relevant on this platform
		let accel_virtual_key_state = match keyboard_platform {
			KeyboardPlatformLayout::Mac => is_key_pressed(ModifierKeys::META_OR_COMMAND),
			KeyboardPlatformLayout::Standard => is_key_pressed(ModifierKeys::CONTROL),
		};
		self.update_modifier_key(Key::Accel, accel_virtual_key_state, responses);
	}

	fn update_modifier_key(&mut self, key: Key, key_is_down: bool, responses: &mut VecDeque<Message>) {
		let key_was_down = self.keyboard.get(key as usize);

		if key_was_down && !key_is_down {
			self.keyboard.unset(key as usize);
			responses.add(InputMapperMessage::KeyUp(key));
		} else if !key_was_down && key_is_down {
			self.keyboard.set(key as usize);
			responses.add(InputMapperMessage::KeyDown(key));
		}
	}

	pub fn document_bounds(&self) -> [DVec2; 2] {
		// IPP bounds are relative to the entire application
		[(0., 0.).into(), self.viewport_bounds.bottom_right - self.viewport_bounds.top_left]
	}
}

#[cfg(test)]
mod test {
	use crate::messages::input_mapper::utility_types::input_keyboard::{Key, ModifierKeys};
	use crate::messages::input_mapper::utility_types::input_mouse::EditorMouseState;
	use crate::messages::portfolio::utility_types::KeyboardPlatformLayout;
	use crate::messages::prelude::*;

	#[test]
	fn process_action_mouse_move_handle_modifier_keys() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();

		let editor_mouse_state = EditorMouseState::from_editor_position(4., 809.);
		let modifier_keys = ModifierKeys::ALT;
		let message = InputPreprocessorMessage::PointerMove { editor_mouse_state, modifier_keys };

		let mut responses = VecDeque::new();

		input_preprocessor.process_message(message, &mut responses, KeyboardPlatformLayout::Standard);

		assert!(input_preprocessor.keyboard.get(Key::Alt as usize));
		assert_eq!(responses.pop_front(), Some(InputMapperMessage::KeyDown(Key::Alt).into()));
	}

	#[test]
	fn process_action_mouse_down_handle_modifier_keys() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();

		let editor_mouse_state = EditorMouseState::new();
		let modifier_keys = ModifierKeys::CONTROL;
		let message = InputPreprocessorMessage::PointerDown { editor_mouse_state, modifier_keys };

		let mut responses = VecDeque::new();

		input_preprocessor.process_message(message, &mut responses, KeyboardPlatformLayout::Standard);

		assert!(input_preprocessor.keyboard.get(Key::Control as usize));
		assert_eq!(responses.pop_front(), Some(InputMapperMessage::KeyDown(Key::Control).into()));
	}

	#[test]
	fn process_action_mouse_up_handle_modifier_keys() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();

		let editor_mouse_state = EditorMouseState::new();
		let modifier_keys = ModifierKeys::SHIFT;
		let message = InputPreprocessorMessage::PointerUp { editor_mouse_state, modifier_keys };

		let mut responses = VecDeque::new();

		input_preprocessor.process_message(message, &mut responses, KeyboardPlatformLayout::Standard);

		assert!(input_preprocessor.keyboard.get(Key::Shift as usize));
		assert_eq!(responses.pop_front(), Some(InputMapperMessage::KeyDown(Key::Shift).into()));
	}

	#[test]
	fn process_action_key_down_handle_modifier_keys() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();
		input_preprocessor.keyboard.set(Key::Control as usize);

		let key = Key::KeyA;
		let key_repeat = false;
		let modifier_keys = ModifierKeys::empty();
		let message = InputPreprocessorMessage::KeyDown { key, key_repeat, modifier_keys };

		let mut responses = VecDeque::new();

		input_preprocessor.process_message(message, &mut responses, KeyboardPlatformLayout::Standard);

		assert!(!input_preprocessor.keyboard.get(Key::Control as usize));
		assert_eq!(responses.pop_front(), Some(InputMapperMessage::KeyUp(Key::Control).into()));
	}

	#[test]
	fn process_action_key_up_handle_modifier_keys() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();

		let key = Key::KeyS;
		let key_repeat = false;
		let modifier_keys = ModifierKeys::CONTROL | ModifierKeys::SHIFT;
		let message = InputPreprocessorMessage::KeyUp { key, key_repeat, modifier_keys };

		let mut responses = VecDeque::new();

		input_preprocessor.process_message(message, &mut responses, KeyboardPlatformLayout::Standard);

		assert!(input_preprocessor.keyboard.get(Key::Control as usize));
		assert!(input_preprocessor.keyboard.get(Key::Shift as usize));
		assert!(responses.contains(&InputMapperMessage::KeyDown(Key::Control).into()));
		assert!(responses.contains(&InputMapperMessage::KeyDown(Key::Control).into()));
	}
}
