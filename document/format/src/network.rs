//! Live collaboration on the [`Gdd`] handle. `share` and `connect` attach a [`Replica`], after which the persist path
//! broadcasts staged hot ops, and retired deltas when hosting. `poll_peers` applies what peers send and persists it
//! like local edits.

use std::collections::HashSet;

use document_graph_storage::{CrdtError, Delta, HeadMove, HistoryMetadata, HotOp, HotOpId, PeerId, Registry, RegistryDelta, ResourceHash, RetiredMarks, Rev, Session, Touched, UserId};
use peer_transport::{Event, Replica, Role, SyncTarget, TargetError, Transport};

use crate::error::Error;
use crate::layout::Layout;
use crate::{Gdd, PendingPersist};

/// What peers changed in the working registry since it was last taken, so a runtime mirror can reconcile
/// just the touched entities.
#[derive(Clone, Debug, Default)]
pub struct RemoteChanges {
	pub touched: Touched,
	/// A full sync replaced the working registry, so the touched set no longer bounds what changed and the mirror
	/// must be rebuilt.
	pub rebuilt: bool,
}

impl RemoteChanges {
	pub fn is_empty(&self) -> bool {
		self.touched.is_empty() && !self.rebuilt
	}

	pub fn extend(&mut self, other: RemoteChanges) {
		self.touched.extend(other.touched);
		self.rebuilt |= other.rebuilt;
	}
}

impl<L: Layout> Gdd<L> {
	pub fn take_remote_changes(&mut self) -> RemoteChanges {
		std::mem::take(&mut self.remote_changes)
	}

	/// Restore a dropped interaction and publish it, recording what it named in the remote changes. Returns the new deltas.
	pub(crate) fn restore_dropped_interaction(&mut self, rev: Rev) -> Result<Vec<Delta>, CrdtError> {
		let revs = self.session.restore_interaction(rev)?;
		let deltas: Vec<Delta> = revs.iter().filter_map(|&rev| self.session.delta(rev).cloned()).collect();
		for delta in &deltas {
			self.remote_changes.touched.record(&delta.kind);
		}
		if let Some(&last) = revs.last() {
			self.session.publish_up_to(last);
		}
		Ok(deltas)
	}

	/// Host this document's room, and remember it is shared so a reopen reconnects.
	pub fn share(&mut self, transport: impl Transport + 'static, user: UserId) -> Result<(), Error> {
		self.network = Some(Replica::host(transport, self.session.peer(), user));
		self.shared = true;
		self.persist_session_state()
	}

	/// Connect to this document's room as host or guest, and remember it is shared so a reopen reconnects.
	pub fn connect(&mut self, transport: impl Transport + 'static, user: UserId) -> Result<(), Error> {
		self.network = Some(Replica::connect(transport, self.session.peer(), user));
		self.shared = true;
		self.persist_session_state()
	}

	/// Whether the document is meant to be in its room.
	pub fn is_shared(&self) -> bool {
		self.shared
	}

	/// Take the host role if none is present once the grace period ends. See [`Replica::decide_role`].
	pub fn decide_role(&mut self) -> Option<Role> {
		self.network.as_mut().and_then(Replica::decide_role)
	}

	/// Leave the room for good, so a reopen does not reconnect. See [`disconnect`](Self::disconnect).
	pub fn leave(&mut self) {
		self.shared = false;
		if let Err(error) = self.persist_session_state() {
			log::error!("Persisting the session state before leaving failed: {error}");
		}
		self.disconnect();
	}

	/// Drop the connection but keep the document shared, so a reopen reconnects. A host first retires every closed
	/// interaction with no ops still on their way.
	pub fn disconnect(&mut self) {
		let closed = self.contiguous_interactions();
		if let Err(error) = self.retire_interactions(&closed) {
			log::error!("Retiring before leaving failed: {error}");
		}
		if let Some(mut replica) = self.network.take() {
			replica.leave();
		}
	}

	pub fn role(&self) -> Option<Role> {
		self.network.as_ref().map(Replica::role)
	}

	pub fn is_synced(&self) -> bool {
		self.network.as_ref().is_none_or(Replica::is_synced)
	}

	/// Announce this peer's display name to the room. A no-op when it has not changed or nobody is connected.
	pub fn set_name(&mut self, name: &str) -> Result<(), Error> {
		if let Some(replica) = &mut self.network {
			replica.set_name(name)?;
		}
		Ok(())
	}

	/// Send this peer's pointer position, `None` when it left the viewport.
	pub fn send_cursor(&mut self, position: Option<peer_transport::CursorPosition>) -> Result<(), Error> {
		if let Some(replica) = &mut self.network {
			replica.send_cursor(position)?;
		}
		Ok(())
	}

