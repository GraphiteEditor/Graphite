use crate::application::Editor;
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::keyboard::{Key, KeyStates, ModifierKeys};
use crate::messages::input_mapper::utility_types::misc::FrameTimeInfo;
use crate::messages::input_mapper::utility_types::pointer::{MouseButton, MouseKeys, PointerState, ViewportPosition};
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
	software_cursor: Option<SoftwareCursor>,
}

// The cursor G/R/S draws while the OS cursor is locked in place
#[derive(Debug, Clone, Copy)]
struct SoftwareCursor {
	position: ViewportPosition,
	last_absolute: ViewportPosition,
	// Set by a locked delta; the next absolute report is the OS cursor coming back, not movement
	locked_delta_seen: bool,
}

#[message_handler_data]
impl<'a> MessageHandler<InputPreprocessorMessage, InputPreprocessorMessageContext<'a>> for InputPreprocessorMessageHandler {
	fn process_message(&mut self, message: InputPreprocessorMessage, responses: &mut VecDeque<Message>, context: InputPreprocessorMessageContext<'a>) {
		let InputPreprocessorMessageContext { viewport } = context;

		match message {
			InputPreprocessorMessage::DoubleClick { editor_mouse_state, modifier_keys } => {
				self.update_states_of_modifier_keys(modifier_keys, responses);

				let pointer_state = editor_mouse_state.to_pointer_state(viewport);
				self.apply_pointer_position(pointer_state.position);
				self.send_software_cursor(viewport, responses);

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
				responses.add(InputMapperMessage::KeyUp(key));
			}
			InputPreprocessorMessage::PointerDown { editor_mouse_state, modifier_keys } => {
				self.update_states_of_modifier_keys(modifier_keys, responses);

				let mut pointer_state = editor_mouse_state.to_pointer_state(viewport);
				pointer_state.position = self.apply_pointer_position(pointer_state.position);
				self.send_software_cursor(viewport, responses);

				self.translate_mouse_event(pointer_state, true, responses);
			}
			InputPreprocessorMessage::PointerMove { editor_mouse_state, modifier_keys } => {
				self.update_states_of_modifier_keys(modifier_keys, responses);

				let mut pointer_state = editor_mouse_state.to_pointer_state(viewport);
				pointer_state.position = self.apply_pointer_position(pointer_state.position);
				self.send_software_cursor(viewport, responses);

				responses.add(InputMapperMessage::PointerMove);

				// While any pointer button is already down, additional button down events are not reported, but they are sent as `pointermove` events
				self.translate_mouse_event(pointer_state, false, responses);
			}
			InputPreprocessorMessage::PointerUp { editor_mouse_state, modifier_keys } => {
				self.update_states_of_modifier_keys(modifier_keys, responses);

				let mut pointer_state = editor_mouse_state.to_pointer_state(viewport);
				pointer_state.position = self.apply_pointer_position(pointer_state.position);
				self.send_software_cursor(viewport, responses);

				self.translate_mouse_event(pointer_state, false, responses);
			}
			InputPreprocessorMessage::PointerShake { editor_mouse_state, modifier_keys } => {
				self.update_states_of_modifier_keys(modifier_keys, responses);

				let pointer_state = editor_mouse_state.to_pointer_state(viewport);
				self.apply_pointer_position(pointer_state.position);
				self.send_software_cursor(viewport, responses);

				responses.add(InputMapperMessage::PointerShake);
			}
			InputPreprocessorMessage::PointerLockMove { delta } => {
				let Some(cursor) = &mut self.software_cursor else { return };

				cursor.position += delta;
				cursor.locked_delta_seen = true;
				self.mouse.position = cursor.position;
				self.send_software_cursor(viewport, responses);

				responses.add(InputMapperMessage::PointerMove);
			}
			InputPreprocessorMessage::BeginSoftwareCursor { position } => {
				if self.software_cursor.is_some() {
					return;
				}

				self.software_cursor = Some(SoftwareCursor {
					position,
					last_absolute: position,
					locked_delta_seen: false,
				});
				self.mouse.position = position;

				self.send_software_cursor(viewport, responses);
				responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::None });
				responses.add(AppWindowMessage::PointerLock);
			}
			InputPreprocessorMessage::EndSoftwareCursor => {
				let Some(cursor) = self.software_cursor.take() else { return };

				responses.add(FrontendMessage::UpdateSoftwareCursor { visible: false, x: 0., y: 0. });
				// Let the active tool re-emit its cursor
				responses.add(ToolMessage::UpdateCursor);
				// Put the pointer where the software cursor ended, not where the lock began
				let position = wrap_software_cursor(cursor.position, viewport.size().into_dvec2());
				responses.add(AppWindowMessage::PointerUnlock { x: position.x, y: position.y });
			}
			InputPreprocessorMessage::CurrentTime { timestamp } => {
				responses.add(AnimationMessage::SetTime { time: timestamp as f64 });
				self.time = timestamp;
				self.frame_time.advance_timestamp(Duration::from_millis(timestamp));
			}
			InputPreprocessorMessage::WheelScroll { editor_mouse_state, modifier_keys } => {
				self.update_states_of_modifier_keys(modifier_keys, responses);

				let pointer_state = editor_mouse_state.to_pointer_state(viewport);
				self.apply_pointer_position(pointer_state.position);
				self.send_software_cursor(viewport, responses);
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
	// The pointer position an event reports, which the software cursor maps through while it's active
	fn apply_pointer_position(&mut self, reported: ViewportPosition) -> ViewportPosition {
		let position = self.update_pointer_position(reported);
		self.mouse.position = position;
		position
	}

	// Advances the tracked pointer by the reported motion, or by the locked deltas while G/R/S has it wrapped
	fn update_pointer_position(&mut self, reported: ViewportPosition) -> ViewportPosition {
		let Some(cursor) = &mut self.software_cursor else { return reported };

		let motion = reported - cursor.last_absolute;
		cursor.last_absolute = reported;

		if motion != ViewportPosition::ZERO {
			// The OS cursor reappears when the lock is lost, and that jump is not movement
			if cursor.locked_delta_seen {
				cursor.locked_delta_seen = false;
			} else {
				cursor.position += motion;
			}
		}

		cursor.position
	}

	// Tells the frontend where to draw the software cursor
	fn send_software_cursor(&self, viewport: &ViewportMessageHandler, responses: &mut VecDeque<Message>) {
		let Some(cursor) = self.software_cursor else { return };

		let position = wrap_software_cursor(cursor.position, viewport.size().into_dvec2());
		responses.add(FrontendMessage::UpdateSoftwareCursor {
			visible: true,
			x: position.x,
			y: position.y,
		});
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

/// Wraps a software cursor position into the viewport bounds.
fn wrap_software_cursor(position: ViewportPosition, size: ViewportPosition) -> ViewportPosition {
	if size.x > 0. && size.y > 0. { position.rem_euclid(size) } else { position }
}

#[cfg(test)]
mod test {
	use crate::messages::input_mapper::utility_types::keyboard::{Key, ModifierKeys};
	use crate::messages::input_mapper::utility_types::pointer::{EditorPointerState, ViewportPosition};
	use crate::messages::prelude::*;

	fn pointer_state(x: f64, y: f64) -> EditorPointerState {
		EditorPointerState {
			editor_position: ViewportPosition::new(x, y),
			..Default::default()
		}
	}

	#[test]
	fn software_cursor_tracks_locked_deltas_and_ignores_the_reported_position() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();
		let viewport = ViewportMessageHandler::default();
		let mut responses = VecDeque::new();
		let context = || InputPreprocessorMessageContext { viewport: &viewport };
		let moved = |x: f64, y: f64| InputPreprocessorMessage::PointerMove {
			editor_mouse_state: pointer_state(x, y),
			modifier_keys: ModifierKeys::empty(),
		};

		let begin = InputPreprocessorMessage::BeginSoftwareCursor {
			position: ViewportPosition::new(10., 10.),
		};
		input_preprocessor.process_message(begin, &mut responses, context());

		input_preprocessor.process_message(moved(10., 10.), &mut responses, context());
		assert_eq!(input_preprocessor.mouse.position, ViewportPosition::new(10., 10.), "a frozen locked pointer shouldn't move the pointer");

		let delta = InputPreprocessorMessage::PointerLockMove {
			delta: ViewportPosition::new(25., -5.),
		};
		input_preprocessor.process_message(delta, &mut responses, context());
		assert_eq!(input_preprocessor.mouse.position, ViewportPosition::new(35., 5.), "a locked delta should move the pointer");
		assert!(
			responses.contains(&FrontendMessage::UpdateSoftwareCursor { visible: true, x: 35., y: 5. }.into()),
			"a locked delta should move the drawn cursor"
		);

		input_preprocessor.process_message(moved(10., 10.), &mut responses, context());
		assert_eq!(
			input_preprocessor.mouse.position,
			ViewportPosition::new(35., 5.),
			"the frozen position is still frozen after a locked delta"
		);

		input_preprocessor.process_message(moved(0., 0.), &mut responses, context());
		assert_eq!(
			input_preprocessor.mouse.position,
			ViewportPosition::new(35., 5.),
			"the OS cursor coming back shouldn't move the pointer"
		);

		input_preprocessor.process_message(moved(0., 4.), &mut responses, context());
		assert_eq!(input_preprocessor.mouse.position, ViewportPosition::new(35., 9.), "movement should track from the restored position");
	}

	#[test]
	fn pointer_buttons_keep_the_wrapped_position_while_the_software_cursor_is_active() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();
		let viewport = ViewportMessageHandler::default();
		let mut responses = VecDeque::new();
		let context = || InputPreprocessorMessageContext { viewport: &viewport };

		let begin = InputPreprocessorMessage::BeginSoftwareCursor {
			position: ViewportPosition::new(10., 10.),
		};
		input_preprocessor.process_message(begin, &mut responses, context());
		input_preprocessor.process_message(
			InputPreprocessorMessage::PointerLockMove {
				delta: ViewportPosition::new(25., -5.),
			},
			&mut responses,
			context(),
		);
		assert_eq!(input_preprocessor.mouse.position, ViewportPosition::new(35., 5.));

		// The OS cursor is still pinned to the lock origin, which is no longer where the tracked pointer is
		let frozen = || pointer_state(10., 10.);
		let keys = ModifierKeys::empty();
		for message in [
			InputPreprocessorMessage::PointerDown {
				editor_mouse_state: frozen(),
				modifier_keys: keys,
			},
			InputPreprocessorMessage::PointerUp {
				editor_mouse_state: frozen(),
				modifier_keys: keys,
			},
			InputPreprocessorMessage::PointerShake {
				editor_mouse_state: frozen(),
				modifier_keys: keys,
			},
			InputPreprocessorMessage::WheelScroll {
				editor_mouse_state: frozen(),
				modifier_keys: keys,
			},
			InputPreprocessorMessage::DoubleClick {
				editor_mouse_state: frozen(),
				modifier_keys: keys,
			},
		] {
			input_preprocessor.process_message(message, &mut responses, context());
			assert_eq!(input_preprocessor.mouse.position, ViewportPosition::new(35., 5.));
		}
	}

	#[test]
	fn test_wrap_software_cursor() {
		let size = ViewportPosition::new(100., 50.);
		assert_eq!(super::wrap_software_cursor(ViewportPosition::new(10., 20.), size), ViewportPosition::new(10., 20.));
		assert_eq!(super::wrap_software_cursor(ViewportPosition::new(110., -10.), size), ViewportPosition::new(10., 40.));
		assert_eq!(
			super::wrap_software_cursor(ViewportPosition::new(10., 20.), ViewportPosition::ZERO),
			ViewportPosition::new(10., 20.),
			"a zero-sized viewport shouldn't move anything"
		);
	}

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
}
