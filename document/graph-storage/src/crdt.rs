use crate::{
	AttributeValue, Attributes, AttributesWrite, Implementation, InputSlot, Network, NetworkId, Node, NodeId, NodeInput, PeerId, ResourceEntry, ResourceId, Rev, SourceKey, TimeStamp, UserId, Value,
	attr, compute_rev,
};
use graphene_resource::ResourceHash;
use serde::{Deserialize, Serialize};

/// Content-addressed delta: `id` is `blake3_128(parents, author, timestamp, delta_type)`.
///
/// `reverse` is state-dependent undo bookkeeping (it captures pre-state at the moment the forward
/// op was applied), so it's serialized for storage but excluded from the identity hash — two peers
/// observing the same forward delta against different local states would otherwise compute
/// different Revs for the same logical op.
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
	/// When the delta entered history, in wall-clock milliseconds since the Unix epoch as its retirer saw them.
	/// For showing when a step happened; order always comes from the graph. Outside the rev like the attributes,
	/// and a plain field rather than one of them since every retired delta carries it. `None` when not recorded,
	/// by a copy with no clock. Written as a bare integer with zero for `None`, which serde's `Option` would
	/// not do: it spends a tag byte whatever the niche, and no retirement happens at the epoch. Last, since
	/// the history codec is positional.
	#[serde(default, with = "zero_as_none")]
	pub retired_at: Option<std::num::NonZeroU64>,
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
			retired_at: None,
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
			retired_at: None,
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

	/// When the delta entered history in wall-clock milliseconds, if its retirer recorded it.
	pub fn retired_at(&self) -> Option<u64> {
		self.retired_at.map(std::num::NonZeroU64::get)
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
	/// `snapshot` lets the reverse `AddNode` rebuild without reading the (already-removed) node from
	/// the registry, mirroring `RemoveNetwork`.
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
	/// `snapshot` lets the reverse delta rebuild without re-walking history.
	RemoveNetwork {
		id: NetworkId,
		snapshot: Network,
	},
	/// Register a whole resource entry at once. Overwrites any existing entry for `id`; the reverse
	/// of `RemoveResource`, the way `AddNetwork` pairs with `RemoveNetwork`.
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
	/// Append-only registration of a device's `PeerId` against its owning `UserId`.
	/// First write wins; conflicting re-registration errors. Duplicate identical registration
	/// is a no-op. Not LWW — the mapping is forever.
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
	/// The last op of its author's transaction. Changes nothing; retirement takes an author's ops through
	/// one of these as one unit and drops the marker itself. See [`Session::closed_transactions`](crate::Session::closed_transactions).
	EndTransaction,
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

/// Lands a single-key write. `floor` is when the map was last written whole: a key the map does not
/// hold is deleted as of then, so a write older than it is dropped. A deletion leaves a tombstone.
pub(crate) fn apply_attribute_delta(delta: AttributeDelta, timestamp: TimeStamp, force: bool, attributes: &mut Attributes, floor: TimeStamp) {
	let AttributeDelta { key, value } = delta;
	let decided = attributes.get(&key).map_or(floor, |existing| existing.timestamp);
	if !force && timestamp <= decided {
		return;
	}
	let entry = match value {
		Some(value) => AttributeValue::new(value, timestamp),
		None => AttributeValue::deleted(timestamp),
	};
	attributes.insert(key, entry);
}

/// An `Option<NonZeroU64>` as a bare integer, zero standing for `None`, so it costs no more on the wire than
/// the integer itself.
mod zero_as_none {
	use serde::{Deserialize, Deserializer, Serializer};
	use std::num::NonZeroU64;

	pub fn serialize<S: Serializer>(value: &Option<NonZeroU64>, serializer: S) -> Result<S::Ok, S::Error> {
		serializer.serialize_u64(value.map_or(0, NonZeroU64::get))
	}

	pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<NonZeroU64>, D::Error> {
		Ok(NonZeroU64::new(u64::deserialize(deserializer)?))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_missing_retirement_time_costs_what_a_zero_costs_and_round_trips() {
		let base = Delta::new(None, PeerId(1), TimeStamp { counter: 1, peer: PeerId(1) }, RegistryDelta::EndTransaction, RegistryDelta::EndTransaction);
		let mut stamped = base.clone();
		stamped.retired_at = std::num::NonZeroU64::new(1_790_000_000_000);

		let bare_base = postcard::to_allocvec(&base).expect("encode");
		let bare_stamped = postcard::to_allocvec(&stamped).expect("encode");
		// Everything before the field is the same, so the difference is the integer's varint: one byte for zero, six
		// for today's milliseconds, and no tag byte in either.
		assert_eq!(bare_stamped.len() - bare_base.len(), postcard::to_allocvec(&1_790_000_000_000u64).unwrap().len() - 1);

		let decoded: Delta = postcard::from_bytes(&bare_stamped).expect("decode");
		assert_eq!(decoded.retired_at(), Some(1_790_000_000_000));
		let decoded: Delta = postcard::from_bytes(&bare_base).expect("decode");
		assert_eq!(decoded.retired_at(), None);
	}
}
