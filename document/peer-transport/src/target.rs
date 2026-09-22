use document_graph_storage::{Delta, HotOp, PeerId, Registry, ResourceHash, Rev, Session, TimeStamp};
use std::collections::HashSet;

pub type TargetError = Box<dyn std::error::Error>;

/// The document state a `Replica` reads from and applies remote changes to.
pub trait SyncTarget {
	fn peer(&self) -> PeerId;
	fn head(&self) -> Option<Rev>;
	fn retired_registry(&self) -> Registry;
	fn hot_log(&self) -> Vec<HotOp>;
	fn known_revs(&self) -> Vec<Rev>;
	fn contains_rev(&self, rev: Rev) -> bool;
	fn deltas_unknown_to(&self, known: &[Rev]) -> Vec<Delta>;

	/// Replace all state with the given retired state.
	fn load(&mut self, registry: Registry, history: Vec<Delta>, head: Option<Rev>) -> Result<(), TargetError>;
	/// Must be idempotent on structural ops: a buffered op may already be reflected by the sync.
	fn apply_remote_hot_ops(&mut self, ops: Vec<HotOp>) -> Result<(), TargetError>;
	fn merge_remote(&mut self, deltas: Vec<Delta>, retires: &[TimeStamp]) -> Result<(), TargetError>;
	/// Make everything applied since the last flush durable. Called once per [`Replica::poll`](crate::Replica::poll),
	/// so a target that rewrites whole files can do it once for a batch rather than once per packet.
	fn flush(&mut self) -> Result<(), TargetError> {
		Ok(())
	}

	/// Referenced resources whose bytes are not available locally.
	fn missing_resources(&self) -> HashSet<ResourceHash> {
		HashSet::new()
	}
	/// `None` when the bytes can't be produced synchronously; the request is then surfaced as an event.
	fn resource_bytes(&self, hash: ResourceHash) -> Option<Vec<u8>> {
		let _ = hash;
		None
	}
	fn store_resource(&mut self, hash: ResourceHash, bytes: &[u8]) -> Result<(), TargetError> {
		let _ = (hash, bytes);
		Ok(())
	}
}

impl SyncTarget for Session {
	fn peer(&self) -> PeerId {
		Session::peer(self)
	}

	fn head(&self) -> Option<Rev> {
		self.head_rev()
	}

	fn retired_registry(&self) -> Registry {
		Session::retired_registry(self).clone()
	}

	fn hot_log(&self) -> Vec<HotOp> {
		Session::hot_log(self).to_vec()
	}

	fn known_revs(&self) -> Vec<Rev> {
		Session::known_revs(self)
	}

	fn contains_rev(&self, rev: Rev) -> bool {
		self.delta(rev).is_some()
	}

	fn deltas_unknown_to(&self, known: &[Rev]) -> Vec<Delta> {
		Session::deltas_unknown_to(self, known.iter().copied()).into_iter().cloned().collect()
	}

	fn load(&mut self, registry: Registry, history: Vec<Delta>, head: Option<Rev>) -> Result<(), TargetError> {
		*self = Session::load(self.peer(), registry, history, head, Vec::new(), self.next_node_counter());
		Ok(())
	}

	fn apply_remote_hot_ops(&mut self, ops: Vec<HotOp>) -> Result<(), TargetError> {
		for hot_op in ops {
			self.replay_hot_op(hot_op)?;
		}
		Ok(())
	}

	fn merge_remote(&mut self, deltas: Vec<Delta>, retires: &[TimeStamp]) -> Result<(), TargetError> {
		// Hot ops stay until after the merge: a delta may target something a still-hot removal took away.
		self.merge(deltas)?;
		self.discard_hot_ops(retires);
		Ok(())
	}
}
