use crate::tools::ToolType;
use crate::Color;
use bitflags::bitflags;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone)]
#[repr(C)]
pub enum Event {
	SelectTool(ToolType),
	SelectPrimaryColor(Color),
	SelectSecondaryColor(Color),
	SwapColors,
	ResetColors,
	MouseDown(MouseState),
	MouseUp(MouseState),
	MouseMovement(CanvasPosition),
	ModifierKeyDown(ModKeys),
	ModifierKeyUp(ModKeys),
	KeyPress(Key),
}

#[derive(Debug, Clone)]
#[repr(C)]
pub enum Response {
	UpdateCanvas,
}

#[derive(Debug, Clone, Default)]
pub struct Trace(Vec<TracePoint>);

impl Deref for Trace {
	type Target = Vec<TracePoint>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl DerefMut for Trace {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl Trace {
	pub fn new() -> Self {
		Self::default()
	}
}

// origin is top left
#[derive(Debug, Copy, Clone, Default, Eq, PartialEq)]
pub struct CanvasPosition {
	pub x: u32,
	pub y: u32,
}

#[derive(Debug, Copy, Clone, Default, Eq, PartialEq)]
pub struct TracePoint {
	pub mouse_state: MouseState,
	pub mod_keys: ModKeys,
}

#[derive(Debug, Copy, Clone, Default, Eq, PartialEq)]
pub struct MouseState {
	pub position: CanvasPosition,
	pub mouse_keys: MouseKeys,
}

impl MouseState {
	pub fn new() -> MouseState {
		Self::default()
	}

	pub fn from_pos(x: u32, y: u32) -> MouseState {
		MouseState {
			position: CanvasPosition { x, y },
			mouse_keys: MouseKeys::default(),
		}
	}
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Key {
	UnknownKey,
}

bitflags! {
	#[derive(Default)]
	#[repr(transparent)]
	pub struct ModKeys: u8 {
		const CONTROL = 0b0000_0001;
		const SHIFT   = 0b0000_0010;
		const ALT     = 0b0000_0100;
	}
}

bitflags! {
	#[derive(Default)]
	#[repr(transparent)]
	pub struct MouseKeys: u8 {
		const LEFT   = 0b0000_0001;
		const RIGHT  = 0b0000_0010;
		const MIDDLE = 0b0000_0100;
	}
}
