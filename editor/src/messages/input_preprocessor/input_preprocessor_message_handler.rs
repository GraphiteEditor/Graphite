use crate::application::Editor;
use crate::consts::DOUBLE_TAP_MILLISECONDS;
use crate::messages::input_mapper::utility_types::keyboard::{Key, KeyStates, ModifierKeys};
use crate::messages::input_mapper::utility_types::misc::FrameTimeInfo;
use crate::messages::input_mapper::utility_types::pointer::{MouseButton, MouseKeys, PointerState};
use crate::messages::prelude::*;
use std::time::Duration;

#[derive(ExtractField)]
pub struct InputPreprocessorMessageContext<'a> {
	pub viewport: &'a ViewportMessageHandler,
}

#[derive(Debug, Default, ExtractField)]
pub struct InputPreprocessorMessageHandler {
	pub frame_time: FrameTimeInfo,
	pub time: u64,
	pub keyboard: KeyStates,
	pub mouse: PointerState,
	pointer_down_time: f64,
	/// Double-tap detection state, timed as one gesture from the first tap's key-down.
	double_tap_state: DoubleTapState,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum DoubleTapState {
	#[default]
	Idle,
	/// The first press of the key, timed from its key-down.
	FirstTap { key: Key, start_time: u64 },
	/// The second press of the key is down, still timed from the first tap's key-down.
	SecondTap { key: Key, start_time: u64 },
}

#[message_handler_data]
impl<'a> MessageHandler<InputPreprocessorMessage, InputPreprocessorMessageContext<'a>> for InputPreprocessorMessageHandler {
	fn process_message(&mut self, message: InputPreprocessorMessage, responses: &mut VecDeque<Message>, context: InputPreprocessorMessageContext<'a>) {
		let InputPreprocessorMessageContext { viewport } = context;

		match message {
			InputPreprocessorMessage::DoubleClick { editor_mouse_state, modifier_keys } => {
				self.clear_double_tap_state();
				self.update_states_of_modifier_keys(modifier_keys, responses);

				let pointer_state = editor_mouse_state.to_pointer_state(viewport);
				self.mouse.position = pointer_state.position;

				for key in pointer_state.mouse_keys {
					responses.add(InputMapperMessage::DoubleClick(match key {
						MouseKeys::LEFT => MouseButton::Left,
						MouseKeys::RIGHT => MouseButton::Right,
						MouseKeys::MIDDLE => MouseButton::Middle,
						MouseKeys::BACK => MouseButton::Back,
						MouseKeys::FORWARD => MouseButton::Forward,
						_ => unimplemented!(),
					}));
				}
			}
			InputPreprocessorMessage::KeyDown { key, key_repeat, modifier_keys } => {
				self.update_states_of_modifier_keys(modifier_keys, responses);
				self.keyboard.set(key as usize);

				if !key_repeat {
					// A press that arrives with a mouse button or modifier held can neither open nor close a double tap,
					// and it breaks a first tap already waiting
					let interrupted = !self.mouse.mouse_keys.is_empty() || !modifier_keys.is_empty();

					self.double_tap_state = match (interrupted, self.double_tap_state) {
						(true, _) => DoubleTapState::Idle,
						(false, DoubleTapState::FirstTap { key: first_key, start_time }) if first_key == key && self.time.saturating_sub(start_time) < DOUBLE_TAP_MILLISECONDS => {
							DoubleTapState::SecondTap { key, start_time }
						}
						// A late second press starts a new first tap instead of carrying a pair that can't succeed,
						// so a slow tap followed by a quick double tap still works
						(false, _) => DoubleTapState::FirstTap { key, start_time: self.time },
					};

					responses.add(InputMapperMessage::KeyDownNoRepeat(key));
				}
				responses.add(InputMapperMessage::KeyDown(key));
			}
			InputPreprocessorMessage::KeyUp { key, key_repeat, modifier_keys } => {
				self.update_states_of_modifier_keys(modifier_keys, responses);
				self.keyboard.unset(key as usize);
				if !key_repeat {
					responses.add(InputMapperMessage::KeyUpNoRepeat(key));
				}
				if let DoubleTapState::SecondTap { key: tapped_key, start_time } = self.double_tap_state
					&& tapped_key == key
					&& self.mouse.mouse_keys.is_empty()
				{
					self.double_tap_state = DoubleTapState::Idle;

					// Note: `self.time` only advances on animation-frame ticks, so while the editor is stalled both taps can
					// share a stale timestamp. That makes a double tap easier to trigger, not harder, so no correction is applied.
					if self.time.saturating_sub(start_time) < DOUBLE_TAP_MILLISECONDS {
						responses.add(InputMapperMessage::DoubleTap(key));
					}
				}
				responses.add(InputMapperMessage::KeyUp(key));
			}
			InputPreprocessorMessage::PointerDown { editor_mouse_state, modifier_keys } => {
				self.clear_double_tap_state();
				self.update_states_of_modifier_keys(modifier_keys, responses);

				let pointer_state = editor_mouse_state.to_pointer_state(viewport);
				self.mouse.position = pointer_state.position;

				self.translate_mouse_event(pointer_state, true, responses);
			}
			InputPreprocessorMessage::PointerMove { editor_mouse_state, modifier_keys } => {
				self.update_states_of_modifier_keys(modifier_keys, responses);

				let pointer_state = editor_mouse_state.to_pointer_state(viewport);
				self.mouse.position = pointer_state.position;

				responses.add(InputMapperMessage::PointerMove);

				// While any pointer button is already down, additional button down events are not reported, but they are sent as `pointermove` events
				self.translate_mouse_event(pointer_state, false, responses);
			}
			InputPreprocessorMessage::PointerUp { editor_mouse_state, modifier_keys } => {
				self.clear_double_tap_state();
				self.update_states_of_modifier_keys(modifier_keys, responses);

				let pointer_state = editor_mouse_state.to_pointer_state(viewport);
				self.mouse.position = pointer_state.position;

				self.translate_mouse_event(pointer_state, false, responses);
			}
			InputPreprocessorMessage::PointerShake { editor_mouse_state, modifier_keys } => {
				self.update_states_of_modifier_keys(modifier_keys, responses);

				let pointer_state = editor_mouse_state.to_pointer_state(viewport);
				self.mouse.position = pointer_state.position;

				responses.add(InputMapperMessage::PointerShake);
			}
			InputPreprocessorMessage::CurrentTime { timestamp } => {
				responses.add(AnimationMessage::SetTime { time: timestamp as f64 });
				self.time = timestamp;
				self.frame_time.advance_timestamp(Duration::from_millis(timestamp));
			}
			InputPreprocessorMessage::WheelScroll { editor_mouse_state, modifier_keys } => {
				self.clear_double_tap_state();
				self.update_states_of_modifier_keys(modifier_keys, responses);

				let pointer_state = editor_mouse_state.to_pointer_state(viewport);
				self.mouse.position = pointer_state.position;
				self.mouse.scroll_delta = pointer_state.scroll_delta;

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
	fn clear_double_tap_state(&mut self) {
		self.double_tap_state = DoubleTapState::Idle;
	}

	fn translate_mouse_event(&mut self, mut new_state: PointerState, allow_first_button_down: bool, responses: &mut VecDeque<Message>) {
		let click_mappings = [
			(MouseKeys::LEFT, Key::MouseLeft),
			(MouseKeys::RIGHT, Key::MouseRight),
			(MouseKeys::MIDDLE, Key::MouseMiddle),
			(MouseKeys::BACK, Key::MouseBack),
			(MouseKeys::FORWARD, Key::MouseForward),
		];

		for (bit_flag, key) in click_mappings {
			// Calculate the intersection between the two key states
			let old_down = self.mouse.mouse_keys & bit_flag == bit_flag;
			let new_down = new_state.mouse_keys & bit_flag == bit_flag;
			if !old_down && new_down {
				if allow_first_button_down || self.mouse.mouse_keys != MouseKeys::empty() {
					self.keyboard.set(key as usize);
					responses.add(InputMapperMessage::KeyDown(key));
				} else {
					// Required to stop a keyup being emitted for a keydown outside canvas
					new_state.mouse_keys ^= bit_flag;
				}
			}
			if old_down && !new_down {
				self.keyboard.unset(key as usize);
				responses.add(InputMapperMessage::KeyUp(key));
			}
		}

		let raw_time = new_state.time.unwrap_or(self.time as f64);
		if self.mouse.mouse_keys == MouseKeys::empty() {
			self.pointer_down_time = raw_time;
		}
		let interacting = (self.mouse.mouse_keys | new_state.mouse_keys) != MouseKeys::empty();
		new_state.time = interacting.then_some(raw_time - self.pointer_down_time);

		self.mouse = new_state;
	}

	fn update_states_of_modifier_keys(&mut self, pressed_modifier_keys: ModifierKeys, responses: &mut VecDeque<Message>) {
		let is_key_pressed = |key_to_check: ModifierKeys| pressed_modifier_keys.contains(key_to_check);

		// Update the state of the concrete modifier keys based on the source state
		self.update_modifier_key(Key::Shift, is_key_pressed(ModifierKeys::SHIFT), responses);
		self.update_modifier_key(Key::Alt, is_key_pressed(ModifierKeys::ALT), responses);
		self.update_modifier_key(Key::Control, is_key_pressed(ModifierKeys::CONTROL), responses);

		// Update the state of either the concrete Meta or the Command keys based on which one is applicable for this platform
		let meta_or_command = match Editor::environment().is_mac() {
			true => Key::Command,
			false => Key::Meta,
		};
		self.update_modifier_key(meta_or_command, is_key_pressed(ModifierKeys::META_OR_COMMAND), responses);

		// Update the state of the virtual Accel key (the primary accelerator key) based on the source state of the Control or Command key, whichever is relevant on this platform
		let accel_virtual_key_state = match Editor::environment().is_mac() {
			true => is_key_pressed(ModifierKeys::META_OR_COMMAND),
			false => is_key_pressed(ModifierKeys::CONTROL),
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
}

#[cfg(test)]
mod test {
	use super::DoubleTapState;
	use crate::consts::DOUBLE_TAP_MILLISECONDS;
	use crate::messages::input_mapper::utility_types::keyboard::{Key, ModifierKeys};
	use crate::messages::input_mapper::utility_types::pointer::{EditorPointerState, MouseKeys};
	use crate::messages::prelude::*;

	#[test]
	fn process_action_mouse_move_handle_modifier_keys() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();

		let editor_mouse_state = EditorPointerState {
			editor_position: (4., 809.).into(),
			..Default::default()
		};
		let modifier_keys = ModifierKeys::ALT;
		let message = InputPreprocessorMessage::PointerMove { editor_mouse_state, modifier_keys };

		let mut responses = VecDeque::new();

		let context = InputPreprocessorMessageContext {
			viewport: &ViewportMessageHandler::default(),
		};
		input_preprocessor.process_message(message, &mut responses, context);

		assert!(input_preprocessor.keyboard.get(Key::Alt as usize));
		assert_eq!(responses.pop_front(), Some(InputMapperMessage::KeyDown(Key::Alt).into()));
	}

	#[test]
	fn process_action_mouse_down_handle_modifier_keys() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();

		let editor_mouse_state = EditorPointerState::default();
		let modifier_keys = ModifierKeys::CONTROL;
		let message = InputPreprocessorMessage::PointerDown { editor_mouse_state, modifier_keys };

		let mut responses = VecDeque::new();

		let context = InputPreprocessorMessageContext {
			viewport: &ViewportMessageHandler::default(),
		};
		input_preprocessor.process_message(message, &mut responses, context);

		assert!(input_preprocessor.keyboard.get(Key::Control as usize));
		assert_eq!(responses.pop_front(), Some(InputMapperMessage::KeyDown(Key::Control).into()));
	}

	#[test]
	fn process_action_mouse_up_handle_modifier_keys() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();

		let editor_mouse_state = EditorPointerState::default();
		let modifier_keys = ModifierKeys::SHIFT;
		let message = InputPreprocessorMessage::PointerUp { editor_mouse_state, modifier_keys };

		let mut responses = VecDeque::new();

		let context = InputPreprocessorMessageContext {
			viewport: &ViewportMessageHandler::default(),
		};
		input_preprocessor.process_message(message, &mut responses, context);

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

		let context = InputPreprocessorMessageContext {
			viewport: &ViewportMessageHandler::default(),
		};
		input_preprocessor.process_message(message, &mut responses, context);

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

		let context = InputPreprocessorMessageContext {
			viewport: &ViewportMessageHandler::default(),
		};
		input_preprocessor.process_message(message, &mut responses, context);

		assert!(input_preprocessor.keyboard.get(Key::Control as usize));
		assert!(input_preprocessor.keyboard.get(Key::Shift as usize));
		assert!(responses.contains(&InputMapperMessage::KeyDown(Key::Control).into()));
		assert!(responses.contains(&InputMapperMessage::KeyDown(Key::Control).into()));
	}

	fn key_down(input_preprocessor: &mut InputPreprocessorMessageHandler, key: Key, responses: &mut VecDeque<Message>) {
		process_input(
			input_preprocessor,
			InputPreprocessorMessage::KeyDown {
				key,
				key_repeat: false,
				modifier_keys: ModifierKeys::empty(),
			},
			responses,
		);
	}

	fn key_up(input_preprocessor: &mut InputPreprocessorMessageHandler, key: Key, responses: &mut VecDeque<Message>) {
		process_input(
			input_preprocessor,
			InputPreprocessorMessage::KeyUp {
				key,
				key_repeat: false,
				modifier_keys: ModifierKeys::empty(),
			},
			responses,
		);
	}

	fn tap(input_preprocessor: &mut InputPreprocessorMessageHandler, key: Key, time: u64, responses: &mut VecDeque<Message>) {
		input_preprocessor.time = time;
		key_down(input_preprocessor, key, responses);
		key_up(input_preprocessor, key, responses);
	}

	fn process_input(input_preprocessor: &mut InputPreprocessorMessageHandler, message: InputPreprocessorMessage, responses: &mut VecDeque<Message>) {
		input_preprocessor.process_message(
			message,
			responses,
			InputPreprocessorMessageContext {
				viewport: &ViewportMessageHandler::default(),
			},
		);
	}

	#[test]
	fn process_double_tap_within_threshold() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();
		let mut responses = VecDeque::new();

		tap(&mut input_preprocessor, Key::Space, 0, &mut responses);
		responses.clear();

		input_preprocessor.time = 50;
		key_down(&mut input_preprocessor, Key::Space, &mut responses);

		assert!(!responses.contains(&InputMapperMessage::DoubleTap(Key::Space).into()));
		// The stored time stays at the first tap's key-down, since the whole gesture is timed from there
		assert_eq!(input_preprocessor.double_tap_state, DoubleTapState::SecondTap { key: Key::Space, start_time: 0 });

		responses.clear();
		key_up(&mut input_preprocessor, Key::Space, &mut responses);

		assert!(responses.contains(&InputMapperMessage::DoubleTap(Key::Space).into()));
		assert_eq!(input_preprocessor.double_tap_state, DoubleTapState::Idle);
	}

	#[test]
	fn process_double_tap_outside_threshold() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();
		let mut responses = VecDeque::new();

		tap(&mut input_preprocessor, Key::Space, 0, &mut responses);
		responses.clear();

		// A second press that already arrives too late starts a new first tap instead of completing the pair
		input_preprocessor.time = DOUBLE_TAP_MILLISECONDS + 1;
		key_down(&mut input_preprocessor, Key::Space, &mut responses);

		assert!(!responses.contains(&InputMapperMessage::DoubleTap(Key::Space).into()));
		assert_eq!(
			input_preprocessor.double_tap_state,
			DoubleTapState::FirstTap {
				key: Key::Space,
				start_time: DOUBLE_TAP_MILLISECONDS + 1
			}
		);

		responses.clear();
		key_up(&mut input_preprocessor, Key::Space, &mut responses);

		assert!(!responses.contains(&InputMapperMessage::DoubleTap(Key::Space).into()));
	}

