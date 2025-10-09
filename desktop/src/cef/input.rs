use cef::sys::{cef_event_flags_t, cef_key_event_type_t, cef_mouse_button_type_t};
use cef::{Browser, ImplBrowser, ImplBrowserHost, KeyEvent, KeyEventType, MouseEvent};
use std::time::Instant;
use winit::dpi::PhysicalPosition;
use winit::event::{ButtonSource, ElementState, MouseButton, MouseScrollDelta, WindowEvent};

mod keymap;
use keymap::{ToNativeKeycode, ToVKBits};

use super::consts::{MULTICLICK_ALLOWED_TRAVEL, MULTICLICK_TIMEOUT, PINCH_ZOOM_SPEED, SCROLL_LINE_HEIGHT, SCROLL_LINE_WIDTH, SCROLL_SPEED_X, SCROLL_SPEED_Y};

pub(crate) fn handle_window_event(browser: &Browser, input_state: &mut InputState, event: &WindowEvent) {
	match event {
		WindowEvent::PointerMoved { position, .. } | WindowEvent::PointerEntered { position, .. } => {
			input_state.cursor_move(position);

			let Some(host) = browser.host() else { return };
			host.send_mouse_move_event(Some(&input_state.into()), 0);
		}
		WindowEvent::PointerLeft { position, .. } => {
			if let Some(position) = position {
				input_state.cursor_move(position);
			}

			let Some(host) = browser.host() else { return };
			host.send_mouse_move_event(Some(&input_state.into()), 1);
		}
		WindowEvent::PointerButton { state, button, .. } => {
			let mouse_button = match button {
				ButtonSource::Mouse(mouse_button) => mouse_button,
				_ => {
					return; // TODO: Handle touch input
				}
			};

			let cef_click_count = input_state.mouse_input(mouse_button, state).into();
			let cef_mouse_up = match state {
				ElementState::Pressed => 0,
				ElementState::Released => 1,
			};
			let cef_button = match mouse_button {
				MouseButton::Left => cef::MouseButtonType::from(cef_mouse_button_type_t::MBT_LEFT),
				MouseButton::Right => cef::MouseButtonType::from(cef_mouse_button_type_t::MBT_RIGHT),
				MouseButton::Middle => cef::MouseButtonType::from(cef_mouse_button_type_t::MBT_MIDDLE),
				_ => return, //TODO: Handle Forward and Back button
			};

			let Some(host) = browser.host() else { return };
			host.send_mouse_click_event(Some(&input_state.into()), cef_button, cef_mouse_up, cef_click_count);
		}
		WindowEvent::MouseWheel { delta, phase: _, device_id: _, .. } => {
			let mouse_event = input_state.into();
			let (mut delta_x, mut delta_y) = match delta {
				MouseScrollDelta::LineDelta(x, y) => (x * SCROLL_LINE_WIDTH as f32, y * SCROLL_LINE_HEIGHT as f32),
				MouseScrollDelta::PixelDelta(physical_position) => (physical_position.x as f32, physical_position.y as f32),
			};
			delta_x *= SCROLL_SPEED_X;
			delta_y *= SCROLL_SPEED_Y;

			let Some(host) = browser.host() else { return };
			host.send_mouse_wheel_event(Some(&mouse_event), delta_x as i32, delta_y as i32);
		}
		WindowEvent::ModifiersChanged(modifiers) => {
			input_state.modifiers_changed(&modifiers.state());
		}
		WindowEvent::KeyboardInput { device_id: _, event, is_synthetic: _ } => {
			let (named_key, character) = match &event.logical_key {
				winit::keyboard::Key::Named(named_key) => (
					Some(named_key),
					match named_key {
						winit::keyboard::NamedKey::Enter => Some('\u{000d}'),
						_ => None,
					},
				),
				winit::keyboard::Key::Character(str) => {
					let char = str.chars().next().unwrap_or('\0');
					(None, Some(char))
				}
				_ => return,
			};

			let native_key_code = event.physical_key.to_native_keycode();

			let modifiers = input_state.cef_modifiers(&event.location, event.repeat).raw();

			let mut key_event = KeyEvent {
				size: size_of::<KeyEvent>(),
				modifiers,
				..Default::default()
			};

			if let Some(named_key) = named_key {
				key_event.windows_key_code = named_key.to_vk_bits();
			} else if let Some(char) = character {
				key_event.windows_key_code = char.to_vk_bits();
			}

			key_event.native_key_code = native_key_code;

			let Some(host) = browser.host() else { return };

			match event.state {
				ElementState::Pressed => {
					key_event.type_ = KeyEventType::from(cef_key_event_type_t::KEYEVENT_RAWKEYDOWN);
					host.send_key_event(Some(&key_event));

					if let Some(char) = character {
						let mut char_key_event = KeyEvent {
							size: size_of::<KeyEvent>(),
							modifiers,
							is_system_key: 0,
							..Default::default()
						};
						let mut buf = [0; 2];
						char.encode_utf16(&mut buf);
						char_key_event.windows_key_code = buf[0] as i32;
						char_key_event.character = buf[0];
						char_key_event.native_key_code = native_key_code;
						let mut buf = [0; 2];
						char.to_lowercase().next().unwrap().encode_utf16(&mut buf);
						char_key_event.unmodified_character = buf[0];
						char_key_event.type_ = KeyEventType::from(cef_key_event_type_t::KEYEVENT_CHAR);
						host.send_key_event(Some(&char_key_event));
					}
				}
				ElementState::Released => {
					key_event.type_ = KeyEventType::from(cef_key_event_type_t::KEYEVENT_KEYUP);
					host.send_key_event(Some(&key_event));
				}
			}
		}
		WindowEvent::PinchGesture { delta, .. } => {
			if !delta.is_normal() {
				return;
			}
			let Some(host) = browser.host() else { return };

			let mut mouse_event: MouseEvent = input_state.into();
			mouse_event.modifiers |= cef_event_flags_t::EVENTFLAG_CONTROL_DOWN as u32;
			mouse_event.modifiers |= cef_event_flags_t::EVENTFLAG_PRECISION_SCROLLING_DELTA as u32;

			let delta = (delta * PINCH_ZOOM_SPEED).round() as i32;

			host.send_mouse_wheel_event(Some(&mouse_event), 0, delta);
		}
		_ => {}
	}
}

