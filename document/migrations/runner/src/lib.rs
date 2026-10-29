//! Applies document migrations: format-version chaining over serialized payloads, then node migrations that rewrite history into the
//! current shape. See `node-graph/rfcs/document-format-migrations.md` for the pipeline design.

mod node;

use document_graph_storage::{CrdtError, NodeId, Rev};
use migration_core::{Declaration, FormatMigration, HistoryPolicy, InputConnector, MigrationError, MigrationSet, Payload};

pub use document_graph_storage::rehash_deltas;
pub use node::{DeclarationTable, RewrittenHistory, apply_migration, rewrite_history};

#[derive(Debug, thiserror::Error)]
pub enum RunnerError {
	#[error("no format migration registered for version {at_version}")]
	MissingFormatStep { at_version: u32 },
	#[error("format migration {id} failed: {source}")]
	Format { id: &'static str, source: MigrationError },
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
	#[error("failed to apply a delta: {0}")]
	Crdt(#[from] CrdtError),
}

/// The serialized payloads of one document, as handed over by the load path before typed
/// deserialization. History records are unframed: one [`Payload`] per retired delta, in stored
/// (topological) order.
pub struct DocumentPayloads {
	pub format_version: u32,
	pub registry: Payload,
	pub history: Vec<Payload>,
	pub session: Option<Payload>,
}

/// What the format tier did to history, so the load path knows whether a `Rev` rehash pass
/// (with session-cursor remap) is required after typed deserialization. Ordered by severity so
/// chained steps combine via `max`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum HistoryOutcome {
	Untouched,
	/// At least one step rewrote records: rehash `Rev`s with [`rehash_deltas`] and remap cursors.
	Rewritten,
	/// At least one step truncated history: reset cursors, the document is a state-only snapshot.
	Truncated,
}

/// Aggregates registered [`MigrationSet`]s and dispatches them over a document.
pub struct MigrationRunner {
	sets: Vec<MigrationSet>,
}

impl MigrationRunner {
	pub fn new(sets: Vec<MigrationSet>) -> Self {
		Self { sets }
	}

	fn format_step(&self, version: u32) -> Option<&dyn FormatMigration> {
		self.sets
			.iter()
			.flat_map(|set| &set.format)
			.find(|migration| migration.migrates_from() == version)
			.map(|migration| migration.as_ref())
	}

	/// Chain format steps from `payloads.format_version` up to `target_version`, bumping the
	/// version once per applied step. A missing step in the chain is a hard error.
	pub fn run_format_migrations(&self, payloads: &mut DocumentPayloads, target_version: u32) -> Result<HistoryOutcome, RunnerError> {
		let mut outcome = HistoryOutcome::Untouched;

		while payloads.format_version < target_version {
			let version = payloads.format_version;
			let step = self.format_step(version).ok_or(RunnerError::MissingFormatStep { at_version: version })?;
			let fail = |source| RunnerError::Format { id: step.id().0, source };

			payloads.registry = step.migrate_registry(&payloads.registry).map_err(fail)?;

			match step.history_policy() {
				HistoryPolicy::Untouched => {}
				HistoryPolicy::Rewrite => {
					for record in &mut payloads.history {
						*record = step.migrate_delta(record).map_err(fail)?;
					}
					outcome = outcome.max(HistoryOutcome::Rewritten);
				}
				HistoryPolicy::Truncate => {
					payloads.history.clear();
					outcome = HistoryOutcome::Truncated;
				}
			}

			if let Some(session) = &payloads.session {
				payloads.session = Some(step.migrate_session(session).map_err(fail)?);
			}

			payloads.format_version += 1;
		}

		Ok(outcome)
	}
}
