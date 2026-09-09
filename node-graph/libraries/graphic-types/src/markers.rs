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
}

/// The item's ordered list of paint passes. An absent or empty value is the undeclared
/// state that inherits the nearest ancestor's appearance through the cascade, so both
/// read as `None`. The stored form is the bare [`crate::appearance::Appearance`], the
/// shape the appearance writers use, which the `attribute!` macro's optional-reference
/// arm cannot express.
pub struct Appearance;

// SAFETY: `read_erased` produces the bare owned appearance `from_stored` reads, `REPARK`
// re-parks that same form, and the empty appearance collapses to the `None` default at
// every read seam.
unsafe impl Attribute for Appearance {
	const NAME: &'static str = "appearance";
	type Value<'e> = Option<&'e crate::appearance::Appearance>;

	fn from_stored<'a>(stored: &'a dyn std::any::Any) -> Option<Self::Value<'a>> {
		stored.downcast_ref::<crate::appearance::Appearance>().map(crate::appearance::Appearance::declared)
	}

	unsafe fn read_erased(ptr: *const u8) -> Box<dyn core_types::list::AnyAttributeValue> {
		Box::new(unsafe { ptr.cast::<Option<&crate::appearance::Appearance>>().read() }.cloned().unwrap_or_default())
	}

	const REPARK: Option<core_types::list::ReparkFn> = {
		unsafe fn repark(value: &dyn core_types::list::AnyAttributeValue, dst: *mut u8, arena: &core_types::arena::Arena) -> Option<()> {
			let owned: &crate::appearance::Appearance = value.as_any().downcast_ref().expect("an appearance attribute replays its bare owned clone");
			let parked = match owned.is_empty() {
				true => None,
				false => {
					let (parked, _) = arena.alloc(owned.clone())?;
					Some(&*parked)
				}
			};
			// SAFETY: the slot is a live field of this marker's value type.
			unsafe { dst.cast::<Option<&crate::appearance::Appearance>>().write(parked) };
			Some(())
		}
		Some(repark)
	};
}

core_types::attribute!(@register Appearance);

/// One coverage row's paint, a bare graphic riding the coverage list as a column. Absent
/// or empty paint draws nothing, so both read as `None`; the stored form is the bare
/// [`Graphic`], the shape [`crate::appearance`]'s row writers use.
pub struct Paint;

// SAFETY: as for `Appearance`, at the bare `Graphic` stored form.
unsafe impl Attribute for Paint {
	const NAME: &'static str = "paint";
	type Value<'e> = Option<&'e Graphic<'static>>;

	fn from_stored<'a>(stored: &'a dyn std::any::Any) -> Option<Self::Value<'a>> {
		stored.downcast_ref::<Graphic<'static>>().map(|paint| (!paint.is_empty()).then_some(paint))
	}

	unsafe fn read_erased(ptr: *const u8) -> Box<dyn core_types::list::AnyAttributeValue> {
		Box::new(unsafe { ptr.cast::<Option<&Graphic<'static>>>().read() }.cloned().unwrap_or_default())
	}

	const REPARK: Option<core_types::list::ReparkFn> = {
		unsafe fn repark(value: &dyn core_types::list::AnyAttributeValue, dst: *mut u8, arena: &core_types::arena::Arena) -> Option<()> {
			let owned: &Graphic<'static> = value.as_any().downcast_ref().expect("a paint attribute replays its bare owned clone");
			let parked = match owned.is_empty() {
				true => None,
				false => {
					let (parked, _) = arena.alloc(owned.clone())?;
					Some(&*parked)
				}
			};
			// SAFETY: the slot is a live field of this marker's value type.
			unsafe { dst.cast::<Option<&Graphic<'static>>>().write(parked) };
			Some(())
		}
		Some(repark)
	};
}

core_types::attribute!(@register Paint);

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
