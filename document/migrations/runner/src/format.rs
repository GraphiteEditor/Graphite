//! Chains format steps over a document's serialized payloads, before they deserialize into the live types.

use crate::RunnerError;
use migration_core::{FormatMigration, HistoryPolicy, Payload};

/// The serialized payloads of one document, as handed over by the load path before typed
/// deserialization. History records are unframed: one [`Payload`] per retired delta, in stored
/// (topological) order. A working copy that only has history has no registry.
pub struct DocumentPayloads {
	pub format_version: u32,
	pub registry: Option<Payload>,
	pub history: Vec<Payload>,
	pub hot_log: Vec<Payload>,
	pub session: Option<Payload>,
}

/// What the format tier did to history, so the load path knows whether a `Rev` rehash pass
/// (with session-cursor remap) is required after typed deserialization. Ordered by severity so
/// chained steps combine via `max`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum HistoryOutcome {
	Untouched,
	/// At least one step rewrote records: rehash `Rev`s with [`rehash_deltas`](crate::rehash_deltas) and remap cursors.
	Rewritten,
	/// At least one step truncated history: reset cursors, the document is a state-only snapshot.
	Truncated,
}

/// Chain `migrations` from `payloads.format_version` up to `target_version`, bumping the version once per applied step.
/// A missing or ambiguous step in the chain is a hard error.
pub fn run_format_migrations(migrations: &[&'static dyn FormatMigration], payloads: &mut DocumentPayloads, target_version: u32) -> Result<HistoryOutcome, RunnerError> {
	let mut outcome = HistoryOutcome::Untouched;

	while payloads.format_version < target_version {
		let version = payloads.format_version;
		let step = match migrations.iter().filter(|migration| migration.migrates_from() == version).collect::<Vec<_>>().as_slice() {
			[step] => **step,
			[] => return Err(RunnerError::MissingFormatStep { at_version: version }),
			_ => return Err(RunnerError::DuplicateFormatStep { at_version: version }),
		};
		let fail = |error| RunnerError::Format { from: version, error };

		if let Some(registry) = &payloads.registry {
			payloads.registry = Some(step.migrate_registry(registry).map_err(fail)?);
		}

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

		// Hot ops aren't in history yet, so they convert under every policy
		for hot_op in &mut payloads.hot_log {
			*hot_op = step.migrate_hot_op(hot_op).map_err(fail)?;
		}
		if let Some(session) = &payloads.session {
			payloads.session = Some(step.migrate_session(session).map_err(fail)?);
		}

		payloads.format_version += 1;
	}

	Ok(outcome)
}
