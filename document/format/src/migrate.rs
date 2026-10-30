//! Runs the migrations this binary links on a document as it opens: format steps over the stored records, then node migrations
//! over history. See `node-graph/rfcs/document-format-migrations.md`.

use std::collections::{BTreeSet, HashMap};

use document_container::AsyncContainer;
use document_graph_storage::{Declarations, Delta, EncodedDeclaration, PeerId, Registry, ResourceEntry, ResourceId, Rev, Session, TimeStamp, apply_retired_op};
use migration_core::{Declaration, DeclarationResolver, NodeMigration, Payload, PayloadCodec};
use migration_runner::{DocumentPayloads, HistoryOutcome, migrate_history, references, run_format_migrations};

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

		let declarations = self.declarations(self).await;
		let resolver = DocumentDeclarations::new(&declarations, &migrations, self.session.peer());

		let hot_ops = self.session.hot_log().iter().map(|hot_op| &hot_op.op);
		let kinds: Vec<_> = self.session.history().map(|delta| &delta.kind).chain(hot_ops).collect();
		let pending: Vec<Declaration> = migrations
			.iter()
			.map(|migration| migration.source())
			.filter(|source| kinds.iter().any(|kind| references(kind, *source, &resolver)))
			.collect();
		if pending.is_empty() {
			return Ok(false);
		}

		// Hot ops join history first, so they migrate with it
		self.session.retire_all()?;

		let history: Vec<Delta> = self.session.history().cloned().collect();
		let rewritten = migrate_history(&migrations, history, &resolver)?;
		let remap = |rev: Rev| rewritten.revs.get(&rev).copied().flatten();

		let head = self.session.head_rev().and_then(remap);
		let redo_stack = self.session.redo_stack().iter().filter_map(|rev| remap(*rev)).collect();
		let registry = registry_at(&rewritten.deltas, head)?;

		let previous = &self.session;
		let mut session = Session::load(previous.peer(), previous.user(), registry, rewritten.deltas, head, redo_stack, previous.next_node_counter());
		if let Some(rev) = previous.last_broadcast_rev().and_then(remap) {
			session.publish_up_to(rev);
		}
		session.restore_hot_sequence(previous.last_hot_sequence());
		session.restore_clock_counter(previous.clock_counter());
		session.absorb_retired_marks(previous.retired_marks());
		self.session = session;

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
}

/// The registry after replaying `deltas` up to and including `head`.
fn registry_at(deltas: &[Delta], head: Option<Rev>) -> Result<Registry, Error> {
	let mut registry = Registry::default();
	let Some(head) = head else { return Ok(registry) };

	for delta in deltas {
		apply_retired_op(&mut registry, delta.kind.clone(), delta.timestamp)?;
		if delta.id == head {
			break;
		}
	}
	Ok(registry)
}

/// Resolves a document's declaration resources to the declarations migrations name, by identifier and version.
struct DocumentDeclarations {
	known: HashMap<ResourceId, Declaration>,
	encoded: HashMap<Declaration, EncodedDeclaration>,
	peer: PeerId,
}

impl DocumentDeclarations {
	fn new(declarations: &Declarations, migrations: &[&'static dyn NodeMigration], peer: PeerId) -> Self {
		let named: BTreeSet<Declaration> = migrations
			.iter()
			.flat_map(|migration| [migration.source(), migration.target()].into_iter().chain(migration.pins().iter().copied()))
			.collect();
		let identifiers: HashMap<&str, &'static str> = named.iter().map(|declaration| (declaration.identifier, declaration.identifier)).collect();

		let mut known = HashMap::new();
		for (id, proto) in declarations {
			if let Some(identifier) = identifiers.get(proto.identifier.as_str()) {
				known.insert(*id, Declaration::new(identifier, proto.version));
			}
		}

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
