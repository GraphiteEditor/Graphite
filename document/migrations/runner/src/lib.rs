//! Dispatches document migrations: format-version chaining over serialized payloads, then
//! delta-expressed content migrations over the typed registry. See
//! `node-graph/rfcs/document-format-migrations.md` for the pipeline design.

use graph_storage::{AttributesRead, AttributesWrite, CrdtError, Implementation, NodeId, Registry, ResourceId, Rev, Session, TimeStamp, attr};
use migration_core::{APPLIED_ATTRIBUTE, DeclarationInfo, FormatMigration, HistoryPolicy, MigrationContext, MigrationError, MigrationHost, MigrationSet, Payload, Selector, Target};

pub use graph_storage::rehash_deltas;

#[derive(Debug, thiserror::Error)]
pub enum RunnerError {
	#[error("no format migration registered for version {at_version}")]
	MissingFormatStep { at_version: u32 },
	#[error("format migration {id} failed: {source}")]
	Format { id: &'static str, source: MigrationError },
	#[error("failed to commit migration deltas: {0}")]
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

	/// Run content migrations over an open session, committing each migration's changes as one
	/// retired gesture. A target that errors has its changes reverted and logged, so one bad node
	/// doesn't block the rest of the document or the remaining migrations. Returns the retired revs.
	pub fn run_content_migrations(&self, session: &mut Session, host: &mut dyn MigrationHost) -> Result<Vec<Rev>, RunnerError> {
		let mut all_revs = Vec::new();

		for migration in self.sets.iter().flat_map(|set| &set.content) {
			let selector = migration.selector();

			// `Document`-selector migrations have no self-gating selector, so provenance gates reruns
			if matches!(selector, Selector::Document) && applied_migrations(session.registry()).iter().any(|applied| applied == migration.id().0) {
				continue;
			}

			let targets = collect_targets(&selector, session.registry(), host);
			if targets.is_empty() {
				continue;
			}

			// Apply per target on a shared clone, restoring the pre-target state on error
			let mut target_registry = session.registry().clone();
			let mut applied = false;
			for target in targets {
				let backup = target_registry.clone();
				let mut context = SessionContext {
					session: &mut *session,
					host: &mut *host,
				};
				match migration.migrate(target, &mut target_registry, &mut context) {
					Ok(()) => applied = true,
					Err(error) => {
						log::error!("Content migration {} failed on {target:?}: {error}", migration.id());
						target_registry = backup;
					}
				}
			}
			if !applied {
				continue;
			}

			if matches!(selector, Selector::Document) {
				record_applied(&mut target_registry, migration.id().0);
			}

			// Commit the mutation as deltas and retire them as one undoable gesture
			if !session.registry().value_equal(&target_registry) {
				session.stage_registry_replace(&target_registry)?;
				let revs = session.retire_all()?;
				if let Some(last) = revs.last() {
					session.mark_interaction_end(*last);
				}
				all_revs.extend(revs);
			}
		}

		Ok(all_revs)
	}
}

fn applied_migrations(registry: &Registry) -> Vec<String> {
	registry.attributes.get_typed::<Vec<String>>(APPLIED_ATTRIBUTE).unwrap_or_default()
}

fn record_applied(registry: &mut Registry, id: &str) {
	let mut applied = applied_migrations(registry);
	if !applied.iter().any(|existing| existing == id) {
		applied.push(id.to_string());
		// The placeholder timestamp is re-stamped by the commit path
		let _ = registry.attributes.set_serialized(APPLIED_ATTRIBUTE, &applied, TimeStamp::default());
	}
}

/// Scan the registry for the entities a selector matches, in deterministic (sorted) order.
fn collect_targets(selector: &Selector, registry: &Registry, host: &dyn MigrationHost) -> Vec<Target> {
	let node_targets = |mut nodes: Vec<NodeId>| {
		nodes.sort_unstable();
		nodes.into_iter().map(Target::Node).collect()
	};

	match selector {
		Selector::Document => vec![Target::Document],
		Selector::Reference(name) => node_targets(
			registry
				.node_instances
				.iter()
				.filter(|(_, node)| node.attributes().get(attr::node::ui::REFERENCE).is_some_and(|value| value.value.as_str() == Some(name)))
				.map(|(id, _)| *id)
				.collect(),
		),
		Selector::Node(node_selector) => node_targets(
			registry
				.node_instances
				.iter()
				.filter(|(_, node)| {
					let Implementation::ProtoNode(resource_id) = node.implementation() else { return false };
					host.declaration_info(*resource_id).is_some_and(|info| node_selector.matches(&info))
				})
				.map(|(id, _)| *id)
				.collect(),
		),
	}
}

/// Combines the open session (ID minting) with host services into the context migrations see.
struct SessionContext<'a> {
	session: &'a mut Session,
	host: &'a mut dyn MigrationHost,
}

impl MigrationHost for SessionContext<'_> {
	fn declaration_info(&self, id: ResourceId) -> Option<DeclarationInfo> {
		self.host.declaration_info(id)
	}
	fn declaration(&self, id: ResourceId) -> Option<serde_json::Value> {
		self.host.declaration(id)
	}
	fn resolve_definition(&mut self, identifier: &str) -> Option<graph_storage::Node> {
		self.host.resolve_definition(identifier)
	}
}

impl MigrationContext for SessionContext<'_> {
	fn mint_node_id(&mut self) -> NodeId {
		self.session.next_node_id()
	}
}
