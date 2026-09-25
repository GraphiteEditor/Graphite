#[cfg(any(feature = "conversion", test))]
use crate::NodeMetadataSource;
#[cfg(any(feature = "conversion", test))]
use crate::from_runtime;
use crate::{ApplyMode, Delta, Document, History, Implementation, LamportClock, NetworkId, NodeId, PeerId, Registry, RegistryDelta, RegistryTarget, ResourceEntry, Rev, TimeStamp, UserId};
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
	/// The registry the runtime was last built from or staged to. Local diffs are taken against it,
	/// so edits peers applied to the working registry in the meantime are not diffed away.
	runtime_base: Option<Registry>,
}

impl Session {
	/// Mints a fresh `PeerId` from the process-wide UUID generator and wraps an empty `Document`.
	/// Two peers in the same process will collide (the generator is seeded once); use `with_peer`
	/// in tests where determinism matters.
	#[cfg(any(feature = "conversion", test))]
	pub fn new() -> Self {
		Self::with_peer(PeerId(core_types::uuid::generate_uuid()))
	}

	/// Construct a session bound to a specific `PeerId`. Used by tests; production code wants
	/// `Session::new`.
	pub fn with_peer(peer: PeerId) -> Self {
		Self {
			document: Document::empty(peer),
			remote_tips: HashMap::new(),
			runtime_base: None,
		}
	}

	pub fn peer(&self) -> PeerId {
		self.document.peer
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
		let base = self.runtime_base.as_ref().unwrap_or(&self.document.working_registry);
		let ops = crate::delta::compute_deltas(base, &conversion.registry);

		// The base moves only once the ops are staged, so a failure leaves the next diff covering them.
		let hot_ops = self.stage_ops(ops)?;
		self.runtime_base = Some(conversion.registry.clone());

		Ok((hot_ops, conversion))
	}

