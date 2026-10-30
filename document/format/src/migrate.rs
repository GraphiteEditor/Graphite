//! Runs the migrations this binary links on a document as it opens: format steps over the stored records, then node migrations
//! over history. See `node-graph/rfcs/document-format-migrations.md`.

use std::collections::{BTreeSet, HashMap};

use document_container::AsyncContainer;
use document_graph_storage::{Delta, EncodedDeclaration, HistoryMetadata, PeerId, Registry, ResourceEntry, ResourceId, Rev, Session, Subject, TimeStamp};
use migration_core::{Declaration, DeclarationResolver, NodeMigration, Payload, PayloadCodec};
use migration_runner::{DocumentPayloads, HistoryOutcome, RunnerError, holds, migrate_history, migrate_registry, references, registry_at, run_format_migrations};

use crate::error::Error;
use crate::layout::Layout;
use crate::load::StoredRecords;
use crate::manifest::{PayloadCodecs, SUPPORTED_FORMAT_VERSION};
use crate::{Codec, Gdd};

/// Bring `records` from `format_version` to [`SUPPORTED_FORMAT_VERSION`]. `None` when they already are.
pub(crate) fn migrate_format(records: &mut StoredRecords, codecs: PayloadCodecs, format_version: u32) -> Result<Option<HistoryOutcome>, Error> {
	if format_version >= SUPPORTED_FORMAT_VERSION {
		return Ok(None);
	}

	let payload = |record: &Vec<u8>, codec: Codec| Payload::new(record.clone(), payload_codec(codec));
	let mut payloads = DocumentPayloads {
		format_version,
		registry: records.registry.as_ref().map(|record| payload(record, codecs.registry)),
		history: records.history.iter().flatten().map(|record| payload(record, codecs.history)).collect(),
		hot_log: records.hot_log.iter().map(|record| payload(record, codecs.hot_log)).collect(),
		session: records.session.as_ref().map(|record| payload(record, codecs.session)),
	};
	let outcome = run_format_migrations(&migration_core::registered_format_migrations(), &mut payloads, SUPPORTED_FORMAT_VERSION)?;
	log::info!("Migrated the document format from version {format_version} to {SUPPORTED_FORMAT_VERSION}");

	records.registry = payloads.registry.map(|payload| payload.bytes);
	records.history = match outcome {
		HistoryOutcome::Truncated => None,
		_ => records.history.as_ref().map(|_| payloads.history.into_iter().map(|payload| payload.bytes).collect()),
	};
	records.hot_log = payloads.hot_log.into_iter().map(|payload| payload.bytes).collect();
	records.session = payloads.session.map(|payload| payload.bytes);

	Ok(Some(outcome))
}

/// The facts in `metadata` with their revs mapped by `remap`, which drops the facts of a rev it has no answer for.
pub(crate) fn remap_metadata(metadata: &HistoryMetadata, remap: impl Fn(Rev) -> Option<Rev>) -> HistoryMetadata {
	let facts = metadata.facts().cloned().filter_map(|mut fact| {
		if let Subject::Rev(rev) = fact.subject {
			fact.subject = Subject::Rev(remap(rev)?);
		}
		Some(fact)
	});
	HistoryMetadata::fold(facts)
}

fn payload_codec(codec: Codec) -> PayloadCodec {
	match codec {
		Codec::Json | Codec::JsonLines => PayloadCodec::Json,
		Codec::Postcard | Codec::PostcardFrames => PayloadCodec::Postcard,
	}
}

impl<L: Layout> Gdd<L> {
	/// Rewrite history through every registered node migration whose source declaration it references. Returns whether any ran.
	pub(crate) async fn migrate_nodes(&mut self) -> Result<bool, Error> {
		let migrations = migration_core::registered_migrations();
		if migrations.is_empty() {
			return Ok(false);
		}

		let resolver = DocumentDeclarations::new(&migrations, self.session.peer());

		let hot_ops = self.session.hot_log().iter().map(|hot_op| &hot_op.op);
		let kinds: Vec<_> = self.session.history().map(|delta| &delta.kind).chain(hot_ops).collect();
		let registry = self.session.registry();
		let pending: Vec<Declaration> = migrations
			.iter()
			.map(|migration| migration.source())
			.filter(|source| holds(registry, *source, &resolver) || kinds.iter().any(|kind| references(kind, *source, &resolver)))
			.collect();
		if pending.is_empty() {
			return Ok(false);
		}

		// Hot ops join history first, so they migrate with it
		self.session.retire_all()?;

		let migrated = migrate_registry(&migrations, self.session.retired_registry(), &resolver)?;
		(self.session, self.metadata) = match self.session_with_rewritten_history(&migrations, &migrated, &resolver) {
			Ok(rewritten) => rewritten,
			Err(error) => {
				log::warn!("Migrating this document's history failed, so undo can't reach before this point: {error}");
				(self.session_with_truncated_history(migrated)?, remap_metadata(&self.metadata, |_| None))
			}
		};

		// Declarations the migrations introduced need their bytes next to the other resources
		let referenced = self.session.all_declaration_resources();
		for declaration in resolver.encoded.values().filter(|declaration| referenced.contains_key(&declaration.id)) {
			let path = self.layout.resource_path(&declaration.hash);
			if !self.working.exists(&path).await {
				self.working.write_non_blocking(&path, &declaration.bytes)?;
			}
		}

		let names: Vec<String> = pending.iter().map(ToString::to_string).collect();
		log::info!("Migrated nodes from {}", names.join(", "));
		Ok(true)
	}

