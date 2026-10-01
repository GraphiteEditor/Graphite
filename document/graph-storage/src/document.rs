use std::collections::btree_map::Entry;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use crate::{
	Attributes, CrdtError, Delta, ExportSlot, History, HotOp, HotOpId, HotSequence, Implementation, InputSlot, LamportClock, MAX_EXPORT_SLOTS, MAX_INPUT_SLOTS, Network, NetworkId, Node, NodeId,
	NodeInput, PeerId, PeerRegistration, Registry, RegistryDelta, ResourceEntry, ResourceId, Rev, SettledHotOps, SourceValue, TimeStamp, Tombstone, UserId, apply_attribute_delta,
};

#[derive(Clone, Debug)]
pub struct Document {
	/// Working registry: retired state with the current hot ops applied on top. This is what live
	/// reads and `registry()` observe.
	pub(crate) working_registry: Registry,
	/// Live broadcast stream, applied to the `working_registry` on receive, GC'd at retirement.
	/// Persisted for crash recovery so in-flight unretired work survives editor restarts.
	pub(crate) hot_log: Vec<HotOp>,
	/// The stamps of the hot log's ops, so a re-announced op is recognised without a scan.
	pub(crate) hot_timestamps: HashSet<TimeStamp>,
	/// Which hot ops are done with, retired into history or taken back by their author, so a late copy is dropped;
	/// retired deltas keep no link back to their hot ops.
	pub(crate) settled: SettledHotOps,
	/// The registry as of the last retirement, with no un-retired hot ops applied. Retirement computes
	/// each delta's `reverse` against this (so LWW reverses capture the true pre-op value, not the
	/// hot-polluted working state) and advances it. It agrees with the working registry by value whenever
	/// the hot log is empty.
	pub(crate) retired_snapshot: Registry,
	/// User's cursor in their local history chain. `None` on an empty document (no commits yet).
	pub(crate) head: Option<Rev>,
	/// Retired delta DAG in topological (append) order. See [`History`](crate::History).
	pub(crate) history: History,
	/// Revs undone past (most-recent last), so `redo` can re-apply them. Local-view state the DAG can't
	/// recover (a parent may have several children). A new edit while non-empty clears it.
	pub(crate) redo_stack: Vec<Rev>,
	pub(crate) clock: LamportClock,
	pub(crate) peer: PeerId,
	/// The person behind this peer; what `RegisterPeer` records for it.
	pub(crate) user: UserId,
	/// Latest retired commit on the local chain that has been broadcast to at least one peer.
	/// Commits after this can be rewritten silently; commits at or before this are published
	/// and require forward reverse-delta ops to undo. `None` means nothing broadcast yet.
	pub(crate) last_broadcast_rev: Option<Rev>,
	/// Shared-monotonic counter feeding `next_node_id`. Bumped on every mint regardless of which
	/// peer is calling; collision avoidance comes from hashing `(self.peer, counter)`, so two peers
	/// reading the same counter still produce distinct IDs.
	pub(crate) next_node_counter: u64,
	/// Counts this peer's own hot ops, so each carries its position in a gap-free run. See [`HotOp::sequence`].
	pub(crate) next_hot_sequence: HotSequence,
}

impl Document {
	/// An empty document for `peer`, at the origin of its clock.
	pub(crate) fn empty(peer: PeerId, user: UserId) -> Self {
		Self {
			working_registry: Registry::default(),
			retired_snapshot: Registry::default(),
			history: History::new(),
			hot_log: Vec::new(),
			hot_timestamps: HashSet::new(),
			settled: SettledHotOps::default(),
			head: None,
			redo_stack: Vec::new(),
			clock: LamportClock::new(peer),
			peer,
			user,
			last_broadcast_rev: None,
			next_node_counter: 0,
			next_hot_sequence: HotSequence::NONE,
		}
	}