#[derive(Default)]
pub(crate) struct InputState {
	modifiers: winit::keyboard::ModifiersState,
	mouse_position: MousePosition,
	mouse_state: MouseState,
	mouse_click_tracker: ClickTracker,
}
impl InputState {
	fn modifiers_changed(&mut self, modifiers: &winit::keyboard::ModifiersState) {
		self.modifiers = *modifiers;
	}

	fn cursor_move(&mut self, position: &PhysicalPosition<f64>) {
		self.mouse_position = position.into();
	}

	fn mouse_input(&mut self, button: &MouseButton, state: &ElementState) -> ClickCount {
		self.mouse_state.update(button, state);
		self.mouse_click_tracker.input(button, state, self.mouse_position)
	}

	fn cef_modifiers(&self, location: &winit::keyboard::KeyLocation, is_repeat: bool) -> CefModifiers {
		CefModifiers::new(self, location, is_repeat)
	}

	fn cef_modifiers_mouse_event(&self) -> CefModifiers {
		self.cef_modifiers(&winit::keyboard::KeyLocation::Standard, false)
	}
}

impl From<InputState> for CefModifiers {
	fn from(val: InputState) -> Self {
		CefModifiers::new(&val, &winit::keyboard::KeyLocation::Standard, false)
	}
}
impl From<&InputState> for MouseEvent {
	fn from(val: &InputState) -> Self {
		MouseEvent {
			x: val.mouse_position.x as i32,
			y: val.mouse_position.y as i32,
			modifiers: val.cef_modifiers_mouse_event().raw(),
		}
	}
}
impl From<&mut InputState> for MouseEvent {
	fn from(val: &mut InputState) -> Self {
		MouseEvent {
			x: val.mouse_position.x as i32,
			y: val.mouse_position.y as i32,
			modifiers: val.cef_modifiers_mouse_event().raw(),
		}
	}
}

#[derive(Default, Clone, Copy)]
pub(crate) struct MousePosition {
	x: usize,
	y: usize,
}
impl From<&PhysicalPosition<f64>> for MousePosition {
	fn from(position: &PhysicalPosition<f64>) -> Self {
		Self {
			x: position.x as usize,
			y: position.y as usize,
		}
	}
}

#[derive(Default, Clone)]
pub(crate) struct MouseState {
	left: bool,
	right: bool,
	middle: bool,
}
impl MouseState {
	pub(crate) fn update(&mut self, button: &MouseButton, state: &ElementState) {
		match state {
			ElementState::Pressed => match button {
				MouseButton::Left => self.left = true,
				MouseButton::Right => self.right = true,
				MouseButton::Middle => self.middle = true,
				_ => {}
			},
			ElementState::Released => match button {
				MouseButton::Left => self.left = false,
				MouseButton::Right => self.right = false,
				MouseButton::Middle => self.middle = false,
				_ => {}
			},
		}
	}
}

