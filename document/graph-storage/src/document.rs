use std::collections::btree_map::Entry;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use crate::{
	Attributes, CrdtError, Delta, ExportSlot, History, HotOp, HotSequence, Implementation, InputSlot, LamportClock, MAX_EXPORT_SLOTS, MAX_INPUT_SLOTS, Network, NetworkId, Node, NodeId, NodeInput,
	PeerId, PeerRegistration, Registry, RegistryDelta, ResourceEntry, ResourceId, Rev, SettledMarks, SourceValue, TimeStamp, Tombstone, UserId, Value, apply_attribute_delta, reverse_attribute_delta,
};
use std::borrow::Cow;

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
	/// Which hot ops are retired into history, so a late copy is dropped.
	pub(crate) settled: SettledMarks,
	/// The registry as of the last retirement, with no un-retired hot ops applied. Retirement computes
	/// each delta's `reverse` against this (so LWW reverses capture the true pre-op value, not the
	/// hot-polluted working state) and advances it. It equals the working registry whenever the hot log is
	/// empty.
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
	/// The latest retired commit a peer holds: commits after it undo silently, commits up to it stay.
	pub(crate) last_broadcast_rev: Option<Rev>,
	/// Shared-monotonic counter feeding `next_node_id`. Bumped on every mint regardless of which
	/// peer is calling; collision avoidance comes from hashing `(self.peer, counter)`, so two peers
	/// reading the same counter still produce distinct IDs.
	pub(crate) next_node_counter: u64,
	/// Counts this peer's own hot ops, so each carries its position in a gap-free run. See [`HotOp::sequence`].
	pub(crate) last_hot_sequence: HotSequence,
}

