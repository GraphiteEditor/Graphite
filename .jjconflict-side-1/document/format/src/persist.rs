//! The per-edit persist path on the [`Gdd`] handle: stage/retire/commit, hot-log and history
//! append, registry snapshots, session-state and manifest writes, plus view-settings setters.
//! Synchronous and read-free (the manifest is cached on the handle); the container's `*_non_blocking`
//! surface absorbs durability.

use document_container::AsyncContainer;
#[cfg(feature = "conversion")]
use document_graph_storage::NodeMetadataSource;
use document_graph_storage::{HotOp, Rev, TimeStamp};
#[cfg(feature = "conversion")]
use graphene_resource::ResourceStorage;

use crate::error::Error;
use crate::layout::Layout;
use crate::manifest::Manifest;
use crate::session_state::SessionState;
use crate::{Gdd, MANIFEST_CODEC, io};

impl<L: Layout> Gdd<L> {
	/// Move the undo cursor back one commit (silent-zone reflog undo) and persist the new cursor. Returns
	/// the undone `Rev`. The working registry is rewound in place by the reverse delta, so re-snapshot it
	/// (alongside `head`) or a reopen would read a `registry.bin` inconsistent with the persisted cursor.
	pub fn undo(&mut self) -> Result<Rev, Error> {
		let rev = self.session.undo()?;
		self.persist_registry_snapshot()?;
		self.persist_session_state()?;
		Ok(rev)
	}

	/// Re-apply the most-recently-undone commit and persist the new cursor and re-snapshotted registry.
	pub fn redo(&mut self) -> Result<Rev, Error> {
		let rev = self.session.redo()?;
		self.persist_registry_snapshot()?;
		self.persist_session_state()?;
		Ok(rev)
	}

	/// Edit the cached manifest and persist it. Always JSON, synchronous.
	pub fn update_manifest(&mut self, edit: impl FnOnce(&mut Manifest)) -> Result<(), Error> {
		edit(&mut self.manifest);
		io::write_single(&self.working, self.layout.manifest_basename(), MANIFEST_CODEC, &self.manifest)?;
		Ok(())
	}

	/// Stage a runtime snapshot as hot ops without retiring: diff the runtime against the working
	/// registry, append the hot frames (so a crash recovers the work), and persist proto-node
	/// declaration bytes. The working registry reflects the edit immediately, but nothing enters durable
	/// retired history until [`retire_pending_interaction`](Self::retire_pending_interaction). Staging on
	/// every edit while retiring only at interaction boundaries lets several edits coalesce into one retired
	/// interaction.
	///
	/// # Errors
	/// [`Error::Commit`] if the runtime diff is rejected by the session. On an [`Error::Container`] /
	/// [`Error::Codec`] from persisting the hot frames, the session has already advanced past what the
	/// working copy reflects, so the caller should treat the document as needing re-persist.
	#[cfg(feature = "conversion")]
	pub fn stage_runtime_snapshot<M: NodeMetadataSource>(
		&mut self,
		network: &graph_craft::document::NodeNetwork,
		metadata: &M,
		resources: &graphene_resource::ResourceRegistry,
		byte_store: &dyn ResourceStorage,
	) -> Result<(), Error> {
		let (hot_ops, declaration_bytes) = self.session.stage_from_runtime(network, metadata, resources)?;

		for hot_op in &hot_ops {
			self.append_hot_frame(hot_op)?;
		}

		// Persist proto-node declaration content to the byte store (the global cache in the editor,
		// the working-copy container for standalone export). Content-addressed, so re-storing
		// identical bytes on every commit is an idempotent no-op.
		for bytes in declaration_bytes.values() {
			byte_store.store(bytes);
		}
		Ok(())
	}

	/// Retire every pending hot op into durable history as a single interaction (marking the batch's last
	/// delta as the interaction boundary), then re-snapshot the registry. One interaction is one undo unit,
	/// so the caller invokes this at each undo-step boundary and before any undo/redo. A no-op when there
	/// are no pending hot ops.
	pub fn retire_pending_interaction(&mut self) -> Result<Vec<Rev>, Error> {
		let Some(up_to) = self.session.hot_log().iter().map(|hot_op| hot_op.timestamp).max() else {
			return Ok(Vec::new());
		};
		self.retire_inner(up_to, true)
	}