	/// The other peers in the room. Empty when the document is not connected.
	pub fn peers(&self) -> impl Iterator<Item = &peer_transport::RemotePeer> {
		self.network.iter().flat_map(|replica| replica.peers())
	}

	pub fn poll_peers(&mut self) -> Vec<Event> {
		let Some(mut replica) = self.network.take() else { return Vec::new() };
		let events = replica.poll(self);
		self.network = Some(replica);
		events
	}

	/// Answer an [`Event::ResourceRequested`].
	pub fn send_resource(&mut self, to: peer_transport::TransportPeerId, hash: ResourceHash, bytes: Vec<u8>) -> Result<(), Error> {
		if let Some(replica) = &mut self.network {
			replica.send_resource(to, hash, bytes)?;
		}
		Ok(())
	}
}

impl<L: Layout> Gdd<L> {
	/// Append deltas already merged in memory, leaving the file for `flush` to rewrite if that fails.
	fn append_history_or_rewrite(&mut self, revs: &[Rev]) {
		if let Err(error) = self.append_history_deltas(revs) {
			log::error!("Appending to the history failed: {error}");
			self.pending_persist.history = true;
		}
	}
}

impl<L: Layout> SyncTarget for Gdd<L> {
	fn peer(&self) -> PeerId {
		self.session.peer()
	}

	fn document_id(&self) -> Option<u64> {
		Some(self.manifest.document_id)
	}

	fn adopt_document_id(&mut self, document_id: u64) -> Result<(), TargetError> {
		self.update_manifest(|manifest| manifest.document_id = document_id)?;
		Ok(())
	}

	fn head(&self) -> Option<Rev> {
		self.session.head_rev()
	}

	fn retired_registry(&self) -> Registry {
		self.session.retired_registry().clone()
	}

	fn hot_log(&self) -> Vec<HotOp> {
		self.session.hot_log().to_vec()
	}

	fn known_revs(&self) -> Vec<Rev> {
		self.session.known_revs()
	}

	fn contains_rev(&self, rev: Rev) -> bool {
		self.session.delta(rev).is_some()
	}

	fn deltas_unknown_to(&mut self, known: &[Rev]) -> Vec<Delta> {
		let deltas = <Session as SyncTarget>::deltas_unknown_to(&mut self.session, known);
		// The published frontier moved, and it persists with the session state.
		self.pending_persist.snapshot |= !deltas.is_empty();
		deltas
	}

	fn load(&mut self, registry: Registry, history: Vec<Delta>, head: Option<Rev>) -> Result<(), TargetError> {
		self.session.load(registry, history, head)?;
		self.remote_changes.rebuilt = true;
		self.pending_persist = PendingPersist {
			history: true,
			hot_log: true,
			snapshot: true,
		};
		Ok(())
	}

	fn apply_remote_hot_ops(&mut self, ops: Vec<HotOp>) -> Result<Vec<HotOp>, TargetError> {
		// An op naming an entity not yet here is handed back for retry. Only applied ops reach the hot frame log.
		let mut deferred = Vec::new();
		for hot_op in ops {
			// Recorded even if the apply fails, as it may still have revived what it references.
			self.remote_changes.touched.record(&hot_op.op);
			if self.session.replay_hot_op(hot_op.clone()).is_err() {
				deferred.push(hot_op);
				continue;
			}
			// The op applied, so a frame that failed to write is left for `flush` to rewrite.
			if let Err(error) = self.append_hot_frame(&hot_op) {
				log::error!("Appending a hot op failed: {error}");
				self.pending_persist.hot_log = true;
			}
		}

		Ok(deferred)
	}

	fn merge_remote(&mut self, deltas: Vec<Delta>, retires: &[HotOpId], head: Option<Rev>) -> Result<(), TargetError> {
		let incoming: HashSet<Rev> = deltas.iter().map(|delta| delta.id).collect();
		let length_before = self.session.history_len();
		for delta in &deltas {
			self.remote_changes.touched.record(&delta.kind);
		}
		SyncTarget::merge_remote(&mut self.session, deltas, retires, head)?;

		// What a peer sent is shared, so undo must not rewind it silently.
		if let Some(head) = self.session.head_rev() {
			self.session.publish_up_to(head);
		}

		// A batch that only extended history, plus any merge delta joining it, is appended to the file. One that sorted
		// earlier deltas after it rewrites the file.
		let tail: Vec<Rev> = self.session.history().skip(length_before).map(|delta| delta.id).collect();
		let appended = tail
			.iter()
			.all(|&rev| incoming.contains(&rev) || self.session.delta(rev).is_some_and(|delta| matches!(delta.kind, RegistryDelta::Merge { .. })));
		if appended {
			self.append_history_or_rewrite(&tail);
		} else {
			self.pending_persist.history = true;
		}
		self.pending_persist.hot_log |= !retires.is_empty();
		self.pending_persist.snapshot = true;
		Ok(())
	}

