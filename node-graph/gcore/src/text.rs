mod font_cache;
mod to_path;

pub use font_cache::*;
pub use to_path::*;

use crate::Node;

pub struct TextGenerator<Font, Size> {
	font: Font,
	font_size: Size,
}
#[node_macro::node_fn(TextGenerator)]
fn text_generator(text: String, font: Font, font_size: f64) -> crate::vector::VectorData {
	crate::vector::VectorData::from_subpaths(to_path(&text, None, font_size, None))
}
