//! Authoring surface for `.gdd` document migrations.
//!
//! Two tiers: a [`FormatMigration`] steps a document's serialized payloads from one format version
//! to the next, and a [`NodeMigration`] (feature `typed`) steps one proto-node declaration to its
//! successor on the typed `Registry`. The `migration-runner` crate applies both. Design rationale
//! lives in `node-graph/rfcs/document-format-migrations.md`.

#[cfg(feature = "typed")]
pub mod connector;
#[cfg(feature = "typed")]
pub mod declaration;
pub mod error;
pub mod format;
#[cfg(feature = "typed")]
pub mod migration;
pub mod payload;
#[cfg(feature = "typed")]
pub mod registry;
#[cfg(feature = "typed")]
pub mod value;
#[cfg(feature = "typed")]
pub mod view;

#[cfg(feature = "typed")]
pub use connector::{InputConnector, NewNode, OutputConnector};
#[cfg(feature = "typed")]
pub use declaration::{Declaration, DeclarationResolver};
#[cfg(feature = "typed")]
pub use document_graph_storage::NodeInput;
pub use error::MigrationError;
pub use format::{FormatMigration, HistoryPolicy};
#[cfg(feature = "typed")]
pub use migration::{NodeMigration, Rename};
#[cfg(feature = "typed")]
pub use migration_macros::{migration, rename};
pub use payload::{Payload, PayloadCodec};
#[cfg(feature = "typed")]
pub use registry::{register_migration, registered_migrations};
#[cfg(feature = "typed")]
pub use value::{encode, literal, literal_of};
#[cfg(feature = "typed")]
pub use view::{AddedNode, AnchorContext, Downstream, Edits, Graph, Inputs, InputsEdit, Node, Region, Upstream, View};

// For the registration `#[migration]` and `rename!` generate
#[cfg(feature = "typed")]
#[doc(hidden)]
pub use ctor;

/// Stable identifier for one format migration, recorded in provenance.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct MigrationId(pub &'static str);

impl std::fmt::Display for MigrationId {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(self.0)
	}
}

/// The format steps one migration crate exports. Node migrations register themselves instead.
#[derive(Default)]
pub struct MigrationSet {
	pub format: Vec<Box<dyn FormatMigration>>,
}
