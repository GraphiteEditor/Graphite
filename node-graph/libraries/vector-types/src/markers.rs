//! Attribute markers whose value types live in this crate, with their name
//! constants for the string-keyed legacy readers and writers.

use core_types::attribute::Attribute;

core_types::attribute! {
	/// Gradient's spread behavior past its endpoints (`Pad`, `Reflect`, `Repeat`, or `Clear`).
	pub GradientSpread("gradient_spread"): crate::gradient::GradientSpread;
	/// Gradient's `GradientSpace`, the color space its stops blend in.
	pub GradientSpace("gradient_space"): crate::gradient::GradientSpace;
	/// Gradient's `GradientHueDirection`, the hue path polar spaces interpolate along.
	pub GradientHueDirection("gradient_hue_direction"): crate::gradient::GradientHueDirection;
	/// Gradient's `bool` (implicit default `false`) for treating the stop list as a cycle, where a wrapped interval
	/// interpolates from the last stop through the 1|0 boundary back to the first.
	pub GradientCyclic("gradient_cyclic"): bool;
	/// Gradient's shape (`Linear` or `Radial`).
	pub GradientForm("gradient_form"): crate::gradient::GradientForm;
	/// Optional `Vector` that overrides the item's own geometry for click-target generation.
	/// Used by the 'Text' node for per-glyph bounding-box rectangles so glyphs are selectable
	/// by clicking anywhere within their bounds, not just the filled letterform. An absent
	/// value means the item's own geometry is the click target.
	pub EditorClickTarget("editor:click_target"): Option<&crate::Vector>;
}

// The value types a name-generic attribute can name here, so a compile-time
// named write or read reaches this crate's enums like any other plain value.
core_types::named_value! {
	for crate::gradient::GradientSpread;
	for crate::gradient::GradientForm;
	for crate::gradient::GradientSpace;
	for crate::gradient::GradientHueDirection;
}

pub const ATTR_GRADIENT_SPREAD: &str = GradientSpread::NAME;
pub const ATTR_GRADIENT_FORM: &str = GradientForm::NAME;
pub const ATTR_GRADIENT_CYCLIC: &str = GradientCyclic::NAME;
pub const ATTR_GRADIENT_SPACE: &str = GradientSpace::NAME;
pub const ATTR_GRADIENT_HUE_DIRECTION: &str = GradientHueDirection::NAME;
pub const ATTR_EDITOR_CLICK_TARGET: &str = EditorClickTarget::NAME;

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::attribute::info;
	use std::any::TypeId;

	#[test]
	fn the_census_carries_this_crates_names() {
		assert_eq!(info("gradient_form").unwrap().value_type, TypeId::of::<crate::gradient::GradientForm>());
		assert_eq!(info("gradient_spread").unwrap().value_type, TypeId::of::<crate::gradient::GradientSpread>());
		assert_eq!(info("editor:click_target").unwrap().value_type, TypeId::of::<Option<&'static crate::Vector>>());
	}

	#[test]
	fn an_absent_click_target_defaults_to_none() {
		assert_eq!(<EditorClickTarget as Attribute>::default(), None);
	}
}