	#[test]
	fn process_double_tap_exceeding_total_duration_fails() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();
		let mut responses = VecDeque::new();

		tap(&mut input_preprocessor, Key::Space, 0, &mut responses);
		responses.clear();

		input_preprocessor.time = 50;
		key_down(&mut input_preprocessor, Key::Space, &mut responses);
		responses.clear();

		// The whole gesture from the first key-down to the second key-up must fit within the limit
		input_preprocessor.time = 50 + DOUBLE_TAP_MILLISECONDS + 1;
		key_up(&mut input_preprocessor, Key::Space, &mut responses);

		assert!(!responses.contains(&InputMapperMessage::DoubleTap(Key::Space).into()));
	}

	#[test]
	fn process_slow_tap_then_quick_double_tap_still_works() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();
		let mut responses = VecDeque::new();

		tap(&mut input_preprocessor, Key::Space, 0, &mut responses);
		responses.clear();

		// Too slow to pair with the first tap, so this becomes a new first tap of its own
		tap(&mut input_preprocessor, Key::Space, DOUBLE_TAP_MILLISECONDS + 1, &mut responses);
		assert!(!responses.contains(&InputMapperMessage::DoubleTap(Key::Space).into()));
		responses.clear();

		// A quick pair after that still registers
		tap(&mut input_preprocessor, Key::Space, DOUBLE_TAP_MILLISECONDS + 51, &mut responses);
		assert!(responses.contains(&InputMapperMessage::DoubleTap(Key::Space).into()));
	}

	#[test]
	fn process_double_tap_interrupted_by_pointer_down() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();
		let mut responses = VecDeque::new();

		tap(&mut input_preprocessor, Key::Space, 0, &mut responses);
		responses.clear();

		input_preprocessor.time = 50;
		key_down(&mut input_preprocessor, Key::Space, &mut responses);
		responses.clear();

		process_input(
			&mut input_preprocessor,
			InputPreprocessorMessage::PointerDown {
				editor_mouse_state: EditorPointerState::default(),
				modifier_keys: ModifierKeys::empty(),
			},
			&mut responses,
		);
		responses.clear();

		key_up(&mut input_preprocessor, Key::Space, &mut responses);

		assert!(!responses.contains(&InputMapperMessage::DoubleTap(Key::Space).into()));
	}

	#[test]
	fn process_double_tap_not_interrupted_by_mouse_movement() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();
		let mut responses = VecDeque::new();

		tap(&mut input_preprocessor, Key::Space, 0, &mut responses);
		responses.clear();

		input_preprocessor.time = 50;
		key_down(&mut input_preprocessor, Key::Space, &mut responses);
		responses.clear();

		process_input(
			&mut input_preprocessor,
			InputPreprocessorMessage::PointerMove {
				editor_mouse_state: EditorPointerState::default(),
				modifier_keys: ModifierKeys::empty(),
			},
			&mut responses,
		);
		responses.clear();

		key_up(&mut input_preprocessor, Key::Space, &mut responses);

		assert!(responses.contains(&InputMapperMessage::DoubleTap(Key::Space).into()));
	}

	#[test]
	fn process_double_tap_blocked_by_mouse_button_held() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();
		let mut responses = VecDeque::new();

		input_preprocessor.mouse.mouse_keys = MouseKeys::LEFT;

		tap(&mut input_preprocessor, Key::Space, 0, &mut responses);
		responses.clear();

		input_preprocessor.time = 50;
		key_down(&mut input_preprocessor, Key::Space, &mut responses);

		assert_eq!(input_preprocessor.double_tap_state, DoubleTapState::Idle);

		responses.clear();
		key_up(&mut input_preprocessor, Key::Space, &mut responses);

		assert!(!responses.contains(&InputMapperMessage::DoubleTap(Key::Space).into()));
	}

	#[test]
	fn process_double_tap_modified_first_press_never_arms() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();
		let mut responses = VecDeque::new();

		// Press Shift+Space first, as for live preview, then a plain Space within the threshold.
		process_input(
			&mut input_preprocessor,
			InputPreprocessorMessage::KeyDown {
				key: Key::Space,
				key_repeat: false,
				modifier_keys: ModifierKeys::SHIFT,
			},
			&mut responses,
		);
		assert_eq!(
			input_preprocessor.double_tap_state,
			DoubleTapState::Idle,
			"a modified first press should leave no double-tap state at all"
		);

		tap(&mut input_preprocessor, Key::Space, 50, &mut responses);

		assert!(!responses.contains(&InputMapperMessage::DoubleTap(Key::Space).into()));
	}

	#[test]
	fn process_double_tap_reset_by_a_different_key() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();
		let mut responses = VecDeque::new();

		tap(&mut input_preprocessor, Key::Space, 0, &mut responses);
		responses.clear();

		// A different key between the taps makes the next Space press start a fresh pair.
		tap(&mut input_preprocessor, Key::KeyA, 50, &mut responses);
		responses.clear();

		input_preprocessor.time = 100;
		key_down(&mut input_preprocessor, Key::Space, &mut responses);
		assert_eq!(
			input_preprocessor.double_tap_state,
			DoubleTapState::FirstTap { key: Key::Space, start_time: 100 },
			"the intervening key should have reset the detector"
		);

		responses.clear();
		key_up(&mut input_preprocessor, Key::Space, &mut responses);
		assert!(!responses.contains(&InputMapperMessage::DoubleTap(Key::Space).into()));
	}

	#[test]
	fn process_double_tap_blocked_by_a_modifier_key() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();
		let mut responses = VecDeque::new();

		tap(&mut input_preprocessor, Key::Space, 0, &mut responses);
		responses.clear();

		// Shift+Space is its own shortcut, so a modifier held on the second tap suppresses the double tap.
		input_preprocessor.time = 50;
		process_input(
			&mut input_preprocessor,
			InputPreprocessorMessage::KeyDown {
				key: Key::Space,
				key_repeat: false,
				modifier_keys: ModifierKeys::SHIFT,
			},
			&mut responses,
		);

		assert_eq!(input_preprocessor.double_tap_state, DoubleTapState::Idle);

		responses.clear();
		key_up(&mut input_preprocessor, Key::Space, &mut responses);
		assert!(!responses.contains(&InputMapperMessage::DoubleTap(Key::Space).into()));
	}

	#[test]
	fn process_double_tap_ignores_key_repeat() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();
		let mut responses = VecDeque::new();

		tap(&mut input_preprocessor, Key::Space, 0, &mut responses);
		responses.clear();

		// A held key's auto-repeat must not read as a second tap.
		input_preprocessor.time = 50;
		process_input(
			&mut input_preprocessor,
			InputPreprocessorMessage::KeyDown {
				key: Key::Space,
				key_repeat: true,
				modifier_keys: ModifierKeys::empty(),
			},
			&mut responses,
		);

		assert_eq!(input_preprocessor.double_tap_state, DoubleTapState::FirstTap { key: Key::Space, start_time: 0 });

		responses.clear();
		key_up(&mut input_preprocessor, Key::Space, &mut responses);
		assert!(!responses.contains(&InputMapperMessage::DoubleTap(Key::Space).into()));
	}
}
