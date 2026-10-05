#[cfg(any(feature = "conversion", test))]
use crate::NodeMetadataSource;
#[cfg(any(feature = "conversion", test))]
use crate::from_runtime;
use crate::{
	ApplyMode, Delta, Document, History, Implementation, LamportClock, NetworkId, NodeId, PeerId, Registry, RegistryDelta, RegistryTarget, ResourceEntry, Rev, TimeStamp, Touched, UserId, Value,
	to_value,
};
use graphene_resource::{ResourceHash, ResourceId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// A live editing session over a `Document`. Owns the document plus runtime collaboration
/// state that isn't persisted (currently just peer heartbeat tracking).
#[derive(Clone, Debug)]
pub struct Session {
	pub(crate) document: Document,
	/// Each peer's `retirement_tip` as reported by their most recent heartbeat. Drives
	/// leader-eligibility computation (lowest PeerId among peers whose tip matches the session max).
	#[expect(dead_code, reason = "Populated once heartbeat/leader-election transport lands; held now so the field and constructors are in place.")]
	remote_tips: HashMap<PeerId, Rev>,
}

impl Session {
	/// Mints a fresh `PeerId` from the process-wide UUID generator and wraps an empty `Document`.
	/// Two peers in the same process will collide (the generator is seeded once); use `with_peer`
	/// in tests where determinism matters.
	#[cfg(any(feature = "conversion", test))]
	pub fn new() -> Self {
		Self::with_peer(PeerId(core_types::uuid::generate_uuid()))
	}

	/// A session for `peer` standing for itself as a person, for tests and tools without a stored identity.
	pub fn with_peer(peer: PeerId) -> Self {
		Self::with_identity(peer, UserId(peer.0))
	}

	/// A session for device `peer` used by person `user`, whom its registration records.
	pub fn with_identity(peer: PeerId, user: UserId) -> Self {
		Self {
			document: Document::empty(peer, user),
			remote_tips: HashMap::new(),
		}
	}

	pub fn peer(&self) -> PeerId {
		self.document.peer
	}

	pub fn user(&self) -> UserId {
		self.document.user
	}

	pub fn user_of(&self, peer: PeerId) -> Option<UserId> {
		let registered = || self.document.working_registry.peer_users.get(&peer).map(|registration| registration.user);
		(peer == self.document.peer).then_some(self.document.user).or_else(registered)
	}

	pub fn registry(&self) -> &Registry {
		&self.document.working_registry
	}

	/// The registry after applying retired history only, without the unretired hot tail. Persisted as the
	/// snapshot alongside `history` + hot log so a reopen restores the same retired-then-hot layering.
	pub fn retired_registry(&self) -> &Registry {
		&self.document.retired_snapshot
	}

	/// Diff the current registry against a fresh conversion of `network`, then commit each emitted
	/// op as its own `Delta` on the local chain. One `clock.tick()` per op (strictly causal within
	/// a commit). Returns the new `Rev`s in commit order (empty if nothing changed) plus the
	/// conversion, whose extracted declarations the caller persists and caches.
	///
	/// Stages the diff as hot ops rather than retired deltas: each op is applied to the registry and
	/// pushed onto the hot log. The caller persists the returned hot frames and then calls `retire`
	/// to promote them into durable history.
	#[cfg(any(feature = "conversion", test))]
	pub fn stage_from_runtime<M: NodeMetadataSource>(
		&mut self,
		network: &graph_craft::document::NodeNetwork,
		metadata: &M,
		resources: &graphene_resource::ResourceRegistry,
	) -> Result<(Vec<HotOp>, from_runtime::RuntimeConversion), CommitError> {
		let conversion = Registry::convert_from_runtime(network, metadata, resources, self.document.peer)?;
		let ops = crate::delta::compute_deltas(&self.document.working_registry, &conversion.registry);
		let hot_ops = self.stage_ops(ops)?;
		Ok((hot_ops, conversion))
	}

	/// Resolve each runtime `network_path` to its stable [`NetworkId`] for this document's peer, so the
	/// caller can key per-network, per-peer view state (`session.json`) by a stable id. Derived from the
	/// network structure alone; resources/declarations are irrelevant to the ids.
	#[cfg(any(feature = "conversion", test))]
	pub fn network_ids<M: NodeMetadataSource>(&self, network: &graph_craft::document::NodeNetwork, metadata: &M) -> Result<HashMap<Vec<core_types::uuid::NodeId>, NetworkId>, CommitError> {
		let conversion = Registry::convert_from_runtime(network, metadata, &graphene_resource::ResourceRegistry::new(), self.document.peer)?;
		Ok(conversion.network_ids)
	}

	/// Register a content-addressed resource as a single `DataSource::Embedded` source resolved to
	/// `hash`, staged as one `AddResource` hot op. The caller owns `id` allocation, persists the
	/// returned hot frame, retires, and persists the bytes into its byte store separately.
	pub fn stage_embedded_resource(&mut self, id: ResourceId, hash: ResourceHash) -> Result<Vec<HotOp>, CrdtError> {
		let entry = ResourceEntry::embedded(hash, self.document.peer, self.document.clock.tick());
		self.stage_ops([RegistryDelta::AddResource { id, entry }])
	}

	/// Commit an `AddSource(Embedded)` retired delta for each given resource, making it the highest-
	/// precedence fallback. Skips resources that already have an `Embedded` source or no longer exist.
	/// Used on a throwaway session clone at export time so the exported registry and history agree;
	/// callers must guarantee the bytes are available in the export's resource store.
	pub fn embed_resource_sources(&mut self, ids: impl IntoIterator<Item = ResourceId>) -> Result<Vec<Rev>, CrdtError> {
		let embedded = to_value(&graphene_resource::DataSource::Embedded).expect("DataSource::Embedded serializes");

		let mut ops = Vec::new();
		for id in ids {
			let Some(entry) = self.document.working_registry.resources.get(&id) else { continue };
			if entry.has_embedded_source() {
				continue;
			}
			let key = entry.highest_precedence_key(self.document.peer);
			ops.push(RegistryDelta::AddSource { id, key, source: embedded.clone() });
		}

		// These are retired deltas, so `commit_ops` advances the retired snapshot and history. The working
		// registry sits at `retired_snapshot + hot tail`, so mirror each committed delta onto it with its own
		// timestamp rather than cloning the snapshot over it, which would discard any unretired hot-zone edits.
		let revs = self.commit_ops(ops, false)?;
		for &rev in &revs {
			let Some(delta) = self.document.history.get(rev) else { continue };
			let (kind, timestamp) = (delta.kind.clone(), delta.timestamp);
			self.document.apply_op_idempotent(kind, timestamp)?;
		}
		Ok(revs)
	}

	/// Stages ops computed outside the session (an incremental runtime projection) as hot ops, the
	/// same way `stage_from_runtime` stages a diff's ops.
	pub fn stage_computed_ops(&mut self, ops: Vec<RegistryDelta>) -> Result<Vec<HotOp>, CrdtError> {
		self.stage_ops(ops)
	}

	/// Apply each op as a hot op with a freshly-ticked timestamp, returning the staged frames in
	/// order. Each tick is strictly later than the last, so the final frame carries the latest
	/// timestamp, which is what the caller passes to `retire`.
	///
	/// The peer's first contribution is preceded by a `RegisterPeer` op, so the device's
	/// `PeerId → UserId` mapping is established (and, under causal delivery, observed by other peers)
	/// before any of its edits. A no-op batch doesn't register, since registration rides a real edit.
	pub(crate) fn stage_ops(&mut self, ops: impl IntoIterator<Item = RegistryDelta>) -> Result<Vec<HotOp>, CrdtError> {
		// A batch is staged whole or not at all.
		let mut pending: Vec<RegistryDelta> = ops.into_iter().collect();
		if pending.is_empty() {
			return Ok(Vec::new());
		}

		let (peer, user) = (self.document.peer, self.document.user);
		if self.document.working_registry.peer_users.get(&peer).map(|registration| registration.user) != Some(user) {
			pending.insert(0, RegistryDelta::RegisterPeer { peer, user });
		}

		// A reused sequence would fall under the settled marks, so a batch running past the end is refused.
		if self.document.last_hot_sequence.0.checked_add(pending.len() as u64).is_none() {
			return Err(CrdtError::SequencesExhausted);
		}

		let sequence_before = self.document.last_hot_sequence;
		let mut staged = Vec::with_capacity(pending.len());
		for op in pending {
			let sequence = HotSequence(self.document.last_hot_sequence.0 + 1);
			let hot_op = HotOp {
				op,
				timestamp: self.document.clock.tick(),
				sequence,
			};
			if let Err(error) = self.document.stage_hot_op(hot_op.clone(), ApplyMode::Strict) {
				// The run continues where it was, so no sequence goes unused.
				self.document.last_hot_sequence = sequence_before;
				if !staged.is_empty() {
					let taken: HashSet<HotOpId> = staged.iter().map(HotOp::id).collect();
					self.document.hot_log.retain(|hot_op| !taken.contains(&hot_op.id()));
					self.document.resync_hot_timestamps();
					self.document.rebuild_working();
				}
				return Err(error);
			}
			self.document.last_hot_sequence = sequence;
			staged.push(hot_op);
		}
		Ok(staged)
	}

	/// Wrap each op as a `Delta`, apply it, and chain it onto the local history. One tick per op.
	///
	/// Operates on the retired snapshot, so each `reverse` holds the true pre-op state; the working registry already
	/// reflects the ops, staged before retirement or equal to the snapshot when nothing is hot.
	///
	/// `idempotent`: pass `true` when the snapshot already reflects the op (retirement of an already-
	/// applied hot op) so duplicate structural inserts no-op rather than error.
	fn commit_ops(&mut self, ops: impl IntoIterator<Item = RegistryDelta>, idempotent: bool) -> Result<Vec<Rev>, CrdtError> {
		self.commit_ops_authored_at(ops.into_iter().map(|op| (op, None)), idempotent)
	}

	/// [`commit_ops`](Self::commit_ops) for ops paired with their authored stamp (`None` to mint one), which retirement keeps
	/// so LWW resolves as it did live.
	fn commit_ops_authored_at(&mut self, ops: impl IntoIterator<Item = (RegistryDelta, Option<TimeStamp>)>, idempotent: bool) -> Result<Vec<Rev>, CrdtError> {
		let target = RegistryTarget::Retired;
		let ops = ops.into_iter();
		let mut produced = Vec::with_capacity(ops.size_hint().0);

		for (op, authored_at) in ops {
			// A new edit abandons any undone-forward branch: those revs stay in the DAG but are no
			// longer reachable via redo. (Mirrors the legacy editor clearing its redo history on
			// commit.) Done on the first real op so a no-op commit doesn't silently disable redo.
			if produced.is_empty() {
				self.document.redo_stack.clear();
			}

			let reverse = crate::prior::capture(self.document.registry_ref(target), &op);
			let timestamp = authored_at.unwrap_or_else(|| self.document.clock.tick());
			let parent = self.document.head;
			let author = timestamp.peer;

			let delta = Delta::new(parent, author, timestamp, op, reverse);
			let rev = delta.id;

			// `parent` is `None` for the root commit; otherwise it must already be in history.
			if let Some(parent) = parent
				&& !self.document.history.contains(parent)
			{
				return Err(CrdtError::NotFoundInHistory(parent));
			}
			let mode = if idempotent { ApplyMode::Idempotent } else { ApplyMode::Strict };
			self.document.apply_op_with(target, delta.kind.clone(), delta.timestamp, mode)?;
			self.document.history.push(delta);
			self.document.head = Some(rev);
			produced.push(rev);
		}

		Ok(produced)
	}

	/// Wrap an already-materialized snapshot. Trusts `registry` to match `history`; advances the
	/// clock past every observed timestamp but does not re-apply ops. `history` is taken in on-disk
	/// (topological) order.
	pub fn load(peer: PeerId, user: UserId, registry: Registry, history: Vec<Delta>, head: Option<Rev>, redo_stack: Vec<Rev>, next_node_counter: u64) -> Self {
		let mut clock = LamportClock::new(peer);
		for delta in &history {
			clock.observe(delta.timestamp);
		}

		Self {
			document: Document {
				// The persisted snapshot is the retired state; hot ops (replayed by the caller after
				// `load`) build the working registry on top, leaving `retired_snapshot` at retired.
				retired_snapshot: registry.clone(),
				working_registry: registry,
				history: History::from_ordered(history),
				head,
				redo_stack,
				clock,
				next_node_counter,
				..Document::empty(peer, user)
			},
			remote_tips: HashMap::new(),
		}
	}

	/// Rebuild the registry from scratch by applying every delta in causal order.
	/// `deltas` must be in causal order (every parent before its children).
	pub fn replay_from_history(peer: PeerId, user: UserId, deltas: impl IntoIterator<Item = Delta>, next_node_counter: u64) -> Result<Self, CrdtError> {
		let mut session = Self::with_identity(peer, user);
		session.document.next_node_counter = next_node_counter;

		for delta in deltas {
			let rev = delta.id;
			session.document.apply_op_idempotent(delta.kind.clone(), delta.timestamp)?;
			session.document.history.push(delta);
			session.document.head = Some(rev);
		}

		// Pure retired-delta replay: no hot ops, so the working registry is fully retired.
		session.document.retired_snapshot = session.document.working_registry.clone();
		Ok(session)
	}

	/// Which hot ops are done with, for a peer catching up and for persisting. See [`SettledMarks`].
	pub fn settled_marks(&self) -> &SettledMarks {
		&self.document.settled
	}

	/// Take on a peer's marks, or this peer's persisted ones, dropping the hot ops they cover. Returns what those named.
	pub fn absorb_settled_marks(&mut self, remote: &SettledMarks) -> Touched {
		self.document.absorb_settled_marks(remote)
	}

	/// Replay a persisted hot op. Idempotent on structural ops, suitable for crash recovery
	/// where the registry may already reflect the op's effect from a prior retired snapshot.
	pub fn replay_hot_op(&mut self, hot_op: HotOp) -> Result<(), CrdtError> {
		self.document.replay_hot_op(hot_op)
	}

	/// Integrate `incoming` retired deltas from another branch and emit a [`RegistryDelta::Merge`]
	/// joining the resulting tips, returning the new merge `Rev` (or `None` if `incoming` adds nothing).
	/// Applies each incoming op to the registry, then hands the set to [`History::merge`]. Incoming
	/// deltas must arrive in causal order.
	pub fn merge(&mut self, incoming: impl IntoIterator<Item = Delta>) -> Result<Option<Rev>, CrdtError> {
		let mut absorbed: Vec<Delta> = Vec::new();
		for delta in incoming {
			if self.document.history.contains(delta.id) {
				continue;
			}
			self.document.apply_op_idempotent(delta.kind.clone(), delta.timestamp)?;
			absorbed.push(delta);
		}
		if absorbed.is_empty() {
			return Ok(None);
		}

		self.document.history.merge(absorbed);
		let tips = self.document.history.tips();
		let timestamp = self.document.clock.tick();
		let merge = Delta::merge(tips, self.document.peer, timestamp);
		let merge_rev = merge.id;
		// The merge's parents are the current tips, so it sorts last: `push` preserves the canonical
		// order without re-sorting the whole history.
		self.document.history.push(merge);
		self.document.head = Some(merge_rev);

		// Merge runs with an empty hot log; keep the retired snapshot in step with the working registry.
		self.document.retired_snapshot = self.document.working_registry.clone();
		Ok(Some(merge_rev))
	}

	/// The retired snapshot as history alone gives it: the fold of `head`'s ancestry, leaving out undone deltas kept for redo.
	pub fn snapshot_from_history(&self) -> Result<Registry, CrdtError> {
		let reachable = self.document.history.ancestors(self.document.head);
		let mut scratch = Document::empty(self.document.peer, self.document.user);
		for delta in self.document.history.iter().filter(|delta| reachable.contains(&delta.id)) {
			scratch.apply_op_with(RegistryTarget::Working, delta.kind.clone(), delta.timestamp, ApplyMode::Idempotent)?;
		}
		Ok(scratch.working_registry)
	}

	pub fn history_len(&self) -> usize {
		self.document.history.len()
	}

	/// The hot ops stamped at or before `up_to`, which [`retire`](Self::retire) drains.
	pub fn hot_ops_up_to(&self, up_to: TimeStamp) -> Vec<HotOpId> {
		self.document.hot_log.iter().filter(|hot_op| hot_op.timestamp <= up_to).map(HotOp::id).collect()
	}

	/// Promote the given hot ops into retired deltas in hot-log order, each keeping its authored stamp.
	///
	/// One retired delta per hot op.
	pub fn retire_hot_ops(&mut self, ids: &[HotOpId]) -> Result<Vec<Rev>, CrdtError> {
		let wanted: HashSet<HotOpId> = ids.iter().copied().collect();
		let (drained, kept): (Vec<HotOp>, Vec<HotOp>) = self.document.hot_log.drain(..).partition(|hot_op| wanted.contains(&hot_op.id()));
		self.document.hot_log = kept;
		self.document.resync_hot_timestamps();
		self.document.settled.extend(drained.iter().map(HotOp::id));

		let ops = drained.into_iter().map(|hot_op| (hot_op.op, Some(hot_op.timestamp)));
		self.commit_ops_authored_at(ops, true)
	}

	/// Promote every hot op stamped at or before `up_to`, whatever its author.
	pub fn retire(&mut self, up_to: TimeStamp) -> Result<Vec<Rev>, CrdtError> {
		let ids = self.hot_ops_up_to(up_to);
		self.retire_hot_ops(&ids)
	}
	/// Mark a retired delta as the end of a user interaction, so the undo cursor treats it as a checkpoint.
	/// Called once per interaction by the editor-facing commit path (not by resource/internal commits).
	pub fn mark_interaction_end(&mut self, rev: Rev) {
		let timestamp = self.document.clock.tick();
		self.document.history.mark_interaction_end(rev, timestamp);
	}

	/// Low-level: set a local annotation attribute (e.g. a commit message) on a retired delta in place.
	/// Excluded from the delta's content-addressed `Rev`, so identity is unchanged. Returns whether the
	/// delta was found. The `Gdd` layer re-persists the affected history frame after calling this.
	pub fn annotate_delta(&mut self, rev: Rev, key: &str, value: Value) -> bool {
		let timestamp = self.document.clock.tick();
		self.document.history.annotate(rev, key, value, timestamp)
	}

	/// Whether the interaction at `head` undoes silently: this user's own, no merge, nothing a peer holds. The document's
	/// first interaction is not undoable, as opening a document starts with an empty undo history.
	pub fn can_undo(&self) -> bool {
		self.document.head.is_some_and(|head| self.interaction_start_parent(head, true).is_some())
	}

	/// Where the cursor rests once the interaction ending at `end` is undone: the previous interaction's end, a merge, or
	/// another user's delta. `None` for a merge, the first interaction, and for a `silent` undo of another's or a published one.
	fn interaction_start_parent(&self, end: Rev, silent: bool) -> Option<Rev> {
		let history = &self.document.history;
		let mut current = history.get(end)?;
		let user = self.user_of(current.author);
		if matches!(current.kind, RegistryDelta::Merge { .. }) || (silent && user != Some(self.document.user)) {
			return None;
		}
		let boundary = |delta: &Delta| delta.is_interaction_end() || matches!(delta.kind, RegistryDelta::Merge { .. }) || self.user_of(delta.author) != user;
		loop {
			if silent && self.document.last_broadcast_rev == Some(current.id) {
				return None;
			}
			let parent = history.get(current.parent?)?;
			if boundary(parent) {
				return Some(parent.id);
			}
			current = parent;
		}
	}

	pub fn can_redo(&self) -> bool {
		!self.document.redo_stack.is_empty()
	}

	/// Silent-zone undo of one interaction: put back what its deltas overwrote, newest first, until `head` rests where it
	/// began (see [`can_undo`](Self::can_undo)). Its `head` rev goes on the redo stack; the DAG is not rewritten.
	pub fn undo(&mut self) -> Result<Rev, CrdtError> {
		let checkpoint = self.document.head.ok_or(CrdtError::NothingToUndo)?;
		let base = self.interaction_start_parent(checkpoint, true).ok_or(CrdtError::NothingToUndo)?;

		while let Some(rev) = self.document.head.filter(|&rev| rev != base) {
			let delta = self.document.history.get(rev).ok_or(CrdtError::NotFoundInHistory(rev))?.clone();
			self.document.revert_delta(&delta);
			self.document.head = delta.parent;
		}

		// With ops hot, working is the rewound snapshot plus them.
		if !self.document.hot_log.is_empty() {
			self.document.rebuild_working();
		}
		self.document.redo_stack.push(checkpoint);
		Ok(checkpoint)
	}

	/// Redo the most-recently-undone interaction: re-apply every delta from the current `head` forward to
	/// (and including) the checkpoint rev, advancing `head` to it. Collects the forward span by walking
	/// parents back from the checkpoint to `head` (the chain is linear in the silent solo zone).
	pub fn redo(&mut self) -> Result<Rev, CrdtError> {
		let checkpoint = self.document.redo_stack.pop().ok_or(CrdtError::NothingToRedo)?;

		let mut forward = Vec::new();
		let mut cursor = Some(checkpoint);
		while cursor != self.document.head {
			let Some(rev) = cursor else { break };
			let delta = self.document.history.get(rev).ok_or(CrdtError::NotFoundInHistory(rev))?.clone();
			cursor = delta.parent;
			forward.push(delta);
		}

		// The snapshot is the fold up to `head`, so applying the deltas as retirement did folds them in again.
		let hot = !self.document.hot_log.is_empty();
		for delta in forward.into_iter().rev() {
			self.document.apply_op_with(RegistryTarget::Retired, delta.kind.clone(), delta.timestamp, ApplyMode::Idempotent)?;
			if !hot {
				self.document.apply_op_with(RegistryTarget::Working, delta.kind, delta.timestamp, ApplyMode::Idempotent)?;
			}
		}
		if hot {
			self.document.rebuild_working();
		}
		self.document.head = Some(checkpoint);

		Ok(checkpoint)
	}

	/// Build a synthetic linear history whose replay reproduces `registry`. Each op gets a
	/// freshly-ticked clock timestamp and chains to the previous op's `Rev`.
	pub fn bootstrap_from_registry(peer: PeerId, user: UserId, registry: Registry) -> Result<Self, CrdtError> {
		let ops = crate::delta::compute_deltas(&Registry::default(), &registry);
		let mut session = Self::with_identity(peer, user);
		session.commit_ops(ops, false)?;
		// No hot ops on this path, so the working registry must mirror the freshly-built snapshot.
		session.document.working_registry = session.document.retired_snapshot.clone();
		Ok(session)
	}

	/// Retired deltas in append order, which is a valid replay order (parents before children).
	pub fn history(&self) -> impl Iterator<Item = &Delta> + '_ {
		self.document.history.iter()
	}

	/// The retired delta for `rev`, or `None` if it isn't in history. O(1) lookup, for callers that
	/// already hold the revs they want (e.g. persisting a freshly-retired batch) and don't need a scan.
	pub fn delta(&self, rev: Rev) -> Option<&Delta> {
		self.document.history.get(rev)
	}

	/// Verify the retired history loaded from an untrusted source: content-addressed ids match their
	/// recomputed hashes, and the deltas are topologically ordered. See [`History::verify`].
	pub fn verify_history(&self) -> Result<(), CrdtError> {
		self.document.history.verify()
	}

	/// Every resource hash referenced by the current registry *or* anywhere in history. Undo removes a
	/// interaction's `AddResource` from the working registry, so a redoable (or re-undoable) interaction's
	/// resources no longer appear in `registry().resources` even though redo still needs them. Resource GC
	/// must keep this whole set alive, not just the current head's, or undo then redo loses declaration
	/// bytes. Walks current resources plus each delta's `AddResource`/`RemoveResource` snapshot and `SetResourceHash`.
	pub fn all_referenced_resource_hashes(&self) -> HashSet<ResourceHash> {
		let mut hashes: HashSet<ResourceHash> = self.document.working_registry.resources.values().filter_map(|entry| entry.hash).collect();
		hashes.extend(self.document.history.resource_hashes());
		hashes
	}

	/// Every proto-node declaration resource referenced by the current registry or anywhere in history,
	/// with its content hash, or `None` where the hash never resolved.
	pub fn all_declaration_resources(&self) -> HashMap<ResourceId, Option<ResourceHash>> {
		let registry = &self.document.working_registry;

		let mut hashes: HashMap<ResourceId, ResourceHash> = registry.resources.iter().filter_map(|(id, entry)| Some((*id, entry.hash?))).collect();
		for delta in self.document.history.iter() {
			match &delta.kind {
				RegistryDelta::AddResource { id, entry } => hashes.extend(entry.hash.map(|hash| (*id, hash))),
				RegistryDelta::RemoveResource { id, snapshot } => hashes.extend(snapshot.hash.map(|hash| (*id, hash))),
				RegistryDelta::SetResourceHash { id, hash: Some(hash) } => {
					hashes.insert(*id, *hash);
				}
				_ => {}
			}
		}

		let current_nodes = registry.node_instances.values();
		let historic_nodes = self.document.history.iter().filter_map(|delta| match &delta.kind {
			RegistryDelta::AddNode { node, .. } => Some(node),
			RegistryDelta::RemoveNode { snapshot, .. } => Some(snapshot),
			_ => None,
		});

		current_nodes
			.chain(historic_nodes)
			.filter_map(|node| match node.implementation() {
				Implementation::ProtoNode(id) => Some(*id),
				Implementation::Network(_) => None,
			})
			.map(|id| (id, hashes.get(&id).copied()))
			.collect()
	}

	pub fn hot_log(&self) -> &[HotOp] {
		&self.document.hot_log
	}

	pub fn head_rev(&self) -> Option<Rev> {
		self.document.head
	}

	/// The latest retired commit broadcast to at least one peer. Commits after it are silently
	/// rewritable; commits at or before it are published. `None` until broadcast transport lands.
	pub fn last_broadcast_rev(&self) -> Option<Rev> {
		self.document.last_broadcast_rev
	}

	/// Advance the published frontier to `rev` as commits are broadcast. The frontier is monotonic, so
	/// this only moves it forward (never back to `None`). Set by the (future) broadcast transport;
	/// persisted in `session.json` so the silent/published boundary survives a reopen.
	pub fn publish_up_to(&mut self, rev: Rev) {
		self.document.last_broadcast_rev = Some(rev);
	}

	/// Test-only: every retired delta, cloned, for feeding one session's branch into another's `merge`.
	#[cfg(test)]
	pub(crate) fn cloned_deltas(&self) -> Vec<Delta> {
		self.document.history.iter().cloned().collect()
	}

	/// Test-only: commit a single op as a retired delta on the local chain, returning the result so a
	/// test can observe a failure (e.g. `NotFoundInHistory`).
	#[cfg(test)]
	pub(crate) fn commit_op_for_test(&mut self, op: RegistryDelta) -> Result<(), CrdtError> {
		self.commit_ops(std::iter::once(op), false).map(|_| ())
	}

	pub fn redo_stack(&self) -> &[Rev] {
		&self.document.redo_stack
	}

	pub fn next_node_counter(&self) -> u64 {
		self.document.next_node_counter
	}

	/// The last hot op sequence this peer authored, carried across a reload so none is spent twice.
	pub fn last_hot_sequence(&self) -> HotSequence {
		self.document.last_hot_sequence
	}

	/// The Lamport counter, persisted so a reopen never mints a spent stamp, even one nothing carries any more.
	pub fn clock_counter(&self) -> u64 {
		self.document.clock.counter
	}

	/// Continue the Lamport clock after a load. Raises only, like observing an op.
	pub fn restore_clock_counter(&mut self, counter: u64) {
		self.document.clock.counter = self.document.clock.counter.max(counter);
	}

	/// Restore the hot op sequence after a load. Raises only, as replaying the hot log may already have.
	pub fn restore_hot_sequence(&mut self, sequence: HotSequence) {
		self.document.last_hot_sequence = self.document.last_hot_sequence.max(sequence);
	}
}

