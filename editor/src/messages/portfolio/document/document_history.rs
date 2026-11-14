use std::collections::VecDeque;
use std::collections::{BTreeMap, HashSet};

use document_graph_storage::{Declarations, Value};
use graph_craft::application_io::resource::{ResourceHash, ResourceId, ResourceRegistry, ResourceStorage};

use super::utility_types::network_interface::NodeNetworkInterface;
use super::utility_types::network_interface::editor_delta::{EditorDelta, StagingError, construct_batch};
use super::utility_types::network_interface::storage_metadata::{StorageMetadataView, build_interface_from_storage, collect_network_view_settings};
use crate::messages::portfolio::history::utility_types::LineChange;

/// Per-document undo/redo state, kept in the `Gdd` working copy.
///
/// Not serialized: a clone shares the working-copy container by `Arc`, so it keeps reading the live working copy.
#[derive(derivative::Derivative)]
#[derivative(Clone, Debug, Default)]
pub struct DocumentHistory {
	/// The `Gdd` working copy: owns the CRDT `Session` and mirrors edits to disk. `None` until the mount
	/// future built by `load_document` resolves.
	#[derivative(Debug = "ignore")]
	storage: Option<document_format::GddV1>,
	/// Decoded proto-node declarations for every registry state the cursor can reach, filled at mount
	/// and extended on each staging, so a cursor rebuild never touches the byte store.
	#[derivative(Debug = "ignore")]
	declarations: Declarations,
	/// Whether the next commit must convert the whole document rather than stage what the store recorded.
	///
	/// True when the working copy holds no baseline for a batch to apply to, and whenever the runtime has
	/// moved without recording it (an upgrade on open), since the recorded batch would then describe only
	/// part of the distance between the two.
	needs_whole_document_stage: bool,
	/// How storage undid each step that redo can bring back, latest last.
	undo_notes: VecDeque<UndoNote>,
}

#[derive(Clone, Debug)]
enum UndoNote {
	/// The step was hot and got taken back.
	Retracted(Vec<document_graph_storage::RegistryDelta>),
	/// The step had retired and was dropped from the shared line.
	Dropped(document_graph_storage::Rev),
	/// The storage cursor moved back over the step.
	Cursor,
}

impl DocumentHistory {
	/// Forget the steps redo could bring back.
	pub fn clear_redo(&mut self) {
		self.undo_notes.clear();
	}

	// ===== Gdd working-copy cursor =====

	/// The `Gdd` working copy, `None` until the mount future resolves.
	pub fn storage(&self) -> Option<&document_format::GddV1> {
		self.storage.as_ref()
	}

	/// Mutable access to the `Gdd` working copy.
	pub fn storage_mut(&mut self) -> Option<&mut document_format::GddV1> {
		self.storage.as_mut()
	}

	pub fn declarations(&self) -> &Declarations {
		&self.declarations
	}

	/// Attach the `Gdd` working copy once the mount future resolves, with the declarations it references.
	pub fn set_storage(&mut self, storage: document_format::GddV1, declarations: Declarations) {
		self.needs_whole_document_stage |= storage.registry().node_instances.is_empty();
		self.storage = Some(storage);
		self.declarations = declarations;
	}

	/// Marks the working copy as needing a whole-document stage on its next commit, for a change to the
	/// runtime that went unrecorded and so cannot be described by the recorded batch.
	pub fn require_whole_document_stage(&mut self) {
		self.needs_whole_document_stage = true;
	}

	pub fn end_storage_interaction(&mut self) {
		let Some(storage) = self.storage.as_mut() else { return };
		if let Err(error) = storage.end_interaction() {
			log::error!("Closing the storage interaction failed: {error}");
		}
	}

	/// Undoes the latest step in storage, so it can be redone. Returns whether a step was undone.
	pub fn undo(&mut self, in_session: bool) -> bool {
		let Some(note) = self.undo_step(in_session) else { return false };

		self.undo_notes.push_back(note);
		if self.undo_notes.len() > crate::consts::MAX_UNDO_HISTORY_LEN {
			self.undo_notes.pop_front();
		}
		true
	}

	/// Undoes the latest step in storage without a way to redo it. Returns whether a step was undone.
	pub fn discard_step(&mut self, in_session: bool) -> bool {
		let Some(note) = self.undo_step(in_session) else { return false };

		if matches!(note, UndoNote::Cursor)
			&& let Some(storage) = self.storage.as_mut()
			&& let Err(error) = storage.forget_latest_undo()
		{
			log::error!("Forgetting the discarded step failed: {error}");
		}
		true
	}

	/// Takes the latest step back. A hot step is retracted, a retired one is dropped from a session's shared line,
	/// and otherwise the cursor moves back over it.
	fn undo_step(&mut self, in_session: bool) -> Option<UndoNote> {
		// A failed retract may already have taken the step back, so no older step is undone after it
		let retracted = match self.retract_storage_interaction() {
			Ok(retracted) => retracted,
			Err(error) => {
				log::error!("Taking the storage interaction back failed: {error}");
				return None;
			}
		};
		if let Some(ops) = retracted {
			return Some(UndoNote::Retracted(ops));
		}
		// A session's history is shared, so its cursor never moves
		if in_session {
			return self.undo_retired_step().map(UndoNote::Dropped);
		}
		self.move_cursor(true).then_some(UndoNote::Cursor)
	}

