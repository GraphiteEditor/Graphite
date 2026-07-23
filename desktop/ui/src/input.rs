use cef::sys::{cef_key_event_type_t, cef_mouse_button_type_t};
use cef::{Browser, ImplBrowser, ImplBrowserHost, KeyEvent, MouseEvent};
use winit::event::{ButtonSource, ElementState, MouseButton, MouseScrollDelta, WindowEvent};

mod keymap;
use keymap::{ToCharRepresentation, ToNativeKeycode, ToVKBits};

mod state;
pub(crate) use state::{CefModifiers, InputState};

use super::consts::{PINCH_ZOOM_SPEED, SCROLL_LINE_HEIGHT, SCROLL_LINE_WIDTH, SCROLL_SPEED_X, SCROLL_SPEED_Y};

/// A window input translated into the plain data CEF consumes — no winit types, so it can
/// be applied to a browser living in another process.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) enum InputEvent {
	MouseMove { data: MouseData, leave: bool },
	MouseClick { data: MouseData, button: MouseButtonKind, up: bool, click_count: i32 },
	MouseWheel { data: MouseData, delta_x: i32, delta_y: i32 },
	Key(KeyData),
}


#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct MouseData {
	pub(crate) x: i32,
	pub(crate) y: i32,
	pub(crate) modifiers: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum MouseButtonKind {
	Left,
	Right,
	Middle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum KeyEventKind {
	RawKeyDown,
	KeyUp,
	Char,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct KeyData {
	pub(crate) kind: KeyEventKind,
	pub(crate) modifiers: u32,
	pub(crate) windows_key_code: i32,
	pub(crate) native_key_code: i32,
	pub(crate) character: u16,
	pub(crate) unmodified_character: u16,
}

/// Turns a winit event into zero or more [`InputEvent`]s, updating the tracked input state
/// (cursor position, click counting, modifiers) along the way.
pub(crate) fn translate(input_state: &mut InputState, event: &WindowEvent) -> Vec<InputEvent> {
	match event {
		WindowEvent::PointerMoved { position, .. } => {
			if !input_state.cursor_move(position) {
				return Vec::new();
			}
			vec![InputEvent::MouseMove {
				data: input_state.mouse_data(),
				leave: false,
			}]
		}
		WindowEvent::PointerEntered { position, .. } => {
			let _ = input_state.cursor_move(position);
			vec![InputEvent::MouseMove {
				data: input_state.mouse_data(),
				leave: false,
			}]
		}
		WindowEvent::PointerLeft { position, .. } => {
			if let Some(position) = position {
				let _ = input_state.cursor_move(position);
			}
			vec![InputEvent::MouseMove {
				data: input_state.mouse_data(),
				leave: true,
			}]
		}
		WindowEvent::PointerButton { state, button, position, .. } => {
			let mouse_button = match button {
				ButtonSource::Mouse(mouse_button) => mouse_button,
				_ => {
					return Vec::new(); // TODO: Handle touch input
				}
			};

			let _ = input_state.cursor_move(position);
			let click_count = input_state.mouse_input(mouse_button, state).into();
			let up = matches!(state, ElementState::Released);
			let button = match mouse_button {
				MouseButton::Left => MouseButtonKind::Left,
				MouseButton::Right => MouseButtonKind::Right,
				MouseButton::Middle => MouseButtonKind::Middle,
				_ => return Vec::new(),
			};

			vec![InputEvent::MouseClick {
				data: input_state.mouse_data(),
				button,
				up,
				click_count,
			}]
		}
		WindowEvent::MouseWheel { delta, phase: _, device_id: _, .. } => {
			let (mut delta_x, mut delta_y) = match delta {
				MouseScrollDelta::LineDelta(x, y) => (x * SCROLL_LINE_WIDTH as f32, y * SCROLL_LINE_HEIGHT as f32),
				MouseScrollDelta::PixelDelta(physical_position) => (physical_position.x as f32, physical_position.y as f32),
			};
			delta_x *= SCROLL_SPEED_X;
			delta_y *= SCROLL_SPEED_Y;

			vec![InputEvent::MouseWheel {
				data: input_state.mouse_data(),
				delta_x: delta_x as i32,
				delta_y: delta_y as i32,
			}]
		}
		WindowEvent::ModifiersChanged(modifiers) => {
			input_state.modifiers_changed(&modifiers.state());
			Vec::new()
		}
		WindowEvent::KeyboardInput { device_id: _, event, is_synthetic: _ } => {
			input_state.modifiers_apply_key_event(&event.logical_key, &event.state);

			let mut kind = match (event.state, &event.logical_key) {
				(ElementState::Pressed, winit::keyboard::Key::Character(_)) => KeyEventKind::Char,
				(ElementState::Pressed, _) => KeyEventKind::RawKeyDown,
				(ElementState::Released, _) => KeyEventKind::KeyUp,
			};

			let modifiers = input_state.cef_modifiers(&event.location, event.repeat).into();

			let windows_key_code = match &event.logical_key {
				winit::keyboard::Key::Named(named) => named.to_vk_bits(),
				winit::keyboard::Key::Character(char) => char.chars().next().unwrap_or_default().to_vk_bits(),
				_ => 0,
			};

			let native_key_code = event.physical_key.to_native_keycode();

			let char_representation = event.logical_key.to_char_representation();
			#[allow(unused_mut)]
			let mut character = char_representation as u16;

			if event.state == ElementState::Pressed && character != 0 {
				kind = KeyEventKind::Char;
			}

			let unmodified_character = event.key_without_modifiers.to_char_representation() as u16;

			#[cfg(target_os = "macos")] // See https://www.magpcss.org/ceforum/viewtopic.php?start=10&t=11650
			if character == 0 && unmodified_character == 0 && event.text_with_all_modifiers.is_some() {
				character = 1;
			}

			let key = KeyData {
				kind,
				modifiers,
				windows_key_code,
				native_key_code,
				character,
				unmodified_character,
			};

			if kind == KeyEventKind::Char {
				// CEF expects a raw key-down before the character event it produces.
				vec![
					InputEvent::Key(KeyData {
						kind: KeyEventKind::RawKeyDown,
						..key
					}),
					InputEvent::Key(KeyData {
						windows_key_code: char_representation as i32,
						..key
					}),
				]
			} else {
				vec![InputEvent::Key(key)]
			}
		}
		WindowEvent::PinchGesture { delta, .. } => {
			if !delta.is_normal() {
				return Vec::new();
			}

			let data = MouseData {
				modifiers: CefModifiers::PINCH_MODIFIERS.into(),
				..input_state.mouse_data()
			};

			vec![InputEvent::MouseWheel {
				data,
				delta_x: 0,
				delta_y: (delta * PINCH_ZOOM_SPEED).round() as i32,
			}]
		}
		_ => Vec::new(),
	}
}

/// Sends a translated [`InputEvent`] to the browser. Must run on the thread owning the browser.
pub(crate) fn apply(browser: &Browser, event: &InputEvent) {
	let Some(host) = browser.host() else { return };
	match event {
		InputEvent::MouseMove { data, leave } => {
			host.send_mouse_move_event(Some(&data.into()), *leave as i32);
		}
		InputEvent::MouseClick { data, button, up, click_count } => {
			let cef_button = cef::MouseButtonType::from(match button {
				MouseButtonKind::Left => cef_mouse_button_type_t::MBT_LEFT,
				MouseButtonKind::Right => cef_mouse_button_type_t::MBT_RIGHT,
				MouseButtonKind::Middle => cef_mouse_button_type_t::MBT_MIDDLE,
			});
			host.send_mouse_click_event(Some(&data.into()), cef_button, *up as i32, *click_count);
		}
		InputEvent::MouseWheel { data, delta_x, delta_y } => {
			host.send_mouse_wheel_event(Some(&data.into()), *delta_x, *delta_y);
		}
		InputEvent::Key(key) => {
			let key_event = KeyEvent {
				type_: match key.kind {
					KeyEventKind::RawKeyDown => cef_key_event_type_t::KEYEVENT_RAWKEYDOWN,
					KeyEventKind::KeyUp => cef_key_event_type_t::KEYEVENT_KEYUP,
					KeyEventKind::Char => cef_key_event_type_t::KEYEVENT_CHAR,
				}
				.into(),
				modifiers: key.modifiers,
				windows_key_code: key.windows_key_code,
				native_key_code: key.native_key_code,
				character: key.character,
				unmodified_character: key.unmodified_character,
				..Default::default()
			};
			host.send_key_event(Some(&key_event));
		}
	}
}

impl From<&MouseData> for MouseEvent {
	fn from(data: &MouseData) -> Self {
		MouseEvent {
			x: data.x,
			y: data.y,
			modifiers: data.modifiers,
		}
	}
}
