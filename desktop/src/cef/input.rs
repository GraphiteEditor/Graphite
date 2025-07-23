use cef::sys::{cef_event_flags_t, cef_key_event_type_t, cef_mouse_button_type_t};
use cef::{ImplBrowser, ImplBrowserHost, KeyEvent, KeyEventType, MouseEvent};
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};

use super::context::{Context, Initialized};

mod keymap;
use keymap::{ToDomBits, ToVKBits};

pub(crate) fn handle_window_event(context: &mut Context<Initialized>, event: WindowEvent) -> Option<WindowEvent> {
	match event {
		WindowEvent::CursorMoved { position, .. } => {
			if let Some(browser) = &context.browser {
				if let Some(host) = browser.host() {
					host.set_focus(1);
				}

				context.input_state.update_mouse_position(&position);
				let mouse_event: MouseEvent = (&context.input_state).into();
				browser.host().unwrap().send_mouse_move_event(Some(&mouse_event), 0);
			}
		}
		WindowEvent::MouseInput { state, button, .. } => {
			if let Some(browser) = &context.browser {
				if let Some(host) = browser.host() {
					host.set_focus(1);

					let mouse_up = match state {
						ElementState::Pressed => 0,
						ElementState::Released => 1,
					};

					let cef_button = match button {
						MouseButton::Left => Some(cef::MouseButtonType::from(cef_mouse_button_type_t::MBT_LEFT)),
						MouseButton::Right => Some(cef::MouseButtonType::from(cef_mouse_button_type_t::MBT_RIGHT)),
						MouseButton::Middle => Some(cef::MouseButtonType::from(cef_mouse_button_type_t::MBT_MIDDLE)),
						MouseButton::Forward => None, //TODO: Handle Forward button
						MouseButton::Back => None,    //TODO: Handle Back button
						_ => None,
					};

					let mut mouse_state = context.input_state.mouse_state.clone();
					match button {
						MouseButton::Left => {
							mouse_state.left = match state {
								ElementState::Pressed => true,
								ElementState::Released => false,
							}
						}
						MouseButton::Right => {
							mouse_state.right = match state {
								ElementState::Pressed => true,
								ElementState::Released => false,
							}
						}
						MouseButton::Middle => {
							mouse_state.middle = match state {
								ElementState::Pressed => true,
								ElementState::Released => false,
							}
						}
						_ => {}
					};
					context.input_state.update_mouse_state(mouse_state);

					let mouse_event: MouseEvent = (&context.input_state).into();

					if let Some(button) = cef_button {
						host.send_mouse_click_event(
							Some(&mouse_event),
							button,
							mouse_up,
							1, // click count
						);
					}
				}
			}
		}
		WindowEvent::MouseWheel { delta, phase: _, device_id: _, .. } => {
			if let Some(browser) = &context.browser {
				if let Some(host) = browser.host() {
					let mouse_event = (&context.input_state).into();
					let line_width = 40; //feels about right, TODO: replace with correct value
					let line_height = 30; //feels about right, TODO: replace with correct value
					let (delta_x, delta_y) = match delta {
						MouseScrollDelta::LineDelta(x, y) => (x * line_width as f32, y * line_height as f32),
						MouseScrollDelta::PixelDelta(physical_position) => (physical_position.x as f32, physical_position.y as f32),
					};
					host.send_mouse_wheel_event(Some(&mouse_event), delta_x as i32, delta_y as i32);
				}
			}
		}
		WindowEvent::ModifiersChanged(modifiers) => {
			context.input_state.update_modifiers(&modifiers.state());
		}
		WindowEvent::KeyboardInput { device_id: _, event, is_synthetic: _ } => {
			if let Some(browser) = &context.browser {
				if let Some(host) = browser.host() {
					host.set_focus(1);

					let (named_key, character) = match &event.logical_key {
						winit::keyboard::Key::Named(named_key) => (
							Some(named_key),
							match named_key {
								winit::keyboard::NamedKey::Space => Some(' '),
								winit::keyboard::NamedKey::Enter => Some('\u{000d}'),
								_ => None,
							},
						),
						winit::keyboard::Key::Character(str) => {
							let char = str.chars().next().unwrap_or('\0');
							(None, Some(char))
						}
						_ => return None,
					};

					let mut key_event = KeyEvent {
						size: size_of::<KeyEvent>(),
						focus_on_editable_field: 1,
						modifiers: context.input_state.cef_modifiers(&event.location, event.repeat).raw(),
						is_system_key: 0,
						..Default::default()
					};

					if let Some(named_key) = named_key {
						key_event.native_key_code = named_key.to_dom_bits();
						key_event.windows_key_code = named_key.to_vk_bits();
					} else if let Some(char) = character {
						key_event.native_key_code = char.to_dom_bits();
						key_event.windows_key_code = char.to_vk_bits();
					}

					match event.state {
						ElementState::Pressed => {
							key_event.type_ = KeyEventType::from(cef_key_event_type_t::KEYEVENT_RAWKEYDOWN);
							host.send_key_event(Some(&key_event));

							if let Some(char) = character {
								let mut buf = [0; 2];
								char.encode_utf16(&mut buf);
								key_event.character = buf[0];
								let mut buf = [0; 2];
								char.to_lowercase().next().unwrap().encode_utf16(&mut buf);
								key_event.unmodified_character = buf[0];

								key_event.type_ = KeyEventType::from(cef_key_event_type_t::KEYEVENT_CHAR);
								host.send_key_event(Some(&key_event));
							}
						}
						ElementState::Released => {
							key_event.type_ = KeyEventType::from(cef_key_event_type_t::KEYEVENT_KEYUP);
							host.send_key_event(Some(&key_event));
						}
					};
				}
			}
		}
		e => return Some(e),
	}
	None
}

#[derive(Default, Clone)]
pub(crate) struct MouseState {
	left: bool,
	right: bool,
	middle: bool,
}

#[derive(Default, Clone, Debug)]
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
pub(crate) struct InputState {
	modifiers: winit::keyboard::ModifiersState,
	mouse_position: MousePosition,
	mouse_state: MouseState,
}

impl InputState {
	fn update_modifiers(&mut self, modifiers: &winit::keyboard::ModifiersState) {
		self.modifiers = *modifiers;
	}

	fn update_mouse_position(&mut self, position: &PhysicalPosition<f64>) {
		self.mouse_position = position.into();
	}

	fn update_mouse_state(&mut self, state: MouseState) {
		self.mouse_state = state;
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
		if input_state.modifiers.super_key() {
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
