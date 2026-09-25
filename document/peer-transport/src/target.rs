use document_graph_storage::{Delta, HeadMove, HotOp, HotOpId, PeerId, Registry, ResourceHash, RetiredHotOps, Rev, Session};
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

	/// Highest hot-op counter retired per author.
	fn retired_marks(&self) -> RetiredHotOps;
	/// Which hot ops were taken back by their authors. See [`Session::retracted_marks`].
	fn retracted_marks(&self) -> RetiredHotOps;
	/// Take on a peer's retirement marks, dropping any hot op they show as already retired.
	fn absorb_retired_marks(&mut self, remote: &RetiredHotOps) -> Result<(), TargetError>;
	/// Take on a peer's retractions, dropping what they cover from the hot log.
	fn absorb_retracted_marks(&mut self, remote: &RetiredHotOps) -> Result<(), TargetError>;

	/// Replace all state with the given retired state.
	fn load(&mut self, registry: Registry, history: Vec<Delta>, head: Option<Rev>) -> Result<(), TargetError>;
	/// Must be idempotent on structural ops: a buffered op may already be reflected by the sync.
	/// Returns the ops it could not apply, whose referents have not arrived yet; the caller retries
	/// them as later ops fill the gaps.
	fn apply_remote_hot_ops(&mut self, ops: Vec<HotOp>) -> Result<Vec<HotOp>, TargetError>;
	/// Take the host's deltas on and follow its head, `head` being where the host is after them (the
	/// batch's last delta when `None`). See [`Session::follow`].
	fn merge_remote(&mut self, deltas: Vec<Delta>, retires: &[HotOpId], head: Option<Rev>) -> Result<(), TargetError>;
	/// A peer took back hot ops of its own: drop them and re-derive what they touched.
	fn retract_hot_ops(&mut self, ops: &[HotOpId]) -> Result<(), TargetError>;
	/// Host only: drop a retired interaction out of the shared line. See [`Session::drop_interaction`].
	fn drop_interaction(&mut self, rev: Rev) -> Result<HeadMove, TargetError>;
	/// Host only: mint a dropped interaction again on top of the line. See [`Session::restore_interaction`].
	fn restore_interaction(&mut self, rev: Rev) -> Result<Vec<Delta>, TargetError>;
	/// Follow the host's cursor move. See [`Session::apply_head_move`].
	fn apply_head_move(&mut self, moved: &HeadMove) -> Result<(), TargetError>;
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
		// Kept across the replace: a sequence this peer already spent must not come round again, or an
		// op of its own would be taken for one already retired.
		let spent = self.next_hot_sequence();
		*self = Session::load(self.peer(), registry, history, head, Vec::new(), self.next_node_counter());
		self.restore_hot_sequence(spent);
		Ok(())
	}

	fn apply_remote_hot_ops(&mut self, ops: Vec<HotOp>) -> Result<Vec<HotOp>, TargetError> {
		// Every op gets its turn even if one fails, since the broadcast that carried them is consumed by
		// the time this runs. A failure usually means the entity the op names has not arrived here yet,
		// so it is handed back to be retried rather than dropped.
		let mut deferred = Vec::new();
		for hot_op in ops {
			if self.replay_hot_op(hot_op.clone()).is_err() {
				deferred.push(hot_op);
			}
		}

		Ok(deferred)
	}

	fn merge_remote(&mut self, deltas: Vec<Delta>, retires: &[HotOpId], head: Option<Rev>) -> Result<(), TargetError> {
		self.follow(deltas, head)?;
		self.discard_hot_ops(retires)?;
		Ok(())
	}

	fn retired_marks(&self) -> RetiredHotOps {
		Session::retired_marks(self).clone()
	}

	fn absorb_retired_marks(&mut self, remote: &RetiredHotOps) -> Result<(), TargetError> {
		Session::absorb_retired_marks(self, remote)?;
		Ok(())
	}

	fn retracted_marks(&self) -> RetiredHotOps {
		Session::retracted_marks(self).clone()
	}

	fn absorb_retracted_marks(&mut self, remote: &RetiredHotOps) -> Result<(), TargetError> {
		Session::absorb_retracted_marks(self, remote);
		Ok(())
	}

	fn retract_hot_ops(&mut self, ops: &[HotOpId]) -> Result<(), TargetError> {
		Session::retract_hot_ops(self, ops);
		Ok(())
	}

	fn drop_interaction(&mut self, rev: Rev) -> Result<HeadMove, TargetError> {
		let (moved, _) = Session::drop_interaction(self, rev)?;
		Ok(moved)
	}

	fn restore_interaction(&mut self, rev: Rev) -> Result<Vec<Delta>, TargetError> {
		let revs = Session::restore_interaction(self, rev)?;
		Ok(revs.into_iter().filter_map(|rev| self.delta(rev).cloned()).collect())
	}

	fn apply_head_move(&mut self, moved: &HeadMove) -> Result<(), TargetError> {
		Session::apply_head_move(self, moved)?;
		Ok(())
	}
}