	/// Mint a fresh `NodeId` scoped to this document's peer. The 64-bit ID is `blake3(peer, counter)`
	/// truncated; the counter is shared across peers and persisted with the document.
	pub fn next_node_id(&mut self) -> NodeId {
		self.next_node_counter += 1;
		let mut hasher = blake3::Hasher::new();
		postcard::to_io(&(self.peer, self.next_node_counter), &mut hasher).expect("(PeerId, counter) must serialize");
		let digest = hasher.finalize();
		let mut truncated = [0u8; 8];
		truncated.copy_from_slice(&digest.as_bytes()[..8]);
		NodeId(u64::from_le_bytes(truncated))
	}

	/// Put back what a delta overwrote in the retired snapshot (silent-zone undo), leaving it as folding history up
	/// to the delta's parent would. The working registry follows: it is the same registry when nothing is hot.
	pub(crate) fn revert_delta(&mut self, delta: &Delta) {
		crate::prior::restore(&mut self.retired_snapshot, &delta.reverse);
		if self.hot_log.is_empty() {
			crate::prior::restore(&mut self.working_registry, &delta.reverse);
		}
	}

	/// Apply a locally staged op: LWW into the registry, append to the hot log, leave history and `head`
	/// alone. Skips the retirement check, so staging never silently drops local work; outside callers use
	/// [`Session::apply_hot_op`](crate::Session::apply_hot_op).
	pub(crate) fn apply_hot_op(&mut self, hot_op: HotOp) -> Result<(), CrdtError> {
		self.apply_op(hot_op.op.clone(), hot_op.timestamp)?;
		self.hot_timestamps.insert(hot_op.timestamp);
		self.hot_log.push(hot_op);
		Ok(())
	}

	/// Replay a hot op recovered from persisted state or received from a peer. Replaying one already
	/// reflected in the registry changes nothing.
	pub fn replay_hot_op(&mut self, hot_op: HotOp) -> Result<(), CrdtError> {
		// Re-adding a settled op would leave a hot log entry no retirement will ever name.
		if self.settled.covers(hot_op.id()) {
			return Ok(());
		}
		// A re-announcement of a held op must not become a second copy.
		if self.hot_timestamps.contains(&hot_op.timestamp) {
			return Ok(());
		}
		// Replaying our own ops is what carries the sequence counter across a reload.
		if hot_op.timestamp.peer == self.peer {
			self.next_hot_sequence = self.next_hot_sequence.max(hot_op.sequence);
		}

		self.apply_op_idempotent(hot_op.op.clone(), hot_op.timestamp)?;
		self.hot_timestamps.insert(hot_op.timestamp);
		self.hot_log.push(hot_op);
		Ok(())
	}

	/// Record newly settled hot ops, dropping any the hot log still holds. Returns what the dropped ops named.
	pub(crate) fn mark_settled(&mut self, settled: impl IntoIterator<Item = HotOpId>) -> crate::Touched {
		self.settled.extend(settled);
		self.drop_settled_hot_ops()
	}

	/// Take on a peer's marks as well as this peer's. Returns what the dropped ops named.
	pub(crate) fn absorb_settled(&mut self, remote: &SettledHotOps) -> crate::Touched {
		self.settled.absorb(remote);
		self.drop_settled_hot_ops()
	}

	/// Take back hot ops for good, re-deriving the working registry without them, since a commuting op
	/// cannot be undone in place. Returns what the ops named and the ops themselves.
	pub(crate) fn retract_hot_ops(&mut self, ids: &[HotOpId]) -> (crate::Touched, Vec<HotOp>) {
		let wanted: HashSet<HotOpId> = ids.iter().copied().collect();
		let taken: Vec<HotOp> = self.hot_log.extract_if(.., |hot_op| wanted.contains(&hot_op.id())).collect();
		taken.iter().for_each(|hot_op| _ = self.hot_timestamps.remove(&hot_op.timestamp));
		self.settled.extend(ids.iter().copied());

		let mut touched = crate::Touched::default();
		for hot_op in &taken {
			touched.record(&hot_op.op);
		}
		if !taken.is_empty() {
			self.rebuild_working();
		}
		(touched, taken)
	}

	/// Bring the stamp index back in line after the hot log was filtered as a whole.
	pub(crate) fn resync_hot_timestamps(&mut self) {
		self.hot_timestamps = self.hot_log.iter().map(|hot_op| hot_op.timestamp).collect();
	}