/// Errors from `Session::commit_from_runtime`.
#[cfg(any(feature = "conversion", test))]
#[derive(Debug, thiserror::Error)]
pub enum CommitError {
	#[error("Failed to convert runtime network: {0}")]
	Conversion(#[from] from_runtime::ConversionError),
	#[error("Failed to apply commit: {0}")]
	Crdt(#[from] CrdtError),
}

#[cfg(any(feature = "conversion", test))]
impl Default for Session {
	fn default() -> Self {
		Self::new()
	}
}

/// One live op in the hot zone. Carries only enough to drive live LWW; no parents (transient),
/// no Rev (not content-addressed in the durable DAG). GC'd at retirement.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HotOp {
	pub op: RegistryDelta,
	pub timestamp: TimeStamp,
	/// Position in its author's run. See [`HotSequence`].
	pub sequence: HotSequence,
}

impl HotOp {
	/// Identifies the op in the settled marks, which track a contiguous prefix per author.
	pub fn id(&self) -> HotOpId {
		HotOpId {
			peer: self.timestamp.peer,
			sequence: self.sequence,
		}
	}
}

/// Position in one author's run of hot ops, counting from 1 with no gaps. Kept apart from the Lamport
/// counter, which skips on observing a higher remote timestamp and so cannot bound a contiguous prefix.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HotSequence(pub u64);

impl HotSequence {
	/// Before this author has written anything, covering no op.
	pub const NONE: Self = Self(0);

