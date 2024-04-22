mod font_cache;
mod to_path;

use core::future::Future;

use crate::vector::VectorData;
use crate::Color;
use crate::{application_io::EditorApi, transform::Footprint};
use alloc::sync::Arc;
use dyn_any::{DynAny, StaticType};
pub use font_cache::*;
use glam::Vec2;
use node_macro::node_fn;
pub use to_path::*;

use crate::Node;

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TextSpan {
	pub offset: usize,
	pub bold: Option<f32>,
	pub italic: Option<f32>,
	pub font: Arc<Font>,
	pub font_size: f32,
	pub letter_spacing: f32,
	pub word_spacing: f32,
	pub line_spacing: f32,
	pub kerning: Vec2,
	pub color: Color,
}

impl core::hash::Hash for TextSpan {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.offset.hash(state);
		self.bold.map(|x| x.to_bits()).hash(state);
		self.italic.map(|x| x.to_bits()).hash(state);
		self.font.hash(state);
		self.font_size.to_bits().hash(state);
		self.letter_spacing.to_bits().hash(state);
		self.word_spacing.to_bits().hash(state);
		self.line_spacing.to_bits().hash(state);
		self.kerning.x.to_bits().hash(state);
		self.kerning.y.to_bits().hash(state);
		self.color.hash(state);
	}
}
impl TextSpan {
	pub fn new(font: impl Into<Arc<Font>>, font_size: f32) -> Self {
		Self {
			offset: 0,
			bold: None,
			italic: None,
			font: font.into(),
			font_size,
			letter_spacing: 0.,
			word_spacing: 0.,
			line_spacing: 1.,
			kerning: Vec2::ZERO,
			color: Color::BLACK,
		}
	}

	pub fn offset(mut self, offset: usize) -> Self {
		self.offset = offset;
		self
	}
	pub fn italic(mut self, italic: Option<f32>) -> Self {
		self.italic = italic;
		self
	}
	pub fn bold(mut self, bold: Option<f32>) -> Self {
		self.bold = bold;
		self
	}
	pub fn letter_spacing(mut self, letter_spacing: f32) -> Self {
		self.letter_spacing = letter_spacing;
		self
	}
	pub fn word_spacing(mut self, word_spacing: f32) -> Self {
		self.word_spacing = word_spacing;
		self
	}
}
#[derive(Clone, Debug, Default, Hash, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RichText {
	pub text: String,
	pub spans: Vec<TextSpan>,
}

impl RichText {
	pub fn new(text: impl Into<String>, spans: impl Into<Vec<TextSpan>>) -> Self {
		Self {
			text: text.into(),
			spans: spans.into(),
		}
	}
}

pub struct TextGeneratorNode<RichTextNode, LineLengthNode, PathNode> {
	text: RichTextNode,
	line_length: LineLengthNode,
	path: PathNode,
}

#[node_fn(TextGeneratorNode)]
async fn generate_text<'a: 'input, T, FV: Future<Output = VectorData>>(
	editor: EditorApi<'a, T>,
	text: RichText,
	line_length: f64,
	path: impl Node<Footprint, Output = FV>,
) -> crate::vector::VectorData {
	let path = self.path.eval(editor.render_config.viewport).await;
	crate::vector::VectorData::from_subpaths(rich_text_to_path(&text, line_length, &path, editor.font_cache))
}