	/// Brings back the latest undone step. Returns whether it was redone.
	pub fn redo(&mut self) -> bool {
		let Some(note) = self.undo_notes.pop_back() else { return false };

		let line_reachable = self.storage.as_ref().is_some_and(|storage| storage.can_change_line());
		let redone = match &note {
			UndoNote::Retracted(ops) => self.restage_ops(ops.clone()),
			UndoNote::Dropped(rev) => line_reachable && self.redo_retired_step(*rev),
			UndoNote::Cursor => self.move_cursor(false),
		};
		if !redone {
			self.undo_notes.push_back(note);
		}
		redone
	}

	/// Move the cursor along the retired interaction chain, flushing any open interaction first. Returns whether it moved.
	fn move_cursor(&mut self, undo: bool) -> bool {
		self.retire_storage_interaction();
		let Some(storage) = self.storage.as_mut() else { return false };

		let moved = if undo {
			if !storage.can_undo() {
				return false;
			}
			storage.undo()
		} else {
			if !storage.can_redo() {
				return false;
			}
			storage.redo()
		};
		moved.inspect_err(|error| log::error!("Storage undo/redo cursor move failed: {error}")).is_ok()
	}

	fn retract_storage_interaction(&mut self) -> Result<Option<Vec<document_graph_storage::RegistryDelta>>, document_format::Error> {
		match self.storage.as_mut() {
			Some(storage) => storage.retract_interaction(),
			None => Ok(None),
		}
	}

	fn restage_ops(&mut self, ops: Vec<document_graph_storage::RegistryDelta>) -> bool {
		let Some(storage) = self.storage.as_mut() else { return false };
		storage.restage_ops(ops).inspect_err(|error| log::error!("Staging the redone step failed: {error}")).is_ok()
	}

	fn undo_retired_step(&mut self) -> Option<document_graph_storage::Rev> {
		self.storage.as_mut()?.undo_retired_step().unwrap_or_else(|error| {
			log::error!("Undoing the retired step failed: {error}");
			None
		})
	}

	pub fn resource_gc_roots(&self) -> HashSet<ResourceHash> {
		self.storage.as_ref().map(|storage| storage.resource_gc_roots()).unwrap_or_default()
	}

	pub fn redo_retired_step(&mut self, rev: document_graph_storage::Rev) -> bool {
		let Some(storage) = self.storage.as_mut() else { return false };
		storage.redo_retired_step(rev).inspect_err(|error| log::error!("Redoing the dropped step failed: {error}")).is_ok()
	}

	/// Returns whether storage made the change.
	pub fn change_line(&mut self, rev: document_graph_storage::Rev, change: LineChange) -> bool {
		let Some(storage) = self.storage.as_mut().filter(|storage| storage.can_change_line()) else {
			return false;
		};
		let result = match change {
			LineChange::MoveHead => storage.move_head(rev),
			LineChange::Remove => storage.drop_step(rev),
			LineChange::Restore => storage.redo_retired_step(rev),
		};
		result.inspect_err(|error| log::error!("Changing the history line with {change:?} failed: {error}")).is_ok()
	}

	pub fn retire_storage_interaction(&mut self) {
		let Some(storage) = self.storage.as_mut() else { return };
		if let Err(error) = storage.retire_pending_interaction() {
			log::error!("Storage interaction retirement failed: {error}");
		}
	}

	/// Stage a `CommitTransaction` into the `Gdd` working copy: the first commit writes the whole document,
	/// every later one stages the `deltas` the store recorded. No-op while unmounted. Proto-node declaration
	/// bytes go into `byte_store` (the app-global resource cache). The staged hot ops are closed into one
	/// interaction by [`end_storage_interaction`](Self::end_storage_interaction) at undo-step boundaries.
	pub fn stage_snapshot(&mut self, deltas: &[EditorDelta], interface: &NodeNetworkInterface, registry: &ResourceRegistry, view_settings: BTreeMap<String, Value>, byte_store: &dyn ResourceStorage) {
		if !self.stage_graph(deltas, interface, registry, byte_store) {
			return;
		}

		self.persist_view_state(interface, view_settings);
	}

