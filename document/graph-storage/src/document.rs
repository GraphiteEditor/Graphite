use std::collections::HashMap;
use std::hash::Hash;

use crate::{
	Attributes, CrdtError, Delta, ExportSlot, History, HotOp, HotOpId, HotSequence, Implementation, InputSlot, LamportClock, MAX_EXPORT_SLOTS, MAX_INPUT_SLOTS, Network, NetworkId, Node, NodeId,
	NodeInput, PeerId, Registry, RegistryDelta, ResourceEntry, ResourceId, RetiredHotOps, Rev, SourceValue, TimeStamp, Tombstone, apply_attribute_delta, reverse_attribute_delta,
};

#[derive(Clone, Debug)]
pub struct Document {
	/// Working registry: retired state with the current hot ops applied on top. This is what live
	/// reads and `registry()` observe, and what undo/redo force-apply against.
	pub(crate) working_registry: Registry,
	/// Live broadcast stream, applied to the `working_registry` on receive, GC'd at retirement.
	/// Persisted for crash recovery so in-flight unretired work survives editor restarts.
	pub(crate) hot_log: Vec<HotOp>,
	/// Which hot ops history already covers, so one arriving after its own retirement is dropped instead
	/// of re-entering the log. Retirement discards the delta's link back to its hot op, leaving this the
	/// only record. See [`RetiredHotOps`].
	pub(crate) retired: RetiredHotOps,
	/// The registry as of the last retirement, with no un-retired hot ops applied. Retirement computes
	/// each delta's `reverse` against this (so LWW reverses capture the true pre-op value, not the
	/// hot-polluted working state) and advances it. Every field of a registry is last-writer-wins on a
	/// timestamp, so this and the working registry agree by value whenever the hot log is empty, whatever
	/// order either applied its ops in.
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
	pub(crate) fn empty(peer: PeerId) -> Self {
		Self {
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
		}
	}

	/// Mint a fresh `NodeId` scoped to this document's peer. The 64-bit ID is `blake3(peer, counter)`
	/// truncated; the counter is shared across peers and persisted with the document.
	pub fn next_node_id(&mut self) -> NodeId {
		self.next_node_counter += 1;
		let bytes = rmp_serde::to_vec(&(self.peer, self.next_node_counter)).expect("(PeerId, counter) must serialize");
		let digest = blake3::hash(&bytes);
		let mut truncated = [0u8; 8];
		truncated.copy_from_slice(&digest.as_bytes()[..8]);
		NodeId(u64::from_le_bytes(truncated))
	}

	/// Apply a delta's `reverse` as the new forward op (silent-zone undo). Force-applied: the reverse
	/// carries the forward op's own timestamp, which would otherwise tie and lose.
	pub(crate) fn revert_delta(&mut self, target: RegistryTarget, mut delta: Delta) -> Result<(), CrdtError> {
		for parent in delta.all_parents() {
			if !self.history.contains(parent) {
				return Err(CrdtError::NotFoundInHistory(parent));
			}
		}
		std::mem::swap(&mut delta.kind, &mut delta.reverse);
		self.apply_op_with(target, delta.kind, delta.timestamp, ApplyMode::Force)
	}

	/// Apply a locally staged op: LWW into the registry, append to the hot log, leave history and `head`
	/// alone. Crate-private because it skips the retirement check an op off the wire needs; staging must
	/// not have a covered sequence silently drop local work. Outside callers use [`Session::apply_hot_op`].
	pub(crate) fn apply_hot_op(&mut self, hot_op: HotOp) -> Result<(), CrdtError> {
		self.apply_op(hot_op.op.clone(), hot_op.timestamp)?;
		self.hot_log.push(hot_op);
		Ok(())
	}

	/// Replay a hot op recovered from persisted state or received from a peer. A repeated op is not newer
	/// than what it wrote, so replaying one already reflected in the registry changes nothing.
	pub fn replay_hot_op(&mut self, hot_op: HotOp) -> Result<(), CrdtError> {
		// Already in retired history; re-adding it would leave an entry no retirement will ever name.
		if self.is_retired(hot_op.id()) {
			return Ok(());
		}

		// Identified by timestamp, so a re-announcement of one already held must not become a second copy.
		if self.hot_log.iter().any(|held| held.timestamp == hot_op.timestamp) {
			return Ok(());
		}

		// Replaying our own ops is what carries the sequence counter across a reload.
		if hot_op.timestamp.peer == self.peer {
			self.next_hot_sequence = self.next_hot_sequence.max(hot_op.sequence);
		}

		self.apply_op_idempotent(hot_op.op.clone(), hot_op.timestamp)?;
		self.hot_log.push(hot_op);
		Ok(())
	}