	/// Re-derive the working registry as the retired snapshot plus every hot op, for a change that cannot
	/// be applied in place. O(N + L), for rare paths only.
	pub(crate) fn rebuild_working(&mut self) {
		self.working_registry = self.retired_snapshot.clone();
		for hot_op in std::mem::take(&mut self.hot_log) {
			// Nothing is deferred, so this cannot fail on a referent; don't strand the op over anything else.
			let _ = self.apply_op_idempotent(hot_op.op.clone(), hot_op.timestamp);
			self.hot_log.push(hot_op);
		}
	}

	/// Drop settled hot ops and re-derive the working registry without them. Whether an op retired or was taken
	/// back needs no telling apart: a retired op's effect comes back through its delta, a retracted one's stays gone.
	fn drop_settled_hot_ops(&mut self) -> crate::Touched {
		let settled = &self.settled;
		let dropped: Vec<HotOp> = self.hot_log.extract_if(.., |hot_op| settled.covers(hot_op.id())).collect();
		let mut touched = crate::Touched::default();
		if !dropped.is_empty() {
			dropped.iter().for_each(|hot_op| touched.record(&hot_op.op));
			self.resync_hot_timestamps();
			self.rebuild_working();
		}
		touched
	}

	/// Apply a retired commit and record it in history.
	pub fn apply_delta(&mut self, delta: Delta) -> Result<(), CrdtError> {
		for parent in delta.all_parents() {
			if !self.history.contains(parent) {
				return Err(CrdtError::NotFoundInHistory(parent));
			}
		}
		self.apply_op_idempotent(delta.kind.clone(), delta.timestamp)?;
		self.history.push(delta);
		Ok(())
	}

	/// The registry an apply reads and writes, resolved from the explicit [`RegistryTarget`].
	fn registry_mut(&mut self, target: RegistryTarget) -> &mut Registry {
		match target {
			RegistryTarget::Working => &mut self.working_registry,
			RegistryTarget::Snapshot => &mut self.retired_snapshot,
		}
	}

	pub(crate) fn registry_ref(&self, target: RegistryTarget) -> &Registry {
		match target {
			RegistryTarget::Working => &self.working_registry,
			RegistryTarget::Snapshot => &self.retired_snapshot,
		}
	}

	/// A fresh local edit against the working registry; see [`ApplyMode::Live`].
	pub(crate) fn apply_op(&mut self, op: RegistryDelta, timestamp: TimeStamp) -> Result<(), CrdtError> {
		self.apply_op_with(RegistryTarget::Working, op, timestamp, ApplyMode::Live)
	}

	/// An op from a peer or from persisted state, against the working registry.
	pub(crate) fn apply_op_idempotent(&mut self, op: RegistryDelta, timestamp: TimeStamp) -> Result<(), CrdtError> {
		self.apply_op_with(RegistryTarget::Working, op, timestamp, ApplyMode::Idempotent)
	}

