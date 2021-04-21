use crate::tools::ToolType;
use bitflags::bitflags;
use crate::Color;
use std::{
	fmt,
	ops::{Deref, DerefMut},
};

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
	MouseMove(ViewportPosition),
	KeyUp(Key),
	KeyDown(Key),
}

#[derive(Debug, Clone)]
#[repr(C)]
// TODO - Make Copy when possible
pub enum Response {
	UpdateCanvas { document: String },
	SetActiveTool { tool_name: String },
}

impl fmt::Display for Response {
	fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		use Response::*;

		let name = match_variant_name!(match (self) {
			UpdateCanvas,
			SetActiveTool
		});

		formatter.write_str(name)
	}
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
pub struct ViewportPosition {
	pub x: u32,
	pub y: u32,
}

impl ViewportPosition {
	pub fn distance(&self, other: &Self) -> f64 {
		let x_diff = other.x as f64 - self.x as f64;
		let y_diff = other.y as f64 - self.y as f64;
		f64::sqrt(x_diff * x_diff + y_diff * y_diff)
	}
}

#[derive(Debug, Copy, Clone, Default, Eq, PartialEq)]
pub struct TracePoint {
	pub mouse_state: MouseState,
	pub mod_keys: ModKeys,
}

#[derive(Debug, Copy, Clone, Default, Eq, PartialEq)]
pub struct MouseState {
	pub position: ViewportPosition,
	pub mouse_keys: MouseKeys,
}

impl MouseState {
	pub fn new() -> MouseState {
		Self::default()
	}

	pub fn from_pos(x: u32, y: u32) -> MouseState {
		MouseState {
			position: ViewportPosition { x, y },
			mouse_keys: MouseKeys::default(),
		}
	}
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Key {
	UnknownKey,
	KeyR,
	KeyM,
	KeyE,
	KeyL,
	KeyV,
	KeyX,
	KeyZ,
	KeyY,
	Key0,
	Key1,
	Key2,
	Key3,
	Key4,
	Key5,
	Key6,
	Key7,
	Key8,
	Key9,
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