impl Document {
	pub(crate) fn empty(peer: PeerId, user: UserId) -> Self {
		Self {
			working_registry: Registry::default(),
			retired_snapshot: Registry::default(),
			history: History::new(),
			hot_log: Vec::new(),
			hot_timestamps: HashSet::new(),
			settled: SettledMarks::default(),
			head: None,
			redo_stack: Vec::new(),
			clock: LamportClock::new(peer),
			peer,
			user,
			last_broadcast_rev: None,
			next_node_counter: 0,
			last_hot_sequence: HotSequence::NONE,
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

	/// Apply a delta's `reverse` as the new forward op (silent-zone undo). Force-applied: structural
	/// ops are idempotent, and LWW arms assign the reverse value unconditionally even though it carries
	/// the same timestamp as the forward op it undoes.
	pub(crate) fn revert_delta(&mut self, target: RegistryTarget, mut delta: Delta) -> Result<(), CrdtError> {
		for parent in delta.all_parents() {
			if !self.history.contains(parent) {
				return Err(CrdtError::NotFoundInHistory(parent));
			}
		}
		std::mem::swap(&mut delta.kind, &mut delta.reverse);
		self.apply_op_with(target, delta.kind, delta.timestamp, ApplyMode::Force)
	}

	/// Stage a local op onto the working registry and the hot log, skipping the settled check so local work is never dropped.
	pub(crate) fn stage_hot_op(&mut self, hot_op: HotOp, mode: ApplyMode) -> Result<(), CrdtError> {
		self.apply_op_with(RegistryTarget::Working, hot_op.op.clone(), hot_op.timestamp, mode)?;
		self.hot_timestamps.insert(hot_op.timestamp);
		self.hot_log.push(hot_op);
		Ok(())
	}

	/// Replay a persisted or received hot op. One already settled or held changes nothing.
	pub fn replay_hot_op(&mut self, hot_op: HotOp) -> Result<(), CrdtError> {
		if self.settled.covers(hot_op.id()) {
			return Ok(());
		}
		if self.hot_timestamps.contains(&hot_op.timestamp) {
			return Ok(());
		}
		// Our own ops raise the sequence counter too, should the persisted one lag.
		if hot_op.timestamp.peer == self.peer {
			self.last_hot_sequence = self.last_hot_sequence.max(hot_op.sequence);
		}

		self.apply_op_idempotent(hot_op.op.clone(), hot_op.timestamp)?;
		self.hot_timestamps.insert(hot_op.timestamp);
		self.hot_log.push(hot_op);
		Ok(())
	}

	/// Take on a peer's marks as well as this peer's. Returns whether a hot op was dropped.
	pub(crate) fn absorb_settled_marks(&mut self, remote: &SettledMarks) -> bool {
		self.settled.absorb(remote);
		self.drop_settled_hot_ops()
	}

	/// Bring the stamp index back in line after the hot log was filtered as a whole.
	pub(crate) fn resync_hot_timestamps(&mut self) {
		self.hot_timestamps = self.hot_log.iter().map(|hot_op| hot_op.timestamp).collect();
	}

	/// Re-derive working as the snapshot plus every hot op, for a change that cannot apply in place. O(registry), so rare.
	pub(crate) fn rebuild_working(&mut self) {
		self.working_registry = self.retired_snapshot.clone();
		for hot_op in std::mem::take(&mut self.hot_log) {
			// No op fails on a referent, and one failing otherwise must not be lost.
			let _ = self.apply_op_idempotent(hot_op.op.clone(), hot_op.timestamp);
			self.hot_log.push(hot_op);
		}
	}

	/// Drop settled hot ops and re-derive working without them: their effect returns with their deltas.
	fn drop_settled_hot_ops(&mut self) -> bool {
		let before = self.hot_log.len();
		let settled = &self.settled;
		self.hot_log.retain(|hot_op| !settled.covers(hot_op.id()));
		let dropped = self.hot_log.len() != before;
		if dropped {
			self.resync_hot_timestamps();
			self.rebuild_working();
		}
		dropped
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
			RegistryTarget::Retired => &mut self.retired_snapshot,
		}
	}

	pub(crate) fn registry_ref(&self, target: RegistryTarget) -> &Registry {
		match target {
			RegistryTarget::Working => &self.working_registry,
			RegistryTarget::Retired => &self.retired_snapshot,
		}
	}

	/// A fresh local edit against the working registry; see [`ApplyMode::Strict`].
	#[cfg(test)]
	pub(crate) fn apply_op(&mut self, op: RegistryDelta, timestamp: TimeStamp) -> Result<(), CrdtError> {
		self.apply_op_with(RegistryTarget::Working, op, timestamp, ApplyMode::Strict)
	}

	/// An op from a peer or from persisted state, against the working registry.
	pub(crate) fn apply_op_idempotent(&mut self, op: RegistryDelta, timestamp: TimeStamp) -> Result<(), CrdtError> {
		self.apply_op_with(RegistryTarget::Working, op, timestamp, ApplyMode::Idempotent)
	}

	/// Silent-zone undo/redo rewind against the working registry: structural ops are idempotent, and
	/// LWW arms assign unconditionally. We own the single-writer chain here, so the precomputed reverse
	/// (undo) or forward (redo) value is authoritative even though its timestamp ties what it replaces.
	pub(crate) fn force_apply_op(&mut self, op: RegistryDelta, timestamp: TimeStamp) -> Result<(), CrdtError> {
		self.apply_op_with(RegistryTarget::Working, op, timestamp, ApplyMode::Force)
	}

	/// Applies one op. Every field, existence included, is last-writer-wins on a timestamp, so ops fold alike in any order:
	/// an addition writes every field, a removal keeps the content as a tombstone, a newer write or reference revives an
	/// older removal, and a write to an entity never seen lands on a placeholder until its addition arrives.
	pub(crate) fn apply_op_with(&mut self, target: RegistryTarget, op: RegistryDelta, timestamp: TimeStamp, mode: ApplyMode) -> Result<(), CrdtError> {
		// Advance the local clock past every observed op, including ones that subsequently no-op or
		// error. Observation is about causality knowledge, not about whether the op took effect.
		self.clock.observe(timestamp);

		let strict = mode == ApplyMode::Strict;
		let force = mode == ApplyMode::Force;

		let registry = self.registry_mut(target);
		match op {
			RegistryDelta::AddNode { id, node } => {
				if strict && registry.node_instances.contains_key(&id) {
					return Err(CrdtError::NodeAlreadyExists(id));
				}
				write_network(registry, node.network, timestamp, mode, |_| Ok(()))?;
				add_entity(&mut registry.node_instances, &mut registry.removed_nodes, id, node, timestamp, force);
			}
			RegistryDelta::RemoveNode { id, snapshot } => {
				remove_entity(&mut registry.node_instances, &mut registry.removed_nodes, id, snapshot, timestamp, force);
			}
			RegistryDelta::SetNodeInputs { id, mut inputs } => {
				inputs.iter_mut().for_each(|slot| stamp_listed_slot(slot, timestamp));
				let referenced: Vec<NodeId> = inputs
					.iter()
					.filter_map(|slot| match slot.input {
						NodeInput::Node { id, .. } => Some(id),
						_ => None,
					})
					.collect();
				ensure_seen(registry, &referenced, mode)?;
				write_node(registry, id, timestamp, mode, |node| {
					if force {
						node.inputs = inputs;
						node.inputs_timestamp = timestamp;
					} else {
						merge_inputs(node, inputs, timestamp);
					}
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
				ensure_seen(registry, referenced.as_slice(), mode)?;
				write_node(registry, id, timestamp, mode, |node| {
					let Some(input) = slot_for_write(node, index as usize, timestamp, mode)? else { return Ok(()) };
					if force || timestamp > input.timestamp {
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
				ensure_seen(registry, &[id], mode)?;
				if let Implementation::Network(network) = implementation {
					write_network(registry, network, timestamp, mode, |_| Ok(()))?;
				}
				write_node(registry, id, timestamp, mode, |node| {
					if force || timestamp > node.implementation_timestamp {
						node.implementation = implementation;
						node.implementation_timestamp = timestamp;
					}
					Ok(())
				})?;
			}
			RegistryDelta::ChangeNodeAttribute { id, delta } => {
				write_node(registry, id, timestamp, mode, |node| {
					apply_attribute_delta(delta, timestamp, force, &mut node.attributes, node.attributes_timestamp);
					Ok(())
				})?;
			}
			RegistryDelta::ChangeNodeInputAttribute { id, index, delta } => {
				write_node(registry, id, timestamp, mode, |node| {
					let Some(input) = slot_for_write(node, index as usize, timestamp, mode)? else { return Ok(()) };
					apply_attribute_delta(delta, timestamp, force, &mut input.attributes, input.attributes_timestamp);
					Ok(())
				})?;
			}
			RegistryDelta::SetNetworkExport { id, index, export } => {
				let referenced = match export {
					Some(NodeInput::Node { id, .. }) => Some(id),
					_ => None,
				};
				ensure_seen(registry, referenced.as_slice(), mode)?;
				write_network(registry, id, timestamp, mode, |net| {
					let slot_idx = index as usize;
					if slot_idx >= net.exports.len() {
						if slot_idx >= MAX_EXPORT_SLOTS {
							return Err(CrdtError::ExportSlotOutOfBounds(index));
						}
						// Past the end of a newer list the slot is gone, as with input slots.
						if !force && timestamp <= net.exports_timestamp {
							return Ok(());
						}
						let shape = net.exports_timestamp;
						net.exports.resize_with(slot_idx + 1, || ExportSlot { target: None, timestamp: shape });
					}

					let existing = &mut net.exports[slot_idx];
					if force || timestamp > existing.timestamp {
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
				if strict && registry.networks.contains_key(&id) {
					return Err(CrdtError::NetworkAlreadyExists(id));
				}
				add_entity(&mut registry.networks, &mut registry.removed_networks, id, network, timestamp, force);
			}
			RegistryDelta::RemoveNetwork { id, snapshot } => {
				remove_entity(&mut registry.networks, &mut registry.removed_networks, id, snapshot, timestamp, force);
			}
			RegistryDelta::ChangeNetworkAttribute { id, delta } => {
				write_network(registry, id, timestamp, mode, |net| {
					apply_attribute_delta(delta, timestamp, force, &mut net.attributes, net.attributes_timestamp);
					Ok(())
				})?;
			}
			RegistryDelta::SetResourceHash { id, hash } => {
				upsert_resource(registry, id, timestamp, |entry| {
					if force || timestamp > entry.hash_timestamp {
						entry.hash = hash;
						entry.hash_timestamp = timestamp;
					}
				});
			}
			RegistryDelta::AddSource { id, key, source } => {
				upsert_resource(registry, id, timestamp, |entry| {
					let value = SourceValue { source, timestamp, deleted: false };
					if force { entry.force_set_source(key, value) } else { entry.set_source(key, value) }
				});
			}
			RegistryDelta::RemoveSource { id, key } => {
				// An upsert, so a removal for an entry never seen leaves a tombstone its addition loses to.
				upsert_resource(registry, id, timestamp, |entry| {
					if force {
						entry.force_remove_source(&key);
					} else {
						entry.remove_source(&key, timestamp);
					}
				});
			}
			RegistryDelta::AddResource { id, entry } => {
				add_entity(&mut registry.resources, &mut registry.removed_resources, id, entry, timestamp, force);
			}
			RegistryDelta::RemoveResource { id, snapshot } => {
				remove_entity(&mut registry.resources, &mut registry.removed_resources, id, snapshot, timestamp, force);
			}
			RegistryDelta::RegisterPeer { peer, user } => {
				// The newest registration of a device wins, whatever order they land in.
				if registry.peer_users.get(&peer).is_none_or(|existing| timestamp > existing.timestamp) {
					registry.peer_users.insert(peer, PeerRegistration { user, timestamp });
				}
			}
			RegistryDelta::ChangeDocumentAttribute { delta } => {
				apply_attribute_delta(delta, timestamp, force, &mut registry.attributes, TimeStamp::ORIGIN);
			}
			// Merge is a structural sync point only; it mutates no registry state.
			RegistryDelta::Merge { .. } | RegistryDelta::EndTransaction | RegistryDelta::Other(_) => {}
		}
		Ok(())
	}

	/// Compute the inverse of `delta` against the registry named by `target`. Retirement passes
	/// [`RegistryTarget::Retired`] so LWW reverses (export target, inputs, attributes, resource hash)
	/// capture the true pre-op value rather than the hot-polluted working state.
	///
	/// A write reads its pre-op value from wherever it lands: the live entity, its tombstone, or for an
	/// entity never seen, a placeholder.
	pub(crate) fn compute_reverse_delta(&self, target: RegistryTarget, delta: &RegistryDelta) -> Result<RegistryDelta, CrdtError> {
		let registry = self.registry_ref(target);
		let node = |id: NodeId| registry.node_or_removed(id).map_or_else(|| Cow::Owned(Node::placeholder()), Cow::Borrowed);
		let unset = InputSlot::unset(TimeStamp::ORIGIN);
		let network = |id: NetworkId| registry.network_or_removed(id);
		let resource = |id: ResourceId| registry.resources.get(&id).or_else(|| registry.removed_resources.get(&id).map(|mark| &mark.content));
		Ok(match delta {
			RegistryDelta::AddNode { id, node } => RegistryDelta::RemoveNode { id: *id, snapshot: node.clone() },
			RegistryDelta::RemoveNode { id, snapshot } => RegistryDelta::AddNode { id: *id, node: snapshot.clone() },
			&RegistryDelta::SetNodeInputs { id, .. } => RegistryDelta::SetNodeInputs { id, inputs: node(id).inputs.clone() },
			&RegistryDelta::SetNodeImplementation { id, .. } => RegistryDelta::SetNodeImplementation {
				id,
				implementation: node(id).implementation.clone(),
			},
			&RegistryDelta::ChangeNodeInput { id, index: input_idx, .. } => {
				let node = node(id);
				let slot = node.inputs().get(input_idx as usize).unwrap_or(&unset);
				RegistryDelta::ChangeNodeInput {
					id,
					index: input_idx,
					new_input: slot.input.clone(),
				}
			}
			&RegistryDelta::ChangeNodeAttribute { id, ref delta } => RegistryDelta::ChangeNodeAttribute {
				id,
				delta: reverse_attribute_delta(delta, node(id).attributes()),
			},
			&RegistryDelta::ChangeNodeInputAttribute { id, index, ref delta } => {
				let node = node(id);
				let input = node.inputs().get(index as usize).unwrap_or(&unset);
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
				let empty = Attributes::new();
				let current = network(id).map_or(&empty, |net| &net.attributes);
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
			RegistryDelta::EndTransaction => RegistryDelta::EndTransaction,
			&RegistryDelta::Other(_) => RegistryDelta::Other(Value::None),
		})
	}
}

/// An entity whose existence and every field are last-writer-wins on a timestamp.
pub(crate) trait Entity: Sized {
	/// The newest stamp of an addition or a write: the latest evidence the entity exists.
	fn presence(&self) -> TimeStamp;
	fn set_presence(&mut self, timestamp: TimeStamp);
	/// Stamps every field at `timestamp`, the way an addition writes all of them.
	fn stamp_all(&mut self, timestamp: TimeStamp);
	/// Folds `other` in, keeping whichever value of each field is newer.
	fn merge(&mut self, other: Self);
	/// Content for writes to land on before the addition, every field at the origin so the addition's win.
	fn placeholder() -> Self;
}

impl Entity for Node {
	fn presence(&self) -> TimeStamp {
		self.presence
	}
	fn set_presence(&mut self, timestamp: TimeStamp) {
		self.presence = timestamp;
	}
	fn placeholder() -> Self {
		Node::new(crate::ROOT_NETWORK, Implementation::ProtoNode(ResourceId::from(0)), 0)
	}
	fn stamp_all(&mut self, timestamp: TimeStamp) {
		self.presence = timestamp;
		self.network_timestamp = timestamp;
		self.inputs_timestamp = timestamp;
		self.implementation_timestamp = timestamp;
		self.inputs.iter_mut().for_each(|slot| stamp_slot(slot, timestamp));
		stamp_attributes(&mut self.attributes, &mut self.attributes_timestamp, timestamp);
	}
	fn merge(&mut self, other: Self) {
		if other.network_timestamp > self.network_timestamp {
			self.network = other.network;
			self.network_timestamp = other.network_timestamp;
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

impl Entity for Network {
	fn presence(&self) -> TimeStamp {
		self.presence
	}
	fn set_presence(&mut self, timestamp: TimeStamp) {
		self.presence = timestamp;
	}
	fn placeholder() -> Self {
		Network::default()
	}
	fn stamp_all(&mut self, timestamp: TimeStamp) {
		self.presence = timestamp;
		self.exports_timestamp = timestamp;
		self.exports.iter_mut().for_each(|slot| slot.timestamp = timestamp);
		stamp_attributes(&mut self.attributes, &mut self.attributes_timestamp, timestamp);
	}
	fn merge(&mut self, other: Self) {
		// As `merge_inputs`: the newer list decides the slot count, and a slot written after that shape survives past it.
		let (mut newer, shape, older) = if other.exports_timestamp > self.exports_timestamp {
			(other.exports, other.exports_timestamp, std::mem::take(&mut self.exports))
		} else {
			(std::mem::take(&mut self.exports), self.exports_timestamp, other.exports)
		};
		for (index, incoming) in older.into_iter().enumerate() {
			if index >= newer.len() {
				if incoming.timestamp <= shape {
					continue;
				}
				newer.resize_with(index + 1, || ExportSlot { target: None, timestamp: shape });
			}
			if incoming.timestamp > newer[index].timestamp {
				newer[index] = incoming;
			}
		}
		self.exports = newer;
		self.exports_timestamp = shape;
		merge_attributes(&mut self.attributes, &mut self.attributes_timestamp, other.attributes, other.attributes_timestamp);
		self.presence = self.presence.max(other.presence);
	}
}

impl Entity for ResourceEntry {
	fn presence(&self) -> TimeStamp {
		self.presence
	}
	fn set_presence(&mut self, timestamp: TimeStamp) {
		self.presence = timestamp;
	}
	fn placeholder() -> Self {
		ResourceEntry::default()
	}
	fn stamp_all(&mut self, timestamp: TimeStamp) {
		self.presence = timestamp;
		self.hash_timestamp = timestamp;
		self.stamp_sources(timestamp);
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

fn stamp_slot(slot: &mut InputSlot, timestamp: TimeStamp) {
	slot.timestamp = timestamp;
	stamp_attributes(&mut slot.attributes, &mut slot.attributes_timestamp, timestamp);
}

/// Stamps a slot of a written input list. Attributes carried over with their own stamps keep them, and a floor carried
/// with them, so they only win where they already did; what arrives unstamped is written at `timestamp`.
fn stamp_listed_slot(slot: &mut InputSlot, timestamp: TimeStamp) {
	if slot.attributes_timestamp == TimeStamp::ORIGIN {
		return stamp_slot(slot, timestamp);
	}
	slot.timestamp = timestamp;
	slot.attributes
		.values_mut()
		.filter(|value| value.timestamp == TimeStamp::ORIGIN)
		.for_each(|value| value.timestamp = timestamp);
}

/// Writes the map whole at `timestamp`: the floor deletes every other key without a tombstone per key.
fn stamp_attributes(attributes: &mut Attributes, floor: &mut TimeStamp, timestamp: TimeStamp) {
	attributes.retain(|_, value| !value.deleted);
	attributes.values_mut().for_each(|value| value.timestamp = timestamp);
	*floor = timestamp;
}

/// Folds `other` in key by key: the newer entry wins, and a key only one map holds dies if the other map's floor is
/// newer, as that map was written whole after it.
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

/// Folds an input list stamped `timestamp` into the node's: the newer list decides the slot count, a slot written after that
/// shape survives past it, and a slot both hold keeps its newer value, so list and slot writes land alike in any order.
fn merge_inputs(node: &mut Node, other: Vec<InputSlot>, timestamp: TimeStamp) {
	let (mut newer, shape, older) = if timestamp > node.inputs_timestamp {
		(other, timestamp, std::mem::take(&mut node.inputs))
	} else {
		(std::mem::take(&mut node.inputs), node.inputs_timestamp, other)
	};
	// A slot the older list lacks was absent as of that list's shape, as a write past its end would find it.
	let older_shape = if shape == timestamp { node.inputs_timestamp } else { timestamp };
	for slot in newer.iter_mut().skip(older.len()) {
		merge_attributes(&mut slot.attributes, &mut slot.attributes_timestamp, Attributes::new(), older_shape);
	}
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

/// The slot at `index` for a write at `timestamp`; past the end it exists only if the write is newer than the list's shape.
fn slot_for_write(node: &mut Node, index: usize, timestamp: TimeStamp, mode: ApplyMode) -> Result<Option<&mut InputSlot>, CrdtError> {
	if index >= node.inputs.len() {
		if index >= MAX_INPUT_SLOTS || mode == ApplyMode::Strict {
			return Err(CrdtError::InputIndexOutOfBounds(index));
		}
		if mode != ApplyMode::Force && timestamp <= node.inputs_timestamp {
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

/// Revives a removed entity whose presence has grown past its removal. A placeholder stays dead.
fn revive_if_newer<K: Hash + Eq + Copy, T: Entity>(live: &mut HashMap<K, T>, dead: &mut HashMap<K, Tombstone<T>>, id: K) {
	if dead.get(&id).is_some_and(|mark| !mark.placeholder && mark.content.presence() > mark.timestamp) {
		let mark = dead.remove(&id).expect("checked above");
		live.insert(id, mark.content);
	}
}

/// Folds an addition's or removal's content into what is held under `id`. It proves the entity was added, so a placeholder
/// holding it stops being one.
fn merge_entity<K: Hash + Eq + Copy, T: Entity>(live: &mut HashMap<K, T>, dead: &mut HashMap<K, Tombstone<T>>, id: K, content: T) {
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
	revive_if_newer(live, dead, id);
}

/// An addition at `timestamp`: every field written at `timestamp`, live unless a newer removal holds it dead.
fn add_entity<K: Hash + Eq + Copy, T: Entity>(live: &mut HashMap<K, T>, dead: &mut HashMap<K, Tombstone<T>>, id: K, mut content: T, timestamp: TimeStamp, force: bool) {
	if force {
		dead.remove(&id);
		live.insert(id, content);
		return;
	}
	content.stamp_all(timestamp);
	merge_entity(live, dead, id, content);
}

/// A removal at `timestamp`: dead unless a newer addition or write holds it live. The snapshot folds in first, so a removal of an
/// entity never seen still lands.
fn remove_entity<K: Hash + Eq + Copy, T: Entity>(live: &mut HashMap<K, T>, dead: &mut HashMap<K, Tombstone<T>>, id: K, snapshot: T, timestamp: TimeStamp, force: bool) {
	merge_entity(live, dead, id, snapshot);
	if let Some(mark) = dead.get_mut(&id) {
		mark.timestamp = if force { timestamp } else { mark.timestamp.max(timestamp) };
	} else if live.get(&id).is_some_and(|content| force || timestamp > content.presence()) {
		let content = live.remove(&id).expect("checked above");
		dead.insert(
			id,
			Tombstone {
				content,
				timestamp,
				placeholder: false,
			},
		);
	}
}

/// A write at `timestamp` to the entity under `id`, wherever its content sits: it revives an older removal, and lands on a
/// placeholder for an entity never seen. A fresh local edit on one gets `missing()` instead.
fn write_entity<K: Hash + Eq + Copy, T: Entity>(
	live: &mut HashMap<K, T>,
	dead: &mut HashMap<K, Tombstone<T>>,
	id: K,
	timestamp: TimeStamp,
	mode: ApplyMode,
	missing: impl FnOnce() -> CrdtError,
	f: impl FnOnce(&mut T) -> Result<(), CrdtError>,
) -> Result<(), CrdtError> {
	let force = mode == ApplyMode::Force;
	let unseen = !live.contains_key(&id) && !dead.contains_key(&id);
	if unseen {
		if mode == ApplyMode::Strict {
			return Err(missing());
		}
		dead.insert(
			id,
			Tombstone {
				content: T::placeholder(),
				timestamp: TimeStamp::ORIGIN,
				placeholder: true,
			},
		);
	}
	let content = content_mut(live, dead, id).expect("held above");
	if let Err(error) = f(content) {
		// A rejected op changes nothing, so the placeholder made for it goes too.
		if unseen {
			dead.remove(&id);
		}
		return Err(error);
	}
	content.set_presence(content.presence().max(timestamp));
	if force && let Some(mark) = dead.remove(&id) {
		live.insert(id, mark.content);
	}
	revive_if_newer(live, dead, id);
	Ok(())
}

/// A fresh local edit cannot name a node never seen. Checked before an op's first write, so a refused op changes nothing.
fn ensure_seen(registry: &Registry, ids: &[NodeId], mode: ApplyMode) -> Result<(), CrdtError> {
	let unseen = |id: &&NodeId| !registry.node_instances.contains_key(id) && !registry.removed_nodes.contains_key(id);
	match ids.iter().find(unseen) {
		Some(&id) if mode == ApplyMode::Strict => Err(CrdtError::TargetNodeDoesNotExist(id)),
		_ => Ok(()),
	}
}

/// [`write_entity()`] to a node; with a no-op `f`, records a reference to it.
fn write_node(registry: &mut Registry, id: NodeId, timestamp: TimeStamp, mode: ApplyMode, f: impl FnOnce(&mut Node) -> Result<(), CrdtError>) -> Result<(), CrdtError> {
	write_entity(
		&mut registry.node_instances,
		&mut registry.removed_nodes,
		id,
		timestamp,
		mode,
		|| CrdtError::TargetNodeDoesNotExist(id),
		f,
	)
}

fn write_network(registry: &mut Registry, id: NetworkId, timestamp: TimeStamp, mode: ApplyMode, f: impl FnOnce(&mut Network) -> Result<(), CrdtError>) -> Result<(), CrdtError> {
	write_entity(&mut registry.networks, &mut registry.removed_networks, id, timestamp, mode, || CrdtError::NetworkDoesNotExist(id), f)
}

/// [`write_entity()`] for a resource, creating an entry never seen first, since single-field resource ops are upserts.
fn upsert_resource(registry: &mut Registry, id: ResourceId, timestamp: TimeStamp, f: impl FnOnce(&mut ResourceEntry)) {
	if !registry.resources.contains_key(&id) && !registry.removed_resources.contains_key(&id) {
		registry.resources.insert(
			id,
			ResourceEntry {
				presence: timestamp,
				..ResourceEntry::default()
			},
		);
	}
	let entry = content_mut(&mut registry.resources, &mut registry.removed_resources, id).expect("held above");
	f(entry);
	entry.set_presence(entry.presence().max(timestamp));
	revive_if_newer(&mut registry.resources, &mut registry.removed_resources, id);
}

/// Which of a [`Document`]'s two registries an apply targets: the working copy (retired state plus
/// live hot ops) or the retired snapshot (retired deltas only). Retirement targets the snapshot so
/// reverses capture pre-op values; the hot path and undo/redo target the working copy.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum RegistryTarget {
	Working,
	Retired,
}

/// How [`Document::apply_op_with`] treats what it finds.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ApplyMode {
	/// A fresh local edit: adding what exists or writing to what was never seen is an error.
	Strict,
	/// An op from a peer or from persisted state: every arm is a timestamp comparison.
	Idempotent,
	/// Silent-zone undo/redo rewind: every write lands, whatever its timestamp.
	Force,
}
