use super::{Attributes, Implementation, InputSlot, Network, NetworkId, Node, NodeId, NodeInput, PeerId, Prior, ResourceEntry, Rev, SourceKey, TimeStamp, UserId, Value};
use graphene_resource::{ResourceHash, ResourceId};
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
	/// What every slot `kind` wrote held before it, stamps included, in write order. See [`Prior`].
	pub reverse: Vec<Prior>,
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
	Meta,
	// Allow for future delta types without a model change
	Other(Value),
}

/// `value: None` means remove. The timestamp comes from the wrapping `Delta`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttributeDelta {
	pub key: String,
	pub value: Option<Value>,
}