	/// Applies one op. Every field of the registry is last-writer-wins on a timestamp, whether an entity
	/// exists included, so the registry folds to the same values whatever order the ops land in:
	///
	/// - An addition writes every field of the entity at its stamp, and is evidence it exists from then.
	/// - A removal is evidence the entity does not exist from its stamp. Its content stays as a tombstone,
	///   and the removal's snapshot folds in like any other write.
	/// - A write, or a reference from another entity's op, is evidence the entity exists at its stamp, so it
	///   revives an older removal; an older write lands on the tombstone, for a later revival to see.
	/// - A write to an entity never seen lands on a placeholder until its addition arrives; see [`write()`].
	pub(crate) fn apply_op_with(&mut self, target: RegistryTarget, op: RegistryDelta, timestamp: TimeStamp, mode: ApplyMode) -> Result<(), CrdtError> {
		// Advance the local clock past every observed op, including ones that subsequently no-op or
		// error. Observation is about causality knowledge, not about whether the op took effect.
		self.clock.observe(timestamp);

		let live = mode == ApplyMode::Live;

		let registry = self.registry_mut(target);
		match op {
			RegistryDelta::AddNode { id, node } => {
				if live && registry.node_instances.contains_key(&id) {
					return Err(CrdtError::NodeAlreadyExists(id));
				}
				write_network(registry, node.network, timestamp, mode, |_| Ok(()))?;
				add(&mut registry.node_instances, &mut registry.removed_nodes, id, node, timestamp);
			}
			RegistryDelta::RemoveNode { id, snapshot } => {
				remove(&mut registry.node_instances, &mut registry.removed_nodes, id, snapshot, timestamp);
			}
			RegistryDelta::SetNodeInputs { id, mut inputs } => {
				inputs.iter_mut().for_each(|slot| stamp_slot(slot, timestamp));
				let referenced: Vec<NodeId> = inputs
					.iter()
					.filter_map(|slot| match slot.input {
						NodeInput::Node { id, .. } => Some(id),
						_ => None,
					})
					.collect();
				write_node(registry, id, timestamp, mode, |node| {
					merge_inputs(node, inputs, timestamp);
					Ok(())
				})?;
				for referenced in referenced {
					write_node(registry, referenced, timestamp, mode, |_| Ok(()))?;
				}
			}
			RegistryDelta::ChangeNodeInput { id, index, new_input } => {
				let referenced = match new_input {
					NodeInput::Node { id, .. } => Some(id),
					_ => None,
				};
				write_node(registry, id, timestamp, mode, |node| {
					let Some(input) = slot_for_write(node, index as usize, timestamp, mode)? else { return Ok(()) };
					if timestamp > input.timestamp {
						input.input = new_input;
						input.timestamp = timestamp;
					}
					Ok(())
				})?;
				if let Some(referenced) = referenced {
					write_node(registry, referenced, timestamp, mode, |_| Ok(()))?;
				}
			}
			RegistryDelta::SetNodeImplementation { id, implementation } => {
				if let Implementation::Network(network) = implementation {
					write_network(registry, network, timestamp, mode, |_| Ok(()))?;
				}
				write_node(registry, id, timestamp, mode, |node| {
					if timestamp > node.implementation_timestamp {
						node.implementation = implementation;
						node.implementation_timestamp = timestamp;
					}
					Ok(())
				})?;
			}
			RegistryDelta::ChangeNodeAttribute { id, delta } => {
				write_node(registry, id, timestamp, mode, |node| {
					apply_attribute_delta(delta, timestamp, &mut node.attributes, node.attributes_timestamp);
					Ok(())
				})?;
			}
			RegistryDelta::ChangeNodeInputAttribute { id, index, delta } => {
				write_node(registry, id, timestamp, mode, |node| {
					let Some(input) = slot_for_write(node, index as usize, timestamp, mode)? else { return Ok(()) };
					apply_attribute_delta(delta, timestamp, &mut input.attributes, input.attributes_timestamp);
					Ok(())
				})?;
			}
			RegistryDelta::SetNetworkExport { id, index, export } => {
				let referenced = match export {
					Some(NodeInput::Node { id, .. }) => Some(id),
					_ => None,
				};
				write_network(registry, id, timestamp, mode, |net| {
					let slot_idx = index as usize;
					if slot_idx >= net.exports.len() {
						if slot_idx >= MAX_EXPORT_SLOTS {
							return Err(CrdtError::ExportSlotOutOfBounds(index));
						}
						net.exports.resize(slot_idx + 1, ExportSlot::default());
					}

					let existing = &mut net.exports[slot_idx];
					if timestamp > existing.timestamp {
						existing.target = export;
						existing.timestamp = timestamp;
					}
					Ok(())
				})?;
				if let Some(referenced) = referenced {
					write_node(registry, referenced, timestamp, mode, |_| Ok(()))?;
				}
			}
			RegistryDelta::AddNetwork { id, network } => {
				if live && registry.networks.contains_key(&id) {
					return Err(CrdtError::NetworkAlreadyExists(id));
				}
				add(&mut registry.networks, &mut registry.removed_networks, id, network, timestamp);
			}
			RegistryDelta::RemoveNetwork { id, snapshot } => {
				remove(&mut registry.networks, &mut registry.removed_networks, id, snapshot, timestamp);
			}
			RegistryDelta::ChangeNetworkAttribute { id, delta } => {
				write_network(registry, id, timestamp, mode, |net| {
					apply_attribute_delta(delta, timestamp, &mut net.attributes, net.attributes_timestamp);
					Ok(())
				})?;
			}
			RegistryDelta::SetResourceHash { id, hash } => {
				upsert_resource(registry, id, timestamp, mode, |entry| {
					if timestamp > entry.hash_timestamp {
						entry.hash = hash;
						entry.hash_timestamp = timestamp;
					}
				});
			}
			RegistryDelta::AddSource { id, key, source } => {
				upsert_resource(registry, id, timestamp, mode, |entry| {
					let value = SourceValue { source, timestamp, deleted: false };
					entry.set_source(key, value);
				});
			}
			RegistryDelta::RemoveSource { id, key } => {
				// An upsert, so a removal for an entry never seen leaves a tombstone its addition loses to.
				upsert_resource(registry, id, timestamp, mode, |entry| {
					entry.remove_source(&key, timestamp);
				});
			}
			RegistryDelta::AddResource { id, entry } => {
				add(&mut registry.resources, &mut registry.removed_resources, id, entry, timestamp);
			}
			RegistryDelta::RemoveResource { id, snapshot } => {
				remove(&mut registry.resources, &mut registry.removed_resources, id, snapshot, timestamp);
			}
			RegistryDelta::RegisterPeer { peer, user } => {
				// The newest registration of a device wins, whatever order they land in.
				if registry.peer_users.get(&peer).is_none_or(|existing| timestamp > existing.at) {
					registry.peer_users.insert(peer, PeerRegistration { user, at: timestamp });
				}
			}
			RegistryDelta::ChangeDocumentAttribute { delta } => {
				apply_attribute_delta(delta, timestamp, &mut registry.attributes, TimeStamp::ORIGIN);
			}
			// Merge is a structural sync point only; it mutates no registry state.
			RegistryDelta::Merge { .. } | RegistryDelta::EndTransaction | RegistryDelta::Other(_) => {}
		}
		Ok(())
	}
}

