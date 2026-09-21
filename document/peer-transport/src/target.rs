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
	fn apply_remote_hot_ops(&mut self, ops: Vec<HotOp>) -> Result<(), TargetError>;
	fn merge_remote(&mut self, deltas: Vec<Delta>, retires_up_to: Option<TimeStamp>) -> Result<(), TargetError>;

	/// Referenced resources whose bytes are not available locally.
	fn missing_resources(&self) -> HashSet<ResourceHash> {
		HashSet::new()
	}
	/// `None` when the bytes can't be produced synchronously; the request is then surfaced as an event.
	fn resource_bytes(&self, hash: ResourceHash) -> Option<Vec<u8>> {
		let _ = hash;
		None
	}
	fn store_resource(&mut self, hash: ResourceHash, bytes: Vec<u8>) -> Result<(), TargetError> {
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
			self.apply_hot_op(hot_op)?;
		}
		Ok(())
	}

	fn merge_remote(&mut self, deltas: Vec<Delta>, retires_up_to: Option<TimeStamp>) -> Result<(), TargetError> {
		if let Some(up_to) = retires_up_to {
			self.discard_hot_ops(up_to);
		}
		self.merge(deltas)?;
		Ok(())
	}
}