	fn merge_divergent(&mut self, deltas: Vec<Delta>, retires: &[HotOpId]) -> Result<Vec<Delta>, TargetError> {
		let length_before = self.session.history_len();
		for delta in &deltas {
			self.remote_changes.touched.record(&delta.kind);
		}
		let absorbed = SyncTarget::merge_divergent(&mut self.session, deltas, retires)?;
		if let Some(head) = self.session.head_rev() {
			self.session.publish_up_to(head);
		}

		// Appended when the tail is just the absorbed deltas and their merge.
		let absorbed_ids: HashSet<Rev> = absorbed.iter().map(|delta| delta.id).collect();
		let tail: Vec<Rev> = self.session.history().skip(length_before).map(|delta| delta.id).collect();
		if tail.iter().all(|rev| absorbed_ids.contains(rev)) {
			self.append_history_or_rewrite(&tail);
		} else {
			self.pending_persist.history = true;
		}
		self.pending_persist.hot_log |= !retires.is_empty();
		self.pending_persist.snapshot = true;
		Ok(absorbed)
	}

	fn retired_marks(&self) -> RetiredMarks {
		SyncTarget::retired_marks(&self.session)
	}

	fn absorb_retired_marks(&mut self, remote: &RetiredMarks) -> Result<(), TargetError> {
		let before = self.session.retired_marks().clone();
		let touched = self.session.absorb_retired_marks(remote);
		if !touched.is_empty() {
			self.remote_changes.touched.extend(touched);
			self.pending_persist.hot_log = true;
		}
		// Also writes the session state, which carries the marks. They count even when they match no op here, so a late
		// copy stays dropped after a reopen.
		self.pending_persist.snapshot |= *self.session.retired_marks() != before;
		Ok(())
	}

	fn metadata(&self) -> HistoryMetadata {
		self.metadata.clone()
	}

	fn absorb_metadata(&mut self, remote: &HistoryMetadata) -> Result<bool, TargetError> {
		// Kept only once written, or a failed write would make the facts look known when they come round again.
		let mut merged = self.metadata.clone();
		let landed = merged.merge(remote);
		if landed.is_empty() {
			return Ok(false);
		}
		self.append_facts(&landed)?;
		self.metadata = merged;
		Ok(true)
	}

	fn retract_hot_ops(&mut self, ops: &[HotOpId]) -> Result<(), TargetError> {
		let touched = self.session.retract_hot_ops(ops);
		self.remote_changes.touched.extend(touched);
		self.pending_persist.hot_log = true;
		self.pending_persist.snapshot = true;
		Ok(())
	}

	fn drop_interaction(&mut self, rev: Rev) -> Result<HeadMove, TargetError> {
		let (moved, touched) = self.session.drop_interaction(rev)?;
		self.remote_changes.touched.extend(touched);
		self.pending_persist.history = true;
		self.pending_persist.hot_log = true;
		self.pending_persist.snapshot = true;
		Ok(moved)
	}

	fn restore_interaction(&mut self, rev: Rev) -> Result<Vec<Delta>, TargetError> {
		let deltas = self.restore_dropped_interaction(rev)?;
		self.pending_persist.history = true;
		self.pending_persist.snapshot = true;
		Ok(deltas)
	}

	fn apply_head_move(&mut self, moved: &HeadMove) -> Result<(), TargetError> {
		let touched = self.session.apply_head_move(moved)?;
		self.remote_changes.touched.extend(touched);
		self.pending_persist.history = true;
		self.pending_persist.hot_log = true;
		self.pending_persist.snapshot = true;
		Ok(())
	}

	fn move_head_to(&mut self, rev: Rev) -> Result<HeadMove, TargetError> {
		let (moved, touched) = self.session.move_head_to(rev)?;
		self.remote_changes.touched.extend(touched);
		self.pending_persist.snapshot = true;
		Ok(moved)
	}

	fn flush(&mut self) -> Result<(), TargetError> {
		// Each flag clears only once its write succeeds, so a failed write is retried on the next flush.
		if self.pending_persist.history {
			self.rewrite_history()?;
			self.pending_persist.history = false;
		}
		if self.pending_persist.hot_log {
			self.rewrite_hot_log()?;
			self.pending_persist.hot_log = false;
		}
		if self.pending_persist.snapshot {
			self.persist_registry_snapshot()?;
			self.persist_session_state()?;
			self.pending_persist.snapshot = false;
		}
		Ok(())
	}

	fn missing_resources(&self) -> HashSet<ResourceHash> {
		self.unstored_resources().into_iter().collect()
	}

	fn store_resource(&mut self, _hash: ResourceHash, bytes: &[u8]) -> Result<(), TargetError> {
		self.hold_resource(bytes)?;
		Ok(())
	}
}