	/// Record that the runtime now reflects the working registry, after the caller rebuilt it from there.
	pub fn mark_runtime_current(&mut self) {
		self.runtime_base = Some(self.document.working_registry.clone());
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
		let embedded = serde_json::to_value(graphene_resource::DataSource::Embedded).expect("DataSource::Embedded serializes");

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
	pub fn stage_ops(&mut self, ops: impl IntoIterator<Item = RegistryDelta>) -> Result<Vec<HotOp>, CrdtError> {
		let mut pending: Vec<RegistryDelta> = ops.into_iter().collect();
		if pending.is_empty() {
			return Ok(Vec::new());
		}

		if !self.document.working_registry.peer_users.contains_key(&self.document.peer) {
			let user = UserId(self.document.peer.0);
			pending.insert(0, RegistryDelta::RegisterPeer { peer: self.document.peer, user });
		}

		let mut staged = Vec::with_capacity(pending.len());
		for op in pending {
			// The counter advances only once the op is in the log, so a failure leaves no gap in the run.
			let sequence = self.document.next_hot_sequence.next();
			let hot_op = HotOp {
				op,
				timestamp: self.document.clock.tick(),
				sequence,
			};
			self.document.apply_hot_op(hot_op.clone())?;

			self.document.next_hot_sequence = sequence;
			staged.push(hot_op);
		}
		Ok(staged)
	}

	/// Wrap each op as a `Delta`, apply it, and chain it onto the local history. One tick per op.
	///
	/// Operates on the *retired snapshot*: reverses are computed against and forward ops applied to it,
	/// so each `reverse` captures the true pre-op value rather than the hot-polluted working state. The
	/// working registry already reflects these ops (they were staged as hot ops before retirement, or
	/// equal the snapshot when there are none), so it is left untouched.
	///
	/// `idempotent`: pass `true` when the snapshot already reflects the op (retirement of an already-
	/// applied hot op) so duplicate structural inserts no-op rather than error.
	fn commit_ops(&mut self, ops: impl IntoIterator<Item = RegistryDelta>, idempotent: bool) -> Result<Vec<Rev>, CrdtError> {
		self.commit_ops_authored_at(ops.into_iter().map(|op| (op, None)), idempotent)
	}

	/// [`commit_ops`](Self::commit_ops) for ops authored elsewhere, paired with the timestamp they were
	/// authored at (`None` to mint one). The delta's timestamp is what the registry resolves LWW on, so
	/// retirement passes the op's own; a coarsened delta passes the newest it fuses.
	fn commit_ops_authored_at(&mut self, ops: impl IntoIterator<Item = (RegistryDelta, Option<TimeStamp>)>, idempotent: bool) -> Result<Vec<Rev>, CrdtError> {
		let target = RegistryTarget::Snapshot;
		let ops = ops.into_iter();
		let mut produced = Vec::with_capacity(ops.size_hint().0);

		for (op, authored_at) in ops {
			// A new edit abandons any undone-forward branch: those revs stay in the DAG but are no
			// longer reachable via redo. (Mirrors the legacy editor clearing its redo history on
			// commit.) Done on the first real op so a no-op commit doesn't silently disable redo.
			if produced.is_empty() {
				self.document.redo_stack.clear();
			}

			let reverse = self.document.compute_reverse_delta(target, &op)?;
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
			let mode = if idempotent { ApplyMode::Idempotent } else { ApplyMode::Live };
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
	pub fn load(peer: PeerId, registry: Registry, history: Vec<Delta>, head: Option<Rev>, redo_stack: Vec<Rev>, next_node_counter: u64) -> Self {
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
				..Document::empty(peer)
			},
			remote_tips: HashMap::new(),
			runtime_base: None,
		}
	}

	/// Rebuild the registry from scratch by applying every delta in causal order.
	/// `deltas` must be in causal order (every parent before its children).
	pub fn replay_from_history(peer: PeerId, deltas: impl IntoIterator<Item = Delta>, next_node_counter: u64) -> Result<Self, CrdtError> {
		let mut session = Self::with_peer(peer);
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

	/// Apply a hot op from another peer. Idempotent, and a no-op for one history already covers: a late
	/// delivery must not put a retired op back in the hot log.
	pub fn apply_hot_op(&mut self, hot_op: HotOp) -> Result<(), CrdtError> {
		self.document.replay_hot_op(hot_op)
	}

	/// Drop hot ops another peer has retired without retiring them locally. The registries need nothing:
	/// each op's effect is a function of its timestamp, so the working registry holds the same fold with
	/// or without the hot copy of an op whose delta has landed.
	pub fn discard_hot_ops(&mut self, retired: &[HotOpId]) -> Result<(), CrdtError> {
		let retired_ids: HashSet<HotOpId> = retired.iter().copied().collect();
		self.document.hot_log.retain(|hot_op| !retired_ids.contains(&hot_op.id()));
		// How a peer that did not retire these learns they are in history now.
		self.document.mark_retired(retired.iter().copied());

		Ok(())
	}

	/// Which hot ops history already covers, for a peer catching up. See [`RetiredHotOps`].
	pub fn retired_marks(&self) -> &RetiredHotOps {
		&self.document.retired
	}

	/// See [`RetiredHotOps::absorb`].
	pub fn absorb_retired_marks(&mut self, remote: &RetiredHotOps) -> Result<(), CrdtError> {
		self.document.absorb_retired(remote);

		Ok(())
	}

	/// Replay a persisted hot op. Idempotent on structural ops, suitable for crash recovery
	/// where the registry may already reflect the op's effect from a prior retired snapshot.
	pub fn replay_hot_op(&mut self, hot_op: HotOp) -> Result<(), CrdtError> {
		self.document.replay_hot_op(hot_op)
	}

	/// The retired snapshot as canonical history alone produces it, independent of arrival order and the
	/// hot log. Folded on a clone, over `head`'s ancestry only: an undone delta stays in the DAG for redo
	/// to find, and folding all of history would restore work the user undid.
	///
	/// The clone's history grows with the replay, as it did when each delta first landed. Resurrection
	/// takes the last removal in canonical order, and with the whole history in view that can be one past
	/// the replay position, whose snapshot places the entity in a network the replay has not created yet.
	pub fn snapshot_from_history(&self) -> Result<Registry, CrdtError> {
		// The oracle for tests and the simulation. The live registries are never rebuilt from history:
		// every op lands the same whatever order it arrives in, so applying each once is the fold.
		let reachable = self.document.history.ancestors(self.document.head);
		let mut scratch = Document::empty(self.document.peer);
		for delta in self.document.history.iter().filter(|delta| reachable.contains(&delta.id)) {
			scratch.apply_op_with(RegistryTarget::Working, delta.kind.clone(), delta.timestamp, ApplyMode::Idempotent)?;
		}
		Ok(scratch.working_registry)
	}

	pub fn history_len(&self) -> usize {
		self.document.history.len()
	}

	/// Integrate `incoming` retired deltas (in causal order) from another peer. Moves `head` forward
	/// without a new delta when the incoming history extends it, otherwise joins `head` and the
	/// incoming tips with a [`RegistryDelta::Merge`].
	pub fn merge(&mut self, incoming: impl IntoIterator<Item = Delta>) -> Result<MergeOutcome, CrdtError> {
		let length_before = self.document.history.len();

		// Each delta lands on both registries once. Every field, presence included, is last-writer-wins on
		// the delta's timestamp, so the order the batch arrives in does not decide anything.
		let mut absorbed_ids = HashSet::new();
		for delta in incoming {
			if self.document.history.contains(delta.id) {
				continue;
			}
			self.document.apply_op_idempotent(delta.kind.clone(), delta.timestamp)?;
			// Working is snapshot plus hot tail, so a delta lands on both; cloning would promote hot ops.
			self.document.apply_op_with(RegistryTarget::Snapshot, delta.kind.clone(), delta.timestamp, ApplyMode::Idempotent)?;
			absorbed_ids.insert(delta.id);
			self.document.history.push(delta);
		}
		if absorbed_ids.is_empty() {
			return Ok(MergeOutcome::NoOp);
		}

		// A batch that chains off the last delta is already in canonical order, which is every retirement
		// a host sends, and costs nothing to place. Anything else is sorted into place, over all of history.
		// The order matters to the history file and to `Rev` determinism only; the registries do not care.
		let extends = self.document.history.extends_canonically(length_before);
		if !extends {
			self.document.history.canonical_sort();
		}

		let history = &self.document.history;
		let last_before = length_before.checked_sub(1).and_then(|position| history.at(position)).map(|delta| delta.id);
		let parents: Vec<Rev> = if extends && self.document.head == last_before {
			// The chain hangs off `head`, so `head` is its ancestor and the chain's end is the one tip.
			vec![history.at(history.len() - 1).expect("the batch is non-empty").id]
		} else {
			let mut candidates: Vec<Rev> = history.tips().into_iter().filter(|tip| absorbed_ids.contains(tip)).collect();
			candidates.extend(self.document.head);
			let is_dominated = |candidate: Rev| candidates.iter().any(|&other| other != candidate && history.is_ancestor(candidate, other));
			candidates.iter().copied().filter(|&candidate| !is_dominated(candidate)).collect()
		};

		let outcome = match parents.as_slice() {
			[tip] => MergeOutcome::FastForward(*tip),
			_ => {
				let timestamp = self.document.clock.tick();
				let merge = Delta::merge(parents, self.document.peer, timestamp);
				let merge_rev = merge.id;
				// Its parents are tips, so `push` keeps the canonical order without a re-sort.
				self.document.history.push(merge);
				MergeOutcome::Merged(merge_rev)
			}
		};
		self.document.head = outcome.head();

		Ok(outcome)
	}

	/// Closes this peer's open transaction with a [`RegistryDelta::EndTransaction`] marker, so the retirer
	/// can take the ops before it as one unit. `None` when nothing is open: an author's transaction is
	/// open once it has an op past its last marker.
	pub fn end_transaction(&mut self) -> Result<Option<HotOp>, CrdtError> {
		let peer = self.document.peer;
		let open = self
			.document
			.hot_log
			.iter()
			.rev()
			.find(|hot_op| hot_op.timestamp.peer == peer)
			.is_some_and(|hot_op| !matches!(hot_op.op, RegistryDelta::EndTransaction));
		if !open {
			return Ok(None);
		}
		Ok(self.stage_ops([RegistryDelta::EndTransaction])?.pop())
	}

	/// Every author's closed transactions in the hot log, the earliest closed first. An author's ops past
	/// its last marker are its open transaction and never appear here, whoever the author is, so a gesture
	/// in progress stays hot while a later one from someone else retires: every op commutes, so the order
	/// transactions retire in does not bear on the registry.
	pub fn closed_transactions(&self) -> Vec<ClosedTransaction> {
		let mut by_author: HashMap<PeerId, Vec<&HotOp>> = HashMap::new();
		for hot_op in &self.document.hot_log {
			by_author.entry(hot_op.timestamp.peer).or_default().push(hot_op);
		}

		let mut closed = Vec::new();
		for (author, mut ops) in by_author {
			ops.sort_by_key(|hot_op| hot_op.sequence);
			let mut expected = self.document.retired.retired_up_to.get(&author).copied().unwrap_or(HotSequence::NONE).next();
			let mut current = Vec::new();
			let mut contiguous = true;
			for hot_op in ops {
				// An op retired past a gap is not in the log and not missing either.
				while expected < hot_op.sequence && self.document.retired.covers(HotOpId { peer: author, sequence: expected }) {
					expected = expected.next();
				}
				contiguous &= hot_op.sequence == expected;
				expected = hot_op.sequence.next();
				current.push(hot_op.id());
				if matches!(hot_op.op, RegistryDelta::EndTransaction) {
					closed.push(ClosedTransaction {
						author,
						ops: std::mem::take(&mut current),
						closed_at: hot_op.timestamp,
						contiguous,
					});
					contiguous = true;
				}
			}
		}
		closed.sort_by_key(|transaction| transaction.closed_at);
		closed
	}

	/// The subsequence of `closed` that can retire now, in order: a transaction is held back while it
	/// refers to an entity the retired snapshot has never seen and no transaction ahead of it adds,
	/// which is one still sitting in another author's open transaction. Applying it would land a write
	/// on an unknown target, the one thing the registry defers rather than resolves.
	pub fn retirable(&self, closed: &[ClosedTransaction]) -> Vec<ClosedTransaction> {
		let snapshot = &self.document.retired_snapshot;
		let mut nodes: HashSet<NodeId> = snapshot.node_instances.keys().chain(snapshot.removed_nodes.keys()).copied().collect();
		let mut networks: HashSet<NetworkId> = snapshot.networks.keys().chain(snapshot.removed_networks.keys()).copied().collect();
		let by_id: HashMap<HotOpId, &HotOp> = self.document.hot_log.iter().map(|hot_op| (hot_op.id(), hot_op)).collect();

		let mut retirable = Vec::new();
		for transaction in closed {
			let mut nodes_after = nodes.clone();
			let mut networks_after = networks.clone();
			let ops = transaction.ops.iter().filter_map(|id| by_id.get(id)).map(|hot_op| &hot_op.op);
			if ops.clone().all(|op| op_referents_known(op, &mut nodes_after, &mut networks_after)) {
				nodes = nodes_after;
				networks = networks_after;
				retirable.push(transaction.clone());
			}
		}
		retirable
	}
	/// Retire one closed transaction as one interaction: its ops become retired deltas in the author's
	/// order, each keeping its authoring stamp, the marker commits nothing, and the last delta is marked as
	/// the interaction's end. Returns the new revs.
	pub fn retire_transaction(&mut self, transaction: &ClosedTransaction) -> Result<Vec<Rev>, CrdtError> {
		let revs = self.retire_hot_ops(&transaction.ops)?;
		if let Some(&last) = revs.last() {
			self.mark_interaction_end(last);
		}
		Ok(revs)
	}

	/// The hot ops stamped at or before `up_to`, which is what [`retire`](Self::retire) drains. Sent with
	/// the deltas for peers to drop exactly these; the cutoff alone doesn't transfer, a lagging op can
	/// arrive below it afterwards.
	pub fn hot_ops_up_to(&self, up_to: TimeStamp) -> Vec<HotOpId> {
		self.document.hot_log.iter().filter(|hot_op| hot_op.timestamp <= up_to).map(HotOp::id).collect()
	}

	/// Promote the given hot ops into retired deltas, in hot-log order, each keeping the timestamp it was
	/// authored at. Markers commit nothing. Ops not in the log are ignored.
	///
	/// Today: one retired delta per hot op. Coarsening is a future step.
	pub fn retire_hot_ops(&mut self, ids: &[HotOpId]) -> Result<Vec<Rev>, CrdtError> {
		let wanted: HashSet<HotOpId> = ids.iter().copied().collect();
		let (drained, kept): (Vec<HotOp>, Vec<HotOp>) = self.document.hot_log.drain(..).partition(|hot_op| wanted.contains(&hot_op.id()));
		self.document.hot_log = kept;
		self.document.mark_retired(drained.iter().map(HotOp::id));

		let ops = drained
			.into_iter()
			.filter(|hot_op| !matches!(hot_op.op, RegistryDelta::EndTransaction))
			.map(|hot_op| (hot_op.op, Some(hot_op.timestamp)));
		self.commit_ops_authored_at(ops, true)
	}

	/// Promote every hot op stamped at or before `up_to`, whatever its author and whether or not its
	/// transaction is closed. For callers that own the whole log, tests mostly; a session retires by
	/// [`retire_transaction`](Self::retire_transaction).
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
	pub fn annotate_delta(&mut self, rev: Rev, key: &str, value: serde_json::Value) -> bool {
		let timestamp = self.document.clock.tick();
		self.document.history.annotate(rev, key, value, timestamp)
	}

	/// Whether there is a retired commit at `head` that can be undone in the silent zone (a commit
	/// after `last_broadcast_rev`). `head == 0` is the empty history; published commits aren't
	/// silently undoable (that needs a forward reverse-delta op, deferred until transport lands).
	///
	/// The earliest interaction (the document's loaded/created base) is *not* undoable: undoing it would
	/// rewind into the pre-base state, which legacy never offers (opening a document gives an empty undo
	/// history). We detect "head is on the earliest interaction" by walking `head`'s interaction back along
	/// first-parents and checking whether it bottoms out at the root with no earlier interaction boundary to
	/// land on. If so, there is nothing before this interaction to undo to, so undo is disabled.
	pub fn can_undo(&self) -> bool {
		let Some(head) = self.document.head else { return false };
		if self.document.last_broadcast_rev == Some(head) {
			return false;
		}
		self.interaction_start_parent(head).is_some()
	}

	/// Walk the interaction containing `rev` back along first-parents to its first delta, returning the
	/// rev the cursor would rest on after undoing this interaction, or `None` if that is the root (the
	/// earliest interaction, which is not undoable). Mirrors the boundary condition in [`undo`](Self::undo):
	/// stop when the parent is an `interaction_end` boundary or the root.
	fn interaction_start_parent(&self, rev: Rev) -> Option<Rev> {
		let mut current = rev;
		loop {
			let parent = self.document.history.get(current)?.parent?;
			if self.document.history.get(parent).is_some_and(|d| d.is_interaction_end()) {
				return Some(parent);
			}
			current = parent;
		}
	}

	pub fn can_redo(&self) -> bool {
		!self.document.redo_stack.is_empty()
	}

	/// Silent-zone undo of one *interaction*: revert deltas back along first-parents until `head` reaches
	/// the previous `interaction_end` boundary or the root. An interaction is a whole `commit_from_runtime`
	/// batch, so the run reverts together. Its `head` rev goes on the redo stack; the DAG is not rewritten.
	pub fn undo(&mut self) -> Result<Rev, CrdtError> {
		if !self.can_undo() {
			return Err(CrdtError::NothingToUndo);
		}
		let checkpoint = self.document.head.ok_or(CrdtError::NothingToUndo)?;
		// The caller rebuilds the runtime from the rewound registry; until then diff against it directly.
		self.runtime_base = None;

		// Revert this interaction's last delta, then keep going back until `head` rests on the previous
		// interaction's boundary (its `interaction_end` delta) or the root.
		loop {
			let rev = self.document.head.ok_or(CrdtError::NothingToUndo)?;
			let delta = self.document.history.get(rev).ok_or(CrdtError::NotFoundInHistory(rev))?.clone();
			let parent = delta.parent;

			self.document.revert_delta(RegistryTarget::Working, delta.clone())?;
			// Working is snapshot plus hot tail, so rewind both; copying would promote unretired hot ops.
			self.document.revert_delta(RegistryTarget::Snapshot, delta)?;

			self.document.head = parent;

			match parent {
				None => break,
				Some(parent) if self.document.history.get(parent).is_some_and(|d| d.is_interaction_end()) => break,
				Some(_) => {}
			}
		}

		self.document.redo_stack.push(checkpoint);
		Ok(checkpoint)
	}

	/// Redo the most-recently-undone interaction: re-apply every delta from the current `head` forward to
	/// (and including) the checkpoint rev, advancing `head` to it. Collects the forward span by walking
	/// parents back from the checkpoint to `head` (the chain is linear in the silent solo zone).
	pub fn redo(&mut self) -> Result<Rev, CrdtError> {
		let checkpoint = self.document.redo_stack.pop().ok_or(CrdtError::NothingToRedo)?;
		self.runtime_base = None;

		let mut forward = Vec::new();
		let mut cursor = Some(checkpoint);
		while cursor != self.document.head {
			let Some(rev) = cursor else { break };
			let delta = self.document.history.get(rev).ok_or(CrdtError::NotFoundInHistory(rev))?.clone();
			cursor = delta.parent;
			forward.push(delta);
		}

		// Force-apply so each forward value wins the LWW tie against the reverse that undo force-applied
		// at the same timestamp. Symmetric with `revert_delta`.
		for delta in forward.into_iter().rev() {
			self.document.force_apply_op(delta.kind.clone(), delta.timestamp)?;
			// Both zones move together, for the same reason `undo` rewinds both.
			self.document.apply_op_with(RegistryTarget::Snapshot, delta.kind.clone(), delta.timestamp, ApplyMode::Force)?;
		}
		self.document.head = Some(checkpoint);

		Ok(checkpoint)
	}

	/// Build a synthetic linear history whose replay reproduces `registry`. Each op gets a
	/// freshly-ticked clock timestamp and chains to the previous op's `Rev`.
	pub fn bootstrap_from_registry(peer: PeerId, registry: Registry) -> Result<Self, CrdtError> {
		let ops = crate::delta::compute_deltas(&Registry::default(), &registry);
		let mut session = Self::with_peer(peer);
		session.commit_ops(ops, false)?;
		// No hot ops on this path, so the working registry must mirror the freshly-built snapshot.
		session.document.working_registry = session.document.retired_snapshot.clone();
		Ok(session)
	}

	/// Retired deltas in append order, which is a valid replay order (parents before children).
	pub fn history(&self) -> impl Iterator<Item = &Delta> + '_ {
		self.document.history.iter()
	}

	/// Revs sampled at exponentially growing distances behind `head`, for a remote peer to locate what
	/// this session is missing (see [`History::sample_chain`]).
	pub fn known_revs(&self) -> Vec<Rev> {
		self.document.head.map(|head| self.document.history.sample_chain(head)).unwrap_or_default()
	}

	/// Retired deltas not reachable from `known`, in replay order.
	pub fn deltas_unknown_to(&self, known: impl IntoIterator<Item = Rev>) -> Vec<&Delta> {
		self.document.history.deltas_unknown_to(known)
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
	/// bytes. Walks current resources plus each delta's `AddResource`/`RemoveResource` snapshot.
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

	/// How many hot ops this peer has authored. Carried across a reload; no sequence is spent twice.
	pub fn next_hot_sequence(&self) -> HotSequence {
		self.document.next_hot_sequence
	}

	/// Restore the authored-op count after a load, keeping a fresh op off a spent sequence. Raises only:
	/// replaying a persisted hot log afterwards cannot lower it.
	pub fn restore_hot_sequence(&mut self, sequence: HotSequence) {
		self.document.next_hot_sequence = self.document.next_hot_sequence.max(sequence);
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MergeOutcome {
	NoOp,
	FastForward(Rev),
	Merged(Rev),
}

impl MergeOutcome {
	/// The head after the merge, or `None` when nothing changed.
	pub fn head(self) -> Option<Rev> {
		match self {
			Self::NoOp => None,
			Self::FastForward(rev) | Self::Merged(rev) => Some(rev),
		}
	}
}

/// One live op in the hot zone. Carries only enough to drive live LWW; no parents (transient),
/// no Rev (not content-addressed in the durable DAG). GC'd at retirement.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HotOp {
	pub op: RegistryDelta,
	pub timestamp: TimeStamp,
	/// Position in its author's run, from 1 with no gaps. A watermark over it therefore means a contiguous
	/// prefix; one over the Lamport counter cannot, since that skips on observing a higher remote stamp.
	pub sequence: HotSequence,
}

impl HotOp {
	/// Identifies the op for retirement, which tracks a contiguous prefix per author.
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

	/// The next position in the run.
	pub fn next(self) -> Self {
		Self(self.0 + 1)
	}
}

/// One hot op's author and position in its run. Retirement names promoted ops by these, which tells a
/// receiver which prefix history covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct HotOpId {
	pub peer: PeerId,
	pub sequence: HotSequence,
}

/// Which hot ops history already covers: `retired_up_to` is each author's gap-free retired prefix, and
/// `unretired` the retired ops past it. Replicated, letting a peer catching up separate an op already in
/// history from one still owed to it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetiredHotOps {
	pub retired_up_to: HashMap<PeerId, HotSequence>,
	/// Retired ops sitting past their author's prefix, as inclusive runs. A gap that never fills pins the
	/// prefix and everything later from that author lands here, so runs keep this proportional to the
	/// number of gaps instead of the number of ops.
	pub retired_beyond: HashMap<PeerId, Vec<(HotSequence, HotSequence)>>,
}

/// Sort and coalesce runs, joining any that touch or abut.
fn coalesce(runs: &mut Vec<(HotSequence, HotSequence)>) {
	runs.sort_unstable();

	let mut merged: Vec<(HotSequence, HotSequence)> = Vec::with_capacity(runs.len());
	for &(start, end) in runs.iter() {
		match merged.last_mut() {
			Some((_, last_end)) if start <= last_end.next() => *last_end = (*last_end).max(end),
			_ => merged.push((start, end)),
		}
	}

	*runs = merged;
}

impl RetiredHotOps {
	/// Whether history already holds this hot op.
	pub fn covers(&self, id: HotOpId) -> bool {
		if self.retired_up_to.get(&id.peer).is_some_and(|&through| id.sequence <= through) {
			return true;
		}

		self.retired_beyond
			.get(&id.peer)
			.is_some_and(|runs| runs.iter().any(|&(start, end)| id.sequence >= start && id.sequence <= end))
	}

