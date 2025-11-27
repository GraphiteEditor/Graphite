use cef::MouseEvent;
use std::time::Instant;
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, MouseButton};

use crate::cef::consts::{MULTICLICK_ALLOWED_TRAVEL, MULTICLICK_TIMEOUT};

#[derive(Default)]
pub(crate) struct InputState {
	modifiers: winit::keyboard::ModifiersState,
	mouse_position: MousePosition,
	mouse_state: MouseState,
	mouse_click_tracker: ClickTracker,
}
impl InputState {
	pub(crate) fn modifiers_changed(&mut self, modifiers: &winit::keyboard::ModifiersState) {
		self.modifiers = *modifiers;
	}

	pub(crate) fn cursor_move(&mut self, position: &PhysicalPosition<f64>) -> bool {
		let new = position.into();
		if self.mouse_position == new {
			return false;
		}
		self.mouse_position = new;
		true
	}

	pub(crate) fn mouse_input(&mut self, button: &MouseButton, state: &ElementState) -> ClickCount {
		self.mouse_state.update(button, state);
		self.mouse_click_tracker.input(button, state, self.mouse_position)
	}

	pub(crate) fn cef_modifiers(&self, location: &winit::keyboard::KeyLocation, is_repeat: bool) -> CefModifiers {
		CefModifiers::new(self, location, is_repeat)
	}

	pub(crate) fn cef_mouse_modifiers(&self) -> CefModifiers {
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
			modifiers: val.cef_mouse_modifiers().raw(),
		}
	}
}
impl From<&mut InputState> for MouseEvent {
	fn from(val: &mut InputState) -> Self {
		MouseEvent {
			x: val.mouse_position.x as i32,
			y: val.mouse_position.y as i32,
			modifiers: val.cef_mouse_modifiers().raw(),
		}
	}
}

#[derive(Default, Clone, Copy, Eq, PartialEq)]
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
pub(crate) enum ClickCount {
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

pub(crate) struct CefModifiers(u32);
impl CefModifiers {
	fn new(input_state: &InputState, location: &winit::keyboard::KeyLocation, is_repeat: bool) -> Self {
		use cef::sys::cef_event_flags_t;

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

	pub(crate) fn raw(&self) -> u32 {
		self.0
	}
}