	/// Commit a runtime snapshot as one complete interaction: stage it, then immediately retire it into
	/// durable history. Convenience for callers that produce a whole interaction atomically (tests, and any
	/// one-shot commit). Equivalent to [`stage_runtime_snapshot`](Self::stage_runtime_snapshot) followed
	/// by [`retire_pending_interaction`](Self::retire_pending_interaction).
	#[cfg(feature = "conversion")]
	pub fn commit_from_runtime<M: NodeMetadataSource>(
		&mut self,
		network: &graph_craft::document::NodeNetwork,
		metadata: &M,
		resources: &graphene_resource::ResourceRegistry,
		byte_store: &dyn ResourceStorage,
	) -> Result<Vec<Rev>, Error> {
		self.stage_runtime_snapshot(network, metadata, resources, byte_store)?;
		self.retire_pending_interaction()
	}

	/// Apply a hot op from the broadcast stream, appending one frame to the hot log.
	///
	/// # Errors
	/// Returns [`Error::Crdt`] if the op is rejected by the session, or an [`Error::Container`] /
	/// [`Error::Codec`] if persisting the hot frame fails. On a persist failure the session has already
	/// advanced past what the working copy reflects, so the caller should treat the document as needing
	/// re-persist (mirrors [`stage_runtime_snapshot`](Self::stage_runtime_snapshot)).
	pub fn apply_hot_op(&mut self, op: HotOp) -> Result<(), Error> {
		self.session.apply_hot_op(op.clone())?;
		self.append_hot_frame(&op)?;
		Ok(())
	}

	/// Persist freshly-staged hot ops and immediately retire them into durable history. Appends each
	/// hot frame (so a crash before retirement still recovers the work), then retires up to the last
	/// staged timestamp, which drains exactly these ops and re-snapshots the registry. Returns the
	/// retired `Rev`s. A no-op when nothing was staged.
	pub(crate) fn append_and_retire(&mut self, hot_ops: &[HotOp], interaction_end: bool) -> Result<Vec<Rev>, Error> {
		let Some(last) = hot_ops.last() else { return Ok(Vec::new()) };

		for hot_op in hot_ops {
			self.append_hot_frame(hot_op)?;
		}

		self.retire_inner(last.timestamp, interaction_end)
	}

	/// Encode the history deltas identified by `revs` and append them to the history file. `revs` comes
	/// from `Session::retire` in append order, which is a valid replay order, so a direct per-rev lookup
	/// preserves replay order without scanning the whole history.
	fn append_history_deltas(&mut self, revs: &[Rev]) -> Result<(), Error> {
		let mut buffer = Vec::new();
		for &rev in revs {
			let Some(delta) = self.session.delta(rev) else {
				log::error!("Retired rev {rev:?} missing from history; skipping its history frame");
				continue;
			};
			self.manifest.codecs.history.append(&mut buffer, delta)?;
		}
		self.working.append_non_blocking(&io::path_for(self.layout.history_basename(), self.manifest.codecs.history), &buffer)?;
		Ok(())
	}

	/// Set a local annotation (e.g. a commit message) on an existing retired delta and re-persist it.
	/// Unlike the per-interaction marker written inline at retire, this targets an already-written delta, so
	/// the whole history file is rewritten in topological order. O(history) — fine for occasional user
	/// labeling, not for per-interaction marking (which uses the inline path). No-op if `rev` is unknown.
	pub fn annotate_delta(&mut self, rev: Rev, key: &str, value: serde_json::Value) -> Result<(), Error> {
		if self.session.annotate_delta(rev, key, value) {
			self.rewrite_history()?;
		}
		Ok(())
	}

	/// Rewrite the entire history file from the in-memory session. `history()` yields deltas in
	/// topological (append) order, which is a valid replay order, so no separate sort is needed.
	fn rewrite_history(&mut self) -> Result<(), Error> {
		let mut buffer = Vec::new();
		for delta in self.session.history() {
			self.manifest.codecs.history.append(&mut buffer, delta)?;
		}
		self.working.write_non_blocking(&io::path_for(self.layout.history_basename(), self.manifest.codecs.history), &buffer)?;
		Ok(())
	}

