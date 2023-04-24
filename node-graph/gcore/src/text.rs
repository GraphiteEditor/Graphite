mod font_cache;
mod to_path;

use crate::EditorApi;
pub use font_cache::*;
pub use to_path::*;

use crate::Node;

pub struct TextGenerator<Text, FontName, Size> {
	text: Text,
	font_name: FontName,
	font_size: Size,
}

impl<'a: 'input, 'input, Text, FontName, Size> Node<'input, EditorApi<'a>> for TextGenerator<Text, FontName, Size>
where
	Text: Node<'input, (), Output = String>,
	FontName: Node<'input, (), Output = Font>,
	Size: Node<'input, (), Output = f64>,
{
	type Output = crate::vector::VectorData;
	#[inline]
	fn eval(&'input self, editor: EditorApi<'a>) -> Self::Output {
		let text = self.text.eval(());
		let font = self.font_name.eval(());
		let font_size = self.font_size.eval(());

		let buzz_face = editor.font_cache.and_then(|cache| cache.get(&font)).map(|data| load_face(data));
		crate::vector::VectorData::from_subpaths(to_path(&text, buzz_face, font_size, None))
	}
}
impl<'input, Text, FontName, Size> TextGenerator<Text, FontName, Size>
where
	Text: Node<'input, (), Output = String>,
	FontName: Node<'input, (), Output = Font>,
	Size: Node<'input, (), Output = f64>,
{
	pub const fn new(text: Text, font_name: FontName, font_size: Size) -> Self {
		Self { text, font_name, font_size }
	}
}
