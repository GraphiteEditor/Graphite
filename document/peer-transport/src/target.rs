use document_graph_storage::{Delta, HeadMove, HistoryMetadata, HotOp, HotOpId, PeerId, Registry, ResourceHash, Rev, Session, SettledMarks};
use std::collections::HashSet;

pub type TargetError = Box<dyn std::error::Error>;

/// The document state a `Replica` reads from and applies remote changes to.
pub trait SyncTarget {
	fn peer(&self) -> PeerId;
	/// The session token is derived from it. See [`SessionToken::for_document`](crate::SessionToken::for_document).
	fn document_id(&self) -> Option<u64> {
		None
	}
	/// Take on the host's document id after a sync, so this copy is the same document.
	fn adopt_document_id(&mut self, _document_id: u64) -> Result<(), TargetError> {
		Ok(())
	}
	fn head(&self) -> Option<Rev>;
	fn retired_registry(&self) -> Registry;
	fn hot_log(&self) -> Vec<HotOp>;
	fn known_revs(&self) -> Vec<Rev>;
	fn contains_rev(&self, rev: Rev) -> bool;
	/// What a peer holding `known` lacks. Handing it out publishes it. See [`Session::deltas_unknown_to`].
	fn deltas_unknown_to(&mut self, known: &[Rev]) -> Vec<Delta>;

	/// Which hot ops are retired or taken back. See [`Session::settled_marks`].
	fn settled_marks(&self) -> SettledMarks;
	/// Take on a peer's marks, dropping what they cover from the hot log.
	fn absorb_settled_marks(&mut self, remote: &SettledMarks) -> Result<(), TargetError>;

	/// The history's users and their names, merged last-writer-wins. Empty for a target that keeps none.
	fn metadata(&self) -> HistoryMetadata {
		HistoryMetadata::default()
	}
	/// Take on a peer's metadata. Returns whether any of it was news.
	fn absorb_metadata(&mut self, _remote: &HistoryMetadata) -> Result<bool, TargetError> {
		Ok(false)
	}

	/// Replace all state with the given retired state.
	fn load(&mut self, registry: Registry, history: Vec<Delta>, head: Option<Rev>) -> Result<(), TargetError>;
	/// Must be idempotent on structural ops, since the sync may already reflect a buffered op. Returns the ops
	/// whose referents have not arrived, for the caller to retry.
	fn apply_remote_hot_ops(&mut self, ops: Vec<HotOp>) -> Result<Vec<HotOp>, TargetError>;
	/// Take the host's deltas on and follow to `head`, or to the batch's last delta when `None`. See [`Session::follow`].
	fn merge_remote(&mut self, deltas: Vec<Delta>, retires: &[HotOpId], head: Option<Rev>) -> Result<(), TargetError>;
	/// Host only: take a peer's deltas on, joining a diverged line with a merge delta rather than following it.
	/// Returns the absorbed deltas and the merge, for the host to broadcast so every guest follows.
	fn merge_divergent(&mut self, deltas: Vec<Delta>, retires: &[HotOpId]) -> Result<Vec<Delta>, TargetError>;
	/// A peer took back hot ops of its own: drop them and re-derive what they touched.
	fn retract_hot_ops(&mut self, ops: &[HotOpId]) -> Result<(), TargetError>;
	/// Host only: drop a retired interaction out of the shared line. See [`Session::drop_interaction`].
	fn drop_interaction(&mut self, rev: Rev) -> Result<HeadMove, TargetError>;
	/// Host only: mint a dropped interaction again on top of the line. See [`Session::restore_interaction`].
	fn restore_interaction(&mut self, rev: Rev) -> Result<Vec<Delta>, TargetError>;
	/// Host only: move the shared head to an ancestor, leaving the line since as a branch. See [`Session::move_head_to`].
	fn move_head_to(&mut self, rev: Rev) -> Result<HeadMove, TargetError> {
		let _ = rev;
		Err("this target cannot move its head".into())
	}
	/// Follow the host's cursor move. See [`Session::apply_head_move`].
	fn apply_head_move(&mut self, moved: &HeadMove) -> Result<(), TargetError>;
	/// Make everything applied since the last flush durable. Called once per [`Replica::poll`](crate::Replica::poll)
	/// so a target that rewrites whole files does it once per batch.
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

	fn deltas_unknown_to(&mut self, known: &[Rev]) -> Vec<Delta> {
		Session::deltas_unknown_to(self, known.iter().copied())
	}

	fn load(&mut self, registry: Registry, history: Vec<Delta>, head: Option<Rev>) -> Result<(), TargetError> {
		// A spent sequence must not come round again, or a new op of this peer's would be taken as retired.
		let spent = self.last_hot_sequence();
		*self = Session::load(self.peer(), self.user(), registry, history, head, Vec::new(), self.next_node_counter());
		self.restore_hot_sequence(spent);
		Ok(())
	}

	fn apply_remote_hot_ops(&mut self, ops: Vec<HotOp>) -> Result<Vec<HotOp>, TargetError> {
		// Every op gets its turn even if one fails, since its broadcast is already consumed. A failure usually
		// means a referent has not arrived, so the op is handed back to retry.
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
		self.discard_hot_ops(retires);
		Ok(())
	}

	fn merge_divergent(&mut self, deltas: Vec<Delta>, retires: &[HotOpId]) -> Result<Vec<Delta>, TargetError> {
		let mut absorbed: Vec<Delta> = deltas.iter().filter(|delta| !self.contains_rev(delta.id)).cloned().collect();
		let outcome = self.merge(deltas)?;
		self.discard_hot_ops(retires);
		if let document_graph_storage::MergeOutcome::Merged(rev) = outcome {
			absorbed.extend(self.delta(rev).cloned());
		}
		Ok(absorbed)
	}

	fn settled_marks(&self) -> SettledMarks {
		Session::settled_marks(self).clone()
	}

	fn absorb_settled_marks(&mut self, remote: &SettledMarks) -> Result<(), TargetError> {
		Session::absorb_settled_marks(self, remote);
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

	fn move_head_to(&mut self, rev: Rev) -> Result<HeadMove, TargetError> {
		Ok(Session::move_head_to(self, rev)?.0)
	}
}
