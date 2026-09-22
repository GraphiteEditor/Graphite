//! Live collaboration on the [`Gdd`] handle. `share` / `join` attach a [`Replica`]; the persist path
//! then broadcasts staged hot ops and (as host) retired deltas, and `poll_peers` applies what peers
//! send through the [`SyncTarget`] impl below, which persists inbound state the same way local
//! edits are.

use std::collections::HashSet;

use document_graph_storage::{Delta, HotOp, PeerId, Registry, ResourceHash, Rev, Session, TimeStamp, UserId};
use peer_transport::{Event, Replica, Role, SyncTarget, TargetError, Transport};

use crate::error::Error;
use crate::layout::Layout;
use crate::{Gdd, PendingPersist};

impl<L: Layout> Gdd<L> {
	pub fn share(&mut self, transport: impl Transport + 'static, user: UserId) {
		self.network = Some(Replica::host(transport, self.session.peer(), user));
	}

	pub fn join(&mut self, transport: impl Transport + 'static, user: UserId) {
		self.network = Some(Replica::guest(transport, self.session.peer(), user));
	}

	pub fn leave(&mut self) {
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
		self.pending_persist = PendingPersist {
			history: true,
			hot_log: true,
			snapshot: true,
		};
		Ok(())
	}

	fn apply_remote_hot_ops(&mut self, ops: Vec<HotOp>) -> Result<(), TargetError> {
		for hot_op in ops {
			self.session.replay_hot_op(hot_op.clone())?;
			self.append_hot_frame(&hot_op)?;
		}
		Ok(())
	}

	fn merge_remote(&mut self, deltas: Vec<Delta>, retires: &[TimeStamp]) -> Result<(), TargetError> {
		self.session.merge_remote(deltas, retires)?;
		self.pending_persist.history = true;
		self.pending_persist.hot_log |= !retires.is_empty();
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
