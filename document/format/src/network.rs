//! Live collaboration on the [`Gdd`] handle. `share` / `join` attach a [`Replica`]; the persist path
//! then broadcasts staged hot ops and (as host) retired deltas, and `poll_peers` applies what peers
//! send through the [`SyncTarget`] impl below, which persists inbound state the same way local
//! edits are.

use std::collections::HashSet;

use document_graph_storage::{Delta, HeadMove, HotOp, HotOpId, PeerId, Registry, RegistryDelta, ResourceHash, RetiredHotOps, Rev, Session, Touched, UserId};
use peer_transport::{Event, Replica, Role, SyncTarget, TargetError, Transport};

use crate::error::Error;
use crate::layout::Layout;
use crate::{Gdd, PendingPersist};

/// What peers changed in the working registry since the editor last asked. The editor keeps a runtime
/// mirror of the registry and brings just the touched entities back into line, unless the registry was
/// rederived wholesale, when the touched set no longer bounds what changed.
#[derive(Clone, Debug, Default)]
pub struct RemoteChanges {
	pub touched: Touched,
	/// The working registry was replaced wholesale by a full sync, so the mirror has to be rebuilt from it.
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

	/// Connect to the room every copy of this document shares, host or guest as the room calls for, and
	/// remember that the document is shared so a reopen reconnects on its own.
	pub fn connect(&mut self, transport: impl Transport + 'static, user: UserId) -> Result<(), Error> {
		self.network = Some(Replica::connect(transport, self.session.peer(), user));
		self.shared = true;
		self.persist_session_state()
	}

	/// Whether the document is meant to be in its room; see [`connect`](Self::connect).
	pub fn is_shared(&self) -> bool {
		self.shared
	}

	/// For a connection still undecided after the grace period: take the host role if no host is there. See
	/// [`Replica::decide_role`].
	pub fn decide_role(&mut self) -> Option<Role> {
		self.network.as_mut().and_then(Replica::decide_role)
	}

	pub fn join(&mut self, transport: impl Transport + 'static, user: UserId) {
		self.network = Some(Replica::guest(transport, self.session.peer(), user));
	}

	/// Leaves the room. A host retires every closed transaction first, so what peers finished is in
	/// history before the retirer goes away.
	pub fn leave(&mut self) {
		self.shared = false;
		if let Err(error) = self.persist_session_state() {
			log::error!("Persisting the session state before leaving failed: {error}");
		}
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

	/// Send this peer's pointer position in document space, `None` when it left the viewport.
	pub fn send_cursor(&mut self, position: Option<[f64; 2]>) -> Result<(), Error> {
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

	fn deltas_unknown_to(&self, known: &[Rev]) -> Vec<Delta> {
		<Session as SyncTarget>::deltas_unknown_to(&self.session, known)
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
		// An op naming an entity that has not arrived here yet is handed back for a later retry, so only
		// what actually applied reaches the hot frame log.
		let mut deferred = Vec::new();
		for hot_op in ops {
			// Recorded whether or not it applies: a failed apply can still have resurrected what it references.
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

		// Whatever a peer sent is shared by definition, so it sits behind the published frontier too: a
		// guest must no more silently rewind the host's history than the host may rewind its own.
		if let Some(head) = self.session.head_rev() {
			self.session.publish_up_to(head);
		}

		// A batch that extended canonical history at the end, with any merge delta joining it there, extends
		// the file the same way. One that sorted earlier deltas after it rewrites the file.
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

		// The absorbed deltas and the merge joining them are the batch; a sort that moved earlier deltas
		// after them rewrites the file.
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

	fn retired_marks(&self) -> RetiredHotOps {
		SyncTarget::retired_marks(&self.session)
	}

	fn absorb_retired_marks(&mut self, remote: &RetiredHotOps) -> Result<(), TargetError> {
		SyncTarget::absorb_retired_marks(&mut self.session, remote)?;
		self.pending_persist.hot_log = true;
		Ok(())
	}

	fn retracted_marks(&self) -> RetiredHotOps {
		SyncTarget::retracted_marks(&self.session)
	}

	fn absorb_retracted_marks(&mut self, remote: &RetiredHotOps) -> Result<(), TargetError> {
		let touched = self.session.absorb_retracted_marks(remote);
		if !touched.is_empty() {
			self.remote_changes.touched.extend(touched);
			self.pending_persist.hot_log = true;
			self.pending_persist.snapshot = true;
		}
		Ok(())
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
