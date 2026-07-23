//! Typed attribute keys whose value types live in this crate. See `core_types::attr` for the trait and macro.

use graphene_resource::Resource;

node_macro::attrs! {
	/// Text item's font, as a resource of the loaded font file.
	Font: Resource,
	/// Text item's horizontal alignment of lines within the block.
	TextAlign: crate::TextAlign,
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::attr::Attr;

	// Key names are the stored document format — pinned as literals so a key rename shows up as a breaking change.
	#[test]
	fn key_names_are_pinned() {
		assert_eq!(Font::name(), "font");
		assert_eq!(TextAlign::name(), "text_align");
	}
}