#[derive(Default)]
struct ClickTracker {
	left: Option<ClickRecord>,
	middle: Option<ClickRecord>,
	right: Option<ClickRecord>,
}
impl ClickTracker {
	fn input(&mut self, button: &MouseButton, state: &ElementState, position: MousePosition) -> ClickCount {
		let record = match button {
			MouseButton::Left => &mut self.left,
			MouseButton::Right => &mut self.right,
			MouseButton::Middle => &mut self.middle,
			_ => return ClickCount::Single,
		};

		let Some(record) = record else {
			*record = Some(ClickRecord { position, ..Default::default() });
			return ClickCount::Single;
		};

		let prev_time = record.time;
		let prev_position = record.position;

		let now = Instant::now();
		record.time = now;
		record.position = position;

		match state {
			ElementState::Pressed if record.down_count == ClickCount::Double => {
				*record = ClickRecord {
					down_count: ClickCount::Single,
					..*record
				};
				return ClickCount::Single;
			}
			ElementState::Released if record.up_count == ClickCount::Double => {
				*record = ClickRecord {
					up_count: ClickCount::Single,
					..*record
				};
				return ClickCount::Single;
			}
			_ => {}
		}

		let dx = position.x.abs_diff(prev_position.x);
		let dy = position.y.abs_diff(prev_position.y);
		let within_dist = dx <= MULTICLICK_ALLOWED_TRAVEL && dy <= MULTICLICK_ALLOWED_TRAVEL;
		let within_time = now.saturating_duration_since(prev_time) <= MULTICLICK_TIMEOUT;

		let count = if within_time && within_dist { ClickCount::Double } else { ClickCount::Single };

		*record = match state {
			ElementState::Pressed => ClickRecord { down_count: count, ..*record },
			ElementState::Released => ClickRecord { up_count: count, ..*record },
		};
		count
	}
}

#[derive(Clone, Copy, PartialEq, Default)]
enum ClickCount {
	#[default]
	Single,
	Double,
}
impl From<ClickCount> for i32 {
	fn from(count: ClickCount) -> i32 {
		match count {
			ClickCount::Single => 1,
			ClickCount::Double => 2,
		}
	}
}

#[derive(Clone, Copy)]
struct ClickRecord {
	time: Instant,
	position: MousePosition,
	down_count: ClickCount,
	up_count: ClickCount,
}

impl Default for ClickRecord {
	fn default() -> Self {
		Self {
			time: Instant::now(),
			position: Default::default(),
			down_count: Default::default(),
			up_count: Default::default(),
		}
	}
}

struct CefModifiers(u32);
impl CefModifiers {
	fn new(input_state: &InputState, location: &winit::keyboard::KeyLocation, is_repeat: bool) -> Self {
		let mut inner = 0;

		if input_state.modifiers.shift_key() {
			inner |= cef_event_flags_t::EVENTFLAG_SHIFT_DOWN as u32;
		}
		if input_state.modifiers.control_key() {
			inner |= cef_event_flags_t::EVENTFLAG_CONTROL_DOWN as u32;
		}
		if input_state.modifiers.alt_key() {
			inner |= cef_event_flags_t::EVENTFLAG_ALT_DOWN as u32;
		}
		if input_state.modifiers.meta_key() {
			inner |= cef_event_flags_t::EVENTFLAG_COMMAND_DOWN as u32;
		}

		if input_state.mouse_state.left {
			inner |= cef_event_flags_t::EVENTFLAG_LEFT_MOUSE_BUTTON as u32;
		}
		if input_state.mouse_state.right {
			inner |= cef_event_flags_t::EVENTFLAG_RIGHT_MOUSE_BUTTON as u32;
		}
		if input_state.mouse_state.middle {
			inner |= cef_event_flags_t::EVENTFLAG_MIDDLE_MOUSE_BUTTON as u32;
		}

		if is_repeat {
			inner |= cef_event_flags_t::EVENTFLAG_IS_REPEAT as u32;
		}

		inner |= match location {
			winit::keyboard::KeyLocation::Left => cef_event_flags_t::EVENTFLAG_IS_LEFT as u32,
			winit::keyboard::KeyLocation::Right => cef_event_flags_t::EVENTFLAG_IS_RIGHT as u32,
			winit::keyboard::KeyLocation::Numpad => cef_event_flags_t::EVENTFLAG_IS_KEY_PAD as u32,
			winit::keyboard::KeyLocation::Standard => 0,
		};

		Self(inner)
	}

	fn raw(&self) -> u32 {
		self.0
	}
}
