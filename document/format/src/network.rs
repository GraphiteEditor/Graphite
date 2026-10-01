//! Live collaboration on the [`Gdd`] handle. `share` / `join` attach a [`Replica`]; the persist path
//! then broadcasts staged hot ops, and retired deltas when hosting. `poll_peers` applies what peers
//! send through the [`SyncTarget`] impl below, which persists it like local edits.

use std::collections::HashSet;

use document_graph_storage::{Delta, HeadMove, HistoryMetadata, HotOp, HotOpId, PeerId, Registry, RegistryDelta, ResourceHash, Rev, Session, SettledMarks, Touched, UserId};
use peer_transport::{Event, Replica, Role, SyncTarget, TargetError, Transport};

use crate::error::Error;
use crate::layout::Layout;
use crate::{Gdd, PendingPersist};

/// What peers changed in the working registry since the editor last asked, so the editor can reconcile
/// just the touched entities of its runtime mirror.
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
	/// Takes what peers changed since the last call. Empty when nothing did.
	pub fn take_remote_changes(&mut self) -> RemoteChanges {
		std::mem::take(&mut self.remote_changes)
	}
}

impl<L: Layout> Gdd<L> {
	pub fn share(&mut self, transport: impl Transport + 'static, user: UserId) {
		self.network = Some(Replica::host(transport, self.session.peer(), user));
	}

	/// Connect to this document's room as host or guest, and remember it is shared so a reopen reconnects.
	pub fn connect(&mut self, transport: impl Transport + 'static, user: UserId) -> Result<(), Error> {
		self.network = Some(Replica::connect(transport, self.session.peer(), user));
		self.shared = true;
		self.persist_session_state()
	}

	/// Whether the document is meant to be in its room; see [`connect`](Self::connect).
	pub fn is_shared(&self) -> bool {
		self.shared
	}

	/// Take the host role if none is present once the grace period ends. See [`Replica::decide_role`].
	pub fn decide_role(&mut self) -> Option<Role> {
		self.network.as_mut().and_then(Replica::decide_role)
	}

	pub fn join(&mut self, transport: impl Transport + 'static, user: UserId) {
		self.network = Some(Replica::guest(transport, self.session.peer(), user));
	}

	/// Leaves the room. A host first retires every closed transaction, so finished work reaches history before
	/// the retirer leaves.
	pub fn leave(&mut self) {
		self.shared = false;
		if let Err(error) = self.persist_session_state() {
			log::error!("Persisting the session state before leaving failed: {error}");
		}
		self.disconnect();
	}

	/// Drops the connection but keeps the document shared, for a transport that went down, so a reopen reconnects.
	pub fn disconnect(&mut self) {
		let closed = self.session.closed_transactions();
		if let Err(error) = self.retire_transactions(&closed) {
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

	/// Announce this peer's display name to the room; a no-op when it has not changed or nobody is connected.
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

	/// The other peers in the room; empty when the document is not connected.
	pub fn peers(&self) -> Vec<peer_transport::RemotePeer> {
		self.network.as_ref().map(|replica| replica.peers().cloned().collect()).unwrap_or_default()
	}

	/// Record that the runtime was rebuilt from the registry, so the next staged diff is taken against it.
	pub fn mark_runtime_current(&mut self) {
		self.session.mark_runtime_current();
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
		// The published frontier moved; it persists with the session state.
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
		// An op naming an entity not yet here is handed back for retry; only applied ops reach the hot frame log.
		let mut deferred = Vec::new();
		for hot_op in ops {
			// Recorded even if the apply fails: it may still have revived what it references.
			self.remote_changes.touched.record(&hot_op.op);
			if self.session.replay_hot_op(hot_op.clone()).is_err() {
				deferred.push(hot_op);
				continue;
			}
			self.append_hot_frame(&hot_op)?;
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

		// Whatever a peer sent is already shared, so it sits behind the published frontier and cannot be silently rewound.
		if let Some(head) = self.session.head_rev() {
			self.session.publish_up_to(head);
		}

		// A batch that only extended history, plus any merge delta joining it, is appended to the file; one that
		// sorted earlier deltas after it rewrites the file.
		let tail: Vec<Rev> = self.session.history().skip(length_before).map(|delta| delta.id).collect();
		let appended = tail
			.iter()
			.all(|&rev| incoming.contains(&rev) || self.session.delta(rev).is_some_and(|delta| matches!(delta.kind, RegistryDelta::Merge { .. })));
		match appended {
			true => self.append_history_deltas(&tail)?,
			false => self.pending_persist.history = true,
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

		// Appended when the tail is just the absorbed deltas and their merge; see `merge_remote`.
		let absorbed_ids: HashSet<Rev> = absorbed.iter().map(|delta| delta.id).collect();
		let tail: Vec<Rev> = self.session.history().skip(length_before).map(|delta| delta.id).collect();
		match tail.iter().all(|rev| absorbed_ids.contains(rev)) {
			true => self.append_history_deltas(&tail)?,
			false => self.pending_persist.history = true,
		}
		self.pending_persist.hot_log |= !retires.is_empty();
		self.pending_persist.snapshot = true;
		Ok(absorbed)
	}

	fn settled_marks(&self) -> SettledMarks {
		SyncTarget::settled_marks(&self.session)
	}

	fn absorb_settled_marks(&mut self, remote: &SettledMarks) -> Result<(), TargetError> {
		let touched = self.session.absorb_settled_marks(remote);
		if !touched.is_empty() {
			self.remote_changes.touched.extend(touched);
			self.pending_persist.hot_log = true;
			// Also writes the session state, which carries the marks.
			self.pending_persist.snapshot = true;
		}
		Ok(())
	}

	fn metadata(&self) -> HistoryMetadata {
		self.metadata.clone()
	}

	fn absorb_metadata(&mut self, remote: &HistoryMetadata) -> Result<bool, TargetError> {
		let landed = self.metadata.merge(remote);
		if landed.is_empty() {
			return Ok(false);
		}
		self.append_facts(&landed)?;
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
		let revs = self.session.restore_interaction(rev)?;
		let deltas: Vec<Delta> = revs.iter().filter_map(|&rev| self.session.delta(rev).cloned()).collect();
		for delta in &deltas {
			self.remote_changes.touched.record(&delta.kind);
		}
		if let Some(&last) = revs.last() {
			self.session.publish_up_to(last);
		}
		self.pending_persist.history = true;
		self.pending_persist.snapshot = true;
		Ok(deltas)
	}

	fn apply_head_move(&mut self, moved: &HeadMove) -> Result<(), TargetError> {
		let touched = self.session.apply_head_move(moved)?;
		self.remote_changes.touched.extend(touched);
		if let Some(head) = self.session.head_rev() {
			self.session.publish_up_to(head);
		}
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
		let pending = std::mem::take(&mut self.pending_persist);

		if pending.history {
			self.rewrite_history()?;
		}
		if pending.hot_log {
			self.rewrite_hot_log()?;
		}
		if pending.snapshot {
			self.persist_registry_snapshot()?;
			self.persist_session_state()?;
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
