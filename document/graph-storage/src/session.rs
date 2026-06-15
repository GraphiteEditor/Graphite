use crate::from_runtime;
use crate::{ApplyMode, Delta, Document, History, LamportClock, NetworkId, NodeId, NodeMetadataSource, PeerId, Registry, RegistryDelta, RegistryTarget, ResourceEntry, Rev, TimeStamp, UserId};
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
				head: None,
				redo_stack: Vec::new(),
				clock: LamportClock::new(peer),
				peer,
				last_broadcast_rev: None,
				next_node_counter: 0,
			},
			remote_tips: HashMap::new(),
		}
	}

	pub fn peer(&self) -> PeerId {
		self.document.peer
	}

	pub fn registry(&self) -> &Registry {
		&self.document.working_registry
	}

	/// Diff the current registry against a fresh conversion of `network`, then commit each emitted
	/// op as its own `Delta` on the local chain. One `clock.tick()` per op (strictly causal within
	/// a commit). Returns the new `Rev`s in commit order (empty if nothing changed) plus the
	/// proto-node declaration bytes the conversion extracted, keyed by content hash, for the caller
	/// to persist into its byte store (`graph-storage` itself is byte-unaware).
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
	) -> Result<(Vec<HotOp>, from_runtime::DeclarationBytes), CommitError> {
		let conversion = Registry::convert_from_runtime(network, metadata, resources, self.document.peer)?;
		let ops = crate::delta::compute_deltas(&self.document.working_registry, &conversion.registry);
		let hot_ops = self.stage_ops(ops)?;
		Ok((hot_ops, conversion.declaration_bytes))
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

		// Caller contract: this runs on a throwaway export clone with no unretired hot ops, so the
		// working registry equals the snapshot. Overwriting working with the advanced snapshot below
		// would otherwise drop hot-zone edits, so reject the call rather than corrupt state.
		if !self.document.hot_log.is_empty() {
			return Err(CrdtError::HotLogNotEmpty);
		}

		let revs = self.commit_ops(ops, false)?;
		self.document.working_registry = self.document.retired_snapshot.clone();
		Ok(revs)
	}

	/// Apply each op as a hot op with a freshly-ticked timestamp, returning the staged frames in
	/// order. Each tick is strictly later than the last, so the final frame carries the latest
	/// timestamp, which is what the caller passes to `retire`.
	///
	/// The peer's first contribution is preceded by a `RegisterPeer` op, so the device's
	/// `PeerId → UserId` mapping is established (and, under causal delivery, observed by other peers)
	/// before any of its edits. A no-op batch doesn't register — registration rides a real edit.
	fn stage_ops(&mut self, ops: impl IntoIterator<Item = RegistryDelta>) -> Result<Vec<HotOp>, CrdtError> {
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
			let hot_op = HotOp {
				op,
				timestamp: self.document.clock.tick(),
			};
			self.document.apply_hot_op(hot_op.clone())?;
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
		let target = RegistryTarget::Snapshot;
		let ops = ops.into_iter();
		let mut produced = Vec::with_capacity(ops.size_hint().0);

		for op in ops {
			// A new edit abandons any undone-forward branch: those revs stay in the DAG but are no
			// longer reachable via redo. (Mirrors the legacy editor clearing its redo history on
			// commit.) Done on the first real op so a no-op commit doesn't silently disable redo.
			if produced.is_empty() {
				self.document.redo_stack.clear();
			}

			let reverse = self.document.compute_reverse_delta(target, &op)?;
			let timestamp = self.document.clock.tick();
			let parent = self.document.head;
			let author = self.document.peer;

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
	/// clock past every observed timestamp but does not re-apply ops. `history` is taken in its
	/// on-disk append order (parents before children) and indexed in that order.
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
				head,
				redo_stack,
				clock,
				peer,
				last_broadcast_rev: None,
				next_node_counter,
			},
			remote_tips: HashMap::new(),
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

	/// Apply a hot op without going through the broadcast stream.
	pub fn apply_hot_op(&mut self, hot_op: HotOp) -> Result<(), CrdtError> {
		self.document.apply_hot_op(hot_op)
	}

	/// Replay a persisted hot op. Idempotent on structural ops, suitable for crash recovery
	/// where the registry may already reflect the op's effect from a prior retired snapshot.
	pub fn replay_hot_op(&mut self, hot_op: HotOp) -> Result<(), CrdtError> {
		self.document.replay_hot_op(hot_op)
	}

	/// Promote hot ops with timestamp `≤ up_to` into retired deltas, re-applied with fresh
	/// retirement timestamps so LWW arms bump field timestamps to `T_retire`.
	///
	/// Today: one retired delta per hot op. Coarsening is a future step.
	pub fn retire(&mut self, up_to: TimeStamp) -> Result<Vec<Rev>, CrdtError> {
		let mut drained = Vec::new();
		let mut remaining = Vec::with_capacity(self.document.hot_log.len());
		for hot_op in self.document.hot_log.drain(..) {
			if hot_op.timestamp <= up_to {
				drained.push(hot_op);
			} else {
				remaining.push(hot_op);
			}
		}
		self.document.hot_log = remaining;

		self.commit_ops(drained.into_iter().map(|hot_op| hot_op.op), true)
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

	/// Silent-zone undo of one *interaction*: revert deltas walking `head` back along first-parents until
	/// it reaches the previous interaction boundary (a delta marked `interaction_end`) or the empty root. One
	/// interaction spans several deltas (one `commit_from_runtime` batch), so undo reverts the whole run,
	/// not a single delta — matching the legacy per-interaction undo granularity. The undone interaction's
	/// `head` rev is pushed onto the redo stack. Reflog semantics: the DAG is never rewritten.
	pub fn undo(&mut self) -> Result<Rev, CrdtError> {
		if !self.can_undo() {
			return Err(CrdtError::NothingToUndo);
		}
		let checkpoint = self.document.head.ok_or(CrdtError::NothingToUndo)?;

		// Revert this interaction's last delta, then keep going back until `head` rests on the previous
		// interaction's boundary (its `interaction_end` delta) or the root.
		loop {
			let rev = self.document.head.ok_or(CrdtError::NothingToUndo)?;
			let delta = self.document.history.get(rev).ok_or(CrdtError::NotFoundInHistory(rev))?.clone();
			let parent = delta.parent;

			self.document.revert_delta(RegistryTarget::Working, delta)?;
			self.document.head = parent;

			match parent {
				None => break,
				Some(parent) if self.document.history.get(parent).is_some_and(|d| d.is_interaction_end()) => break,
				Some(_) => {}
			}
		}

		// Undo runs with an empty hot log, so keep the retired snapshot in lockstep with the rewound
		// working registry (the next interaction's reverses are computed against it).
		self.document.retired_snapshot = self.document.working_registry.clone();
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

		// Force-apply so each forward value wins the LWW tie against the reverse that undo force-applied
		// at the same timestamp. Symmetric with `revert_delta`.
		for delta in forward.into_iter().rev() {
			self.document.force_apply_op(delta.kind.clone(), delta.timestamp)?;
		}
		self.document.head = Some(checkpoint);

		// Redo runs with an empty hot log; keep the retired snapshot in lockstep with the working registry.
		self.document.retired_snapshot = self.document.working_registry.clone();
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

		for delta in self.document.history.iter() {
			match &delta.kind {
				RegistryDelta::AddResource { entry, .. } => hashes.extend(entry.hash),
				RegistryDelta::RemoveResource { snapshot, .. } => hashes.extend(snapshot.hash),
				_ => {}
			}
		}

		hashes
	}

	pub fn hot_log(&self) -> &[HotOp] {
		&self.document.hot_log
	}

	pub fn head_rev(&self) -> Option<Rev> {
		self.document.head
	}

	pub fn redo_stack(&self) -> &[Rev] {
		&self.document.redo_stack
	}

	pub fn next_node_counter(&self) -> u64 {
		self.document.next_node_counter
	}
}

/// Errors from `Session::commit_from_runtime`.
#[derive(Debug, thiserror::Error)]
pub enum CommitError {
	#[error("Failed to convert runtime network: {0}")]
	Conversion(#[from] from_runtime::ConversionError),
	#[error("Failed to apply commit: {0}")]
	Crdt(#[from] CrdtError),
}

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
	#[error("Operation requires an empty hot log")]
	HotLogNotEmpty,
	#[error("Delta stored under {stored} hashes to {expected}")]
	RevMismatch { stored: Rev, expected: Rev },
}
