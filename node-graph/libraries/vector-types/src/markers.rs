//! Attribute markers whose value types live in this crate, with their name
//! constants for the string-keyed legacy readers and writers.

use core_types::attribute::Attribute;
use core_types::list::AnyAttributeValue;

core_types::attribute! {
	/// Gradient's spread behavior past its endpoints (`Pad`, `Reflect`, or `Repeat`).
	pub SpreadMethod("spread_method"): crate::gradient::GradientSpreadMethod;
	/// Gradient's shape (`Linear` or `Radial`).
	pub GradientType("gradient_type"): crate::gradient::GradientType;
}

/// Optional `Vector` that overrides the item's own geometry for click-target generation.
/// Used by the 'Text' node for per-glyph bounding-box rectangles so glyphs are selectable
/// by clicking anywhere within their bounds, not just the filled letterform. An absent
/// value means the item's own geometry is the click target.
pub struct EditorClickTarget;

impl Attribute for EditorClickTarget {
	const NAME: &'static str = "editor:click_target";
	type Value<'e> = Option<&'e crate::Vector>;

	unsafe fn read_erased(ptr: *const u8) -> Box<dyn AnyAttributeValue> {
		Box::new(unsafe { ptr.cast::<Option<&crate::Vector>>().read() }.cloned())
	}

	const REPARK: Option<unsafe fn(&dyn AnyAttributeValue, *mut u8, &core_types::arena::Arena) -> Option<()>> = {
		unsafe fn repark(value: &dyn AnyAttributeValue, dst: *mut u8, arena: &core_types::arena::Arena) -> Option<()> {
			let owned: &Option<crate::Vector> = value.as_any().downcast_ref().expect("an optional vector attribute replays its owned clone");
			let parked = match owned {
				Some(vector) => Some(arena.alloc(vector.clone())?.0),
				None => None,
			};
			unsafe { dst.cast::<Option<&crate::Vector>>().write(parked) };
			Some(())
		}
		Some(repark)
	};
}

core_types::attribute!(@register EditorClickTarget);

pub const ATTR_SPREAD_METHOD: &str = SpreadMethod::NAME;
pub const ATTR_GRADIENT_TYPE: &str = GradientType::NAME;
pub const ATTR_EDITOR_CLICK_TARGET: &str = EditorClickTarget::NAME;

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::attribute::info;
	use std::any::TypeId;

	#[test]
	fn the_census_carries_this_crates_names() {
		assert_eq!(info("gradient_type").unwrap().value_type, TypeId::of::<crate::gradient::GradientType>());
		assert_eq!(info("spread_method").unwrap().value_type, TypeId::of::<crate::gradient::GradientSpreadMethod>());
		assert_eq!(info("editor:click_target").unwrap().value_type, TypeId::of::<Option<&'static crate::Vector>>());
	}

	#[test]
	fn an_absent_click_target_defaults_to_none() {
		assert_eq!(<EditorClickTarget as Attribute>::default(), None);
	}
}
