//! Attribute markers whose value types live in this crate, with their name
//! constants for the string-keyed legacy readers and writers.

use core_types::attribute::Attribute;

core_types::attribute! {
	/// Text item's font, as a resource of the loaded font file.
	pub Font("font"): &graphene_resource::Resource;
	/// Text item's horizontal alignment of lines within the block.
	pub TextAlign("text_align"): crate::TextAlign;
}

pub const ATTR_FONT: &str = Font::NAME;
pub const ATTR_TEXT_ALIGN: &str = TextAlign::NAME;

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::attribute::info;
	use std::any::TypeId;

	#[test]
	fn the_census_carries_this_crates_names() {
		assert_eq!(info("font").unwrap().value_type, TypeId::of::<&'static graphene_resource::Resource>());
		assert_eq!(info("text_align").unwrap().value_type, TypeId::of::<crate::TextAlign>());
	}

	#[test]
	fn the_font_default_is_the_empty_resource() {
		assert!(<Font as Attribute>::default().is_empty());
	}
}