	/// Take on `remote`'s coverage as well as this one's.
	pub fn absorb(&mut self, remote: &Self) {
		for (&peer, &remote_through) in &remote.retired_up_to {
			let through = self.retired_up_to.entry(peer).or_default();
			*through = (*through).max(remote_through);
		}
		for (&peer, runs) in &remote.retired_beyond {
			self.retired_beyond.entry(peer).or_default().extend(runs.iter().copied());
		}

		self.compact();
	}

	/// Record newly retired ops.
	pub fn extend(&mut self, retired: impl IntoIterator<Item = HotOpId>) {
		for id in retired {
			self.retired_beyond.entry(id.peer).or_default().push((id.sequence, id.sequence));
		}

		self.compact();
	}

	/// Fold runs that continue their author's prefix into `retired_up_to`, leaving only those past a gap.
	fn compact(&mut self) {
		for (&peer, runs) in &mut self.retired_beyond {
			coalesce(runs);

			let mut through = self.retired_up_to.get(&peer).copied().unwrap_or(HotSequence::NONE);
			while runs.first().is_some_and(|&(start, _)| start <= through.next()) {
				let (_, end) = runs.remove(0);
				through = through.max(end);
			}

			if through != HotSequence::NONE {
				self.retired_up_to.insert(peer, through);
			}
		}

		self.retired_beyond.retain(|_, runs| !runs.is_empty());
	}
}

#[derive(Debug, thiserror::Error)]
pub enum CrdtError {
	#[error("Target node {0} does not exist")]
	TargetNodeDoesNotExist(NodeId),
	#[error("Network {0} does not exist")]
	NetworkDoesNotExist(NetworkId),
	#[error("Resource {0:?} does not exist")]
	ResourceDoesNotExist(ResourceId),
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
	/// PeerId is already registered to a different UserId.
	#[error("Peer {0:?} is already registered to a different user")]
	PeerRegistrationConflict(PeerId),
	#[error("Delta stored under {stored} hashes to {expected}")]
	RevMismatch { stored: Rev, expected: Rev },
}

/// Whether every entity `op` writes to or refers to is in the sets, adding what the op itself brings into
/// being. Mirrors the cases in which [`Document::apply_op_with`] errors on an entity never seen.
fn op_referents_known(op: &RegistryDelta, nodes: &mut HashSet<NodeId>, networks: &mut HashSet<NetworkId>) -> bool {
	let input_known = |input: &crate::NodeInput, nodes: &HashSet<NodeId>| match input {
		crate::NodeInput::Node { id, .. } => nodes.contains(id),
		_ => true,
	};
	match op {
		RegistryDelta::AddNode { id, node } => {
			let known = networks.contains(&node.network);
			nodes.insert(*id);
			known
		}
		RegistryDelta::RemoveNode { id, .. } => {
			nodes.insert(*id);
			true
		}
		RegistryDelta::SetNodeInputs { id, inputs } => nodes.contains(id) && inputs.iter().all(|slot| input_known(&slot.input, nodes)),
		RegistryDelta::ChangeNodeInput { id, new_input, .. } => nodes.contains(id) && input_known(new_input, nodes),
		RegistryDelta::SetNodeImplementation { id, implementation } => {
			nodes.contains(id)
				&& match implementation {
					Implementation::Network(network) => networks.contains(network),
					Implementation::ProtoNode(_) => true,
				}
		}
		RegistryDelta::ChangeNodeAttribute { id, .. } | RegistryDelta::ChangeNodeInputAttribute { id, .. } => nodes.contains(id),
		RegistryDelta::SetNetworkExport { id, export, .. } => networks.contains(id) && export.as_ref().is_none_or(|input| input_known(input, nodes)),
		RegistryDelta::AddNetwork { id, .. } | RegistryDelta::RemoveNetwork { id, .. } => {
			networks.insert(*id);
			true
		}
		RegistryDelta::ChangeNetworkAttribute { id, .. } => networks.contains(id),
		_ => true,
	}
}

/// An author's closed transaction sitting in the hot log, as [`Session::closed_transactions`] lists them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClosedTransaction {
	pub author: PeerId,
	/// Every op of the transaction in the author's order, the closing marker last.
	pub ops: Vec<HotOpId>,
	/// When the author closed it: the marker's stamp.
	pub closed_at: TimeStamp,
	/// Whether every op from the author's retired frontier through the marker is here. One with a gap is
	/// waiting on a re-announcement; retiring it anyway commits what arrived.
	pub contiguous: bool,
}