/// An entity whose existence and every field are last-writer-wins on a timestamp.
pub(crate) trait Presence: Sized {
	/// The newest stamp of an addition or a write: the latest evidence the entity exists.
	fn presence(&self) -> TimeStamp;
	fn set_presence(&mut self, at: TimeStamp);
	/// Stamps every field at `at`, the way an addition writes all of them.
	fn stamp_all(&mut self, at: TimeStamp);
	/// Folds `other` in, keeping whichever value of each field is newer.
	fn merge(&mut self, other: Self);
	/// Dead content for an entity nothing has added yet, for writes to land on until an addition comes.
	/// Every field is at the origin, so the addition's values win.
	fn placeholder() -> Self;
}

impl Presence for Node {
	fn presence(&self) -> TimeStamp {
		self.presence
	}
	fn set_presence(&mut self, at: TimeStamp) {
		self.presence = at;
	}
	fn placeholder() -> Self {
		Node::new(crate::ROOT_NETWORK, Implementation::ProtoNode(ResourceId::from(0)), 0)
	}
	fn stamp_all(&mut self, at: TimeStamp) {
		self.presence = at;
		self.added = at;
		self.inputs_timestamp = at;
		self.implementation_timestamp = at;
		self.inputs.iter_mut().for_each(|slot| stamp_slot(slot, at));
		stamp_attributes(&mut self.attributes, &mut self.attributes_timestamp, at);
	}
	fn merge(&mut self, other: Self) {
		if other.added > self.added {
			self.network = other.network;
			self.added = other.added;
		}
		if other.implementation_timestamp > self.implementation_timestamp {
			self.implementation = other.implementation;
			self.implementation_timestamp = other.implementation_timestamp;
		}
		merge_inputs(self, other.inputs, other.inputs_timestamp);
		merge_attributes(&mut self.attributes, &mut self.attributes_timestamp, other.attributes, other.attributes_timestamp);
		self.presence = self.presence.max(other.presence);
	}
}

