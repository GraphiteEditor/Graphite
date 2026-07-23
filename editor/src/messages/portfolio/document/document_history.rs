use std::collections::VecDeque;
use std::collections::{BTreeMap, HashSet};

use document_graph_storage::Registry;
use graph_craft::application_io::resource::{ResourceId, ResourceRegistry, ResourceStorage};

use super::utility_types::guide::GuideLine;
use super::utility_types::network_interface::NodeNetworkInterface;
use super::utility_types::network_interface::storage_metadata::{StorageMetadataView, collect_network_view_settings};

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
	lagacy_undo_stack: VecDeque<NodeNetworkInterface>,
	/// Stack of document network snapshots for future history states.
	lagacy_redo_stack: VecDeque<NodeNetworkInterface>,
	/// Guide-line state paired with each legacy undo snapshot.
	guide_undo_stack: VecDeque<Vec<GuideLine>>,
	/// Guide-line state paired with each legacy redo snapshot.
	guide_redo_stack: VecDeque<Vec<GuideLine>>,
	/// The `Gdd` working copy: owns the CRDT `Session` and mirrors edits to disk. `None` until the mount
	/// future built by `load_document` resolves.
	#[derivative(Debug = "ignore")]
	storage: Option<document_format::GddV1>,
}

impl DocumentHistory {
	// ===== Legacy snapshot stacks =====

	/// Push a snapshot onto the undo stack, evicting the oldest entry past the history cap.
	pub fn push_undo(&mut self, snapshot: NodeNetworkInterface) {
		Self::push_capped(&mut self.lagacy_undo_stack, snapshot);
	}

	/// Push a guide-line snapshot alongside the most recent undo network snapshot.
	pub fn push_guide_undo(&mut self, guide_lines: Vec<GuideLine>) {
		Self::push_capped(&mut self.guide_undo_stack, guide_lines);
	}

	/// Push a snapshot onto the redo stack, evicting the oldest entry past the history cap.
	pub fn push_redo(&mut self, snapshot: NodeNetworkInterface) {
		Self::push_capped(&mut self.lagacy_redo_stack, snapshot);
	}

	/// Push a guide-line snapshot alongside the most recent redo network snapshot.
	pub fn push_guide_redo(&mut self, guide_lines: Vec<GuideLine>) {
		Self::push_capped(&mut self.guide_redo_stack, guide_lines);
	}

	/// Pop the most recent undo snapshot, or `None` when the stack is empty.
	pub fn pop_undo(&mut self) -> Option<NodeNetworkInterface> {
		self.lagacy_undo_stack.pop_back()
	}

	/// Pop the guide-line snapshot paired with the most recent undo entry.
	pub fn pop_guide_undo(&mut self) -> Option<Vec<GuideLine>> {
		self.guide_undo_stack.pop_back()
	}

	/// Pop the most recent redo snapshot, or `None` when the stack is empty.
	pub fn pop_redo(&mut self) -> Option<NodeNetworkInterface> {
		self.lagacy_redo_stack.pop_back()
	}

	/// Pop the guide-line snapshot paired with the most recent redo entry.
	pub fn pop_guide_redo(&mut self) -> Option<Vec<GuideLine>> {
		self.guide_redo_stack.pop_back()
	}

	/// Drop the most recently pushed undo snapshot (used to cancel a transaction that ended up unmodified).
	pub fn discard_last_undo(&mut self) {
		self.lagacy_undo_stack.pop_back();
		self.guide_undo_stack.pop_back();
	}

	/// Clear the redo stack, called when a fresh edit invalidates the redo future.
	pub fn clear_redo(&mut self) {
		self.lagacy_redo_stack.clear();
		self.guide_redo_stack.clear();
	}

