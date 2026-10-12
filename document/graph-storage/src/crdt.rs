use crate::{Attributes, Implementation, InputSlot, Network, NetworkId, Node, NodeId, NodeInput, PeerId, ResourceEntry, ResourceId, Rev, SourceKey, TimeStamp, UserId, Value, attr, compute_rev};
use graphene_resource::ResourceHash;
use serde::{Deserialize, Serialize};

/// Content-addressed delta: `id` is `blake3_128(parents, author, timestamp, delta_type)`.
///
/// `reverse` is undo bookkeeping that depends on local state, so it is stored but left out of the identity hash.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Delta {
	pub id: Rev,
	/// Primary parent; `None` for the root delta.
	pub parent: Option<Rev>,
	pub author: PeerId,
	pub timestamp: TimeStamp,
	pub kind: RegistryDelta,
	pub reverse: RegistryDelta,
	/// Local, mutable annotations on this commit (interaction-end marker, future commit messages / labels).
	/// Deliberately excluded from `compute_rev`: relabeling a commit must not change its content-addressed
	/// identity, and two peers annotating the same op differently must still dedup to one `Rev`.
	#[serde(default)]
	pub attributes: Attributes,
	/// When the delta entered history, in Unix milliseconds by the retiring peer's clock; display only and outside the rev.
	/// Zero means unrecorded, saving the tag byte of an `Option`. Last, since the history codec is positional.
	#[serde(default)]
	pub(crate) retired_at_ms: u64,
}

impl Delta {
	pub fn new(parent: Option<Rev>, author: PeerId, timestamp: TimeStamp, kind: RegistryDelta, reverse: RegistryDelta) -> Self {
		let id = compute_rev(parent, author, timestamp, &kind);
		Self {
			id,
			parent,
			author,
			timestamp,
			kind,
			reverse,
			attributes: Attributes::default(),
			retired_at_ms: 0,
		}
	}

	/// Build a merge delta joining `tips` into one node. See [`RegistryDelta::Merge`] for the semantics.
	pub fn merge(tips: impl IntoIterator<Item = Rev>, author: PeerId, timestamp: TimeStamp) -> Self {
		let mut parents: Vec<Rev> = tips.into_iter().collect();
		parents.sort_unstable();
		parents.dedup();
		let parent = parents.first().copied();
		let extra_parents = parents.split_first().map(|(_, rest)| rest.to_vec()).unwrap_or_default();
		let kind = RegistryDelta::Merge { extra_parents };
		let id = compute_rev(parent, author, timestamp, &kind);
		Self {
			id,
			parent,
			author,
			timestamp,
			reverse: kind.clone(),
			kind,
			attributes: Attributes::default(),
			retired_at_ms: 0,
		}
	}

	/// Every parent: the primary `parent` (absent for the root) plus a merge's `extra_parents`.
	pub fn all_parents(&self) -> impl Iterator<Item = Rev> + '_ {
		let extras = match &self.kind {
			RegistryDelta::Merge { extra_parents } => extra_parents.as_slice(),
			_ => &[],
		};
		self.parent.into_iter().chain(extras.iter().copied())
	}

	/// Mark this delta as the last op of a user interaction, so the undo cursor treats it as a checkpoint.
	pub fn mark_interaction_end(&mut self, timestamp: TimeStamp) {
		self.attributes.set(attr::delta::INTERACTION_END, Value::Bool(true), timestamp);
	}

	pub fn is_interaction_end(&self) -> bool {
		self.attributes.get(attr::delta::INTERACTION_END).is_some_and(|marker| marker.value == Value::Bool(true))
	}

	/// When the delta entered history in wall-clock milliseconds, if the peer that retired it recorded it.
	pub fn retired_at(&self) -> Option<u64> {
		(self.retired_at_ms != 0).then_some(self.retired_at_ms)
	}

	/// The content-addressed `Rev` this delta's identity fields hash to. Equals `id` for a delta built
	/// via `new`/`merge`; differs only if `id` was tampered with or the hash derivation changed.
	pub fn recomputed_id(&self) -> Rev {
		compute_rev(self.parent, self.author, self.timestamp, &self.kind)
	}

	/// Whether `id` matches the recomputed content hash. `Delta` deserializes without checking this
	/// (the hash is not cheap over a large history); callers verify explicitly when they don't trust
	/// the source via [`Session::verify_history`].
	pub fn has_valid_id(&self) -> bool {
		self.id == self.recomputed_id()
	}
}