impl Presence for Network {
	fn presence(&self) -> TimeStamp {
		self.presence
	}
	fn set_presence(&mut self, at: TimeStamp) {
		self.presence = at;
	}
	fn placeholder() -> Self {
		Network::default()
	}
	fn stamp_all(&mut self, at: TimeStamp) {
		self.presence = at;
		self.exports.iter_mut().for_each(|slot| slot.timestamp = at);
		stamp_attributes(&mut self.attributes, &mut self.attributes_timestamp, at);
	}
	fn merge(&mut self, other: Self) {
		let len = self.exports.len().max(other.exports.len());
		self.exports.resize_with(len, ExportSlot::default);
		for (slot, incoming) in self.exports.iter_mut().zip(other.exports) {
			if incoming.timestamp > slot.timestamp {
				*slot = incoming;
			}
		}
		merge_attributes(&mut self.attributes, &mut self.attributes_timestamp, other.attributes, other.attributes_timestamp);
		self.presence = self.presence.max(other.presence);
	}
}

impl Presence for ResourceEntry {
	fn presence(&self) -> TimeStamp {
		self.presence
	}
	fn set_presence(&mut self, at: TimeStamp) {
		self.presence = at;
	}
	fn placeholder() -> Self {
		ResourceEntry::default()
	}
	fn stamp_all(&mut self, at: TimeStamp) {
		self.presence = at;
		self.hash_timestamp = at;
		self.stamp_sources(at);
	}
	fn merge(&mut self, other: Self) {
		if other.hash_timestamp > self.hash_timestamp {
			self.hash = other.hash;
			self.hash_timestamp = other.hash_timestamp;
		}
		self.merge_sources(other.sources, other.sources_timestamp);
		self.presence = self.presence.max(other.presence);
	}
}

fn stamp_slot(slot: &mut InputSlot, at: TimeStamp) {
	slot.timestamp = at;
	stamp_attributes(&mut slot.attributes, &mut slot.attributes_timestamp, at);
}

/// Writes the map whole at `at`: the floor deletes every other key without a tombstone per key.
fn stamp_attributes(attributes: &mut Attributes, floor: &mut TimeStamp, at: TimeStamp) {
	attributes.retain(|_, value| !value.deleted);
	attributes.values_mut().for_each(|value| value.timestamp = at);
	*floor = at;
}

/// Folds `other` in key by key. A key only one map holds is dead when the other map's floor is newer, since
/// that map was written whole after it; an entry stamped exactly at a floor was written by that whole-map
/// write and survives. Where both hold a key, the newer entry wins.
fn merge_attributes(attributes: &mut Attributes, floor: &mut TimeStamp, other: Attributes, other_floor: TimeStamp) {
	attributes.retain(|_, value| value.timestamp >= other_floor);
	for (key, value) in other {
		match attributes.entry(key) {
			Entry::Occupied(entry) if value.timestamp > entry.get().timestamp => *entry.into_mut() = value,
			Entry::Vacant(entry) if value.timestamp >= *floor => {
				entry.insert(value);
			}
			_ => {}
		}
	}
	*floor = (*floor).max(other_floor);
}

/// Folds an input list stamped `at` into the node's. The newer list decides the slot count, except that a
/// slot written after that shape still exists, with the gap up to it filled by unset slots stamped by the
/// shape. A slot both lists hold keeps its newer value, and its attributes fold key by key. A whole-list
/// write and a per-slot write thus land the same in either order.
fn merge_inputs(node: &mut Node, other: Vec<InputSlot>, at: TimeStamp) {
	let (mut newer, shape, older) = if at > node.inputs_timestamp {
		(other, at, std::mem::take(&mut node.inputs))
	} else {
		(std::mem::take(&mut node.inputs), node.inputs_timestamp, other)
	};
	for (index, incoming) in older.into_iter().enumerate() {
		if index >= newer.len() {
			if slot_newest(&incoming) <= shape {
				continue;
			}
			newer.resize_with(index + 1, || InputSlot::unset(shape));
		}
		let slot = &mut newer[index];
		if incoming.timestamp > slot.timestamp {
			slot.input = incoming.input;
			slot.timestamp = incoming.timestamp;
		}
		merge_attributes(&mut slot.attributes, &mut slot.attributes_timestamp, incoming.attributes, incoming.attributes_timestamp);
	}
	node.inputs = newer;
	node.inputs_timestamp = shape;
}