	/// Add the resources referenced by every snapshot in both history stacks into `resources`, so
	/// history-only resources stay alive for legacy undo/redo.
	pub fn collect_used_resources(&self, resources: &mut HashSet<ResourceId>) {
		for interface in self.lagacy_undo_stack.iter().chain(&self.lagacy_redo_stack) {
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

	/// Attach (or clear) the `Gdd` working copy once the mount future resolves.
	pub fn set_storage(&mut self, storage: Option<document_format::GddV1>) {
		self.storage = storage;
	}

	/// Retire the pending staged hot ops into durable Gdd history as one undo unit. Called at each undo-step
	/// boundary (a new `StartTransaction`) and before undo/redo, so the per-`CommitTransaction` staging
	/// coalesces into one interaction aligned with the legacy step. No-op while unmounted.
	pub fn retire_storage_interaction(&mut self) {
		let Some(storage) = self.storage.as_mut() else { return };
		if let Err(error) = storage.retire_pending_interaction() {
			log::error!("Storage interaction retirement failed: {error}");
		}
	}

	/// Stage the runtime snapshot into the `Gdd` working copy at each `CommitTransaction`. No-op while
	/// unmounted. Proto-node declaration bytes go into `byte_store` (the app-global resource cache). The
	/// staged hot ops are retired by [`retire_storage_interaction`](Self::retire_storage_interaction) at
	/// undo-step boundaries. `validate` (the `validate_storage_round_trip` preference) gates the per-commit
	/// round-trip check, off by default for its perf cost.
	pub fn stage_snapshot(
		&mut self,
		interface: &NodeNetworkInterface,
		registry: &ResourceRegistry,
		view_settings: BTreeMap<String, serde_json::Value>,
		legacy_document: &str,
		byte_store: &dyn ResourceStorage,
	) {
		let Some(storage) = self.storage.as_mut() else { return };

		let network = interface.document_network();
		let metadata_view = StorageMetadataView::new(interface);

		let network_view_settings = storage
			.network_ids(network, &metadata_view)
			.ok()
			.map(|network_ids| collect_network_view_settings(interface, &network_ids));

		// Stage without retiring: a tool drag fires several `CommitTransaction`s but is one legacy undo
		// step, so the deltas accumulate as hot ops and coalesce at the next undo-step boundary.
		if let Err(error) = storage.stage_runtime_snapshot(network, &metadata_view, registry, byte_store) {
			log::error!("Storage snapshot staging failed: {error}");
			return;
		}

		if let Err(error) = storage.set_view_settings(view_settings) {
			log::error!("Persisting view settings failed: {error}");
		}

		if let Some(network_view_settings) = network_view_settings
			&& let Err(error) = storage.set_network_view_settings(network_view_settings)
		{
			log::error!("Persisting per-network view settings failed: {error}");
		}

		// Dual-write soak: embed the legacy `.graphite` bytes so the new format
		// can be validated against (and recovered from) the old one on open.
		if let Err(error) = storage.store_legacy_document(legacy_document.as_bytes()) {
			log::error!("Embedding legacy document into working copy failed: {error}");
		}
	}

	/// Move the `Gdd` undo/redo cursor along the retired interaction chain, flushing any open interaction
	/// first. Returns a clone of the post-move `Gdd` (`Arc`-shared) so a `'static` rebuild future can read
	/// the rewound state while the live document keeps its cursor. `None` when there is nothing to move to,
	/// unmounted, or the move failed.
	pub fn move_cursor(&mut self, undo: bool) -> Option<document_format::GddV1> {
		self.retire_storage_interaction();

		let storage = self.storage.as_mut()?;

		let moved = if undo {
			if !storage.can_undo() {
				return None;
			}
			storage.undo().map(|_| ())
		} else {
			if !storage.can_redo() {
				return None;
			}
			storage.redo().map(|_| ())
		};
		if let Err(error) = moved {
			log::error!("Storage undo/redo cursor move failed: {error}");
			return None;
		}

		Some(storage.clone())
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
		let declarations = match conversion.declarations() {
			Ok(declarations) => declarations,
			Err(error) => {
				log::error!("storage round-trip: declaration rebuild failed: {error}");
				return;
			}
		};

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

		let (round_tripped, _entries) = match stored.to_runtime_with_metadata(&declarations) {
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

	fn push_capped<T>(stack: &mut VecDeque<T>, snapshot: T) {
		stack.push_back(snapshot);
		if stack.len() > crate::consts::MAX_UNDO_HISTORY_LEN {
			stack.pop_front();
		}
	}
}
