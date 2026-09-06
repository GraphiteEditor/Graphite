pub(crate) mod event;
pub(crate) use event::InputEvent;

mod keymap;
use keymap::{ToCharRepresentation, ToNativeKeycode, ToVKBits};

use cef::sys::{cef_event_flags_t, cef_key_event_type_t, cef_mouse_button_type_t};
use cef::{Browser, ImplBrowser, ImplBrowserHost, KeyEvent, MouseEvent};
use winit::dpi::PhysicalPosition;
use winit::keyboard::{Key, KeyLocation, NamedKey};

use super::consts::{PINCH_ZOOM_SPEED, SCROLL_LINE_HEIGHT, SCROLL_LINE_WIDTH, SCROLL_SPEED_X, SCROLL_SPEED_Y};
use event::{InputEventKind, KeyAction, Modifiers, MouseButton, PointerAction};

#[derive(Default)]
pub(crate) struct InputState {
	position: PhysicalPosition<f64>,
	buttons: ButtonStates,
}

impl InputState {
	fn pointer_move(&mut self, position: PhysicalPosition<f64>) -> bool {
		let moved = (position.x as i32, position.y as i32) != (self.position.x as i32, self.position.y as i32);
		self.position = position;
		moved
	}
}

pub(crate) fn process(state: &mut InputState, browser: &Browser, event: &InputEvent) {
	let Some(host) = browser.host() else { return };
	match &event.kind {
		InputEventKind::Pointer { position, action } => {
			let position = position.unwrap_or(state.position);
			match *action {
				PointerAction::Move => {
					if !state.pointer_move(position) {
						return;
					}
					let flags = event_flags(&event.modifiers, &state.buttons);
					host.send_mouse_move_event(Some(&mouse_event(position, flags)), 0);
				}
				PointerAction::Enter => {
					state.pointer_move(position);
					let flags = event_flags(&event.modifiers, &state.buttons);
					host.send_mouse_move_event(Some(&mouse_event(position, flags)), 0);
				}
				PointerAction::Exit => {
					state.pointer_move(position);
					let flags = event_flags(&event.modifiers, &state.buttons);
					host.send_mouse_move_event(Some(&mouse_event(position, flags)), 1);
				}
				PointerAction::Press { button, count } | PointerAction::Release { button, count } => {
					state.pointer_move(position);
					let cef_button = match button {
						MouseButton::Left => cef_mouse_button_type_t::MBT_LEFT,
						MouseButton::Right => cef_mouse_button_type_t::MBT_RIGHT,
						MouseButton::Middle => cef_mouse_button_type_t::MBT_MIDDLE,
						MouseButton::Unknown => return,
					};
					let up = matches!(action, PointerAction::Release { .. });
					state.buttons.update(button, !up);

					// CEF only understands single, double and triple clicks; further clicks alternate between double and triple.
					let count = match count {
						0 | 1 => 1,
						count if count % 2 == 0 => 2,
						_ => 3,
					};

					let flags = event_flags(&event.modifiers, &state.buttons);
					host.send_mouse_click_event(Some(&mouse_event(position, flags)), cef::MouseButtonType::from(cef_button), up as i32, count);
				}
				PointerAction::ScrollLines { x, y } => {
					let flags = event_flags(&event.modifiers, &state.buttons);
					let delta_x = (x * SCROLL_LINE_WIDTH * SCROLL_SPEED_X) as i32;
					let delta_y = (y * SCROLL_LINE_HEIGHT * SCROLL_SPEED_Y) as i32;
					host.send_mouse_wheel_event(Some(&mouse_event(position, flags)), delta_x, delta_y);
				}
				PointerAction::ScrollPixels { x, y } => {
					let flags = event_flags(&event.modifiers, &state.buttons) | PRECISION_SCROLLING_DELTA;
					host.send_mouse_wheel_event(Some(&mouse_event(position, flags)), (x * SCROLL_SPEED_X) as i32, (y * SCROLL_SPEED_Y) as i32);
				}
				PointerAction::Zoom(delta) => {
					if !delta.is_normal() {
						return;
					}
					let flags = CONTROL_DOWN | PRECISION_SCROLLING_DELTA;
					host.send_mouse_wheel_event(Some(&mouse_event(position, flags)), 0, (delta * PINCH_ZOOM_SPEED).round() as i32);
				}
			}
		}
		InputEventKind::Key {
			key,
			key_without_modifiers,
			physical_key,
			location,
			text,
			action,
		} => {
			let mut flags = event_flags(&event.modifiers, &state.buttons);

			let own_flag = match key {
				Key::Named(NamedKey::Shift) => SHIFT_DOWN,
				Key::Named(NamedKey::Control) => CONTROL_DOWN,
				Key::Named(NamedKey::Alt) => ALT_DOWN,
				Key::Named(NamedKey::AltGraph) => ALTGR_DOWN,
				Key::Named(NamedKey::Meta) => COMMAND_DOWN,
				_ => 0,
			};
			match action {
				KeyAction::Press | KeyAction::Repeat => flags |= own_flag,
				KeyAction::Release => flags &= !own_flag,
			}

			flags |= match location {
				KeyLocation::Left => IS_LEFT,
				KeyLocation::Right => IS_RIGHT,
				KeyLocation::Numpad => IS_KEY_PAD,
				KeyLocation::Standard => 0,
			};
			if *action == KeyAction::Repeat {
				flags |= IS_REPEAT;
			}

			let windows_key_code = match key {
				Key::Named(named) => named.to_vk_bits(),
				Key::Character(char) => char.chars().next().unwrap_or_default().to_vk_bits(),
				_ => 0,
			};
			let native_key_code = physical_key.to_native_keycode();

			let char_representation = key.to_char_representation();
			#[allow(unused_mut)]
			let mut character = char_representation as u16;
			let unmodified_character = key_without_modifiers.to_char_representation() as u16;

			#[cfg(target_os = "macos")] // See https://www.magpcss.org/ceforum/viewtopic.php?start=10&t=11650
			if character == 0 && unmodified_character == 0 && text.is_some() {
				character = 1;
			}
			#[cfg(not(target_os = "macos"))]
			let _ = text;

			let key_event = |kind: cef_key_event_type_t, windows_key_code: i32| KeyEvent {
				type_: kind.into(),
				modifiers: flags,
				windows_key_code,
				native_key_code,
				character,
				unmodified_character,
				..Default::default()
			};

			match action {
				KeyAction::Press | KeyAction::Repeat if char_representation != '\0' => {
					host.send_key_event(Some(&key_event(cef_key_event_type_t::KEYEVENT_RAWKEYDOWN, windows_key_code)));
					host.send_key_event(Some(&key_event(cef_key_event_type_t::KEYEVENT_CHAR, char_representation as i32)));
				}
				KeyAction::Press | KeyAction::Repeat => {
					host.send_key_event(Some(&key_event(cef_key_event_type_t::KEYEVENT_RAWKEYDOWN, windows_key_code)));
				}
				KeyAction::Release => {
					host.send_key_event(Some(&key_event(cef_key_event_type_t::KEYEVENT_KEYUP, windows_key_code)));
				}
			}
		}
	}
}