/// Op payload. Timestamps live on the wrapping `Delta` — one per delta, applied to all LWW-eligible
/// writes within. See `notes/document-format-collaboration.md`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum RegistryDelta {
	AddNode {
		id: NodeId,
		node: Node,
	},
	/// `snapshot` is the node as removed, which folds in like any write so a removal of a node never seen still lands.
	RemoveNode {
		id: NodeId,
		snapshot: Node,
	},
	ChangeNodeInput {
		id: NodeId,
		index: u32,
		new_input: NodeInput,
	},
	/// A node's whole input list, for a change to the number or order of its slots that the
	/// index-addressed `ChangeNodeInput` cannot express. Assigns rather than merging: concurrent
	/// per-slot edits are lost, which is inherent to the indices themselves moving.
	///
	/// Touches only the inputs, leaving the node's attributes and implementation alone, so it composes
	/// with attribute ops on the same node instead of reverting them.
	SetNodeInputs {
		id: NodeId,
		inputs: Vec<InputSlot>,
	},
	/// A node's implementation, for swapping what it computes without rebuilding the node.
	///
	/// Removing and re-adding the node would express the same change, but would also clear every
	/// attribute it carries, so restating them would clobber whatever a concurrent peer wrote to the
	/// node's name, lock or pin.
	SetNodeImplementation {
		id: NodeId,
		implementation: Implementation,
	},
	ChangeNodeAttribute {
		id: NodeId,
		delta: AttributeDelta,
	},
	ChangeNodeInputAttribute {
		id: NodeId,
		index: u32,
		delta: AttributeDelta,
	},
	/// LWW per slot. `export == None` removes the slot.
	SetNetworkExport {
		id: NetworkId,
		index: u32,
		export: Option<NodeInput>,
	},
	/// Per-network attribute change, LWW per key. Mirrors `ChangeDocumentAttribute`.
	ChangeNetworkAttribute {
		id: NetworkId,
		delta: AttributeDelta,
	},
	AddNetwork {
		id: NetworkId,
		network: Network,
	},
	/// `snapshot` is the network as removed; see [`RemoveNode`](Self::RemoveNode).
	RemoveNetwork {
		id: NetworkId,
		snapshot: Network,
	},
	/// Register a whole resource entry at once, the way `AddNetwork` pairs with `RemoveNetwork`.
	AddResource {
		id: ResourceId,
		entry: ResourceEntry,
	},
	/// LWW on a resource's resolved content hash. Creates the resource entry if absent.
	/// Concurrent resolves agree by construction (the hash is content-derived), so LWW is safe.
	SetResourceHash {
		id: ResourceId,
		hash: Option<ResourceHash>,
	},
	/// Remove a whole resource entry. `snapshot` is the state of the resource before it was removed.
	RemoveResource {
		id: ResourceId,
		snapshot: ResourceEntry,
	},
	/// Add (or LWW-overwrite) one entry in a resource's source fallback chain. The source body is
	/// type-erased; `key` carries the fractional priority + peer that order it. Add-wins: concurrent
	/// adds at distinct keys all survive. Creates the resource entry if absent.
	AddSource {
		id: ResourceId,
		key: SourceKey,
		source: Value,
	},
	/// Remove one entry from a resource's source chain. LWW against the entry's timestamp.
	RemoveSource {
		id: ResourceId,
		key: SourceKey,
	},
	/// Registers a device's `PeerId` to the `UserId` behind it; the newest registration wins.
	RegisterPeer {
		peer: PeerId,
		user: UserId,
	},
	ChangeDocumentAttribute {
		delta: AttributeDelta,
	},
	/// Joins divergent history tips into one shared node. A registry no-op on replay (it only collapses
	/// tips so `head` stays a single `Rev`); the joined tips are `Delta::parent` (the lowest `Rev`) plus
	/// these `extra_parents` (sorted). Identity is the parent set alone, so two peers merging the same
	/// tips mint the identical delta and it dedups.
	Merge {
		extra_parents: Vec<Rev>,
	},
	// Allow for future delta types without a model change
	Other(Value),
}

/// `value: None` means remove. The timestamp comes from the wrapping `Delta`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttributeDelta {
	pub key: String,
	pub value: Option<Value>,
}

pub(crate) fn reverse_attribute_delta(delta: &AttributeDelta, attributes: &Attributes) -> AttributeDelta {
	AttributeDelta {
		key: delta.key.clone(),
		value: attributes.get(&delta.key).filter(|previous| !previous.deleted).map(|previous| previous.value.clone()),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn unrecorded_retired_at_costs_one_byte_and_round_trips() {
		let base = Delta::new(
			None,
			PeerId(1),
			TimeStamp { counter: 1, peer: PeerId(1) },
			RegistryDelta::Other(Value::None),
			RegistryDelta::Other(Value::None),
		);
		let mut stamped = base.clone();
		stamped.retired_at_ms = 1_790_000_000_000;

		let bare_base = postcard::to_allocvec(&base).expect("encode");
		let bare_stamped = postcard::to_allocvec(&stamped).expect("encode");
		// Only the integer's varint differs: one byte for zero, six for today's milliseconds, no tag byte in either.
		assert_eq!(bare_stamped.len() - bare_base.len(), postcard::to_allocvec(&1_790_000_000_000u64).unwrap().len() - 1);

		let decoded: Delta = postcard::from_bytes(&bare_stamped).expect("decode");
		assert_eq!(decoded.retired_at(), Some(1_790_000_000_000));
		let decoded: Delta = postcard::from_bytes(&bare_base).expect("decode");
		assert_eq!(decoded.retired_at(), None);
	}
}
