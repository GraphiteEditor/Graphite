//! Attribute markers whose value types live in this crate, with their name
//! constants for the string-keyed legacy readers and writers.

use core_types::attribute::Attribute;

core_types::attribute! {
	/// Gradient's spread behavior past its endpoints (`Pad`, `Reflect`, or `Repeat`).
	pub SpreadMethod("spread_method"): crate::gradient::GradientSpreadMethod;
	/// Gradient's shape (`Linear` or `Radial`).
	pub GradientType("gradient_type"): crate::gradient::GradientType;
	/// Optional `Vector` that overrides the item's own geometry for click-target generation.
	/// Used by the 'Text' node for per-glyph bounding-box rectangles so glyphs are selectable
	/// by clicking anywhere within their bounds, not just the filled letterform. An absent
	/// value means the item's own geometry is the click target.
	pub EditorClickTarget("editor:click_target"): Option<&crate::Vector>;
	/// Stroke coverage's line thickness. Absent when equal to the stroke default.
	pub Weight("weight"): f64;
	/// Stroke coverage's dash lengths, alternating dash and gap. Absent when the pattern is solid.
	pub DashPattern("dash_pattern"): Option<&Vec<f64>>;
	/// Stroke coverage's phase offset into the dash pattern. Absent when equal to the stroke default.
	pub DashOffset("dash_offset"): f64;
	/// Stroke coverage's shape at open endpoints. Absent when equal to the stroke default.
	pub Cap("cap"): crate::vector::style::StrokeCap;
	/// Stroke coverage's corner curvature. Absent when equal to the stroke default.
	pub Join("join"): crate::vector::style::StrokeJoin;
	/// Stroke coverage's miter-to-bevel conversion threshold. Absent when equal to the stroke default.
	pub JoinMiterLimit("join_miter_limit"): f64 = 4.;
	/// Stroke coverage's alignment to the path centerline. Absent when equal to the stroke default.
	pub Align("align"): crate::vector::style::StrokeAlign;
}

// The value types a name-generic attribute can name here, so a compile-time
// named write or read reaches this crate's enums like any other plain value.
core_types::named_value! {
	for crate::gradient::GradientSpreadMethod;
	for crate::gradient::GradientType;
}

pub const ATTR_SPREAD_METHOD: &str = SpreadMethod::NAME;
pub const ATTR_GRADIENT_TYPE: &str = GradientType::NAME;
pub const ATTR_EDITOR_CLICK_TARGET: &str = EditorClickTarget::NAME;
pub const ATTR_WEIGHT: &str = Weight::NAME;
pub const ATTR_DASH_PATTERN: &str = DashPattern::NAME;
pub const ATTR_DASH_OFFSET: &str = DashOffset::NAME;
pub const ATTR_CAP: &str = Cap::NAME;
pub const ATTR_JOIN: &str = Join::NAME;
pub const ATTR_JOIN_MITER_LIMIT: &str = JoinMiterLimit::NAME;
pub const ATTR_ALIGN: &str = Align::NAME;

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
		assert_eq!(info("weight").unwrap().value_type, TypeId::of::<f64>());
		assert_eq!(info("dash_pattern").unwrap().value_type, TypeId::of::<Option<&'static Vec<f64>>>());
		assert_eq!(info("dash_offset").unwrap().value_type, TypeId::of::<f64>());
		assert_eq!(info("cap").unwrap().value_type, TypeId::of::<crate::vector::style::StrokeCap>());
		assert_eq!(info("join").unwrap().value_type, TypeId::of::<crate::vector::style::StrokeJoin>());
		assert_eq!(info("join_miter_limit").unwrap().value_type, TypeId::of::<f64>());
		assert_eq!(info("align").unwrap().value_type, TypeId::of::<crate::vector::style::StrokeAlign>());
	}

	#[test]
	fn stroke_parameter_defaults_match_the_stroke_struct() {
		let defaults = crate::vector::style::Stroke::default();
		assert_eq!(<Weight as Attribute>::default(), defaults.weight);
		assert_eq!(<DashOffset as Attribute>::default(), defaults.dash_offset);
		assert_eq!(<JoinMiterLimit as Attribute>::default(), defaults.join_miter_limit);
		assert_eq!(<Cap as Attribute>::default(), defaults.cap);
		assert_eq!(<Join as Attribute>::default(), defaults.join);
		assert_eq!(<Align as Attribute>::default(), defaults.align);
	}

	#[test]
	fn an_absent_click_target_defaults_to_none() {
		assert_eq!(<EditorClickTarget as Attribute>::default(), None);
	}
}