#[derive(Default)]
struct ButtonStates {
	left: bool,
	right: bool,
	middle: bool,
}

impl ButtonStates {
	fn update(&mut self, button: MouseButton, held: bool) {
		match button {
			MouseButton::Left => self.left = held,
			MouseButton::Right => self.right = held,
			MouseButton::Middle => self.middle = held,
			MouseButton::Unknown => {}
		}
	}
}

fn mouse_event(position: PhysicalPosition<f64>, flags: u32) -> MouseEvent {
	MouseEvent {
		x: position.x as i32,
		y: position.y as i32,
		modifiers: flags,
	}
}

fn event_flags(modifiers: &Modifiers, buttons: &ButtonStates) -> u32 {
	let bit = |condition: bool, flag: u32| if condition { flag } else { 0 };
	bit(modifiers.shift, SHIFT_DOWN)
		| bit(modifiers.control, CONTROL_DOWN)
		| bit(modifiers.alt, ALT_DOWN)
		| bit(modifiers.alt_graph, ALTGR_DOWN)
		| bit(modifiers.meta, COMMAND_DOWN)
		| bit(modifiers.caps_lock, CAPS_LOCK_ON)
		| bit(modifiers.num_lock, NUM_LOCK_ON)
		| bit(buttons.left, LEFT_MOUSE_BUTTON)
		| bit(buttons.right, RIGHT_MOUSE_BUTTON)
		| bit(buttons.middle, MIDDLE_MOUSE_BUTTON)
}

const fn flag(flag: cef_event_flags_t) -> u32 {
	#[cfg(not(target_os = "windows"))]
	{
		flag.0
	}
	#[cfg(target_os = "windows")]
	{
		flag.0 as u32
	}
}

const SHIFT_DOWN: u32 = flag(cef_event_flags_t::EVENTFLAG_SHIFT_DOWN);
const CONTROL_DOWN: u32 = flag(cef_event_flags_t::EVENTFLAG_CONTROL_DOWN);
const ALT_DOWN: u32 = flag(cef_event_flags_t::EVENTFLAG_ALT_DOWN);
const ALTGR_DOWN: u32 = flag(cef_event_flags_t::EVENTFLAG_ALTGR_DOWN);
const COMMAND_DOWN: u32 = flag(cef_event_flags_t::EVENTFLAG_COMMAND_DOWN);
const CAPS_LOCK_ON: u32 = flag(cef_event_flags_t::EVENTFLAG_CAPS_LOCK_ON);
const NUM_LOCK_ON: u32 = flag(cef_event_flags_t::EVENTFLAG_NUM_LOCK_ON);
const LEFT_MOUSE_BUTTON: u32 = flag(cef_event_flags_t::EVENTFLAG_LEFT_MOUSE_BUTTON);
const MIDDLE_MOUSE_BUTTON: u32 = flag(cef_event_flags_t::EVENTFLAG_MIDDLE_MOUSE_BUTTON);
const RIGHT_MOUSE_BUTTON: u32 = flag(cef_event_flags_t::EVENTFLAG_RIGHT_MOUSE_BUTTON);
const IS_LEFT: u32 = flag(cef_event_flags_t::EVENTFLAG_IS_LEFT);
const IS_RIGHT: u32 = flag(cef_event_flags_t::EVENTFLAG_IS_RIGHT);
const IS_KEY_PAD: u32 = flag(cef_event_flags_t::EVENTFLAG_IS_KEY_PAD);
const IS_REPEAT: u32 = flag(cef_event_flags_t::EVENTFLAG_IS_REPEAT);
const PRECISION_SCROLLING_DELTA: u32 = flag(cef_event_flags_t::EVENTFLAG_PRECISION_SCROLLING_DELTA);
