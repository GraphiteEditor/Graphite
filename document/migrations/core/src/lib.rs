//! Trait definitions for `.gdd` document migrations.
//!
//! Two tiers: a [`FormatMigration`] steps a document's serialized payloads from one format version
//! to the next, and a [`ContentMigration`] (feature `typed`) upgrades node usages on the typed
//! `Registry` within the current version. Migration crates export a [`MigrationSet`] via a plain
//! constructor function; the `migration-runner` crate aggregates and dispatches them. Design
//! rationale lives in `node-graph/rfcs/document-format-migrations.md`.

#[cfg(feature = "typed")]
pub mod content;
pub mod error;
pub mod format;
pub mod payload;

#[cfg(feature = "typed")]
pub use content::{ContentMigration, DeclarationInfo, MigrationContext, MigrationHost, NodeSelector, Selector, Target};
pub use error::MigrationError;
pub use format::{FormatMigration, HistoryPolicy};
pub use payload::{Payload, PayloadCodec};

/// Document attribute holding the list of applied `Document`-selector migration IDs.
pub const APPLIED_ATTRIBUTE: &str = "migrations::applied";

/// Stable identifier for one migration, recorded in provenance and used for skip checks.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct MigrationId(pub &'static str);

impl std::fmt::Display for MigrationId {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(self.0)
	}
}

/// What one migration crate exports: its migrations, in application order.
/// Staged node upgrades must be registered in ascending target-version order.
#[derive(Default)]
pub struct MigrationSet {
	pub format: Vec<Box<dyn FormatMigration>>,
	#[cfg(feature = "typed")]
	pub content: Vec<Box<dyn ContentMigration>>,
}