	/// The position after this one, for adjacency. Saturates: a run ending at the last position has no successor.
	pub fn next(self) -> Self {
		Self(self.0.saturating_add(1))
	}
}

/// One hot op's author and position in its run, as the settled marks name it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct HotOpId {
	pub peer: PeerId,
	pub sequence: HotSequence,
}

/// Which hot ops are retired: each author's gap-free prefix, plus runs past it while ops are in flight. Persisted, so
/// a late copy of a retired op is dropped.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettledMarks {
	pub settled_up_to: HashMap<PeerId, HotSequence>,
	/// Settled ops past their author's prefix, as inclusive runs, one per gap in flight.
	pub settled_runs: HashMap<PeerId, Vec<(HotSequence, HotSequence)>>,
}

/// Sort and coalesce runs, joining any that touch or abut.
fn coalesce(runs: &mut Vec<(HotSequence, HotSequence)>) {
	runs.sort_unstable();
	runs.dedup_by(|(start, end), (_, last_end)| {
		let joins = *start <= last_end.next();
		if joins {
			*last_end = (*last_end).max(*end);
		}
		joins
	});
}

impl SettledMarks {
	/// Whether history already holds this hot op.
	pub fn covers(&self, id: HotOpId) -> bool {
		self.settled_up_to.get(&id.peer).is_some_and(|&through| id.sequence <= through)
			|| self
				.settled_runs
				.get(&id.peer)
				.is_some_and(|runs| runs.iter().any(|&(start, end)| (start..=end).contains(&id.sequence)))
	}

