use crate::shims::Error;
use editor::consts::FILE_SAVE_SUFFIX;
use editor::input::keyboard::Key;
use editor::tool::{SelectAppendMode, ToolType};
use editor::Color as InnerColor;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn file_save_suffix() -> String {
	FILE_SAVE_SUFFIX.into()
}

#[wasm_bindgen]
pub fn i32_max() -> i32 {
	i32::MAX
}

#[wasm_bindgen]
pub fn i32_min() -> i32 {
	i32::MIN
}

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

pub fn translate_key(name: &str) -> Key {
	log::trace!("Key event received: {}", name);
	use Key::*;
	match name.to_lowercase().as_str() {
		"a" => KeyA,
		"b" => KeyB,
		"c" => KeyC,
		"d" => KeyD,
		"e" => KeyE,
		"f" => KeyF,
		"g" => KeyG,
		"h" => KeyH,
		"i" => KeyI,
		"j" => KeyJ,
		"k" => KeyK,
		"l" => KeyL,
		"m" => KeyM,
		"n" => KeyN,
		"o" => KeyO,
		"p" => KeyP,
		"q" => KeyQ,
		"r" => KeyR,
		"s" => KeyS,
		"t" => KeyT,
		"u" => KeyU,
		"v" => KeyV,
		"w" => KeyW,
		"x" => KeyX,
		"y" => KeyY,
		"z" => KeyZ,
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
		"enter" => KeyEnter,
		"=" => KeyEquals,
		"+" => KeyPlus,
		"-" => KeyMinus,
		"shift" => KeyShift,
		// When using linux + chrome + the neo keyboard layout, the shift key is recognized as caps
		"capslock" => KeyShift,
		"control" => KeyControl,
		"delete" => KeyDelete,
		"backspace" => KeyBackspace,
		"alt" => KeyAlt,
		"escape" => KeyEscape,
		"tab" => KeyTab,
		"arrowup" => KeyArrowUp,
		"arrowdown" => KeyArrowDown,
		"arrowleft" => KeyArrowLeft,
		"arrowright" => KeyArrowRight,
		"[" => KeyLeftBracket,
		"]" => KeyRightBracket,
		"{" => KeyLeftCurlyBracket,
		"}" => KeyRightCurlyBracket,
		"pageup" => KeyPageUp,
		"pagedown" => KeyPageDown,
		"," => KeyComma,
		"." => KeyPeriod,
		_ => UnknownKey,
	}
}
