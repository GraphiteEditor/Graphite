use crate::{MigrationError, MigrationId, Payload};

/// How stored history survives a format step.
///
/// Under `Rewrite`, the migration transforms each record's payload via
/// [`FormatMigration::migrate_delta`]; the runner then recomputes `Rev`s in topological order and
/// remaps parent links and session cursors. Because the rewrite is a pure function of content and
/// `Rev`s are content-addressed, peers applying the same migration converge on identical rewritten
/// history without coordination.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum HistoryPolicy {
	/// The shape change does not affect stored deltas.
	Untouched,
	/// Each history record is rewritten via `migrate_delta`.
	Rewrite,
	/// No faithful rewrite exists; the document becomes a state-only snapshot.
	Truncate,
}

/// One whole-document format step, migrating version `migrates_from()` to `migrates_from() + 1`.
///
/// Implementations freeze whatever old struct shapes they need locally (deserialized from
/// [`Payload`]s); the active codebase never carries them. Migrations only transform payload
/// content, never identity fields (`id`, `parent`) — Merkle bookkeeping belongs to the runner.
pub trait FormatMigration {
	/// Stable identifier recorded in provenance.
	fn id(&self) -> MigrationId;
	/// The format version this step upgrades from.
	fn migrates_from(&self) -> u32;
	/// Transform the serialized registry payload.
	fn migrate_registry(&self, registry: &Payload) -> Result<Payload, MigrationError>;
	/// How stored history survives this step.
	fn history_policy(&self) -> HistoryPolicy {
		HistoryPolicy::Untouched
	}
	/// Transform one history record. Called only under [`HistoryPolicy::Rewrite`].
	fn migrate_delta(&self, delta: &Payload) -> Result<Payload, MigrationError> {
		Ok(delta.clone())
	}
	/// Transform the per-peer session payload.
	fn migrate_session(&self, session: &Payload) -> Result<Payload, MigrationError> {
		Ok(session.clone())
	}
}
