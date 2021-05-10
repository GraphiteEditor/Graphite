use crate::shims::Error;
use editor_core::events;
use editor_core::tools::{SelectAppendMode, ToolType};
use editor_core::Color as InnerColor;
use events::Response;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Color(InnerColor);

#[wasm_bindgen]
impl Color {
	#[wasm_bindgen(constructor)]
	pub fn new(red: f32, green: f32, blue: f32, alpha: f32) -> Result<Color, JsValue> {
		match InnerColor::from_rgbaf32(red, green, blue, alpha) {
			Some(v) => Ok(Self(v)),
			None => Err(Error::new("invalid color").into()),
		}
	}
}

impl Color {
	pub fn inner(&self) -> InnerColor {
		self.0
	}
}

#[derive(Serialize, Deserialize)]
pub struct WasmResponse(Response);

impl WasmResponse {
	pub fn new(response: Response) -> Self {
		Self(response)
	}
}

macro_rules! match_string_to_enum {
    (match ($e:expr) {$($var:ident),* $(,)?}) => {
		match $e {
			$(
			stringify!($var) => Some($var),
			)*
			_ => None
		}
	};
}

pub fn translate_tool(name: &str) -> Option<ToolType> {
	use ToolType::*;

	match_string_to_enum!(match (name) {
		Select,
		Crop,
		Navigate,
		Eyedropper,
		Text,
		Fill,
		Gradient,
		Brush,
		Heal,
		Clone,
		Patch,
		BlurSharpen,
		Relight,
		Path,
		Pen,
		Freehand,
		Spline,
		Line,
		Rectangle,
		Ellipse,
		Shape
	})
}

pub fn translate_append_mode(name: &str) -> Option<SelectAppendMode> {
	use SelectAppendMode::*;

	match_string_to_enum!(match (name) {
		New,
		Add,
		Subtract,
		Intersect
	})
}

pub fn translate_key(name: &str) -> events::Key {
	log::trace!("pressed key: {}", name);
	use events::Key::*;
	match name {
		"e" => KeyE,
		"v" => KeyV,
		"l" => KeyL,
		"p" => KeyP,
		"r" => KeyR,
		"m" => KeyM,
		"x" => KeyX,
		"z" => KeyZ,
		"y" => KeyY,
		"0" => Key0,
		"1" => Key1,
		"2" => Key2,
		"3" => Key3,
		"4" => Key4,
		"5" => Key5,
		"6" => Key6,
		"7" => Key7,
		"8" => Key8,
		"9" => Key9,
		"Enter" => KeyEnter,
		"Shift" => KeyShift,
		"CapsLock" => KeyCaps,
		"Control" => KeyControl,
		"Alt" => KeyAlt,
		"Escape" => KeyEscape,
		_ => UnknownKey,
	}
}
