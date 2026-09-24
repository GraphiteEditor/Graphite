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
			document: Document {
				working_registry: Registry::default(),
				retired_snapshot: Registry::default(),
				history: History::new(),
				hot_log: Vec::new(),
				retired: RetiredHotOps::default(),
				head: None,
				redo_stack: Vec::new(),
				clock: LamportClock::new(peer),
				peer,
				last_broadcast_rev: None,
				next_node_counter: 0,
				next_hot_sequence: HotSequence::NONE,
				refold_owed: false,
				working_rederivations: 0,
				refolds: 0,
				fold: None,
			},
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

			// The reverse must read the pre-op value of a target that may have been concurrently removed.
			self.document.ensure_referenced_exist(target, &op)?;
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
				hot_log: Vec::new(),
				retired: RetiredHotOps::default(),
				head,
				redo_stack,
				clock,
				peer,
				last_broadcast_rev: None,
				next_node_counter,
				next_hot_sequence: HotSequence::NONE,
				refold_owed: false,
				working_rederivations: 0,
				refolds: 0,
				fold: None,
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

	/// The hot ops a `retire(up_to)` call would drain. Sent with the deltas for peers to drop exactly
	/// these; the cutoff alone doesn't transfer, a lagging op can arrive below it afterwards.
	pub fn hot_ops_up_to(&self, up_to: TimeStamp) -> Vec<HotOpId> {
		self.document.hot_log[..self.retirement_prefix(up_to)].iter().map(HotOp::id).collect()
	}

	/// How much of the hot log `retire(up_to)` drains: everything through the last op stamped at or
	/// before `up_to`. A prefix of the log rather than the ops under the cutoff, so retirement commits
	/// them in the order the working registry applied them and the snapshot folds to the same values
	/// without a refold. An op stamped later that sits earlier in the log goes with the prefix.
	fn retirement_prefix(&self, up_to: TimeStamp) -> usize {
		self.document.hot_log.iter().rposition(|hot_op| hot_op.timestamp <= up_to).map_or(0, |last| last + 1)
	}

	/// Drop hot ops another peer has retired without retiring them locally. Refolds like
	/// [`absorb_retired_marks`](Self::absorb_retired_marks).
	pub fn discard_hot_ops(&mut self, retired: &[HotOpId]) -> Result<(), CrdtError> {
		let before = self.document.hot_log.len();
		let retired_ids: HashSet<HotOpId> = retired.iter().copied().collect();
		self.document.hot_log.retain(|hot_op| !retired_ids.contains(&hot_op.id()));
		// How a peer that did not retire these learns they are in history now.
		self.document.mark_retired(retired.iter().copied());

		if before != self.document.hot_log.len() {
			self.refold_registries()?;
		}

		Ok(())
	}

	/// Which hot ops history already covers, for a peer catching up. See [`RetiredHotOps`].
	pub fn retired_marks(&self) -> &RetiredHotOps {
		&self.document.retired
	}

	/// See [`RetiredHotOps::absorb`]. A drop owes a refold, since the working registry holds the op's
	/// effect in hot-log order where the snapshot holds it in canonical order.
	pub fn absorb_retired_marks(&mut self, remote: &RetiredHotOps) -> Result<(), CrdtError> {
		if self.document.absorb_retired(remote) {
			self.refold_registries()?;
		}

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
		// The oracle for tests and the simulation; the live path folds in place through `refold_registries`.
		self.clone().document.fold_snapshot_from_history()
	}

	/// Rebuild both registries from canonical history, then re-layer the hot tail. A full replay; callers
	/// check it is needed first.
	fn refold_registries(&mut self) -> Result<(), CrdtError> {
		self.document.refolds += 1;
		// Cleared only on the way out. The deltas are in history either way, and a failure here leaves the
		// registries derived from an older one until something retries.
		self.document.refold_owed = true;
		self.document.retired_snapshot = self.document.fold_snapshot_from_history()?;

		// One left unapplicable is kept, not dropped: a later delta or hot op can still supply its referent.
		let previous = std::mem::replace(&mut self.document.working_registry, self.document.retired_snapshot.clone());
		let mut failure = None;
		for hot_op in std::mem::take(&mut self.document.hot_log) {
			if let Err(error) = self.document.replay_hot_op(hot_op.clone()) {
				self.document.hot_log.push(hot_op);
				failure = failure.or(Some(error));
			}
		}

		// A mirror of the working registry follows it op by op and cannot follow a rederivation, so it
		// is told when one produced something the ops alone do not account for. Usually they do: dropping
		// a hot op that history now covers refolds to the same values, just stamped differently.
		if !previous.value_equal(&self.document.working_registry) {
			self.document.working_rederivations += 1;
		}

		if let Some(error) = failure {
			return Err(error);
		}

		self.document.refold_owed = false;
		Ok(())
	}

	/// How many times a refold has left the working registry with different values than the ops applied to
	/// it would have. A caller mirroring the registry op by op compares this before and after, and rebuilds
	/// its mirror wholesale when it moved.
	pub fn working_rederivations(&self) -> u64 {
		self.document.working_rederivations
	}

	/// How many refolds have run, whatever they produced.
	pub fn refolds(&self) -> u64 {
		self.document.refolds
	}

	pub fn history_len(&self) -> usize {
		self.document.history.len()
	}

	/// Integrate `incoming` retired deltas (in causal order) from another peer. Moves `head` forward
	/// without a new delta when the incoming history extends it, otherwise joins `head` and the
	/// incoming tips with a [`RegistryDelta::Merge`].
	pub fn merge(&mut self, incoming: impl IntoIterator<Item = Delta>) -> Result<MergeOutcome, CrdtError> {
		let length_before = self.document.history.len();

		// Each delta enters history before the next is applied, so a later delta in the batch that
		// targets something an earlier one removed can resurrect it.
		let mut absorbed_ids = HashSet::new();
		let mut arrival_order = Vec::new();
		for delta in incoming {
			if self.document.history.contains(delta.id) {
				continue;
			}
			arrival_order.push(delta.id);
			self.document.apply_op_idempotent(delta.kind.clone(), delta.timestamp)?;
			// Working is snapshot plus hot tail, so a delta lands on both; cloning would promote hot ops.
			self.document.apply_op_with(RegistryTarget::Snapshot, delta.kind.clone(), delta.timestamp, ApplyMode::Idempotent)?;
			absorbed_ids.insert(delta.id);
			self.document.history.push(delta);
		}
		if absorbed_ids.is_empty() {
			// Nothing new, but a refold left owed by an earlier failure is still the only way the registries
			// catch up with the history they are derived from.
			if self.document.refold_owed {
				self.refold_registries()?;
			}

			return Ok(MergeOutcome::NoOp);
		}

		// A batch that chains off the last delta is already in canonical order, which is every retirement
		// a host sends, and costs nothing to place. Anything else is sorted into place, over all of history.
		let extends = self.document.history.extends_canonically(length_before);
		if !extends {
			self.document.history.canonical_sort();
		}

		// The loop folds in arrival order, and concurrent ops do not all commute: a remove and a change to
		// the same target resolve by whichever lands second.
		let folded_in_canonical_order = extends || self.document.history.iter().skip(length_before).map(|delta| delta.id).eq(arrival_order.iter().copied());

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

		// After `head`, since the fold is over its ancestry.
		if !folded_in_canonical_order || self.document.refold_owed {
			self.refold_registries()?;
		}

		Ok(outcome)
	}

	/// Promote the hot-log prefix through the last op stamped at or before `up_to` into retired deltas,
	/// each keeping the timestamp it was authored at. See [`retirement_prefix`](Self::retirement_prefix)
	/// for why the prefix and not the ops under the cutoff.
	///
	/// Today: one retired delta per hot op. Coarsening is a future step.
	pub fn retire(&mut self, up_to: TimeStamp) -> Result<Vec<Rev>, CrdtError> {
		// Hot-log order is causal, so the deltas commit in an order their references survive.
		let prefix = self.retirement_prefix(up_to);
		let drained: Vec<HotOp> = self.document.hot_log.drain(..prefix).collect();
		self.document.mark_retired(drained.iter().map(HotOp::id));

		let revs = self.commit_ops_authored_at(drained.into_iter().map(|hot_op| (hot_op.op, Some(hot_op.timestamp))), true)?;

		Ok(revs)
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
	/// test can observe a resurrection failure (e.g. `NotFoundInHistory`).
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
	#[error("Input index {0} out of bounds")]
	InputIndexOutOfBounds(usize),
	#[error("Export slot index {0} out of bounds")]
	ExportSlotOutOfBounds(u32),
	#[error("Delta {0} not found in history")]
	NotFoundInHistory(Rev),
	#[error("No history entry resurrects node {0}")]
	NodeNotInHistory(NodeId),
	#[error("No history entry resurrects network {0}")]
	NetworkNotInHistory(NetworkId),
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