/// The newest write the slot carries, its attributes included.
fn slot_newest(slot: &InputSlot) -> TimeStamp {
	slot.attributes
		.values()
		.map(|value| value.timestamp)
		.fold(slot.timestamp.max(slot.attributes_timestamp), TimeStamp::max)
}

/// The slot at `index` for a write at `at`. A slot past the end comes into being when the write is newer
/// than the list's shape, as in [`merge_inputs`], and is `None` when the shape is newer. For a fresh local
/// edit the index is out of bounds.
fn slot_for_write(node: &mut Node, index: usize, at: TimeStamp, mode: ApplyMode) -> Result<Option<&mut InputSlot>, CrdtError> {
	if index >= node.inputs.len() {
		if index >= MAX_INPUT_SLOTS || mode == ApplyMode::Live {
			return Err(CrdtError::InputIndexOutOfBounds(index));
		}
		if at <= node.inputs_timestamp {
			return Ok(None);
		}
		let shape = node.inputs_timestamp;
		node.inputs.resize_with(index + 1, || InputSlot::unset(shape));
	}
	Ok(node.inputs.get_mut(index))
}

/// The entity's content wherever it sits: live, or held by its tombstone.
fn content_mut<'a, K: Hash + Eq + Copy, T>(live: &'a mut HashMap<K, T>, dead: &'a mut HashMap<K, Tombstone<T>>, id: K) -> Option<&'a mut T> {
	live.get_mut(&id).or_else(|| dead.get_mut(&id).map(|mark| &mut mark.content))
}

/// Brings a removed entity back when its presence has grown past its removal. A placeholder stays dead
/// whatever its presence.
fn settle<K: Hash + Eq + Copy, T: Presence>(live: &mut HashMap<K, T>, dead: &mut HashMap<K, Tombstone<T>>, id: K) {
	if dead.get(&id).is_some_and(|mark| !mark.placeholder && mark.content.presence() > mark.at) {
		let mark = dead.remove(&id).expect("checked above");
		live.insert(id, mark.content);
	}
}

/// Folds `content` into whatever is held under `id`, or holds it as new. Content comes from an addition or
/// a removal's snapshot, so it proves the entity was added and a placeholder holding it stops being one.
fn fold<K: Hash + Eq + Copy, T: Presence>(live: &mut HashMap<K, T>, dead: &mut HashMap<K, Tombstone<T>>, id: K, content: T) {
	if let Some(mark) = dead.get_mut(&id).filter(|mark| mark.placeholder) {
		// The placeholder holds only early writes: they land on the real content as if the addition came first.
		let early = std::mem::replace(&mut mark.content, content);
		mark.content.merge(early);
		mark.placeholder = false;
	} else if let Some(existing) = content_mut(live, dead, id) {
		existing.merge(content);
	} else {
		live.insert(id, content);
	}
	settle(live, dead, id);
}

/// An addition at `at`: every field of `content` is written at `at`, and the entity exists from then
/// unless a newer removal holds it dead.
fn add<K: Hash + Eq + Copy, T: Presence>(live: &mut HashMap<K, T>, dead: &mut HashMap<K, Tombstone<T>>, id: K, mut content: T, at: TimeStamp) {
	content.stamp_all(at);
	fold(live, dead, id, content);
}

