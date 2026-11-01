use std::collections::VecDeque;
use std::collections::{BTreeMap, HashSet};

use document_graph_storage::{Declarations, Registry, Value};
use graph_craft::application_io::resource::{ResourceHash, ResourceId, ResourceRegistry, ResourceStorage};

use super::utility_types::network_interface::NodeNetworkInterface;
use super::utility_types::network_interface::editor_delta::{EditorDelta, StagingError, construct_batch};
use super::utility_types::network_interface::storage_metadata::{StorageMetadataView, build_interface_from_storage, collect_network_view_settings};

/// Per-document undo/redo state: the legacy snapshot stacks plus the `Gdd` working-copy cursor that is
/// becoming the authoritative history. Owns the dual-stack bookkeeping push/pop/clear and the cursor's stage/retire/move/verify
/// lifecycle, so the handler drives history through one surface rather than three loose fields.
///
/// Not serialized: the legacy stacks are runtime-only, and a clone shares the working-copy container by
/// `Arc`, so it keeps reading the live working copy.
#[derive(derivative::Derivative)]
#[derivative(Clone, Debug, Default)]
pub struct DocumentHistory {
	/// Stack of document network snapshots for previous history states.
	legacy_undo_stack: VecDeque<NodeNetworkInterface>,
	/// Stack of document network snapshots for future history states.
	legacy_redo_stack: VecDeque<NodeNetworkInterface>,
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
	/// How storage undid each step on the legacy redo stack, aligned with its top.
	undo_notes: Vec<UndoNote>,
}

#[derive(Clone, Debug)]
enum UndoNote {
	/// The step was still hot and its ops were taken back.
	Retracted(Vec<document_graph_storage::RegistryDelta>),
	/// The step had retired in a session and was dropped out of the shared line.
	Dropped(document_graph_storage::Rev),
	/// The storage cursor moved back over the step.
	Cursor,
}

/// Why [`DocumentHistory::move_cursor`] produced no interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorMoveError {
	/// Nothing to move to, unmounted, or the move itself failed. The cursor did not move.
	NotMoved,
	/// The cursor moved but the rebuild from it failed. It stays moved unless the caller reverts it.
	RebuildFailed,
}

impl DocumentHistory {
	// ===== Legacy snapshot stacks =====

	/// Push a snapshot onto the undo stack, evicting the oldest entry past the history cap.
	pub fn push_undo(&mut self, snapshot: NodeNetworkInterface) {
		Self::push_capped(&mut self.legacy_undo_stack, snapshot);
	}

	/// Push a snapshot onto the redo stack, evicting the oldest entry past the history cap.
	pub fn push_redo(&mut self, snapshot: NodeNetworkInterface) {
		Self::push_capped(&mut self.legacy_redo_stack, snapshot);
	}

	/// Pop the most recent undo snapshot, or `None` when the stack is empty.
	pub fn pop_undo(&mut self) -> Option<NodeNetworkInterface> {
		self.legacy_undo_stack.pop_back()
	}

	/// Pop the most recent redo snapshot, or `None` when the stack is empty.
	pub fn pop_redo(&mut self) -> Option<NodeNetworkInterface> {
		self.legacy_redo_stack.pop_back()
	}

	/// Drop the most recently pushed undo snapshot (used to cancel a transaction that ended up unmodified).
	pub fn discard_last_undo(&mut self) {
		self.legacy_undo_stack.pop_back();
	}

	/// Clear the redo stack, called when a fresh edit invalidates the redo future.
	pub fn clear_redo(&mut self) {
		self.legacy_redo_stack.clear();
		self.undo_notes.clear();
	}

