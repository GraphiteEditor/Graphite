use crate::{
	ApplyMode, CrdtError, Delta, ExportSlot, HotOp, LamportClock, MAX_EXPORT_SLOTS, NetworkId, NodeId, NodeInput, PeerId, Registry, RegistryDelta, RegistryTarget, ResourceEntry, Rev, SourceValue,
	TimeStamp, Value, apply_attribute_delta, attr, mint_node_id, reverse_attribute_delta,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Document {
	/// Working registry: retired state with the current hot ops applied on top. This is what live
	/// reads and `registry()` observe, and what undo/redo force-apply against.
	pub(crate) registry: Registry,
	/// The registry as of the last retirement, with no un-retired hot ops applied. Retirement computes
	/// each delta's `reverse` against this (so LWW reverses capture the true pre-op value, not the
	/// hot-polluted working state) and advances it, stamping fields at the fresh `T_retire`. Kept equal
	/// to `registry` *by value* whenever the hot log is empty (undo/redo resync it after moving the
	/// cursor), but field timestamps can differ: retirement bumps the snapshot's to `T_retire` while the
	/// working registry keeps the staging-time timestamps. Benign while the local monotonic clock makes
	/// new edits win; matters once remote ops can race a local field (collaboration milestone).
	pub(crate) retired_snapshot: Registry,
	pub(crate) history: HashMap<Rev, Delta>,
	/// Live broadcast stream — applied to the registry on receive, GC'd at retirement.
	/// Persisted for crash recovery so in-flight unretired work survives editor restarts.
	pub(crate) hot_log: Vec<HotOp>,
	/// User's cursor in their local chain.
	pub(crate) head: Rev,
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
}

impl Document {
	/// Mint a fresh `NodeId` scoped to this document's peer. The 64-bit ID is `blake3(peer, counter)`
	/// truncated; the counter is shared across peers and persisted with the document.
	pub fn next_node_id(&mut self) -> NodeId {
		self.next_node_counter += 1;
		mint_node_id(self.peer, self.next_node_counter)
	}

	pub fn restore_node_from_history(&mut self, target: RegistryTarget, old_node_id: NodeId) -> Result<(), CrdtError> {
		let delta = self
			.history_iter()
			.find(|d| matches!(d.reverse, RegistryDelta::AddNode { node_id, .. } if node_id == old_node_id))
			.ok_or(CrdtError::NotFoundInHistory)?
			.clone();
		self.revert_delta(target, delta)
	}

	pub fn restore_network_from_history(&mut self, target: RegistryTarget, network_id: NetworkId) -> Result<(), CrdtError> {
		// Find the Delta whose forward op removed this network. Its `reverse` is `AddNetwork`,
		// which is what we want to re-apply.
		let delta = self
			.history_iter()
			.find(|d| matches!(&d.reverse, RegistryDelta::AddNetwork { network, .. } if *network == network_id))
			.ok_or(CrdtError::NotFoundInHistory)?
			.clone();
		self.revert_delta(target, delta)
	}

	/// Apply a delta's `reverse` as the new forward op (silent-zone undo). Force-applied: structural
	/// ops are idempotent, and LWW arms assign the reverse value unconditionally even though it carries
	/// the same timestamp as the forward op it undoes.
	pub fn revert_delta(&mut self, target: RegistryTarget, mut delta: Delta) -> Result<(), CrdtError> {
		std::mem::swap(&mut delta.delta_type, &mut delta.reverse);
		for parent in &delta.parents {
			assert!(self.history.contains_key(parent));
		}
		self.apply_op_with(target, delta.delta_type, delta.timestamp, ApplyMode::Force)
	}

	/// Apply a live broadcast op. Updates the registry via LWW and appends to the hot log.
	/// Doesn't touch history or `head` — hot ops are transient.
	pub fn apply_hot_op(&mut self, hot_op: HotOp) -> Result<(), CrdtError> {
		self.apply_hot_op_with(hot_op, false)
	}

	/// Replay a hot op recovered from persisted state. Idempotent on structural ops so that
	/// re-applying an op whose effect is already reflected in the registry is a no-op rather
	/// than an error.
	pub fn replay_hot_op(&mut self, hot_op: HotOp) -> Result<(), CrdtError> {
		self.apply_hot_op_with(hot_op, true)
	}

	fn apply_hot_op_with(&mut self, hot_op: HotOp, idempotent: bool) -> Result<(), CrdtError> {
		if idempotent {
			self.apply_op_idempotent(hot_op.op.clone(), hot_op.timestamp)?;
		} else {
			self.apply_op(hot_op.op.clone(), hot_op.timestamp)?;
		}
		self.hot_log.push(hot_op);
		Ok(())
	}

	/// Apply a retired commit. Idempotent on structural ops (AddNode/AddNetwork on existing
	/// targets, Remove on missing ones) since hot ops already produced the structural state.
	/// The point is to bump field timestamps to T_retire via the LWW arms.
	pub fn apply_retired_delta(&mut self, delta: Delta) -> Result<(), CrdtError> {
		for parent in &delta.parents {
			assert!(self.history.contains_key(parent));
		}
		self.apply_op_idempotent(delta.delta_type.clone(), delta.timestamp)?;
		self.history.insert(delta.id, delta);
		Ok(())
	}

	/// The registry an apply reads and writes, resolved from the explicit [`RegistryTarget`].
	fn registry_mut(&mut self, target: RegistryTarget) -> &mut Registry {
		match target {
			RegistryTarget::Working => &mut self.registry,
			RegistryTarget::Snapshot => &mut self.retired_snapshot,
		}
	}

	fn registry_ref(&self, target: RegistryTarget) -> &Registry {
		match target {
			RegistryTarget::Working => &self.registry,
			RegistryTarget::Snapshot => &self.retired_snapshot,
		}
	}

	/// New local/remote op against the working registry: structural ops error on duplicate/missing
	/// targets; LWW arms keep the newer-timestamp value (strict `>`). The common entry point for edits.
	pub(crate) fn apply_op(&mut self, op: RegistryDelta, timestamp: TimeStamp) -> Result<(), CrdtError> {
		self.apply_op_with(RegistryTarget::Working, op, timestamp, ApplyMode::Live)
	}

	/// Replay/retire against the working registry: structural ops skip duplicate/missing targets (the
	/// state is already present from hot ops or a prior snapshot); LWW arms still gate on strict `>`.
	pub(crate) fn apply_op_idempotent(&mut self, op: RegistryDelta, timestamp: TimeStamp) -> Result<(), CrdtError> {
		self.apply_op_with(RegistryTarget::Working, op, timestamp, ApplyMode::Idempotent)
	}

	/// Silent-zone undo/redo rewind against the working registry: structural ops are idempotent, and
	/// LWW arms assign unconditionally. We own the single-writer chain here, so the precomputed reverse
	/// (undo) or forward (redo) value is authoritative even though its timestamp ties what it replaces.
	pub(crate) fn force_apply_op(&mut self, op: RegistryDelta, timestamp: TimeStamp) -> Result<(), CrdtError> {
		self.apply_op_with(RegistryTarget::Working, op, timestamp, ApplyMode::Force)
	}

	pub(crate) fn apply_op_with(&mut self, target: RegistryTarget, op: RegistryDelta, timestamp: TimeStamp, mode: ApplyMode) -> Result<(), CrdtError> {
		// Advance the local clock past every observed op, including ones that subsequently no-op or
		// error. Observation is about causality knowledge, not about whether the op took effect.
		self.clock.observe(timestamp);

		// Structural ops skip (rather than error) on duplicate/missing targets when not a fresh edit;
		// LWW arms assign unconditionally only under `Force`.
		let idempotent = mode != ApplyMode::Live;
		let force = mode == ApplyMode::Force;

		// Resurrect any concurrently-removed targets the op references before binding the registry
		// (resurrection re-borrows `self` via history), so the mutation below holds one `registry` ref.
		self.ensure_referenced_exist(target, &op)?;

		let registry = self.registry_mut(target);
		match op {
			RegistryDelta::AddNode { node_id, node } => {
				if registry.node_instances.contains_key(&node_id) {
					if idempotent {
						// Hot ops already created this node; skip rather than error.
						return Ok(());
					}
					return Err(CrdtError::NodeAlreadyExists);
				}
				registry.node_instances.insert(node_id, node);
			}
			RegistryDelta::RemoveNode { node_id, .. } => {
				registry.node_instances.remove(&node_id);
			}
			RegistryDelta::ChangeNodeInput { node_id, input_idx, new_input } => {
				let node = registry.node_instances.get_mut(&node_id).ok_or(CrdtError::TargetNodeDoesNotExist)?;
				let slot = node.inputs.get_mut(input_idx).ok_or(CrdtError::InputIndexOutOfBounds)?;
				if force || timestamp > slot.timestamp {
					slot.input = new_input;
					slot.timestamp = timestamp;
				}
			}
			RegistryDelta::ChangeNodeAttribute { node_id, delta } => {
				let node = registry.node_instances.get_mut(&node_id).ok_or(CrdtError::TargetNodeDoesNotExist)?;
				apply_attribute_delta(delta, timestamp, force, &mut node.attributes);
			}
			RegistryDelta::ChangeNodeInputAttribute { node_id, input_idx, delta } => {
				let node = registry.node_instances.get_mut(&node_id).ok_or(CrdtError::TargetNodeDoesNotExist)?;
				let input_attributes = node.inputs_attributes.get_mut(input_idx).ok_or(CrdtError::InputIndexOutOfBounds)?;
				apply_attribute_delta(delta, timestamp, force, input_attributes);
			}
			RegistryDelta::SetExport { network, slot, target: export_target } => {
				let net = registry.networks.get_mut(&network).ok_or(CrdtError::NetworkDoesNotExist)?;
				let slot_idx = slot as usize;

				if slot_idx >= net.exports.len() {
					if slot_idx >= MAX_EXPORT_SLOTS {
						return Err(CrdtError::ExportSlotOutOfBounds);
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
					existing.target = export_target;
					existing.timestamp = timestamp;
				}
			}
			RegistryDelta::AddNetwork { network, contents } => {
				if registry.networks.contains_key(&network) {
					if idempotent {
						return Ok(());
					}
					return Err(CrdtError::NetworkAlreadyExists);
				}
				registry.networks.insert(network, contents);
			}
			RegistryDelta::RemoveNetwork { network, .. } => {
				registry.networks.remove(&network);
			}
			RegistryDelta::SetExportedNodes { nodes } => {
				let current_ts = registry.attributes.get(attr::EXPORTED_NODES_TS).map(|v| v.timestamp).unwrap_or(TimeStamp::ORIGIN);
				if force || timestamp > current_ts {
					registry.exported_nodes = nodes;
					registry.attributes.insert(
						attr::EXPORTED_NODES_TS.to_string(),
						Value {
							value: serde_json::Value::Null,
							timestamp,
						},
					);
				}
			}
			RegistryDelta::ChangeNetworkAttribute { network, delta } => {
				let net = registry.networks.get_mut(&network).ok_or(CrdtError::NetworkDoesNotExist)?;
				apply_attribute_delta(delta, timestamp, force, &mut net.attributes);
			}
			RegistryDelta::ChangeDocumentAttribute { delta } => {
				apply_attribute_delta(delta, timestamp, force, &mut registry.attributes);
			}
			RegistryDelta::RegisterPeer { peer, user } => match registry.peer_users.get(&peer) {
				Some(existing) if *existing != user => return Err(CrdtError::PeerRegistrationConflict),
				Some(_) => {}
				None => {
					registry.peer_users.insert(peer, user);
				}
			},
			RegistryDelta::SetResourceHash { id, hash } => {
				let entry = registry.resources.entry(id).or_default();
				if force || timestamp > entry.hash_timestamp {
					entry.hash = hash;
					entry.hash_timestamp = timestamp;
				}
			}
			RegistryDelta::AddSource { id, key, source } => {
				let entry = registry.resources.entry(id).or_default();
				let value = SourceValue { source, timestamp };
				if force { entry.force_set_source(key, value) } else { entry.set_source(key, value) }
			}
			RegistryDelta::RemoveSource { id, key } => {
				if let Some(entry) = registry.resources.get_mut(&id) {
					if force {
						entry.force_remove_source(&key);
					} else {
						entry.remove_source(&key, timestamp);
					}
				}
			}
			RegistryDelta::AddResource { id, entry } => {
				registry.resources.insert(id, entry);
			}
			RegistryDelta::RemoveResource { id, .. } => {
				registry.resources.remove(&id);
			}
		}
		Ok(())
	}

	/// Resurrect (from history) any nodes/networks an op references that were concurrently removed, so
	/// the op applies against a consistent registry. Cascading: a node's owning network is restored
	/// before the node. No-op for ops that reference nothing absent.
	fn ensure_referenced_exist(&mut self, target: RegistryTarget, op: &RegistryDelta) -> Result<(), CrdtError> {
		match op {
			RegistryDelta::AddNode { node, .. } => self.ensure_network_exists(target, node.network())?,
			RegistryDelta::ChangeNodeInput { node_id, new_input, .. } => {
				if let NodeInput::Node { node_id: referenced, .. } = new_input {
					self.ensure_node_exists(target, *referenced)?;
				}
				self.ensure_node_exists(target, *node_id)?;
			}
			RegistryDelta::ChangeNodeAttribute { node_id, .. } | RegistryDelta::ChangeNodeInputAttribute { node_id, .. } => self.ensure_node_exists(target, *node_id)?,
			RegistryDelta::SetExport { network, target: export_target, .. } => {
				if let Some(NodeInput::Node { node_id: referenced, .. }) = export_target {
					self.ensure_node_exists(target, *referenced)?;
				}
				self.ensure_network_exists(target, *network)?;
			}
			RegistryDelta::ChangeNetworkAttribute { network, .. } => self.ensure_network_exists(target, *network)?,
			_ => {}
		}
		Ok(())
	}

	fn ensure_node_exists(&mut self, target: RegistryTarget, node_id: u64) -> Result<(), CrdtError> {
		if !self.registry_ref(target).node_instances.contains_key(&node_id) {
			self.restore_node_from_history(target, node_id)?;
		}
		Ok(())
	}

	fn ensure_network_exists(&mut self, target: RegistryTarget, network_id: NetworkId) -> Result<(), CrdtError> {
		if !self.registry_ref(target).networks.contains_key(&network_id) {
			self.restore_network_from_history(target, network_id)?;
		}
		Ok(())
	}

	/// Compute the inverse of `delta` against the registry named by `target`. Retirement passes
	/// [`RegistryTarget::Snapshot`] so LWW reverses (export target, inputs, attributes, resource hash)
	/// capture the true pre-op value rather than the hot-polluted working state.
	pub(crate) fn compute_reverse_delta(&self, target: RegistryTarget, delta: &RegistryDelta) -> Result<RegistryDelta, CrdtError> {
		let registry = self.registry_ref(target);
		Ok(match delta {
			RegistryDelta::AddNode { node_id, node } => RegistryDelta::RemoveNode {
				node_id: *node_id,
				snapshot: node.clone(),
			},
			RegistryDelta::RemoveNode { node_id, snapshot } => RegistryDelta::AddNode {
				node_id: *node_id,
				node: snapshot.clone(),
			},
			&RegistryDelta::ChangeNodeInput { node_id, input_idx, .. } => {
				let node = registry.node_instances.get(&node_id).ok_or(CrdtError::TargetNodeDoesNotExist)?;
				let slot = node.inputs().get(input_idx).ok_or(CrdtError::InputIndexOutOfBounds)?;
				RegistryDelta::ChangeNodeInput {
					node_id,
					input_idx,
					new_input: slot.input.clone(),
				}
			}
			&RegistryDelta::ChangeNodeAttribute { node_id, ref delta } => {
				let node = registry.node_instances.get(&node_id).ok_or(CrdtError::TargetNodeDoesNotExist)?;
				RegistryDelta::ChangeNodeAttribute {
					node_id,
					delta: reverse_attribute_delta(delta, node.attributes()),
				}
			}
			&RegistryDelta::ChangeNodeInputAttribute { node_id, input_idx, ref delta } => {
				let node = registry.node_instances.get(&node_id).ok_or(CrdtError::TargetNodeDoesNotExist)?;
				let input_attributes = node.inputs_attributes().get(input_idx).ok_or(CrdtError::InputIndexOutOfBounds)?;
				RegistryDelta::ChangeNodeInputAttribute {
					node_id,
					input_idx,
					delta: reverse_attribute_delta(delta, input_attributes),
				}
			}
			&RegistryDelta::SetExport { network, slot, .. } => {
				// If the network is absent the forward op will resurrect it; the reverse is "set the export to None"
				// since pre-forward there was no export to point at.
				let export_target = registry.networks.get(&network).and_then(|net| net.exports.get(slot as usize)).and_then(|s| s.target.clone());
				RegistryDelta::SetExport { network, slot, target: export_target }
			}
			RegistryDelta::AddNetwork { network, contents } => RegistryDelta::RemoveNetwork {
				network: *network,
				snapshot: contents.clone(),
			},
			&RegistryDelta::RemoveNetwork { network, ref snapshot } => RegistryDelta::AddNetwork { network, contents: snapshot.clone() },
			RegistryDelta::SetExportedNodes { .. } => RegistryDelta::SetExportedNodes {
				nodes: registry.exported_nodes.clone(),
			},
			&RegistryDelta::ChangeNetworkAttribute { network, ref delta } => {
				let current = registry.networks.get(&network).map(|net| &net.attributes).ok_or(CrdtError::NetworkDoesNotExist)?;
				RegistryDelta::ChangeNetworkAttribute {
					network,
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
				hash: registry.resources.get(&id).and_then(|entry| entry.hash),
			},
			&RegistryDelta::AddSource { id, key, .. } => match registry.resources.get(&id).and_then(|entry| entry.source(&key)) {
				// The slot already held a source: undo restores it.
				Some(existing) => RegistryDelta::AddSource {
					id,
					key,
					source: existing.source.clone(),
				},
				// The slot was empty: undo removes what this op added.
				None => RegistryDelta::RemoveSource { id, key },
			},
			&RegistryDelta::RemoveSource { id, key } => match registry.resources.get(&id).and_then(|entry| entry.source(&key)) {
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
		})
	}

	/// Retired-only walk from `head` along first parents. Hot ops are excluded by design.
	fn history_iter(&self) -> HistoryIter<'_> {
		HistoryIter {
			document: self,
			parent_rev: self.head,
		}
	}
}

struct HistoryIter<'a> {
	document: &'a Document,
	parent_rev: Rev,
}

impl<'a> Iterator for HistoryIter<'a> {
	type Item = &'a Delta;

	fn next(&mut self) -> Option<Self::Item> {
		let delta = self.document.history.get(&self.parent_rev)?;
		// First parent only for now. Local-chain walking (filter by author) is a follow-up.
		self.parent_rev = *delta.parents.first()?;
		Some(delta)
	}
}
