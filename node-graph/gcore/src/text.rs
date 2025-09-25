mod font_cache;
mod path_builder;
mod text_context;
mod to_path;

use dyn_any::DynAny;
pub use font_cache::*;
pub use text_context::TextContext;
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

#[derive(PartialEq, Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct TypesettingConfig {
	pub font_size: f64,
	pub line_height_ratio: f64,
	pub character_spacing: f64,
	pub max_width: Option<f64>,
	pub max_height: Option<f64>,
	pub tilt: f64,
	pub align: TextAlign,
}

impl Default for TypesettingConfig {
	fn default() -> Self {
		Self {
			font_size: 24.,
			line_height_ratio: 1.2,
			character_spacing: 0.,
			max_width: None,
			max_height: None,
			tilt: 0.,
			align: TextAlign::default(),
		}
	}
}
