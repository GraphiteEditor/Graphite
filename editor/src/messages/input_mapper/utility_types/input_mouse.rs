use bitflags::bitflags;
use glam::DVec2;
use serde::{Deserialize, Serialize};

// Origin is top left
pub type ViewportPosition = DVec2;
pub type EditorPosition = DVec2;

#[derive(PartialEq, Clone, Debug, Default, Serialize, Deserialize)]
pub struct ViewportBounds {
	pub top_left: DVec2,
	pub bottom_right: DVec2,
}

impl ViewportBounds {
	pub fn from_slice(slice: &[f64]) -> Self {
		Self {
			top_left: DVec2::from_slice(&slice[0..2]),
			bottom_right: DVec2::from_slice(&slice[2..4]),
		}
	}

	pub fn size(&self) -> DVec2 {
		self.bottom_right - self.top_left
	}

	pub fn center(&self) -> DVec2 {
		self.bottom_right.lerp(self.top_left, 0.5)
	}
}

#[derive(Debug, Copy, Clone, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct ScrollDelta {
	// TODO: Switch these to `f64` values (not trivial because floats don't provide PartialEq, Eq, and Hash)
	pub x: i32,
	pub y: i32,
	pub z: i32,
}

impl ScrollDelta {
	pub fn new(x: i32, y: i32, z: i32) -> Self {
		Self { x, y, z }
	}

	pub fn as_dvec2(&self) -> DVec2 {
		DVec2::new(self.x as f64, self.y as f64)
	}

	pub fn scroll_delta(&self) -> f64 {
		let (dx, dy) = (self.x, self.y);
		dy.signum() as f64 * ((dy * dy + i32::min(dy.abs(), dx.abs()).pow(2)) as f64).sqrt()
	}
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MouseState {
	pub position: ViewportPosition,
	pub mouse_keys: MouseKeys,
	pub scroll_delta: ScrollDelta,
}

impl MouseState {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn from_position(x: f64, y: f64) -> Self {
		Self {
			position: (x, y).into(),
			mouse_keys: MouseKeys::default(),
			scroll_delta: ScrollDelta::default(),
		}
	}

	pub fn from_keys_and_editor_position(keys: u8, position: ViewportPosition) -> Self {
		let mouse_keys = MouseKeys::from_bits(keys).expect("Invalid modifier keys");

		Self {
			position,
			mouse_keys,
			scroll_delta: ScrollDelta::default(),
		}
	}
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EditorMouseState {
	pub editor_position: EditorPosition,
	pub mouse_keys: MouseKeys,
	pub scroll_delta: ScrollDelta,
}

impl EditorMouseState {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn from_editor_position(x: f64, y: f64) -> Self {
		Self {
			editor_position: (x, y).into(),
			mouse_keys: MouseKeys::default(),
			scroll_delta: ScrollDelta::default(),
		}
	}

	pub fn from_keys_and_editor_position(keys: u8, editor_position: EditorPosition) -> Self {
		let mouse_keys = MouseKeys::from_bits(keys).expect("Invalid modifier keys");

		Self {
			editor_position,
			mouse_keys,
			scroll_delta: ScrollDelta::default(),
		}
	}

	pub fn to_mouse_state(&self, active_viewport_bounds: &ViewportBounds) -> MouseState {
		MouseState {
			position: self.editor_position - active_viewport_bounds.top_left,
			mouse_keys: self.mouse_keys,
			scroll_delta: self.scroll_delta,
		}
	}
}

bitflags! {
	#[derive(Default, Serialize, Deserialize)]
	#[repr(transparent)]
	pub struct MouseKeys: u8 {
		const LEFT   = 0b0000_0001;
		const RIGHT  = 0b0000_0010;
		const MIDDLE = 0b0000_0100;
		const NONE   = 0b0000_0000;
	}
}
