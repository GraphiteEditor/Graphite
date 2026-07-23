//! Typed attribute keys whose value types live in this crate. See `core_types::attr` for the trait and macro.

use crate::gradient::GradientSpreadMethod;
use crate::vector::Vector;

node_macro::attrs! {
	/// Gradient's spread method (`Pad`, `Reflect`, or `Repeat`).
	SpreadMethod: GradientSpreadMethod,
	/// Gradient's type (`Linear` or `Radial`).
	GradientType: crate::gradient::GradientType,
	editor {
		/// Vector that overrides the item's own geometry for click-target generation.
		/// Used by the 'Text' node for per-glyph bounding-box rectangles so glyphs are selectable
		/// by clicking anywhere within their bounds, not just the filled letterform.
		ClickTarget: Vector,
	},
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::attr::Attr;

	// Key names are the stored document format — pinned as literals so a key rename shows up as a breaking change.
	#[test]
	fn key_names_are_pinned() {
		assert_eq!(SpreadMethod::name(), "spread_method");
		assert_eq!(GradientType::name(), "gradient_type");
		assert_eq!(editor::ClickTarget::name(), "editor:click_target");
	}
}