	/// [`stage_snapshot`](Self::stage_snapshot) without the view state. Returns whether the working copy took the batch.
	pub fn stage_graph(&mut self, deltas: &[EditorDelta], interface: &NodeNetworkInterface, registry: &ResourceRegistry, byte_store: &dyn ResourceStorage) -> bool {
		let Some(storage) = self.storage.as_mut() else { return false };

		let staged = match self.needs_whole_document_stage {
			true => Self::stage_whole_document(storage, interface, registry, byte_store),
			// An autosave with nothing edited since the last commit still persists the view state below.
			false if deltas.is_empty() => Ok(Declarations::new()),
			// The batch is drained by the time it reaches here, so a failed staging would leave the working copy
			// permanently behind. Converting the whole document restages the same edit from whatever the working
			// copy holds, including a batch that failed partway through.
			false => Self::stage_recorded(storage, deltas, registry, byte_store).or_else(|error| {
				match error {
					StagingError::NeedsWholeDocument(_) => log::debug!("{error}"),
					_ => log::error!("Staging recorded deltas failed, falling back to a whole document snapshot: {error}"),
				}
				Self::stage_whole_document(storage, interface, registry, byte_store)
			}),
		};
		match staged {
			Ok(declarations) => {
				self.declarations.extend(declarations);
				self.needs_whole_document_stage = false;
				true
			}
			Err(error) => {
				log::error!("Storage snapshot staging failed: {error}");
				false
			}
		}
	}

	/// Converts the whole document and stages the difference from what the working copy holds.
	///
	/// Stages without retiring: a tool drag fires several `CommitTransaction`s but is one legacy undo
	/// step, so the ops accumulate in the hot log and coalesce at the next undo-step boundary.
	fn stage_whole_document(storage: &mut document_format::GddV1, interface: &NodeNetworkInterface, registry: &ResourceRegistry, byte_store: &dyn ResourceStorage) -> Result<Declarations, String> {
		let metadata_view = StorageMetadataView::new(interface);
		storage
			.stage_runtime_snapshot(interface.document_network(), &metadata_view, registry, byte_store)
			.map_err(|error| error.to_string())
	}

	/// Stages what the store recorded since the last drain.
	///
	/// This and the whole-document conversion produce the same registry for the same edit. They differ in
	/// what they cost and in what they can miss: a conversion sees every change however it was made,
	/// while this sees only what went through the store, so a write that bypasses it is not persisted.
	fn stage_recorded(storage: &mut document_format::GddV1, deltas: &[EditorDelta], registry: &ResourceRegistry, byte_store: &dyn ResourceStorage) -> Result<Declarations, StagingError> {
		let peer = storage.session().peer();
		let constructed = construct_batch(deltas, storage.registry(), registry, peer)?;

		storage
			.stage_constructed_ops(constructed.ops, &constructed.declarations.bytes, byte_store)
			.map_err(|error| StagingError::Storage(error.to_string()))?;
		Ok(constructed.declarations.decoded)
	}

	/// The parts of a commit that are not the graph: the per-peer and per-network view settings.
	fn persist_view_state(&mut self, interface: &NodeNetworkInterface, view_settings: BTreeMap<String, Value>) {
		let Some(storage) = self.storage.as_mut() else { return };

		let network_view_settings = storage
			.network_ids(interface.document_network(), &StorageMetadataView::new(interface))
			.ok()
			.map(|network_ids| collect_network_view_settings(interface, &network_ids));

		if let Err(error) = storage.set_view_settings(view_settings) {
			log::error!("Persisting view settings failed: {error}");
		}

		if let Some(network_view_settings) = network_view_settings
			&& let Err(error) = storage.set_network_view_settings(network_view_settings)
		{
			log::error!("Persisting per-network view settings failed: {error}");
		}
	}

	/// Build a new interface from the working registry and cached declarations. `None` when it doesn't convert.
	pub fn rebuild_interface(&self) -> Option<NodeNetworkInterface> {
		let storage = self.storage.as_ref()?;

		storage
			.registry()
			.to_runtime_with_full_metadata(&self.declarations, storage.blobs())
			.map_err(|error| error.to_string())
			.and_then(|(network, node_entries, network_entries)| build_interface_from_storage(network, node_entries, network_entries).map_err(|error| error.to_string()))
			.inspect_err(|error| log::error!("Storage interface rebuild failed: {error}"))
			.ok()
	}

	pub fn undecoded_declaration_hashes(&self) -> Vec<ResourceHash> {
		let Some(storage) = self.storage.as_ref() else { return Vec::new() };

		let mut hashes: Vec<ResourceHash> = storage
			.session()
			.all_declaration_resources()
			.into_iter()
			.filter_map(|(id, hash)| hash.filter(|_| !self.declarations.contains_key(&id)))
			.collect();
		hashes.sort_unstable();
		hashes.dedup();
		hashes
	}

	/// Cache a peer's resource as a declaration if one names its hash. Returns false when the bytes don't decode.
	pub fn cache_declaration_bytes(&mut self, hash: ResourceHash, bytes: &[u8]) -> bool {
		let Some(storage) = self.storage.as_ref() else { return true };

		let referencing: Vec<ResourceId> = storage
			.session()
			.all_declaration_resources()
			.into_iter()
			.filter_map(|(id, declaration_hash)| (declaration_hash == Some(hash)).then_some(id))
			.collect();
		if referencing.is_empty() {
			return true;
		}

		match document_graph_storage::decode_declaration(bytes) {
			Ok(declaration) => {
				self.declarations.extend(referencing.into_iter().map(|id| (id, declaration.clone())));
				true
			}
			Err(error) => {
				log::error!("Failed to deserialize a received ProtoNode declaration: {error}");
				false
			}
		}
	}
}
