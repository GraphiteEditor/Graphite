use crate::consts::DRAG_THRESHOLD;
use crate::messages::prelude::*;
use bitflags::bitflags;
use glam::DVec2;
use std::collections::VecDeque;

// Origin is top left
pub type DocumentPosition = DVec2;
pub type ViewportPosition = DVec2;
pub type EditorPosition = DVec2;

#[derive(PartialEq, Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
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
		(self.bottom_right - self.top_left).ceil()
	}

	pub fn center(&self) -> DVec2 {
		(self.bottom_right - self.top_left).ceil() / 2.
	}

	pub fn in_bounds(&self, position: ViewportPosition) -> bool {
		position.x >= 0. && position.y >= 0. && position.x <= self.bottom_right.x && position.y <= self.bottom_right.y
	}
}

use std::hash::{Hash, Hasher};

#[derive(Debug, Copy, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ScrollDelta {
	pub x: f64,
	pub y: f64,
	pub z: f64,
}

impl PartialEq for ScrollDelta {
	fn eq(&self, other: &Self) -> bool {
		self.x == other.x && self.y == other.y && self.z == other.z
	}
}

impl Eq for ScrollDelta {}

impl Hash for ScrollDelta {
	fn hash<H: Hasher>(&self, state: &mut H) {
		let no_negative_zero = |value: f64| if value == 0. { 0. } else { value };

		no_negative_zero(self.x).to_bits().hash(state);
		no_negative_zero(self.y).to_bits().hash(state);
		no_negative_zero(self.z).to_bits().hash(state);
	}
}

impl ScrollDelta {
	pub fn new(x: f64, y: f64, z: f64) -> Self {
		Self { x, y, z }
	}

	pub fn as_dvec2(&self) -> DVec2 {
		DVec2::new(self.x, self.y)
	}

	pub fn scroll_delta(&self) -> f64 {
		let (dx, dy) = (self.x, self.y);
		dy.signum() * (dy * dy + f64::min(dy.abs(), dx.abs()).powi(2)).sqrt()
	}
}

// TODO: Document the difference between this and EditorMouseState
#[derive(Debug, Copy, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MouseState {
	pub position: ViewportPosition,
	pub mouse_keys: MouseKeys,
	pub scroll_delta: ScrollDelta,
}

impl MouseState {
	pub fn finish_transaction(&self, drag_start: DVec2, responses: &mut VecDeque<Message>) {
		let drag_too_small = drag_start.distance(self.position) <= DRAG_THRESHOLD;
		let response = if drag_too_small { DocumentMessage::AbortTransaction } else { DocumentMessage::EndTransaction };
		responses.add(response);
	}
}

// TODO: Document the difference between this and MouseState
#[derive(Debug, Copy, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EditorMouseState {
	pub editor_position: EditorPosition,
	pub mouse_keys: MouseKeys,
	pub scroll_delta: ScrollDelta,
}

impl EditorMouseState {
	pub fn from_keys_and_editor_position(keys: u8, editor_position: EditorPosition) -> Self {
		let mouse_keys = MouseKeys::from_bits(keys).expect("Invalid decoding of MouseKeys");

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
	/// Based on <https://developer.mozilla.org/en-US/docs/Web/API/MouseEvent/buttons#value>.
	#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
	#[repr(transparent)]
	pub struct MouseKeys: u8 {
		const NONE    = 0b0000_0000;
		const LEFT    = 0b0000_0001;
		const RIGHT   = 0b0000_0010;
		const MIDDLE  = 0b0000_0100;
		const BACK    = 0b0000_1000;
		const FORWARD = 0b0001_0000;
	}
}

#[impl_message(Message, InputMapperMessage, DoubleClick)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, specta::Type, num_enum::TryFromPrimitive)]
#[repr(u8)]
pub enum MouseButton {
	Left,
	Right,
	Middle,
	Back,
	Forward,
}

pub const NUMBER_OF_MOUSE_BUTTONS: usize = 5; // Should be the number of variants in MouseButton