	/// Take on `remote`'s coverage as well as this one's.
	pub(crate) fn absorb(&mut self, remote: &Self) {
		for (&peer, &remote_through) in &remote.settled_up_to {
			let through = self.settled_up_to.entry(peer).or_default();
			*through = (*through).max(remote_through);
		}
		for (&peer, runs) in &remote.settled_runs {
			self.settled_runs.entry(peer).or_default().extend(runs.iter().copied());
		}
		self.compact();
	}

	/// Record newly settled ops.
	pub fn extend(&mut self, settled: impl IntoIterator<Item = HotOpId>) {
		for id in settled {
			self.settled_runs.entry(id.peer).or_default().push((id.sequence, id.sequence));
		}
		self.compact();
	}

	/// Fold runs that continue their author's prefix into `settled_up_to`, leaving only those past a gap.
	fn compact(&mut self) {
		for (&peer, runs) in &mut self.settled_runs {
			coalesce(runs);
			let mut through = self.settled_up_to.get(&peer).copied().unwrap_or(HotSequence::NONE);
			let joined = runs.iter().take_while(|&&(start, end)| {
				let joins = start <= through.next();
				if joins {
					through = through.max(end);
				}
				joins
			});
			let joined = joined.count();
			runs.drain(..joined);
			if through != HotSequence::NONE {
				self.settled_up_to.insert(peer, through);
			}
		}
		self.settled_runs.retain(|_, runs| !runs.is_empty());
	}
}

#[derive(Debug, thiserror::Error)]
pub enum CrdtError {
	#[error("This peer has authored every hot op sequence there is")]
	SequencesExhausted,
	#[error("Target node {0} does not exist")]
	TargetNodeDoesNotExist(NodeId),
	#[error("Network {0} does not exist")]
	NetworkDoesNotExist(NetworkId),
	#[error("Input index {0} out of bounds")]
	InputIndexOutOfBounds(usize),
	#[error("Export slot index {0} out of bounds")]
	ExportSlotOutOfBounds(u32),
	#[error("Delta {0} not found in history")]
	NotFoundInHistory(Rev),
	#[error("Nothing to undo")]
	NothingToUndo,
	#[error("Nothing to redo")]
	NothingToRedo,
	#[error("Node {0} already exists")]
	NodeAlreadyExists(NodeId),
	#[error("Network {0} already exists")]
	NetworkAlreadyExists(NetworkId),
	#[error("Delta stored under {stored} hashes to {expected}")]
	RevMismatch { stored: Rev, expected: Rev },
}
