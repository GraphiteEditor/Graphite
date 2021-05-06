use crate::tools::ToolType;
use crate::Color;
use bitflags::bitflags;

use serde::{Deserialize, Serialize};

#[doc(inline)]
pub use document_core::DocumentResponse;

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
	LmbDown(MouseState),
	RmbDown(MouseState),
	MmbDown(MouseState),
	LmbUp(MouseState),
	RmbUp(MouseState),
	MmbUp(MouseState),
	MouseMove(ViewportPosition),
	KeyUp(Key),
	KeyDown(Key),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(C)]
pub enum ToolResponse {
	SetActiveTool { tool_name: String },
	UpdateCanvas { document: String },
}

impl fmt::Display for ToolResponse {
	fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		use ToolResponse::*;

		let name = match_variant_name!(match (self) {
			SetActiveTool,
			UpdateCanvas,
		});

		formatter.write_str(name)
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(C)]
// TODO - Make Copy when possible
pub enum Response {
	Tool(ToolResponse),
	Document(DocumentResponse),
}

impl From<ToolResponse> for Response {
	fn from(response: ToolResponse) -> Self {
		Response::Tool(response)
	}
}

impl From<DocumentResponse> for Response {
	fn from(response: DocumentResponse) -> Self {
		Response::Document(response)
	}
}

impl fmt::Display for Response {
	fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		use Response::*;

		let name = match_variant_name!(match (self) {
			Tool,
			Document
		});
		let appendix = match self {
			Tool(t) => t.to_string(),
			Document(d) => d.to_string(),
		};

		formatter.write_str(format!("{}::{}", name, appendix).as_str())
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

// From top left at -1,-1
#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub struct CanvasPosition {
	pub x: f64,
	pub y: f64,
}

// origin is top left
#[derive(Debug, Copy, Clone, Default, Eq, PartialEq)]
pub struct ViewportPosition {
	pub x: u32,
	pub y: u32,
}

// A position on the infinite canvas
impl CanvasPosition {
	pub fn distance(&self, other: &Self) -> f64 {
		let x_diff = other.x - self.x;
		let y_diff = other.y - self.y;
		f64::sqrt(x_diff * x_diff + y_diff * y_diff)
	}
	pub fn rotate(&mut self, theta: f64) -> &mut Self {
		let cosine = theta.cos();
		let sine = theta.sin();
		log::info!("Before {},{}", self.x, self.y);
		self.x = self.x * cosine - self.y * sine;
		self.y = self.x * sine + self.y * cosine;
		log::info!("After {},{}", self.x, self.y);
		self
	}
}

// The location of the viewport (or anything else) in the canvas
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct CanvasTransform {
	pub location: CanvasPosition,
	pub rotation: f64,
	pub scale: f64,
}

impl Default for CanvasTransform {
	fn default() -> Self {
		Self {
			location: CanvasPosition::default(),
			scale: 1.,
			rotation: 45.,
		}
	}
}

impl ViewportPosition {
	pub fn distance(&self, other: &Self) -> f64 {
		let x_diff = other.x as f64 - self.x as f64;
		let y_diff = other.y as f64 - self.y as f64;
		f64::sqrt(x_diff * x_diff + y_diff * y_diff)
	}
	pub fn to_canvas_position(&self, canvas_transform: &CanvasTransform) -> CanvasPosition {
		*CanvasPosition {
			x: self.x as f64 * canvas_transform.scale + canvas_transform.location.x,
			y: self.y as f64 * canvas_transform.scale + canvas_transform.location.y,
		}
		.rotate(canvas_transform.rotation)
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
	KeyP,
	KeyV,
	KeyX,
	KeyZ,
	KeyY,
	KeyEnter,
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
	KeyShift,
	KeyAlt,
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
