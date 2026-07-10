pub mod node {
	pub const CALL_ARGUMENT: &str = "call_argument";
	pub const CONTEXT_FEATURES: &str = "context_features";
	pub const VISIBLE: &str = "visible";
	pub const SKIP_DEDUPLICATION: &str = "skip_deduplication";
	pub const REFLECTION_METADATA: &str = "reflection_metadata";
	pub const ORIGINAL_NODE_ID: &str = "original_node_id";

	pub mod input {
		pub const IMPORT_TYPE: &str = "import_type";

		pub mod ui {
			pub const NAME: &str = "ui::name";
			pub const DESCRIPTION: &str = "ui::description";
			pub const WIDGET_OVERRIDE: &str = "ui::widget_override";
			/// Prefix for `InputPersistentMetadata::data` entries. Full key: `ui::data::<sub_key>`.
			pub const DATA_PREFIX: &str = "ui::data::"; // TODO: Remove and make runtime strongly typed again
		}
	}

	pub mod ui {
		pub const POSITION: &str = "ui::position";
		pub const IS_LAYER: &str = "ui::is_layer";
		pub const DISPLAY_NAME: &str = "ui::display_name";
		pub const LOCKED: &str = "ui::locked";
		pub const PINNED: &str = "ui::pinned";
		pub const OUTPUT_NAMES: &str = "ui::output_names";
		pub const REFERENCE: &str = "ui::reference"; // TODO: Remove?
	}
}

pub mod session {
	pub mod network {
		pub const PREVIEWING: &str = "ui::previewing";

		// TODO: Remove these graph ui nav-specific attributes
		pub const NAV_PTZ: &str = "ui::nav::ptz";
		pub const NAV_TRANSFORM: &str = "ui::nav::transform";
		pub const NAV_WIDTH: &str = "ui::nav::width";
	}

	pub mod doc {
		// Document-level editor chrome, stored in `Registry.attributes` (document scope). Each setting is
		// its own key so concurrent edits to one don't clobber another.
		pub const PTZ: &str = "ui::ptz";
		pub const RENDER_MODE: &str = "ui::render_mode";
		pub const OVERLAYS: &str = "ui::overlays";
		pub const RULERS_VISIBLE: &str = "ui::rulers_visible";
		pub const SNAPPING: &str = "ui::snapping";
		pub const COLLAPSED: &str = "ui::collapsed";
	}
}

pub mod registry {
	pub const EXPORTED_NODES: &str = "exported_nodes";
}

pub mod network {
	/// Whole-map LWW of a network's `scope_injections` (`key -> (storage NodeId, Type)`), stored as a
	/// serialized blob so its shape can evolve (e.g. dropping the `Type`) without a model change. The
	/// node references use stable storage IDs, resolved back to runtime-local IDs on conversion.
	pub const SCOPE_INJECTIONS: &str = "scope_injections";
}

pub mod delta {
	/// Marks the last delta of a user interaction, so the undo cursor steps per-interaction, not per-delta.
	pub const INTERACTION_END: &str = "interaction_end";
}
