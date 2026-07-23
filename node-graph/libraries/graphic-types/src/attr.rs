//! Typed attribute keys whose value types live in this crate. See `core_types::attr` for the trait and macro.

use crate::graphic::Graphic;
use core_types::list::List;

node_macro::attrs! {
	/// Vector graphics object's filled area paint.
	Fill: List<Graphic>,
	/// Vector graphics object's stroke paint.
	Stroke: List<Graphic>,
	editor {
		/// Snapshot of the upstream content that fed into a destructive merge (Boolean Operation,
		/// Rasterize, etc.), so the editor can still surface click targets for the original child
		/// layers after their content has been collapsed.
		MergedLayers: List<Graphic>,
	},
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::attr::Attr;

	// Key names are the stored document format — pinned as literals so a key rename shows up as a breaking change.
	#[test]
	fn key_names_are_pinned() {
		assert_eq!(Fill::name(), "fill");
		assert_eq!(Stroke::name(), "stroke");
		assert_eq!(editor::MergedLayers::name(), "editor:merged_layers");
	}
}
