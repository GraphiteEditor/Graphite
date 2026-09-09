//! Attribute markers whose value types live in this crate, with their name
//! constants for the string-keyed legacy readers and writers.
//!
//! The list-valued markers may carry native [`Graphic::Group`] content: the
//! registered deep field glue owns it across persistence seams, and legacy
//! products convert it through [`crate::graphic::map_paint_attrs_to_legacy`].

use crate::Graphic;
use core_types::attribute::Attribute;
use core_types::list::List;

core_types::attribute! {
	/// Vector graphics object's filled area paint, a graphic list in the canonical paint form.
	/// An absent value means no fill.
	pub Fill("fill"): Option<&List<Graphic<'static>>>;
	/// Vector graphics object's stroke paint, a graphic list in the canonical paint form.
	/// An absent value means no stroke paint.
	pub Stroke("stroke"): Option<&List<Graphic<'static>>>;
	/// Snapshot of the upstream content that fed into a destructive merge (Boolean Operation,
	/// Rasterize, etc.), so the editor can still surface click targets for the original child
	/// layers after their content has been collapsed.
	pub EditorMergedLayers("editor:merged_layers"): Option<&List<Graphic<'static>>>;
	/// The item's ordered list of paint passes. An absent or empty value is the undeclared
	/// state that inherits the nearest ancestor's appearance through the cascade.
	pub Appearance("appearance"): Option<&crate::appearance::Appearance>;
	/// One coverage row's paint, a bare graphic riding the coverage list as a column.
	/// Absent when the coverage paints nothing.
	pub Paint("paint"): Option<&Graphic<'static>>;
}

pub const ATTR_FILL: &str = Fill::NAME;
pub const ATTR_STROKE: &str = Stroke::NAME;
pub const ATTR_EDITOR_MERGED_LAYERS: &str = EditorMergedLayers::NAME;
pub const ATTR_APPEARANCE: &str = Appearance::NAME;
pub const ATTR_PAINT: &str = Paint::NAME;

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
		assert_eq!(info("appearance").unwrap().value_type, TypeId::of::<Option<&'static crate::appearance::Appearance>>());
		assert_eq!(info("paint").unwrap().value_type, TypeId::of::<Option<&'static Graphic>>());
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
