use crate::helpers::match_string_to_enum;

use editor::input::keyboard::Key;
use editor::viewport_tools::tool::ToolType;

use graphene::boolean_ops::BooleanOperation;
use graphene::layers::blend_mode::BlendMode;
use graphene::layers::style::ViewMode;

pub fn translate_tool_type(name: &str) -> Option<ToolType> {
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

pub fn translate_blend_mode(blend_mode_svg_style_name: &str) -> Option<BlendMode> {
	use BlendMode::*;

	let blend_mode = match blend_mode_svg_style_name {
		"Normal" => Normal,
		"Multiply" => Multiply,
		"Darken" => Darken,
		"ColorBurn" => ColorBurn,
		"Screen" => Screen,
		"Lighten" => Lighten,
		"ColorDodge" => ColorDodge,
		"Overlay" => Overlay,
		"SoftLight" => SoftLight,
		"HardLight" => HardLight,
		"Difference" => Difference,
		"Exclusion" => Exclusion,
		"Hue" => Hue,
		"Saturation" => Saturation,
		"Color" => Color,
		"Luminosity" => Luminosity,
		_ => return None,
	};

	Some(blend_mode)
}

pub fn translate_key(name: &str) -> Key {
	use Key::*;

	log::trace!("Key event received: {}", name);

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
		" " => KeySpace,
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

pub fn translate_boolean_operation(operation: &str) -> Option<BooleanOperation> {
	match operation {
		"Union" => Some(BooleanOperation::Union),
		"Difference" => Some(BooleanOperation::Difference),
		"Intersection" => Some(BooleanOperation::Intersection),
		"SubtractFront" => Some(BooleanOperation::SubtractFront),
		"SubtractBack" => Some(BooleanOperation::SubtractBack),
		_ => None,
	}
}

pub fn translate_view_mode(name: &str) -> Option<ViewMode> {
	Some(match name {
		"Normal" => ViewMode::Normal,
		"Outline" => ViewMode::Outline,
		"Pixels" => ViewMode::Pixels,
		_ => return None,
	})
}
