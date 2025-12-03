use cef::sys::{cef_key_event_type_t, cef_mouse_button_type_t};
use cef::{Browser, ImplBrowser, ImplBrowserHost, KeyEvent, MouseEvent};
use winit::event::{ButtonSource, ElementState, MouseButton, MouseScrollDelta, WindowEvent};

mod keymap;
use keymap::{ToCharRepresentation, ToNativeKeycode, ToVKBits};

mod state;
pub(crate) use state::{CefModifiers, InputState};

use super::consts::{PINCH_ZOOM_SPEED, SCROLL_LINE_HEIGHT, SCROLL_LINE_WIDTH, SCROLL_SPEED_X, SCROLL_SPEED_Y};

pub(crate) fn handle_window_event(browser: &Browser, input_state: &mut InputState, event: &WindowEvent) {
	match event {
		WindowEvent::PointerMoved { position, .. } | WindowEvent::PointerEntered { position, .. } => {
			if !input_state.cursor_move(position) {
				return;
			}

			let Some(host) = browser.host() else { return };
			host.send_mouse_move_event(Some(&input_state.into()), 0);
		}
		WindowEvent::PointerLeft { position, .. } => {
			if let Some(position) = position {
				let _ = input_state.cursor_move(position);
			}

			let Some(host) = browser.host() else { return };
			host.send_mouse_move_event(Some(&(input_state.into())), 1);
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
			let Some(host) = browser.host() else { return };

			let mut key_event = KeyEvent {
				type_: match (event.state, &event.logical_key) {
					(ElementState::Pressed, winit::keyboard::Key::Character(_)) => cef_key_event_type_t::KEYEVENT_CHAR,
					(ElementState::Pressed, _) => cef_key_event_type_t::KEYEVENT_RAWKEYDOWN,
					(ElementState::Released, _) => cef_key_event_type_t::KEYEVENT_KEYUP,
				}
				.into(),
				..Default::default()
			};

			key_event.modifiers = input_state.cef_modifiers(&event.location, event.repeat).into();

			key_event.windows_key_code = match &event.logical_key {
				winit::keyboard::Key::Named(named) => named.to_vk_bits(),
				winit::keyboard::Key::Character(char) => char.chars().next().unwrap_or_default().to_vk_bits(),
				_ => 0,
			};

			key_event.native_key_code = event.physical_key.to_native_keycode();

			key_event.character = event.logical_key.to_char_representation() as u16;

			// Mitigation for CEF on Mac bug to prevent NSMenu being triggered by this key event.
			//
			// CEF converts the key event into an `NSEvent` internally and passes that to Chromium.
			// In some cases the `NSEvent` gets to the native Cocoa application, is considered "unhandled" and can trigger menus.
			//
			// Why mitigation works:
			// Leaving `key_event.unmodified_character = 0` still leads to CEF forwarding a "unhandled" event to the native application
			// but that event is discarded because `key_event.unmodified_character = 0` is considered non-printable and not used for shortcut matching.
			//
			// See https://github.com/chromiumembedded/cef/issues/3857
			//
			// TODO: Remove mitigation once bug is fixed or a better solution is found.
			#[cfg(not(target_os = "macos"))]
			{
				key_event.unmodified_character = event.key_without_modifiers.to_char_representation() as u16;
			}

			#[cfg(target_os = "macos")] // See https://www.magpcss.org/ceforum/viewtopic.php?start=10&t=11650
			if key_event.character == 0 && key_event.unmodified_character == 0 && event.text_with_all_modifiers.is_some() {
				key_event.character = 1;
			}

			if key_event.type_ == cef_key_event_type_t::KEYEVENT_CHAR.into() {
				let mut key_down_event = key_event.clone();
				key_down_event.type_ = cef_key_event_type_t::KEYEVENT_RAWKEYDOWN.into();
				host.send_key_event(Some(&key_down_event));

				key_event.windows_key_code = event.logical_key.to_char_representation() as i32;
			}

			host.send_key_event(Some(&key_event));
		}
		WindowEvent::PinchGesture { delta, .. } => {
			if !delta.is_normal() {
				return;
			}
			let Some(host) = browser.host() else { return };

			let mouse_event = MouseEvent {
				modifiers: CefModifiers::PINCH_MODIFIERS.into(),
				..input_state.into()
			};

			let delta = (delta * PINCH_ZOOM_SPEED).round() as i32;

			host.send_mouse_wheel_event(Some(&mouse_event), 0, delta);
		}
		_ => {}
	}
}
