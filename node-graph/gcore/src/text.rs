mod font_cache;
mod to_path;

use dyn_any::StaticType;
pub use font_cache::*;
pub use to_path::*;

/// Alignment of a layout.
#[derive(Copy, Clone, Default, PartialEq, Eq, Debug, node_macro::ChoiceType, serde::Deserialize, serde::Serialize, Hash)]
#[repr(u8)]
pub enum TextAlignment {
	/// This is [`Alignment::Left`] for LTR text and [`Alignment::Right`] for RTL text.
	#[default]
	Start,
	/// This is [`Alignment::Right`] for LTR text and [`Alignment::Left`] for RTL text.
	End,
	/// Align content to the left edge.
	///
	/// For alignment that should be aware of text direction, use [`Alignment::Start`] or
	/// [`Alignment::End`] instead.
	Left,
	/// Align each line centered within the container.
	Middle,
	/// Align content to the right edge.
	///
	/// For alignment that should be aware of text direction, use [`Alignment::Start`] or
	/// [`Alignment::End`] instead.
	Right,
	/// Justify each line by spacing out content, except for the last line.
	Justified,
}

impl Into<parley::Alignment> for TextAlignment {
	fn into(self) -> parley::Alignment {
		match self {
			TextAlignment::Start => parley::Alignment::Start,
			TextAlignment::End => parley::Alignment::End,
			TextAlignment::Left => parley::Alignment::Left,
			TextAlignment::Middle => parley::Alignment::Middle,
			TextAlignment::Right => parley::Alignment::Right,
			TextAlignment::Justified => parley::Alignment::Justified,
		}
	}
}

unsafe impl StaticType for TextAlignment {
	type Static = TextAlignment;
}
