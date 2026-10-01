//! The per-edit persist path on the [`Gdd`] handle: stage/retire/commit, hot-log and history
//! append, registry snapshots, session-state and manifest writes, plus view-settings setters.
//! Synchronous and read-free (the manifest is cached on the handle); the container's `*_non_blocking`
//! surface absorbs durability.

use document_container::AsyncContainer;
#[cfg(feature = "conversion")]
use document_graph_storage::NodeMetadataSource;
use document_graph_storage::{Delta, HistoryMetadata, HotOp, MetadataFact, RegistryDelta, Rev, TimeStamp, UserId, Value, WallStamp};
#[cfg(feature = "conversion")]
use graphene_resource::ResourceStorage;

use crate::error::Error;
use crate::layout::Layout;
use crate::manifest::Manifest;
use crate::session_state::SessionState;
use crate::{Gdd, MANIFEST_CODEC, METADATA_CODEC, io};

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

	/// Set the person using this device. A change re-registers the peer on its next staged batch, and the newer
	/// registration wins everywhere.
	pub fn set_user(&mut self, user: UserId) -> Result<(), Error> {
		if self.session.user() == user {
			return Ok(());
		}
		self.session.set_user(user);
		self.persist_session_state()
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
	/// interaction. Returns the decoded declarations the snapshot references, for the caller's cache.
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
	) -> Result<document_graph_storage::Declarations, Error> {
		let (hot_ops, conversion) = self.session.stage_from_runtime(network, metadata, resources)?;
		self.persist_staged(&hot_ops)?;

		// Persist proto-node declaration content to the byte store (the global cache in the editor,
		// the working-copy container for standalone export). Content-addressed, so re-storing
		// identical bytes on every commit is an idempotent no-op.
		for bytes in conversion.declaration_bytes.values() {
			byte_store.store(bytes);
		}
		Ok(conversion.declarations)
	}

	/// Stage ops the caller built from what the editor recorded, rather than deriving them by converting
	/// the whole document and diffing it against the working registry.
	///
	/// Otherwise identical to [`stage_runtime_snapshot`](Self::stage_runtime_snapshot): each op becomes a
	/// hot frame, is broadcast to any session, and the proto-node declaration bytes go to the byte store.
	#[cfg(feature = "conversion")]
	pub fn stage_constructed_ops(
		&mut self,
		ops: Vec<document_graph_storage::RegistryDelta>,
		declaration_bytes: &document_graph_storage::from_runtime::DeclarationBytes,
		byte_store: &dyn ResourceStorage,
	) -> Result<Vec<HotOp>, Error> {
		let hot_ops = self.stage_ops(ops)?;
		for bytes in declaration_bytes.values() {
			byte_store.store(bytes);
		}
		Ok(hot_ops)
	}

	/// Stage raw registry ops as hot ops, for callers that don't go through the runtime diff.
	pub fn stage_ops(&mut self, ops: impl IntoIterator<Item = RegistryDelta>) -> Result<Vec<HotOp>, Error> {
		let hot_ops = self.session.stage_computed_ops(ops.into_iter().collect())?;
		self.persist_staged(&hot_ops)?;
		Ok(hot_ops)
	}

	/// Every staged hot op ends here: the frame that survives a crash, then the broadcast that reaches the room.
	fn persist_staged(&mut self, hot_ops: &[HotOp]) -> Result<(), Error> {
		if hot_ops.iter().any(|hot_op| !matches!(hot_op.op, RegistryDelta::EndTransaction)) {
			self.own_staged_since_tick = true;
		}
		for hot_op in hot_ops {
			self.append_hot_frame(hot_op)?;
		}
		#[cfg(feature = "network")]
		if let Some(replica) = &mut self.network {
			replica.broadcast_hot_ops(hot_ops)?;
		}
		Ok(())
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
		self.session.replay_hot_op(op.clone())?;
		self.append_hot_frame(&op)?;
		Ok(())
	}

	/// Persist freshly-staged hot ops and retire exactly them into durable history as one unit. Frames are
	/// appended first, so a crash before retirement still recovers the work. Returns the retired `Rev`s;
	/// on a peer that does not retire the ops stay hot.
	pub(crate) fn append_and_retire(&mut self, hot_ops: &[HotOp], interaction_end: bool) -> Result<Vec<Rev>, Error> {
		if hot_ops.is_empty() {
			return Ok(Vec::new());
		}
		for hot_op in hot_ops {
			self.append_hot_frame(hot_op)?;
		}
		if !self.retires_locally() {
			return Ok(Vec::new());
		}

		let ids: Vec<_> = hot_ops.iter().map(HotOp::id).collect();
		let revs = self.session.retire_hot_ops(&ids)?;
		// Marked before the history frames are written, so the boundary is on disk with them.
		if interaction_end && let Some(&last) = revs.last() {
			self.session.mark_interaction_end(last);
		}
		self.finish_retirement(&revs, &ids)?;
		Ok(revs)
	}

	/// Encode the history deltas identified by `revs` and append them to the history file. `revs` comes
	/// from `Session::retire` in append order, which is a valid replay order, so a direct per-rev lookup
	/// preserves replay order without scanning the whole history.
	pub(crate) fn append_history_deltas(&mut self, revs: &[Rev]) -> Result<(), Error> {
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
	/// the whole history file is rewritten in topological order. O(history), fine for occasional user
	/// labeling, not for per-interaction marking (which uses the inline path). No-op if `rev` is unknown.
	pub fn annotate_delta(&mut self, rev: Rev, key: &str, value: Value) -> Result<(), Error> {
		if self.session.annotate_delta(rev, key, value) {
			self.rewrite_history()?;
		}
		Ok(())
	}

	/// Rewrite the entire history file from the in-memory session. `history()` yields deltas in
	/// topological (append) order, which is a valid replay order, so no separate sort is needed.
	pub(crate) fn rewrite_history(&mut self) -> Result<(), Error> {
		let mut buffer = Vec::new();
		for delta in self.session.history() {
			self.manifest.codecs.history.append(&mut buffer, delta)?;
		}
		self.working.write_non_blocking(&io::path_for(self.layout.history_basename(), self.manifest.codecs.history), &buffer)?;
		Ok(())
	}

	/// What people state about the history, such as user names.
	pub fn metadata(&self) -> &HistoryMetadata {
		&self.metadata
	}

	/// Record a fact about a user at wall time `wall_ms`, persisted and told to the room. Returns whether it
	/// changed anything.
	pub fn record_user_attribute(&mut self, user: UserId, key: &str, value: serde_json::Value, wall_ms: f64) -> Result<bool, Error> {
		let stamp = WallStamp {
			ms: wall_ms.max(0.) as u64,
			peer: self.session.peer(),
		};
		let Some(fact) = self.metadata.set_user_attribute(user, key, value, stamp) else {
			return Ok(false);
		};
		self.append_facts(&[fact])?;
		#[cfg(feature = "network")]
		if let Some(replica) = &mut self.network {
			replica.send_metadata(&self.metadata)?;
		}
		Ok(true)
	}

	/// Record a fact about a rev, such as a label or tag on the interaction it closes. Otherwise like
	/// [`record_user_attribute`](Self::record_user_attribute).
	pub fn record_rev_attribute(&mut self, rev: Rev, key: &str, value: serde_json::Value, wall_ms: f64) -> Result<bool, Error> {
		let stamp = WallStamp {
			ms: wall_ms.max(0.) as u64,
			peer: self.session.peer(),
		};
		let Some(fact) = self.metadata.set_rev_attribute(rev, key, value, stamp) else {
			return Ok(false);
		};
		self.append_facts(&[fact])?;
		#[cfg(feature = "network")]
		if let Some(replica) = &mut self.network {
			replica.send_metadata(&self.metadata)?;
		}
		Ok(true)
	}

	/// Append facts to the metadata file, one line each; the file is never rewritten.
	pub(crate) fn append_facts(&mut self, facts: &[MetadataFact]) -> Result<(), Error> {
		let mut buffer = Vec::new();
		for fact in facts {
			METADATA_CODEC.append(&mut buffer, fact)?;
		}
		self.working.append_non_blocking(&io::path_for(self.layout.metadata_basename(), METADATA_CODEC), &buffer)?;
		Ok(())
	}

	pub(crate) fn persist_session_state(&mut self) -> Result<(), Error> {
		let state = SessionState {
			peer_id: self.session.peer(),
			user_id: self.session.user(),
			head_rev: self.session.head_rev(),
			last_broadcast_rev: self.session.last_broadcast_rev(),
			redo_stack: self.session.redo_stack().to_vec(),
			next_node_counter: self.session.next_node_counter(),
			last_hot_sequence: self.session.last_hot_sequence(),
			shared: self.shared,
			settled_marks: self.session.settled_marks().clone(),
			clock_counter: self.session.clock_counter(),
			view_settings: self.view_settings.clone(),
			network_view_settings: self.network_view_settings.clone(),
		};
		io::write_single(&self.working, self.layout.session_basename(), self.manifest.codecs.session, &state)?;
		Ok(())
	}

	/// Re-snapshot the retired registry to `registry.bin`. `Session::load` trusts it to match the persisted `head`
	/// and replays the hot log on top, so it must hold no hot op, and any cursor move must re-persist it.
	pub(crate) fn persist_registry_snapshot(&mut self) -> Result<(), Error> {
		io::write_single(&self.working, self.layout.registry_basename(), self.manifest.codecs.registry, self.session.retired_registry())?;
		Ok(())
	}

	/// Replace the per-peer view settings and persist them to `session.json`. Called by the editor when
	/// the viewport or a document-level toggle changes; never enters the registry, history, or CRDT.
	pub fn set_view_settings(&mut self, view_settings: std::collections::BTreeMap<String, document_graph_storage::Value>) -> Result<(), Error> {
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
		network_view_settings: std::collections::BTreeMap<document_graph_storage::NetworkId, std::collections::BTreeMap<String, document_graph_storage::Value>>,
	) -> Result<(), Error> {
		self.network_view_settings = network_view_settings;
		self.persist_session_state()
	}

	pub(crate) fn append_hot_frame(&mut self, op: &HotOp) -> Result<(), Error> {
		let mut buffer = Vec::new();
		self.manifest.codecs.hot_log.append(&mut buffer, op)?;
		self.working.append_non_blocking(&io::path_for(self.layout.hot_log_basename(), self.manifest.codecs.hot_log), &buffer)?;
		Ok(())
	}

	/// How many closed transactions must be waiting before they retire. Retiring later fuses more of a gesture
	/// into one delta and leaves more for undo to rewind silently.
	pub const RETIRE_AFTER_TRANSACTIONS: usize = 10;
	/// How long a closed transaction sits before it may retire, so every peer has seen all of it.
	pub const RETIRE_MIN_AGE_MS: f64 = 2_000.0;
	/// The longest anything waits: a quiet session still retires, and a transaction with a gap in its author's
	/// run retires with what arrived.
	pub const RETIRE_MAX_AGE_MS: f64 = 60_000.0;

	/// Closes this peer's open transaction, if any, with a marker broadcast like any other hot op. The editor
	/// calls this at each undo-step boundary.
	pub fn end_transaction(&mut self) -> Result<(), Error> {
		let staged = self.session.end_transaction()?;
		self.persist_staged(&staged)?;
		Ok(())
	}

	/// Undo this peer's latest transaction while it is still hot: its ops leave the hot log here, and on every
	/// peer the retraction reaches, and never retire. `None` means the step already retired and undo has to
	/// move the cursor. What the ops named lands in the remote changes, so the mirror picks up what others
	/// wrote there meanwhile.
	pub fn retract_transaction(&mut self) -> Result<Option<Vec<RegistryDelta>>, Error> {
		let Some(document_graph_storage::Retraction { ids, touched, ops }) = self.session.retract_transaction()? else {
			return Ok(None);
		};
		self.own_last_staged_ms = None;
		#[cfg(feature = "network")]
		self.remote_changes.touched.extend(touched);
		#[cfg(not(feature = "network"))]
		let _ = touched;
		self.rewrite_hot_log()?;
		self.persist_registry_snapshot()?;
		self.persist_session_state()?;
		#[cfg(feature = "network")]
		if let Some(replica) = &mut self.network {
			replica.broadcast_retraction(&ids)?;
		}
		#[cfg(not(feature = "network"))]
		let _ = ids;
		Ok(Some(ops))
	}

	/// Undo this peer's latest retired step, the one [`Session::latest_own_interaction`] names, through
	/// [`drop_step`](Self::drop_step). `None` when this peer has no retired step of its own on the line.
	#[cfg(feature = "network")]
	pub fn undo_retired_step(&mut self) -> Result<Option<Rev>, Error> {
		let Some(rev) = self.session.latest_own_interaction() else {
			return Ok(None);
		};
		self.drop_step(rev)?;
		Ok(Some(rev))
	}

	/// Drop a retired interaction out of the line, later steps minted again on its parent; see
	/// [`Session::drop_interaction`]. The retirer does it, records what it named in the remote changes and
	/// tells the room; anyone else asks the retirer and sees the head move come back through the poll.
	#[cfg(feature = "network")]
	pub fn drop_step(&mut self, rev: Rev) -> Result<(), Error> {
		if self.retires_locally() {
			let (moved, touched) = self.session.drop_interaction(rev)?;
			self.remote_changes.touched.extend(touched);
			if let Some(head) = self.session.head_rev() {
				self.session.publish_up_to(head);
			}
			self.rewrite_history()?;
			self.rewrite_hot_log()?;
			self.persist_registry_snapshot()?;
			self.persist_session_state()?;
			if let Some(replica) = &mut self.network {
				replica.broadcast_head_move(moved)?;
			}
		} else if let Some(replica) = &mut self.network {
			replica.request_undo(rev, false)?;
		}
		Ok(())
	}

	/// Move the head to an ancestor in a session, everything since staying as a branch; see
	/// [`Session::move_head_to`]. The retirer does it and tells the room; anyone else asks the retirer.
	#[cfg(feature = "network")]
	pub fn move_head(&mut self, rev: Rev) -> Result<(), Error> {
		if self.retires_locally() {
			let (moved, touched) = self.session.move_head_to(rev)?;
			self.remote_changes.touched.extend(touched);
			if let Some(head) = self.session.head_rev() {
				self.session.publish_up_to(head);
			}
			self.persist_registry_snapshot()?;
			self.persist_session_state()?;
			if let Some(replica) = &mut self.network {
				replica.broadcast_head_move(moved)?;
			}
		} else if let Some(replica) = &mut self.network {
			replica.request_move(rev)?;
		}
		Ok(())
	}

	/// Redo a step [`undo_retired_step`](Self::undo_retired_step) dropped, as a copy on top of the line.
	/// The retirer does it; anyone else asks the retirer.
	#[cfg(feature = "network")]
	pub fn redo_retired_step(&mut self, rev: Rev) -> Result<(), Error> {
		if self.retires_locally() {
			let revs = self.session.restore_interaction(rev)?;
			let deltas: Vec<Delta> = revs.iter().filter_map(|&rev| self.session.delta(rev).cloned()).collect();
			for delta in &deltas {
				self.remote_changes.touched.record(&delta.kind);
			}
			if let Some(&last) = revs.last() {
				self.session.publish_up_to(last);
			}
			self.rewrite_history()?;
			self.rewrite_hot_log()?;
			self.persist_registry_snapshot()?;
			self.persist_session_state()?;
			if let Some(replica) = &mut self.network {
				replica.broadcast_retired(&deltas, &[])?;
			}
		} else if let Some(replica) = &mut self.network {
			replica.request_undo(rev, true)?;
		}
		Ok(())
	}

	/// Stage ops a retraction took back as a fresh transaction, recording them as changes for the mirror:
	/// redoing a retracted step is a new edit to everyone, this peer included.
	pub fn restage_ops(&mut self, ops: Vec<RegistryDelta>) -> Result<(), Error> {
		#[cfg(feature = "network")]
		for op in &ops {
			self.remote_changes.touched.record(op);
		}
		self.stage_ops(ops)?;
		Ok(())
	}

	/// Retire every transaction the policy says is due at `now_ms`, the wall clock in milliseconds that retirements
	/// also record: closed transactions that have waited [`RETIRE_MIN_AGE_MS`](Self::RETIRE_MIN_AGE_MS), once
	/// [`RETIRE_AFTER_TRANSACTIONS`](Self::RETIRE_AFTER_TRANSACTIONS) of them are waiting or the oldest has waited
	/// [`RETIRE_MAX_AGE_MS`](Self::RETIRE_MAX_AGE_MS). Open transactions never retire. The editor calls this every
	/// frame with whether it is between steps; a peer that does not retire only closes its own quiet transaction.
	pub fn retire_due(&mut self, now_ms: f64, idle: bool) -> Result<Vec<Rev>, Error> {
		// The editor passes the wall clock, which retirements record as when a step happened.
		self.wall_clock_ms = Some(now_ms);
		// A transaction the editor never closed, or one cut off by a reopen, closes once quiet, but only between
		// steps: a pause mid-gesture must not split what undo takes back as one.
		if self.own_staged_since_tick {
			self.own_staged_since_tick = false;
			self.own_last_staged_ms = Some(now_ms);
		} else if idle && self.own_last_staged_ms.is_some_and(|last| now_ms - last >= Self::RETIRE_MIN_AGE_MS) {
			self.end_transaction()?;
			self.own_last_staged_ms = None;
		}
		if !self.retires_locally() {
			return Ok(Vec::new());
		}

		let closed = self.session.closed_transactions();
		let mut seen = std::mem::take(&mut self.transactions_seen);
		let mut due = Vec::new();
		let mut oldest_age: f64 = 0.0;
		for transaction in &closed {
			let Some(&marker) = transaction.ops.last() else { continue };
			let age = now_ms - *seen.entry(marker).or_insert(now_ms);
			if age >= Self::RETIRE_MIN_AGE_MS && (transaction.contiguous || age >= Self::RETIRE_MAX_AGE_MS) {
				due.push(transaction.clone());
				oldest_age = oldest_age.max(age);
			}
		}
		seen.retain(|marker, _| closed.iter().any(|transaction| transaction.ops.last() == Some(marker)));
		self.transactions_seen = seen;

		if due.len() >= Self::RETIRE_AFTER_TRANSACTIONS || oldest_age >= Self::RETIRE_MAX_AGE_MS {
			self.retire_transactions(&due)
		} else {
			Ok(Vec::new())
		}
	}

	/// Close this peer's open transaction and retire every closed one, whoever authored it. The undo path calls
	/// this first so the interaction being undone is in history; a peer that does not retire only closes its own.
	pub fn retire_pending_interaction(&mut self) -> Result<Vec<Rev>, Error> {
		self.end_transaction()?;
		self.own_last_staged_ms = None;
		let closed = self.session.closed_transactions();
		self.retire_transactions(&closed)
	}

	/// Retire the given closed transactions, each as one interaction, in one history append and one
	/// broadcast. A no-op for a peer that leaves retirement to the session host.
	pub(crate) fn retire_transactions(&mut self, transactions: &[document_graph_storage::ClosedTransaction]) -> Result<Vec<Rev>, Error> {
		if !self.retires_locally() || transactions.is_empty() {
			return Ok(Vec::new());
		}
		let mut revs = Vec::new();
		let mut retired_hot_ops = Vec::new();
		for transaction in transactions {
			revs.extend(self.session.retire_transaction(transaction)?);
			retired_hot_ops.extend(transaction.ops.iter().copied());
		}
		self.finish_retirement(&revs, &retired_hot_ops)?;
		Ok(revs)
	}

	/// Working-copy checkpoint: retire every hot op stamped at or before `up_to`, whatever its author or
	/// transaction state. For callers that own the whole log; a session uses [`retire_transactions`](Self::retire_transactions).
	pub fn retire(&mut self, up_to: TimeStamp) -> Result<Vec<Rev>, Error> {
		if !self.retires_locally() {
			return Ok(Vec::new());
		}
		let retired_hot_ops = self.session.hot_ops_up_to(up_to);
		let revs = self.session.retire(up_to)?;
		self.finish_retirement(&revs, &retired_hot_ops)?;
		Ok(revs)
	}

	/// What every retirement ends with: the published frontier moves, the deltas reach the history file,
	/// the hot log and snapshot are rewritten, and the room hears about it.
	fn finish_retirement(&mut self, new_revs: &[Rev], retired_hot_ops: &[document_graph_storage::HotOpId]) -> Result<(), Error> {
		// Stamped before the frames are written, so the time is on disk and in the broadcast.
		if let Some(wall_ms) = self.wall_clock_ms
			&& !new_revs.is_empty()
		{
			self.session.stamp_retired_at(new_revs, wall_ms as u64);
		}
		// These deltas are about to reach peers, so the published frontier moves with them: rewinding a commit
		// peers hold would silently diverge, so undo past it moves the shared head instead. Set before the
		// persists below so it survives a reopen.
		#[cfg(feature = "network")]
		if self.network.is_some()
			&& let Some(&last) = new_revs.last()
		{
			self.session.publish_up_to(last);
		}

		if !new_revs.is_empty() {
			self.append_history_deltas(new_revs)?;
		}

		self.rewrite_hot_log()?;
		self.persist_registry_snapshot()?;
		self.persist_session_state()?;

		#[cfg(feature = "network")]
		if let Some(replica) = &mut self.network {
			let deltas: Vec<_> = new_revs.iter().filter_map(|&rev| self.session.delta(rev).cloned()).collect();
			replica.broadcast_retired(&deltas, retired_hot_ops)?;
		}
		#[cfg(not(feature = "network"))]
		let _ = retired_hot_ops;

		Ok(())
	}
	/// Guests leave retirement to the host and keep their hot ops until its retired deltas arrive.
	fn retires_locally(&self) -> bool {
		#[cfg(feature = "network")]
		{
			self.network.as_ref().is_none_or(|replica| replica.role() == peer_transport::Role::Host)
		}
		#[cfg(not(feature = "network"))]
		true
	}

	pub(crate) fn rewrite_hot_log(&mut self) -> Result<(), Error> {
		let mut buffer = Vec::new();
		for hot_op in self.session.hot_log() {
			self.manifest.codecs.hot_log.append(&mut buffer, hot_op)?;
		}
		self.working.write_non_blocking(&io::path_for(self.layout.hot_log_basename(), self.manifest.codecs.hot_log), &buffer)?;
		Ok(())
	}
}