	/// Whether this hot op has already been promoted into history.
	pub(crate) fn is_retired(&self, id: HotOpId) -> bool {
		self.retired.covers(id)
	}

	/// Record newly retired hot ops, dropping any the hot log still holds.
	pub(crate) fn mark_retired(&mut self, retired: impl IntoIterator<Item = HotOpId>) {
		self.retired.extend(retired);

		self.drop_retired_hot_ops();
	}

	/// Take on a peer's retirement marks as well as this peer's. Returns whether a hot op was dropped.
	pub(crate) fn absorb_retired(&mut self, remote: &RetiredHotOps) -> bool {
		self.retired.absorb(remote);

		self.drop_retired_hot_ops()
	}

	/// Drop hot ops the retired snapshot already accounts for. A mark alone will not do: it can arrive
	/// ahead of the delta carrying the op into history, stranding its effect in the working registry.
	fn drop_retired_hot_ops(&mut self) -> bool {
		let before = self.hot_log.len();
		self.hot_log.retain(|hot_op| !self.history.contains_timestamp(hot_op.timestamp));

		before != self.hot_log.len()
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

	fn registry_ref(&self, target: RegistryTarget) -> &Registry {
		match target {
			RegistryTarget::Working => &self.working_registry,
			RegistryTarget::Snapshot => &self.retired_snapshot,
		}
	}

	/// A fresh local edit against the working registry. Adding a node or network that exists errors,
	/// since a local edit cannot mean that, and a write to something never seen errors.
	pub(crate) fn apply_op(&mut self, op: RegistryDelta, timestamp: TimeStamp) -> Result<(), CrdtError> {
		self.apply_op_with(RegistryTarget::Working, op, timestamp, ApplyMode::Live)
	}

	/// An op from a peer or from persisted state, against the working registry.
	pub(crate) fn apply_op_idempotent(&mut self, op: RegistryDelta, timestamp: TimeStamp) -> Result<(), CrdtError> {
		self.apply_op_with(RegistryTarget::Working, op, timestamp, ApplyMode::Idempotent)
	}

	/// Silent-zone undo/redo rewind against the working registry: every write lands regardless of
	/// timestamp. We own the single-writer chain here, so the precomputed reverse (undo) or forward
	/// (redo) value is authoritative even though its timestamp ties what it replaces.
	pub(crate) fn force_apply_op(&mut self, op: RegistryDelta, timestamp: TimeStamp) -> Result<(), CrdtError> {
		self.apply_op_with(RegistryTarget::Working, op, timestamp, ApplyMode::Force)
	}

	/// Applies one op. Every field of the registry is last-writer-wins on a timestamp, whether an entity
	/// exists included, so the registry folds to the same values whatever order the ops land in:
	///
	/// - An addition writes every field of the entity at its stamp, and is evidence the entity exists
	///   from then.
	/// - A removal is evidence the entity does not exist from its stamp. The entity's content stays as a
	///   tombstone, and the removal's snapshot folds in like any other write.
	/// - A write to an entity is evidence it exists at its stamp, so it revives a removal older than
	///   itself; one older than the removal lands on the tombstone, so a later revival sees it. The same
	///   goes for what an op refers to: the network a node is added into, and the node an input or an
	///   export is wired to.
	/// - A write to an entity never seen is an error, for the caller to defer until its addition arrives.
	pub(crate) fn apply_op_with(&mut self, target: RegistryTarget, op: RegistryDelta, timestamp: TimeStamp, mode: ApplyMode) -> Result<(), CrdtError> {
		// Advance the local clock past every observed op, including ones that subsequently no-op or
		// error. Observation is about causality knowledge, not about whether the op took effect.
		self.clock.observe(timestamp);

		let live = mode == ApplyMode::Live;
		let force = mode == ApplyMode::Force;

		let registry = self.registry_mut(target);
		match op {
			RegistryDelta::AddNode { id, node } => {
				if live && registry.node_instances.contains_key(&id) {
					return Err(CrdtError::NodeAlreadyExists(id));
				}
				touch_network(registry, node.network, timestamp, force)?;
				add(&mut registry.node_instances, &mut registry.removed_nodes, id, node, timestamp, force);
			}
			RegistryDelta::RemoveNode { id, snapshot } => {
				remove(&mut registry.node_instances, &mut registry.removed_nodes, id, snapshot, timestamp, force);
			}
			RegistryDelta::SetNodeInputs { id, inputs } => {
				for slot in &inputs {
					if let NodeInput::Node { id: referenced, .. } = slot.input {
						touch_node(registry, referenced, timestamp, force)?;
					}
				}
				let mut inputs = inputs;
				for slot in &mut inputs {
					stamp_slot(slot, timestamp);
				}
				write_node(registry, id, timestamp, force, |node| {
					if force {
						node.inputs = inputs;
						node.inputs_timestamp = timestamp;
					} else {
						merge_inputs(node, inputs, timestamp);
					}
					Ok(())
				})?;
			}
			RegistryDelta::ChangeNodeInput { id, index, new_input } => {
				if let NodeInput::Node { id: referenced, .. } = new_input {
					touch_node(registry, referenced, timestamp, force)?;
				}
				write_node(registry, id, timestamp, force, |node| {
					let Some(input) = slot_for_write(node, index as usize, timestamp, mode)? else { return Ok(()) };
					if force || timestamp > input.timestamp {
						input.input = new_input;
						input.timestamp = timestamp;
					}
					Ok(())
				})?;
			}
			RegistryDelta::SetNodeImplementation { id, implementation } => {
				if let Implementation::Network(network) = implementation {
					touch_network(registry, network, timestamp, force)?;
				}
				write_node(registry, id, timestamp, force, |node| {
					if force || timestamp > node.implementation_timestamp {
						node.implementation = implementation;
						node.implementation_timestamp = timestamp;
					}
					Ok(())
				})?;
			}
			RegistryDelta::ChangeNodeAttribute { id, delta } => {
				write_node(registry, id, timestamp, force, |node| {
					apply_attribute_delta(delta, timestamp, force, &mut node.attributes, node.attributes_timestamp);
					Ok(())
				})?;
			}
			RegistryDelta::ChangeNodeInputAttribute { id, index, delta } => {
				write_node(registry, id, timestamp, force, |node| {
					let Some(input) = slot_for_write(node, index as usize, timestamp, mode)? else { return Ok(()) };
					apply_attribute_delta(delta, timestamp, force, &mut input.attributes, input.attributes_timestamp);
					Ok(())
				})?;
			}
			RegistryDelta::SetNetworkExport { id, index, export } => {
				if let Some(NodeInput::Node { id: referenced, .. }) = export {
					touch_node(registry, referenced, timestamp, force)?;
				}
				write_network(registry, id, timestamp, force, |net| {
					let slot_idx = index as usize;
					if slot_idx >= net.exports.len() {
						if slot_idx >= MAX_EXPORT_SLOTS {
							return Err(CrdtError::ExportSlotOutOfBounds(index));
						}
						net.exports.resize(
							slot_idx + 1,
							ExportSlot {
								target: None,
								timestamp: TimeStamp::ORIGIN,
							},
						);
					}

					let existing = &mut net.exports[slot_idx];
					if force || timestamp > existing.timestamp {
						existing.target = export;
						existing.timestamp = timestamp;
					}
					Ok(())
				})?;
			}
			RegistryDelta::AddNetwork { id, network } => {
				if live && registry.networks.contains_key(&id) {
					return Err(CrdtError::NetworkAlreadyExists(id));
				}
				add(&mut registry.networks, &mut registry.removed_networks, id, network, timestamp, force);
			}
			RegistryDelta::RemoveNetwork { id, snapshot } => {
				remove(&mut registry.networks, &mut registry.removed_networks, id, snapshot, timestamp, force);
			}
			RegistryDelta::ChangeNetworkAttribute { id, delta } => {
				write_network(registry, id, timestamp, force, |net| {
					apply_attribute_delta(delta, timestamp, force, &mut net.attributes, net.attributes_timestamp);
					Ok(())
				})?;
			}
			RegistryDelta::SetResourceHash { id, hash } => {
				upsert_resource(registry, id, timestamp, force, |entry| {
					if force || timestamp > entry.hash_timestamp {
						entry.hash = hash;
						entry.hash_timestamp = timestamp;
					}
				});
			}
			RegistryDelta::AddSource { id, key, source } => {
				upsert_resource(registry, id, timestamp, force, |entry| {
					let value = SourceValue { source, timestamp };
					if force { entry.force_set_source(key, value) } else { entry.set_source(key, value) }
				});
			}
			RegistryDelta::RemoveSource { id, key } => {
				// A source removal names nothing to create the entry from; one for an entry never seen is dropped.
				let missing = || CrdtError::ResourceDoesNotExist(id);
				let _ = write(&mut registry.resources, &mut registry.removed_resources, id, timestamp, force, missing, |entry| {
					if force {
						entry.force_remove_source(&key);
					} else {
						entry.remove_source(&key, timestamp);
					}
					Ok(())
				});
			}
			RegistryDelta::AddResource { id, entry } => {
				add(&mut registry.resources, &mut registry.removed_resources, id, entry, timestamp, force);
			}
			RegistryDelta::RemoveResource { id, snapshot } => {
				remove(&mut registry.resources, &mut registry.removed_resources, id, snapshot, timestamp, force);
			}
			RegistryDelta::RegisterPeer { peer, user } => match registry.peer_users.get(&peer) {
				Some(existing) if *existing != user => return Err(CrdtError::PeerRegistrationConflict(peer)),
				Some(_) => {}
				None => {
					registry.peer_users.insert(peer, user);
				}
			},
			RegistryDelta::ChangeDocumentAttribute { delta } => {
				apply_attribute_delta(delta, timestamp, force, &mut registry.attributes, TimeStamp::ORIGIN);
			}
			// Merge is a structural sync point only; it mutates no registry state.
			RegistryDelta::Merge { .. } | RegistryDelta::Other(_) => {}
		}
		Ok(())
	}

	/// Compute the inverse of `delta` against the registry named by `target`. Retirement passes
	/// [`RegistryTarget::Snapshot`] so LWW reverses (export target, inputs, attributes, resource hash)
	/// capture the true pre-op value rather than the hot-polluted working state.
	///
	/// A write to a removed entity reads its pre-op value from the tombstone, since applying the write
	/// revives the entity from there.
	pub(crate) fn compute_reverse_delta(&self, target: RegistryTarget, delta: &RegistryDelta) -> Result<RegistryDelta, CrdtError> {
		let registry = self.registry_ref(target);
		let node = |id: NodeId| {
			registry
				.node_instances
				.get(&id)
				.or_else(|| registry.removed_nodes.get(&id).map(|mark| &mark.content))
				.ok_or(CrdtError::TargetNodeDoesNotExist(id))
		};
		let network = |id: NetworkId| registry.networks.get(&id).or_else(|| registry.removed_networks.get(&id).map(|mark| &mark.content));
		let resource = |id: ResourceId| registry.resources.get(&id).or_else(|| registry.removed_resources.get(&id).map(|mark| &mark.content));
		Ok(match delta {
			RegistryDelta::AddNode { id, node } => RegistryDelta::RemoveNode { id: *id, snapshot: node.clone() },
			RegistryDelta::RemoveNode { id, snapshot } => RegistryDelta::AddNode { id: *id, node: snapshot.clone() },
			&RegistryDelta::SetNodeInputs { id, .. } => RegistryDelta::SetNodeInputs { id, inputs: node(id)?.inputs.clone() },
			&RegistryDelta::SetNodeImplementation { id, .. } => RegistryDelta::SetNodeImplementation {
				id,
				implementation: node(id)?.implementation.clone(),
			},
			&RegistryDelta::ChangeNodeInput { id, index: input_idx, .. } => {
				let slot = node(id)?.inputs().get(input_idx as usize).ok_or(CrdtError::InputIndexOutOfBounds(input_idx as usize))?;
				RegistryDelta::ChangeNodeInput {
					id,
					index: input_idx,
					new_input: slot.input.clone(),
				}
			}
			&RegistryDelta::ChangeNodeAttribute { id, ref delta } => RegistryDelta::ChangeNodeAttribute {
				id,
				delta: reverse_attribute_delta(delta, node(id)?.attributes()),
			},
			&RegistryDelta::ChangeNodeInputAttribute { id, index, ref delta } => {
				let input = node(id)?.inputs().get(index as usize).ok_or(CrdtError::InputIndexOutOfBounds(index as usize))?;
				RegistryDelta::ChangeNodeInputAttribute {
					id,
					index,
					delta: reverse_attribute_delta(delta, &input.attributes),
				}
			}
			&RegistryDelta::SetNetworkExport { id, index, .. } => {
				// Absent network or slot: pre-op there was no export to point at.
				let export_target = network(id).and_then(|net| net.exports.get(index as usize)).and_then(|s| s.target.clone());
				RegistryDelta::SetNetworkExport { id, index, export: export_target }
			}
			RegistryDelta::AddNetwork { id, network } => RegistryDelta::RemoveNetwork { id: *id, snapshot: network.clone() },
			&RegistryDelta::RemoveNetwork { id, ref snapshot } => RegistryDelta::AddNetwork { id, network: snapshot.clone() },
			&RegistryDelta::ChangeNetworkAttribute { id, ref delta } => {
				let current = network(id).map(|net| &net.attributes).ok_or(CrdtError::NetworkDoesNotExist(id))?;
				RegistryDelta::ChangeNetworkAttribute {
					id,
					delta: reverse_attribute_delta(delta, current),
				}
			}
			RegistryDelta::ChangeDocumentAttribute { delta } => RegistryDelta::ChangeDocumentAttribute {
				delta: reverse_attribute_delta(delta, &registry.attributes),
			},
			// Registrations are append-only and not user-undoable; reverse is the same op,
			// which applies as a no-op on the already-registered PeerId.
			&RegistryDelta::RegisterPeer { peer, user } => RegistryDelta::RegisterPeer { peer, user },
			&RegistryDelta::SetResourceHash { id, .. } => RegistryDelta::SetResourceHash {
				id,
				hash: resource(id).and_then(|entry| entry.hash),
			},
			&RegistryDelta::AddSource { id, key, .. } => match resource(id).and_then(|entry| entry.source(&key)) {
				// The slot already held a source: undo restores it.
				Some(existing) => RegistryDelta::AddSource {
					id,
					key,
					source: existing.source.clone(),
				},
				// The slot was empty: undo removes what this op added.
				None => RegistryDelta::RemoveSource { id, key },
			},
			&RegistryDelta::RemoveSource { id, key } => match resource(id).and_then(|entry| entry.source(&key)) {
				Some(existing) => RegistryDelta::AddSource {
					id,
					key,
					source: existing.source.clone(),
				},
				// Nothing to restore; reverse is a no-op removal.
				None => RegistryDelta::RemoveSource { id, key },
			},
			&RegistryDelta::AddResource { id, .. } => match registry.resources.get(&id) {
				// Overwrote an existing entry: undo restores it.
				Some(existing) => RegistryDelta::AddResource { id, entry: existing.clone() },
				// Created a new entry: undo removes what this op added (snapshot is empty since there was nothing prior).
				None => RegistryDelta::RemoveResource {
					id,
					snapshot: ResourceEntry::default(),
				},
			},
			&RegistryDelta::RemoveResource { id, .. } => {
				let snapshot = registry.resources.get(&id).cloned().unwrap_or_default();
				RegistryDelta::AddResource { id, entry: snapshot }
			}
			RegistryDelta::Merge { extra_parents } => RegistryDelta::Merge { extra_parents: extra_parents.clone() },
			&RegistryDelta::Other(_) => RegistryDelta::Other(serde_json::Value::Null),
		})
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
}

impl Presence for Node {
	fn presence(&self) -> TimeStamp {
		self.presence
	}
	fn set_presence(&mut self, at: TimeStamp) {
		self.presence = at;
	}
	fn stamp_all(&mut self, at: TimeStamp) {
		self.presence = at;
		self.added = at;
		self.inputs_timestamp = at;
		self.implementation_timestamp = at;
		for slot in &mut self.inputs {
			stamp_slot(slot, at);
		}
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
	fn stamp_all(&mut self, at: TimeStamp) {
		self.presence = at;
		for slot in &mut self.exports {
			slot.timestamp = at;
		}
		stamp_attributes(&mut self.attributes, &mut self.attributes_timestamp, at);
	}
	fn merge(&mut self, other: Self) {
		if other.exports.len() > self.exports.len() {
			self.exports.resize(
				other.exports.len(),
				ExportSlot {
					target: None,
					timestamp: TimeStamp::ORIGIN,
				},
			);
		}
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
	fn stamp_all(&mut self, at: TimeStamp) {
		self.presence = at;
		self.hash_timestamp = at;
		for (_, value) in &mut self.sources {
			value.timestamp = at;
		}
	}
	fn merge(&mut self, other: Self) {
		if other.hash_timestamp > self.hash_timestamp {
			self.hash = other.hash;
			self.hash_timestamp = other.hash_timestamp;
		}
		for (key, value) in other.sources {
			self.set_source(key, value);
		}
		self.presence = self.presence.max(other.presence);
	}
}

fn stamp_slot(slot: &mut InputSlot, at: TimeStamp) {
	slot.timestamp = at;
	stamp_attributes(&mut slot.attributes, &mut slot.attributes_timestamp, at);
}

/// Writes the map whole at `at`: every key it holds is written then, and every other key is deleted as
/// of then, which the floor records without a tombstone per key.
fn stamp_attributes(attributes: &mut Attributes, floor: &mut TimeStamp, at: TimeStamp) {
	attributes.retain(|_, value| !value.deleted);
	for value in attributes.values_mut() {
		value.timestamp = at;
	}
	*floor = at;
}

/// Folds `other` in key by key. A key only one map holds is dead when the other map's floor is newer
/// than it: the other map was written whole after it. An entry stamped exactly at a floor was written by
/// the whole-map write that set the floor, so it is not older than it. Every entry, tombstones included,
/// is at or past its map's floor, so the newer entry wins where both hold a key.
fn merge_attributes(attributes: &mut Attributes, floor: &mut TimeStamp, other: Attributes, other_floor: TimeStamp) {
	attributes.retain(|_, value| value.timestamp >= other_floor);
	for (key, value) in other {
		match attributes.entry(key) {
			std::collections::btree_map::Entry::Occupied(mut entry) => {
				if value.timestamp > entry.get().timestamp {
					entry.insert(value);
				}
			}
			std::collections::btree_map::Entry::Vacant(entry) => {
				if value.timestamp >= *floor {
					entry.insert(value);
				}
			}
		}
	}
	*floor = (*floor).max(other_floor);
}

/// Folds an input list stamped `at` into the node.s. The list.s shape is one value with its own
/// timestamp, so the newer list decides how many slots there are, except that a slot written after that
/// shape was decided exists in spite of it, with the gap up to it filled by unset slots stamped by the
/// shape. A slot both lists hold keeps whichever of its values is newer, and its attributes fold key by
/// key. A whole-list write and a per-slot write thus land the same in either order.
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

/// The slot at `index` for a write at `at`. A slot past the end of the list comes into being when the
/// write is newer than the list.s shape, with the gap up to it filled by unset slots stamped by the
/// shape, and is `None` when the shape is newer: the shape decided the slot away. A fresh local edit
/// cannot mean either, so for one the index is out of bounds.
fn slot_for_write(node: &mut Node, index: usize, at: TimeStamp, mode: ApplyMode) -> Result<Option<&mut InputSlot>, CrdtError> {
	if index >= node.inputs.len() {
		if index >= MAX_INPUT_SLOTS || mode == ApplyMode::Live {
			return Err(CrdtError::InputIndexOutOfBounds(index));
		}
		if mode != ApplyMode::Force && at <= node.inputs_timestamp {
			return Ok(None);
		}
		let shape = node.inputs_timestamp;
		node.inputs.resize_with(index + 1, || InputSlot::unset(shape));
	}
	Ok(node.inputs.get_mut(index))
}

/// The entity's content wherever it sits: live, or held by its tombstone.
fn content_mut<'a, K: Hash + Eq + Copy, T>(live: &'a mut HashMap<K, T>, dead: &'a mut HashMap<K, Tombstone<T>>, id: K) -> Option<&'a mut T> {
	match live.get_mut(&id) {
		Some(content) => Some(content),
		None => dead.get_mut(&id).map(|mark| &mut mark.content),
	}
}

/// Brings a removed entity back when its presence has grown past its removal.
fn settle<K: Hash + Eq + Copy, T: Presence>(live: &mut HashMap<K, T>, dead: &mut HashMap<K, Tombstone<T>>, id: K) {
	if dead.get(&id).is_some_and(|mark| mark.content.presence() > mark.at) {
		let mark = dead.remove(&id).expect("checked above");
		live.insert(id, mark.content);
	}
}

/// Folds `content` into whatever is held under `id`, or holds it as new.
fn fold<K: Hash + Eq + Copy, T: Presence>(live: &mut HashMap<K, T>, dead: &mut HashMap<K, Tombstone<T>>, id: K, content: T) {
	match content_mut(live, dead, id) {
		Some(existing) => {
			existing.merge(content);
			settle(live, dead, id);
		}
		None => {
			live.insert(id, content);
		}
	}
}

/// An addition at `at`: every field of `content` is written at `at`, and the entity exists from then
/// unless a newer removal holds it dead. `force` puts `content` in place as it is.
fn add<K: Hash + Eq + Copy, T: Presence>(live: &mut HashMap<K, T>, dead: &mut HashMap<K, Tombstone<T>>, id: K, mut content: T, at: TimeStamp, force: bool) {
	if force {
		dead.remove(&id);
		live.insert(id, content);
		return;
	}
	content.stamp_all(at);
	fold(live, dead, id, content);
}

/// A removal at `at`: the entity is dead from then unless a newer addition or write holds it live. The
/// snapshot folds in first, so a removal of an entity never seen still lands and the addition it was
/// based on is recognised as older when it arrives.
fn remove<K: Hash + Eq + Copy, T: Presence>(live: &mut HashMap<K, T>, dead: &mut HashMap<K, Tombstone<T>>, id: K, snapshot: T, at: TimeStamp, force: bool) {
	fold(live, dead, id, snapshot);
	if let Some(mark) = dead.get_mut(&id) {
		if force || at > mark.at {
			mark.at = at;
		}
		return;
	}
	if live.get(&id).is_some_and(|content| force || at > content.presence()) {
		let content = live.remove(&id).expect("checked above");
		dead.insert(id, Tombstone { content, at });
	}
}

/// A write at `at` to the entity under `id`, applied by `write` to its content wherever it sits. The
/// write is evidence the entity exists at `at`, so it revives a removal older than that. An entity
/// never seen is `missing()`, for the caller to defer the write until its addition arrives.
fn write<K: Hash + Eq + Copy, T: Presence>(
	live: &mut HashMap<K, T>,
	dead: &mut HashMap<K, Tombstone<T>>,
	id: K,
	at: TimeStamp,
	force: bool,
	missing: impl FnOnce() -> CrdtError,
	write: impl FnOnce(&mut T) -> Result<(), CrdtError>,
) -> Result<(), CrdtError> {
	let Some(content) = content_mut(live, dead, id) else { return Err(missing()) };
	write(content)?;
	if at > content.presence() {
		content.set_presence(at);
	}
	if force && let Some(mark) = dead.remove(&id) {
		live.insert(id, mark.content);
	}
	settle(live, dead, id);
	Ok(())
}

fn write_node(registry: &mut Registry, id: NodeId, at: TimeStamp, force: bool, f: impl FnOnce(&mut Node) -> Result<(), CrdtError>) -> Result<(), CrdtError> {
	write(&mut registry.node_instances, &mut registry.removed_nodes, id, at, force, || CrdtError::TargetNodeDoesNotExist(id), f)
}

fn write_network(registry: &mut Registry, id: NetworkId, at: TimeStamp, force: bool, f: impl FnOnce(&mut Network) -> Result<(), CrdtError>) -> Result<(), CrdtError> {
	write(&mut registry.networks, &mut registry.removed_networks, id, at, force, || CrdtError::NetworkDoesNotExist(id), f)
}

/// A reference to a node is evidence it exists at `at`.
fn touch_node(registry: &mut Registry, id: NodeId, at: TimeStamp, force: bool) -> Result<(), CrdtError> {
	write_node(registry, id, at, force, |_| Ok(()))
}

/// A reference to a network is evidence it exists at `at`.
fn touch_network(registry: &mut Registry, id: NetworkId, at: TimeStamp, force: bool) -> Result<(), CrdtError> {
	write_network(registry, id, at, force, |_| Ok(()))
}

/// Like [`write`] for a resource, except that an entry never seen is created first: the resource ops
/// that write one field are upserts. The entry starts with every other field at the origin, so the
/// write itself lands and an addition arriving later fills the rest in.
fn upsert_resource(registry: &mut Registry, id: ResourceId, at: TimeStamp, force: bool, f: impl FnOnce(&mut ResourceEntry)) {
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
	write(&mut registry.resources, &mut registry.removed_resources, id, at, force, missing, |entry| {
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
	/// A fresh local edit: adding what exists is an error, since a local edit cannot mean that.
	Live,
	/// An op from a peer or from persisted state: every arm is a timestamp comparison.
	Idempotent,
	/// Silent-zone undo/redo rewind: every write lands, whatever its timestamp.
	Force,
}
