use crate::shims::Error;
use editor_core::events;
use editor_core::tools::{SelectAppendMode, ToolType};
use editor_core::Color as InnerColor;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Color(InnerColor);

#[wasm_bindgen]
impl Color {
	#[wasm_bindgen(constructor)]
	pub fn new(red: f32, green: f32, blue: f32, alpha: f32) -> Result<Color, JsValue> {
		match InnerColor::from_rgbaf32(red, green, blue, alpha) {
			Ok(v) => Ok(Self(v)),
			Err(e) => Err(Error::new(&e.to_string()).into()),
		}
	}
}

impl Color {
	pub fn inner(&self) -> InnerColor {
		self.0
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
		Sample,
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
	use events::Key as K;
	match name {
		"e" => K::KeyE,
		"v" => K::KeyV,
		"r" => K::KeyR,
		"m" => K::KeyM,
		"x" => K::KeyX,
		"0" => K::Key0,
		"1" => K::Key1,
		"2" => K::Key2,
		"3" => K::Key3,
		"4" => K::Key4,
		"5" => K::Key5,
		"6" => K::Key6,
		"7" => K::Key7,
		"8" => K::Key8,
		"9" => K::Key9,
		_ => K::UnknownKey,
	}
}