	fn persist_session_state(&mut self) -> Result<(), Error> {
		let state = SessionState {
			peer_id: self.session.peer(),
			head_rev: self.session.head_rev(),
			last_broadcast_rev: self.session.last_broadcast_rev(),
			redo_stack: self.session.redo_stack().to_vec(),
			next_node_counter: self.session.next_node_counter(),
			view_settings: self.view_settings.clone(),
			network_view_settings: self.network_view_settings.clone(),
		};
		io::write_single(&self.working, self.layout.session_basename(), self.manifest.codecs.session, &state)?;
		Ok(())
	}

	/// Re-snapshot the materialized working registry to `registry.bin`. `Session::load` trusts the stored
	/// registry to match the persisted `head`, so any cursor move (undo/redo) that rewinds the working
	/// registry without retiring must re-persist it or a reopen would read a registry inconsistent with
	/// `head`. Synchronous and hot-path-safe (`write_non_blocking`).
	fn persist_registry_snapshot(&mut self) -> Result<(), Error> {
		io::write_single(&self.working, self.layout.registry_basename(), self.manifest.codecs.registry, self.session.registry())?;
		Ok(())
	}

	/// Replace the per-peer view settings and persist them to `session.json`. Called by the editor when
	/// the viewport or a document-level toggle changes; never enters the registry, history, or CRDT.
	pub fn set_view_settings(&mut self, view_settings: std::collections::BTreeMap<String, serde_json::Value>) -> Result<(), Error> {
		self.view_settings = view_settings;
		self.persist_session_state()
	}

	/// Advance the published frontier to `rev` and persist it to `session.json`, so the silent/published
	/// undo boundary survives a reopen. Called by the (future) broadcast transport as commits are shared.
	pub fn publish_up_to(&mut self, rev: document_graph_storage::Rev) -> Result<(), Error> {
		self.session.publish_up_to(rev);
		self.persist_session_state()
	}

	/// Replace the per-network view settings and persist them to `session.json`. Per-peer, per-network; never
	/// enters the registry, history, or CRDT.
	pub fn set_network_view_settings(
		&mut self,
		network_view_settings: std::collections::BTreeMap<document_graph_storage::NetworkId, std::collections::BTreeMap<String, serde_json::Value>>,
	) -> Result<(), Error> {
		self.network_view_settings = network_view_settings;
		self.persist_session_state()
	}

	fn append_hot_frame(&mut self, op: &HotOp) -> Result<(), Error> {
		let mut buffer = Vec::new();
		self.manifest.codecs.hot_log.append(&mut buffer, op)?;
		self.working.append_non_blocking(&io::path_for(self.layout.hot_log_basename(), self.manifest.codecs.hot_log), &buffer)?;
		Ok(())
	}

	/// Working-copy checkpoint: promote hot ops with timestamp `≤ up_to` into retired deltas,
	/// append them to the history file, rewrite the hot log with remaining (unretired) ops, and
	/// re-snapshot the registry. Synchronous.
	pub fn retire(&mut self, up_to: TimeStamp) -> Result<Vec<Rev>, Error> {
		self.retire_inner(up_to, false)
	}

	/// `interaction_end`: mark the batch's last delta as an interaction boundary (one undo unit) before its
	/// history frame is written, so the marker persists on reopen without a later frame rewrite.
	fn retire_inner(&mut self, up_to: TimeStamp, interaction_end: bool) -> Result<Vec<Rev>, Error> {
		let new_revs = self.session.retire(up_to)?;

		// Mark before `append_history_deltas` so the on-disk frame carries the boundary.
		if interaction_end && let Some(&last) = new_revs.last() {
			self.session.mark_interaction_end(last);
		}

		if !new_revs.is_empty() {
			self.append_history_deltas(&new_revs)?;
		}

		// Rewrite hot log with whatever survived retirement.
		let mut hot_buffer = Vec::new();
		for hot_op in self.session.hot_log() {
			self.manifest.codecs.hot_log.append(&mut hot_buffer, hot_op)?;
		}
		self.working
			.write_non_blocking(&io::path_for(self.layout.hot_log_basename(), self.manifest.codecs.hot_log), &hot_buffer)?;

		self.persist_registry_snapshot()?;
		self.persist_session_state()?;

		Ok(new_revs)
	}
}
