use document_graph_storage::{Delta, HotOp, PeerId, Registry, ResourceHash, Rev, TimeStamp, UserId};
use serde::{Deserialize, Serialize};

/// The host is the single peer that retires hot ops.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
	Host,
	Guest,
}

/// MessagePack-encoded on the wire.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SyncPacket {
	/// Sent first on every new connection. `seq` anchors the receiver's counter for this sender, since
	/// broadcasts made before the connection were never sent to it.
	Hello {
		peer: PeerId,
		user: UserId,
		role: Role,
		seq: u64,
	},
	/// `known_revs` come from `Session::known_revs`, so the host can send only what's missing.
	SyncRequest {
		known_revs: Vec<Rev>,
	},
	Sync(Box<SyncPayload>),
	Broadcast(Broadcast),
	ResourceRequest(Vec<ResourceHash>),
	Resource {
		hash: ResourceHash,
		#[serde(with = "serde_bytes")]
		bytes: Vec<u8>,
	},
}

/// The host's answer to a `SyncRequest`. `registry` is only sent when the host recognized none of
/// the requester's revs; otherwise `deltas` extend the requester's history. `seen` is the host's
/// delivery vector at snapshot time, which the guest adopts so later broadcasts line up.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncPayload {
	pub registry: Option<Registry>,
	pub deltas: Vec<Delta>,
	pub head: Option<Rev>,
	pub hot_log: Vec<HotOp>,
	pub known_revs: Vec<Rev>,
	pub seen: Vec<(PeerId, u64)>,
}

/// Causal broadcast envelope. `seq` numbers the sender's broadcasts from 1, and `seen` is the
/// sender's delivery vector; a receiver holds the packet until it has delivered everything in `seen`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Broadcast {
	pub seq: u64,
	pub seen: Vec<(PeerId, u64)>,
	pub body: BroadcastBody,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BroadcastBody {
	HotOps(Vec<HotOp>),
	/// `deltas` are in causal order. `retires` names the hot ops they replace (empty for a plain history transfer).
	Deltas {
		deltas: Vec<Delta>,
		retires: Vec<TimeStamp>,
	},
}

#[derive(Debug, thiserror::Error)]
pub enum PacketError {
	#[error("failed to encode packet: {0}")]
	Encode(#[from] rmp_serde::encode::Error),
	#[error("failed to decode packet: {0}")]
	Decode(#[from] rmp_serde::decode::Error),
}

impl SyncPacket {
	pub fn encode(&self) -> Result<Box<[u8]>, PacketError> {
		Ok(rmp_serde::to_vec(self)?.into_boxed_slice())
	}

	pub fn decode(bytes: &[u8]) -> Result<Self, PacketError> {
		Ok(rmp_serde::from_slice(bytes)?)
	}
}
