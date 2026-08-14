//! Persistent cursor state for the local peer. Separate from [`crate::Manifest`] because the
//! manifest describes document identity (what this document *is*), while [`SessionState`]
//! describes where the local peer's cursor sits inside it.
//!
//! Lives in `session.json`. Rewritten on retirement.

use document_graph_storage::{NetworkId, PeerId, Rev};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SessionState {
	/// This peer's identity for the document, stable per (device, document). Per-peer rather than
	/// document identity, so it lives with the cursor here, not in the manifest. Used for CRDT
	/// tiebreaking and minting peer-scoped IDs.
	#[serde(default)]
	pub peer_id: PeerId,
	/// Local-chain cursor. Points at the most recently applied retired delta, or `None` on an empty
	/// document (no commits yet).
	#[serde(default)]
	pub head_rev: Option<Rev>,
	/// Published frontier: the latest retired commit broadcast to a peer. Commits after it are silently
	/// rewritable on undo; commits at or before it are published. `None` until broadcast transport lands.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub last_broadcast_rev: Option<Rev>,
	/// Revs the user has undone past, so redo survives a reopen. (The legacy `VecDeque` redo history
	/// is not persisted, so within the shadow phase this is strictly more capable than the live editor.)
	#[serde(default)]
	pub redo_stack: Vec<Rev>,
	/// Shared-monotonic counter feeding `Document::next_node_id`. Persisted so reopens don't
	/// collide on minted IDs.
	#[serde(default)]
	pub next_node_counter: u64,
	/// Per-peer view settings (PTZ, rulers, overlays, snapping, panel collapse). Local to the viewer,
	/// so kept out of the CRDT/history. Editor owns the keys/values (opaque `ui::doc::*` blobs).
	#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
	pub view_settings: BTreeMap<String, serde_json::Value>,
	/// Per-network view settings (node-graph nav + previewing), keyed by the stable storage [`NetworkId`].
	/// Per-peer like [`view_settings`](Self::view_settings); opaque `ui::nav::*` / `ui::previewing` blobs.
	#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
	pub network_view_settings: BTreeMap<NetworkId, BTreeMap<String, serde_json::Value>>,
}
