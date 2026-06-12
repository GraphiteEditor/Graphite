use crate::{Attributes, AttributesWrite, Network, NetworkId, Node, NodeId, NodeInput, PeerId, ResourceEntry, ResourceId, Rev, SourceKey, TimeStamp, UserId, Value, attr, compute_rev};
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
	pub parents: Vec<Rev>,
	pub author: PeerId,
	pub timestamp: TimeStamp,
	pub kind: RegistryDelta,
	pub reverse: RegistryDelta,
	/// Local, mutable annotations on this commit (interaction-end marker, future commit messages / labels).
	/// Deliberately excluded from `compute_rev`: relabeling a commit must not change its content-addressed
	/// identity, and two peers annotating the same op differently must still dedup to one `Rev`.
	#[serde(default, skip_serializing_if = "Attributes::is_empty")]
	pub attributes: Attributes,
}

impl Delta {
	pub fn new(parents: Vec<Rev>, author: PeerId, timestamp: TimeStamp, kind: RegistryDelta, reverse: RegistryDelta) -> Self {
		let id = compute_rev(&parents, author, timestamp, &kind);
		Self {
			id,
			parents,
			author,
			timestamp,
			kind,
			reverse,
			attributes: Attributes::default(),
		}
	}

	/// Mark this delta as the last op of a user interaction, so the undo cursor treats it as a checkpoint.
	pub fn mark_interaction_end(&mut self, timestamp: TimeStamp) {
		self.attributes.set(attr::delta::INTERACTION_END, serde_json::Value::Bool(true), timestamp);
	}

	pub fn is_interaction_end(&self) -> bool {
		self.attributes.get(attr::delta::INTERACTION_END).is_some_and(|marker| marker.value == serde_json::Value::Bool(true))
	}

	/// The content-addressed `Rev` this delta's identity fields hash to. Equals `id` for a delta built
	/// via `new`; differs only if `id` was tampered with or the hash derivation changed.
	pub fn recomputed_id(&self) -> Rev {
		compute_rev(&self.parents, self.author, self.timestamp, &self.kind)
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
#[derive(Clone, Debug, Serialize, Deserialize)]
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
		source: serde_json::Value,
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
	// Allow for future delta types without a model change
	Other(serde_json::Value),
}

/// `value: None` means remove. The timestamp comes from the wrapping `Delta`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AttributeDelta {
	pub key: String,
	pub value: Option<serde_json::Value>,
}

pub(crate) fn reverse_attribute_delta(delta: &AttributeDelta, attributes: &Attributes) -> AttributeDelta {
	AttributeDelta {
		key: delta.key.clone(),
		value: attributes.get(&delta.key).map(|previous| previous.value.clone()),
	}
}

pub(crate) fn apply_attribute_delta(delta: AttributeDelta, timestamp: TimeStamp, force: bool, attributes: &mut Attributes) {
	let AttributeDelta { key, value } = delta;
	match value {
		Some(value) => match attributes.entry(key) {
			std::collections::btree_map::Entry::Occupied(mut entry) => {
				if force || timestamp > entry.get().timestamp {
					entry.insert(Value { value, timestamp });
				}
			}
			std::collections::btree_map::Entry::Vacant(entry) => {
				entry.insert(Value { value, timestamp });
			}
		},
		None => {
			let should_remove = force || attributes.get(&key).is_none_or(|existing| timestamp > existing.timestamp);
			if should_remove {
				attributes.remove(&key);
			}
		}
	}
}