/// A removal at `at`: the entity is dead from then unless a newer addition or write holds it live. The
/// snapshot folds in first, so a removal of an entity never seen still lands and its addition loses when
/// it arrives.
fn remove<K: Hash + Eq + Copy, T: Presence>(live: &mut HashMap<K, T>, dead: &mut HashMap<K, Tombstone<T>>, id: K, snapshot: T, at: TimeStamp) {
	fold(live, dead, id, snapshot);
	if let Some(mark) = dead.get_mut(&id) {
		mark.at = mark.at.max(at);
	} else if live.get(&id).is_some_and(|content| at > content.presence()) {
		let content = live.remove(&id).expect("checked above");
		dead.insert(id, Tombstone { content, at, placeholder: false });
	}
}

/// A write at `at` to the entity under `id`, applied by `write` to its content wherever it sits. The
/// write is evidence the entity exists at `at`, so it revives an older removal. A write to an entity
/// never seen lands on a placeholder tombstone that stays dead until an addition folds into it, so ops
/// land the same in any order and nothing is deferred. For a fresh local edit the entity is `missing()`.
fn write<K: Hash + Eq + Copy, T: Presence>(
	live: &mut HashMap<K, T>,
	dead: &mut HashMap<K, Tombstone<T>>,
	id: K,
	at: TimeStamp,
	mode: ApplyMode,
	missing: impl FnOnce() -> CrdtError,
	write: impl FnOnce(&mut T) -> Result<(), CrdtError>,
) -> Result<(), CrdtError> {
	let unseen = !live.contains_key(&id) && !dead.contains_key(&id);
	if unseen {
		if mode == ApplyMode::Live {
			return Err(missing());
		}
		dead.insert(
			id,
			Tombstone {
				content: T::placeholder(),
				at: TimeStamp::ORIGIN,
				placeholder: true,
			},
		);
	}
	let content = content_mut(live, dead, id).expect("held above");
	if let Err(error) = write(content) {
		// A rejected op changes nothing, so the placeholder made for it goes too.
		if unseen {
			dead.remove(&id);
		}
		return Err(error);
	}
	content.set_presence(content.presence().max(at));
	settle(live, dead, id);
	Ok(())
}

/// [`write()`] to a node; with a no-op `f`, records a reference to it.
fn write_node(registry: &mut Registry, id: NodeId, at: TimeStamp, mode: ApplyMode, f: impl FnOnce(&mut Node) -> Result<(), CrdtError>) -> Result<(), CrdtError> {
	write(&mut registry.node_instances, &mut registry.removed_nodes, id, at, mode, || CrdtError::TargetNodeDoesNotExist(id), f)
}

fn write_network(registry: &mut Registry, id: NetworkId, at: TimeStamp, mode: ApplyMode, f: impl FnOnce(&mut Network) -> Result<(), CrdtError>) -> Result<(), CrdtError> {
	write(&mut registry.networks, &mut registry.removed_networks, id, at, mode, || CrdtError::NetworkDoesNotExist(id), f)
}

/// Like [`write()`] for a resource, except that an entry never seen is created first, since single-field
/// resource ops are upserts. Every other field starts at the origin, so a later addition fills them in.
fn upsert_resource(registry: &mut Registry, id: ResourceId, at: TimeStamp, mode: ApplyMode, f: impl FnOnce(&mut ResourceEntry)) {
	if !registry.resources.contains_key(&id) && !registry.removed_resources.contains_key(&id) {
		registry.resources.insert(
			id,
			ResourceEntry {
				presence: at,
				..ResourceEntry::default()
			},
		);
	}
	let missing = || CrdtError::ResourceDoesNotExist(id);
	write(&mut registry.resources, &mut registry.removed_resources, id, at, mode, missing, |entry| {
		f(entry);
		Ok(())
	})
	.expect("added above");
}

/// Which of a [`Document`]'s two registries an apply targets: the working copy (retired state plus
/// live hot ops) or the retired snapshot (retired deltas only). Retirement targets the snapshot so
/// reverses capture pre-op values; the hot path and undo/redo target the working copy.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum RegistryTarget {
	Working,
	Snapshot,
}

/// How [`Document::apply_op_with`] treats what it finds.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ApplyMode {
	/// A fresh local edit: adding what exists, or writing to what was never seen, is an error, since a local
	/// edit cannot mean either.
	Live,
	/// An op from a peer or from persisted state: every arm is a timestamp comparison.
	Idempotent,
}