	/// A session over the rewritten history, as long as replaying that history reaches `migrated`, which a history missing its beginning wouldn't.
	fn session_with_rewritten_history(&self, migrations: &[&'static dyn NodeMigration], migrated: &Registry, resolver: &DocumentDeclarations) -> Result<(Session, HistoryMetadata), RunnerError> {
		let history: Vec<Delta> = self.session.history().cloned().collect();
		let rewritten = migrate_history(migrations, history, resolver)?;
		let remap = |rev: Rev| rewritten.revs.get(&rev).copied().flatten();

		let head = self.session.head_rev().and_then(remap);
		let registry = registry_at(&rewritten.deltas, head)?;
		if !registry.value_equal(migrated) {
			return Err(RunnerError::ReplayDiffers);
		}

		let previous = &self.session;
		let redo_stack = previous.redo_stack().iter().filter_map(|rev| remap(*rev)).collect();
		let mut session = Session::load(previous.peer(), previous.user(), registry, rewritten.deltas, head, redo_stack, previous.next_node_counter());
		if let Some(rev) = previous.last_broadcast_rev().and_then(remap) {
			session.publish_up_to(rev);
		}
		session.restore_hot_sequence(previous.last_hot_sequence());
		session.restore_clock_counter(previous.clock_counter());
		session.absorb_retired_marks(previous.retired_marks());
		Ok((session, remap_metadata(&self.metadata, remap)))
	}

	/// A session whose history restarts from `migrated`.
	fn session_with_truncated_history(&self, migrated: Registry) -> Result<Session, Error> {
		let previous = &self.session;
		let restarted = Session::bootstrap_from_registry(previous.peer(), previous.user(), migrated.clone())?;

		let history: Vec<Delta> = restarted.history().cloned().collect();
		let mut session = Session::load(previous.peer(), previous.user(), migrated, history, restarted.head_rev(), Vec::new(), previous.next_node_counter());
		session.restore_hot_sequence(previous.last_hot_sequence());
		session.restore_clock_counter(previous.clock_counter().max(restarted.clock_counter()));
		session.absorb_retired_marks(previous.retired_marks());
		Ok(session)
	}
}

/// Resolves declaration resources to the declarations migrations name. A declaration's resource derives from its identifier and version
/// alone, so this needs none of the stored bytes, which the editor keeps in its global cache rather than the document's container.
struct DocumentDeclarations {
	known: HashMap<ResourceId, Declaration>,
	encoded: HashMap<Declaration, EncodedDeclaration>,
	peer: PeerId,
}

impl DocumentDeclarations {
	fn new(migrations: &[&'static dyn NodeMigration], peer: PeerId) -> Self {
		let named: BTreeSet<Declaration> = migrations
			.iter()
			.flat_map(|migration| [migration.source(), migration.target()].into_iter().chain(migration.pins().iter().copied()))
			.collect();

		let mut known = HashMap::new();
		let mut encoded = HashMap::new();
		for declaration in named {
			match EncodedDeclaration::new(declaration.identifier.to_string(), declaration.version) {
				Ok(declaration_resource) => {
					known.insert(declaration_resource.id, declaration);
					encoded.insert(declaration, declaration_resource);
				}
				Err(error) => log::error!("Encoding the declaration {declaration} failed: {error}"),
			}
		}

		Self { known, encoded, peer }
	}
}

impl DeclarationResolver for DocumentDeclarations {
	fn declaration(&self, resource: ResourceId) -> Option<Declaration> {
		self.known.get(&resource).copied()
	}

	fn resource(&self, declaration: Declaration) -> ResourceId {
		match self.encoded.get(&declaration) {
			Some(encoded) => encoded.id,
			None => {
				log::error!("No migration names the declaration {declaration}, so it has no resource");
				ResourceId::default()
			}
		}
	}

	fn resource_entry(&self, declaration: Declaration) -> Option<ResourceEntry> {
		let encoded = self.encoded.get(&declaration)?;
		Some(ResourceEntry::embedded(encoded.hash, self.peer, TimeStamp::ORIGIN))
	}
}
