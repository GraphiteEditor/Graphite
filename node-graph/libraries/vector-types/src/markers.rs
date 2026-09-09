//! Attribute markers whose value types live in this crate, with their name
//! constants for the string-keyed legacy readers and writers.
//!
//! The gradient names follow master's vocabulary: `gradient_form` for the
//! shape and `gradient_spread` for the endpoint behavior, replacing the
//! earlier `gradient_type` and `spread_method`.

use core_types::attribute::Attribute;

core_types::attribute! {
	/// Gradient's spread behavior past its endpoints (`Pad`, `Reflect`, `Repeat`, or `Clear`).
	pub GradientSpread("gradient_spread"): crate::gradient::GradientSpread;
	/// Gradient's shape (`Linear` or `Radial`).
	pub GradientForm("gradient_form"): crate::gradient::GradientForm;
	/// The color space a gradient's stops interpolate in.
	pub GradientSpace("gradient_space"): crate::gradient::GradientSpace;
	/// Which way around the hue wheel the stops interpolate when the space is polar.
	pub GradientHueDirection("gradient_hue_direction"): crate::gradient::GradientHueDirection;
	/// The path a gradient's stops interpolate along, so whether the ramp jumps,
	/// turns corners, or flows smoothly through them.
	pub GradientInterpolation("gradient_interpolation"): crate::gradient::GradientInterpolation;
	/// Whether the stop list is a cycle, so a wrapped interval interpolates from the
	/// last stop through the 1|0 boundary back to the first.
	pub GradientCyclic("gradient_cyclic"): bool;
	/// A gradient stop's position from 0 to 1, on the `List<Color>` inside a `Gradient`.
	/// An absent column distributes the stops evenly.
	pub Position("position"): f64;
	/// A gradient stop's midpoint factor from 0 to 1 across the distance to the next stop.
	pub Midpoint("midpoint"): f64 = 0.5;
	/// A stroke coverage's line thickness.
	pub Weight("weight"): f64;
	/// A stroke coverage's dash phase offset distance.
	pub DashOffset("dash_offset"): f64;
	/// A stroke coverage's line cap.
	pub Cap("cap"): crate::vector::style::StrokeCap;
	/// A stroke coverage's line join.
	pub Join("join"): crate::vector::style::StrokeJoin;
	/// A stroke coverage's miter limit threshold.
	pub JoinMiterLimit("join_miter_limit"): f64 = 4.;
	/// A stroke coverage's alignment across the path.
	pub Align("align"): crate::vector::style::StrokeAlign;
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
}

pub const ATTR_GRADIENT_SPREAD: &str = GradientSpread::NAME;
pub const ATTR_GRADIENT_FORM: &str = GradientForm::NAME;
pub const ATTR_GRADIENT_SPACE: &str = GradientSpace::NAME;
pub const ATTR_GRADIENT_HUE_DIRECTION: &str = GradientHueDirection::NAME;
pub const ATTR_GRADIENT_INTERPOLATION: &str = GradientInterpolation::NAME;
pub const ATTR_GRADIENT_CYCLIC: &str = GradientCyclic::NAME;
pub const ATTR_POSITION: &str = Position::NAME;
pub const ATTR_MIDPOINT: &str = Midpoint::NAME;
pub const ATTR_WEIGHT: &str = Weight::NAME;
pub const ATTR_DASH_OFFSET: &str = DashOffset::NAME;
pub const ATTR_CAP: &str = Cap::NAME;
pub const ATTR_JOIN: &str = Join::NAME;
pub const ATTR_JOIN_MITER_LIMIT: &str = JoinMiterLimit::NAME;
pub const ATTR_ALIGN: &str = Align::NAME;
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

	#[test]
	fn an_absent_midpoint_defaults_to_the_halfway_point() {
		assert_eq!(<Midpoint as Attribute>::default(), 0.5);
	}
}
