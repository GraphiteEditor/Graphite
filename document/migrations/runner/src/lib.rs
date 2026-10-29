//! Applies document migrations: format-version chaining over serialized payloads, then node migrations that rewrite history into the
//! current shape. See `node-graph/rfcs/document-format-migrations.md` for the pipeline design.

mod format;
mod node;
mod order;

use document_graph_storage::{CrdtError, NodeId, Rev};
use migration_core::{Declaration, InputConnector, MigrationError};

pub use document_graph_storage::rehash_deltas;
pub use format::{DocumentPayloads, HistoryOutcome, run_format_migrations};
pub use node::{DeclarationTable, RewrittenHistory, apply_migration, rewrite_history};
pub use order::{migrate_history, order_migrations, references};

#[derive(Debug, thiserror::Error)]
pub enum RunnerError {
	#[error("no format migration registered from version {at_version}")]
	MissingFormatStep { at_version: u32 },
	#[error("two format migrations start from version {at_version}")]
	DuplicateFormatStep { at_version: u32 },
	#[error("format migration from version {from} failed: {error}")]
	Format { from: u32, error: MigrationError },
	#[error("migration from {migration} failed on node {anchor:?}: {error}")]
	Migration { migration: Declaration, anchor: NodeId, error: MigrationError },
	#[error("node {anchor:?} adds two nodes of {declaration} under the key {key:?}")]
	DuplicateAddedNode { anchor: NodeId, declaration: Declaration, key: String },
	#[error("added node {0:?} already exists")]
	AddedNodeExists(NodeId),
	#[error("{0:?} is rewired by one anchor while its own migration replaces or removes it")]
	ConflictingSlot(InputConnector),
	#[error("{0:?} does not exist in the migrated state")]
	MissingConsumer(InputConnector),
	#[error("delta {0:?} branches or merges, which the rewrite does not handle yet")]
	NonLinearHistory(Rev),
	#[error("a rewritten chain both removes and adds node {0:?}, which a shared timestamp can't order")]
	ChainWritesTwice(NodeId),
	#[error("two migrations start from {0}")]
	DuplicateMigration(Declaration),
	#[error("migration from {from} to {to} does not step exactly one version")]
	InvalidStep { from: Declaration, to: Declaration },
	#[error("migrations from {0:?} depend on each other in a cycle")]
	OrderCycle(Vec<Declaration>),
	#[error("failed to apply a delta: {0}")]
	Crdt(#[from] CrdtError),
}
