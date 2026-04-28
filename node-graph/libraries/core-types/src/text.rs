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
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash, DynAny, node_macro::ChoiceType)]
#[widget(Radio)]
pub enum TextAlign {
	#[default]
	Left,
	Center,
	Right,
	#[label("Justify Left")]
	JustifyLeft,
	#[label("Justify Center")]
	JustifyCenter,
	#[label("Justify Right")]
	JustifyRight,
	#[label("Justify All")]
	JustifyAll,
}

impl From<TextAlign> for parley::Alignment {
	fn from(val: TextAlign) -> Self {
		match val {
			TextAlign::Left => parley::Alignment::Left,
			TextAlign::Center => parley::Alignment::Center,
			TextAlign::Right => parley::Alignment::Right,
			TextAlign::JustifyLeft | TextAlign::JustifyCenter | TextAlign::JustifyRight | TextAlign::JustifyAll => parley::Alignment::Justify,
		}
	}
}

impl TextAlign {

	pub fn last_line_correction(self) -> Option<parley::Alignment> {
		match self {
			Self::JustifyCenter => Some(parley::Alignment::Center),
			Self::JustifyRight => Some(parley::Alignment::Right),
			Self::JustifyAll => Some(parley::Alignment::Justify),
			_ => None,
		}
	}

	pub fn is_justify(self) -> bool {
		matches!(self, Self::JustifyLeft | Self::JustifyCenter | Self::JustifyRight | Self::JustifyAll)
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
