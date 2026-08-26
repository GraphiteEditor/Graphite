//! Attribute markers whose value types live in this crate, with their name
//! constants for the string-keyed legacy readers and writers.
//!
//! The list-valued markers deep-copy by cloning the list, so their values must
//! stay free of arena-borrowing content such as [`Graphic::Group`].

use crate::Graphic;
use core_types::attribute::Attribute;
use core_types::list::List;

core_types::attribute! {
	/// Vector graphics object's filled area paint, a graphic list in the canonical paint form.
	/// An absent value means no fill.
	pub Fill("fill"): Option<&List<Graphic>>;
	/// Vector graphics object's stroke paint, a graphic list in the canonical paint form.
	/// An absent value means no stroke paint.
	pub Stroke("stroke"): Option<&List<Graphic>>;
	/// Snapshot of the upstream content that fed into a destructive merge (Boolean Operation,
	/// Rasterize, etc.), so the editor can still surface click targets for the original child
	/// layers after their content has been collapsed.
	pub EditorMergedLayers("editor:merged_layers"): Option<&List<Graphic>>;
}

pub const ATTR_FILL: &str = Fill::NAME;
pub const ATTR_STROKE: &str = Stroke::NAME;
pub const ATTR_EDITOR_MERGED_LAYERS: &str = EditorMergedLayers::NAME;

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::attribute::info;
	use std::any::TypeId;

	#[test]
	fn the_census_carries_this_crates_names() {
		for name in ["fill", "stroke", "editor:merged_layers"] {
			assert_eq!(info(name).unwrap().value_type, TypeId::of::<Option<&'static List<Graphic>>>());
		}
	}

	#[test]
	fn an_absent_paint_defaults_to_none() {
		assert_eq!(<Fill as Attribute>::default(), None);
	}

	#[test]
	fn a_paint_marker_reads_back_what_the_paint_writer_stored() {
		use core_types::lane::LaneSource;

		let paint = List::new_from_element(Graphic::default());
		let mut list = List::new_from_element(Graphic::default());
		crate::graphic::set_paint_attribute_at(&mut list, 0, ATTR_FILL, paint.clone());

		assert_eq!(list.attr::<Fill>(0), Some(&paint));
		assert_eq!(list.attr::<Stroke>(0), None);
	}
}
