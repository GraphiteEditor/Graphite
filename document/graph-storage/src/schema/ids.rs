use serde::{Deserialize, Serialize};

/// Stable, document-scoped identity for a node. Minted as a truncated `blake3(peer, counter)` so it
/// reproduces across `to_runtime` -> `from_runtime` round trips. Used purely as an opaque key.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(pub u64);

/// Stable identity for a node network. `ROOT_NETWORK` for the renderable graph; nested networks get a
/// path-derived ID via `owned_network_id`. Used purely as an opaque key.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NetworkId(pub u64);

/// Content-addressed identity for a `Delta`.
/// 128-bit blake3 truncation: comfortable collision headroom for any plausible document lifetime
/// without being adversarial-grade. Same delta content always produces the same `Rev`. Non-zero so
/// `Option<Rev>` (a missing/root parent) is niche-optimized to the same size as a bare `Rev`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Rev(pub std::num::NonZeroU128);

/// Per-device identity. Stable per `(device, document)`. Used for CRDT tiebreaking and `NodeId`
/// scoping. Globally unique across all peers ever in a document.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PeerId(pub u64);

/// Per-human identity. Stable across devices (one user, many devices). Used for identity display
/// and undo-chain walking. Derived from `PeerId` via `Registry.peer_users`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UserId(pub u64);

/// Lamport timestamp with a peer-ID tiebreak. Higher counter wins; ties broken by peer.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
pub struct TimeStamp {
	pub counter: u64,
	pub peer: PeerId,
}