	/// Add the resources referenced by every snapshot in both history stacks into `resources`, so
	/// history-only resources stay alive for legacy undo/redo.
	pub fn collect_used_resources(&self, resources: &mut HashSet<ResourceId>) {
		for interface in self.legacy_undo_stack.iter().chain(&self.legacy_redo_stack) {
			interface.collect_used_resources(resources);
		}
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

	/// The decoded proto-node declarations the working copy's registry states reference.
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

	/// Close the open storage interaction at an undo-step boundary, so it aligns with the legacy step. No-op while unmounted.
	pub fn end_storage_interaction(&mut self) {
		let Some(storage) = self.storage.as_mut() else { return };
		if let Err(error) = storage.end_interaction() {
			log::error!("Closing the storage interaction failed: {error}");
		}
	}

	/// Undoes the latest step in storage alone, moving its legacy snapshot to the redo stack uninstalled. A hot step is
	/// taken back so it never becomes history. In a session, a retired step is dropped out of the shared line.
	/// Returns whether storage undid it.
	pub fn undo_in_storage(&mut self, in_session: bool) -> bool {
		if self.legacy_undo_stack.is_empty() {
			return false;
		}
		let note = if let Some(ops) = self.retract_storage_interaction() {
			UndoNote::Retracted(ops)
		} else if in_session && let Some(rev) = self.undo_retired_step() {
			UndoNote::Dropped(rev)
		} else {
			return false;
		};

		if let Some(snapshot) = self.pop_undo() {
			self.push_redo(snapshot);
		}
		self.note_undo(note);
		true
	}

	/// Redoes a step [`undo_in_storage`](Self::undo_in_storage) undid, moving its legacy snapshot back uninstalled.
	/// Returns whether the next redo step was one.
	pub fn redo_in_storage(&mut self) -> bool {
		if self.legacy_redo_stack.is_empty() {
			return false;
		}
		let reachable = self.storage.as_ref().is_some_and(|storage| storage.can_change_line());
		let Some(note) = self
			.undo_notes
			.pop_if(|note| matches!(note, UndoNote::Retracted(_)) || (reachable && matches!(note, UndoNote::Dropped(_))))
		else {
			return false;
		};

		if let Some(snapshot) = self.pop_redo() {
			self.push_undo(snapshot);
		}
		match note {
			UndoNote::Retracted(ops) => self.restage_ops(ops),
			UndoNote::Dropped(rev) => {
				self.redo_retired_step(rev);
			}
			UndoNote::Cursor => {}
		}
		true
	}

	/// Records that the snapshot just pushed onto the redo stack was undone by moving the storage cursor.
	pub fn note_cursor_undo(&mut self) {
		self.note_undo(UndoNote::Cursor);
	}

	/// Forgets the note of the snapshot just popped off the redo stack.
	pub fn forget_undo_note(&mut self) {
		self.undo_notes.pop();
	}

	fn note_undo(&mut self, note: UndoNote) {
		self.undo_notes.push(note);
		let evicted = self.undo_notes.len().saturating_sub(self.legacy_redo_stack.len());
		self.undo_notes.drain(..evicted);
	}

	fn retract_storage_interaction(&mut self) -> Option<Vec<document_graph_storage::RegistryDelta>> {
		self.storage.as_mut()?.retract_interaction().unwrap_or_else(|error| {
			log::error!("Taking the storage interaction back failed: {error}");
			None
		})
	}

	fn restage_ops(&mut self, ops: Vec<document_graph_storage::RegistryDelta>) {
		let Some(storage) = self.storage.as_mut() else { return };
		if let Err(error) = storage.restage_ops(ops) {
			log::error!("Staging the redone step failed: {error}");
		}
	}

	fn undo_retired_step(&mut self) -> Option<document_graph_storage::Rev> {
		self.storage.as_mut()?.undo_retired_step().unwrap_or_else(|error| {
			log::error!("Undoing the retired step failed: {error}");
			None
		})
	}

	/// The resource bytes garbage collection must keep for the stored document.
	pub fn resource_gc_roots(&self) -> HashSet<ResourceHash> {
		self.storage.as_ref().map(|storage| storage.resource_gc_roots()).unwrap_or_default()
	}

	/// Puts a step dropped out of a session's line back on top.
	pub fn redo_retired_step(&mut self, rev: document_graph_storage::Rev) -> bool {
		self.change_line(|storage| storage.redo_retired_step(rev), "Redoing the dropped step failed")
	}

	/// Drops a retired interaction out of the line. See [`document_format::Gdd::drop_step`].
	pub fn drop_step(&mut self, rev: document_graph_storage::Rev) -> bool {
		self.change_line(|storage| storage.drop_step(rev), "Removing the step failed")
	}

	/// Moves the shared head to an ancestor. See [`document_format::Gdd::move_head`].
	pub fn move_head_to(&mut self, rev: document_graph_storage::Rev) -> bool {
		self.change_line(|storage| storage.move_head(rev), "Moving the head failed")
	}

	/// Returns whether the line could be changed, which needs a host to ask when this peer does not retire.
	fn change_line(&mut self, change: impl FnOnce(&mut document_format::GddV1) -> Result<(), document_format::Error>, failure: &str) -> bool {
		let Some(storage) = self.storage.as_mut().filter(|storage| storage.can_change_line()) else {
			return false;
		};
		change(storage).inspect_err(|error| log::error!("{failure}: {error}")).is_ok()
	}

	/// Forgets the legacy snapshots, which describe states off the line once the head moved elsewhere.
	pub fn clear_legacy(&mut self) {
		self.legacy_undo_stack.clear();
		self.legacy_redo_stack.clear();
		self.undo_notes.clear();
	}

	/// Close this peer's open interaction and retire every closed one into durable Gdd history. No-op while unmounted.
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
			false => Self::stage_recorded(storage, deltas, interface, registry, byte_store).or_else(|error| {
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
	/// This and the whole-document conversion produce the same registry for the same edit, which
	/// `verify_round_trip` checks when the `validate_storage_round_trip` preference is on. They differ in
	/// what they cost and in what they can miss: a conversion sees every change however it was made,
	/// while this sees only what went through the store, so a write that bypasses it is not persisted.
	fn stage_recorded(
		storage: &mut document_format::GddV1,
		deltas: &[EditorDelta],
		interface: &NodeNetworkInterface,
		registry: &ResourceRegistry,
		byte_store: &dyn ResourceStorage,
	) -> Result<Declarations, StagingError> {
		let peer = storage.session().peer();
		let metadata_view = StorageMetadataView::new(interface);
		let constructed = construct_batch(deltas, storage.registry(), registry, &metadata_view, peer)?;

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

	/// Move the `Gdd` undo/redo cursor along the retired interaction chain, flushing any open interaction
	/// first, and rebuild the interface from the rewound registry using the declaration cache.
	pub fn move_cursor(&mut self, undo: bool) -> Result<NodeNetworkInterface, CursorMoveError> {
		self.retire_storage_interaction();

		let storage = self.storage.as_mut().ok_or(CursorMoveError::NotMoved)?;

		let moved = if undo {
			if !storage.can_undo() {
				return Err(CursorMoveError::NotMoved);
			}
			storage.undo().map(|_| ())
		} else {
			if !storage.can_redo() {
				return Err(CursorMoveError::NotMoved);
			}
			storage.redo().map(|_| ())
		};
		if let Err(error) = moved {
			log::error!("Storage undo/redo cursor move failed: {error}");
			return Err(CursorMoveError::NotMoved);
		}

		self.rebuild_interface().ok_or(CursorMoveError::RebuildFailed)
	}

	/// Build a fresh interface from the working registry, using the declaration cache so no resource load
	/// is needed. `None` when the registry does not convert.
	pub fn rebuild_interface(&self) -> Option<NodeNetworkInterface> {
		let storage = self.storage.as_ref()?;

		storage
			.registry()
			.to_runtime_with_full_metadata(&self.declarations)
			.map_err(|error| error.to_string())
			.and_then(|(network, node_entries, network_entries)| build_interface_from_storage(network, node_entries, network_entries).map_err(|error| error.to_string()))
			.inspect_err(|error| log::error!("Storage interface rebuild failed: {error}"))
			.ok()
	}

	/// The hashes of declaration resources the working copy names but holds no decoded declaration for.
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

	/// Cache a resource that arrived from a peer as a proto-node declaration, under every declaration
	/// resource referencing its hash. Ignores resources no declaration refers to, such as images and fonts.
	///
	/// Returns false when the bytes do not decode.
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

	/// Step the cursor back the other way, undoing a [`move_cursor`](Self::move_cursor) in the `undo`
	/// direction whose rebuild failed.
	pub fn revert_cursor(&mut self, undo: bool) {
		let Some(storage) = self.storage.as_mut() else { return };

		let reverted = if undo { storage.redo() } else { storage.undo() };
		if let Err(error) = reverted {
			log::error!("Storage undo/redo cursor revert failed: {error}");
		}
	}

	// Soak round-trip verification (runtime-gated by `validate_storage_round_trip`)
	// These log drift rather than panicking, since the soak can run in release where a crash is
	// unacceptable; tests still fail loud via the `#[cfg(test)]` panics.

	/// Soak check: the stored registry should equal a fresh `from_runtime`, and a `to_runtime` of it should
	/// equal the original network.
	pub fn verify_round_trip(&self, interface: &NodeNetworkInterface, registry: &ResourceRegistry) {
		use super::diff_networks;
		use super::document_diff::diff_registries;

		let Some(storage) = &self.storage else { return };
		let peer = storage.session().peer();

		let network = interface.document_network();
		let metadata_view = StorageMetadataView::new(interface);

		let conversion = match Registry::convert_from_runtime(network, &metadata_view, registry, peer) {
			Ok(conversion) => conversion,
			Err(error) => {
				log::error!("storage round-trip: from_runtime failed: {error}");
				return;
			}
		};
		let target = &conversion.registry;
		let declarations = &conversion.declarations;

		let stored = storage.registry();
		if !stored.value_equal(target) {
			log::error!("storage round-trip: registry value drift after commit\n{}", diff_registries(stored, target));
			#[cfg(test)]
			panic!("storage round-trip: registry value drift after commit");
		}
		if !stored.order_consistent(target) {
			log::error!("storage round-trip: timestamp order inconsistent between stored and target");
			#[cfg(test)]
			panic!("storage round-trip: timestamp order inconsistent between stored and target");
		}

		let (round_tripped, _entries) = match stored.to_runtime_with_metadata(declarations) {
			Ok(result) => result,
			Err(error) => {
				log::error!("storage round-trip: to_runtime failed: {error}");
				return;
			}
		};
		if &round_tripped != network {
			log::error!("storage round-trip: network drift after to_runtime\n{}", diff_networks(network, &round_tripped));
			#[cfg(test)]
			panic!("storage round-trip: network drift after to_runtime");
		}
	}

	/// Soak check: after a cursor move, the cursor's registry should equal a fresh `from_runtime` of the
	/// current (legacy-restored) interface. `current_resources` are the resources the live network
	/// references; history-only resources the cursor dropped are expected, not drift.
	pub fn verify_cursor_matches_runtime(&self, interface: &NodeNetworkInterface, registry: &ResourceRegistry, current_resources: &HashSet<ResourceId>) {
		use super::document_diff::diff_registries;

		let Some(storage) = &self.storage else { return };
		let peer = storage.session().peer();

		let network = interface.document_network();
		let metadata_view = StorageMetadataView::new(interface);

		let Ok(mut conversion) = Registry::convert_from_runtime(network, &metadata_view, registry, peer) else {
			log::error!("undo/redo shadow: from_runtime failed");
			return;
		};

		let stored = storage.registry();

		// The cursor reverts the interaction's `AddResource` while the runtime keeps the resource alive for
		// legacy redo, so a history-only resource the cursor dropped is expected. Drop those before comparing.
		conversion.registry.resources.retain(|id, _| stored.resources.contains_key(id) || current_resources.contains(id));

		if !stored.value_equal(&conversion.registry) {
			log::error!(
				"undo/redo shadow: cursor registry diverged from the restored interface\n{}",
				diff_registries(stored, &conversion.registry)
			);
		}
	}

	fn push_capped(stack: &mut VecDeque<NodeNetworkInterface>, snapshot: NodeNetworkInterface) {
		stack.push_back(snapshot);
		if stack.len() > crate::consts::MAX_UNDO_HISTORY_LEN {
			stack.pop_front();
		}
	}
}
