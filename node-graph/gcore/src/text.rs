mod font_cache;
mod to_path;

use dyn_any::DynAny;
pub use font_cache::*;
pub use to_path::*;

/// Alignment of lines of type within a text block.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash, DynAny, specta::Type, node_macro::ChoiceType)]
#[widget(Radio)]
pub enum TextAlign {
	#[default]
	Left,
	Center,
	Right,
	#[label("Justify")]
	JustifyLeft,
	// TODO: JustifyCenter, JustifyRight, JustifyAll
}

impl From<TextAlign> for parley::Alignment {
	fn from(val: TextAlign) -> Self {
		match val {
			TextAlign::Left => parley::Alignment::Left,
			TextAlign::Center => parley::Alignment::Middle,
			TextAlign::Right => parley::Alignment::Right,
			TextAlign::JustifyLeft => parley::Alignment::Justified,
		}
	}
}
