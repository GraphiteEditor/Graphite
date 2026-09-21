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
	/// Sent first on every new connection.
	Hello {
		peer: PeerId,
		user: UserId,
		role: Role,
	},
	/// `known_revs` come from `Session::known_revs`, so the host can send only what's missing.
	SyncRequest {
		known_revs: Vec<Rev>,
	},
	Sync(Box<SyncPayload>),
	HotOps(Vec<HotOp>),
	/// `deltas` are in causal order. With `retires_up_to`, they replace hot ops stamped at or before it.
	Deltas {
		deltas: Vec<Delta>,
		retires_up_to: Option<TimeStamp>,
	},
	ResourceRequest(Vec<ResourceHash>),
	Resource {
		hash: ResourceHash,
		#[serde(with = "serde_bytes")]
		bytes: Vec<u8>,
	},
}

/// The host's answer to a `SyncRequest`. `registry` is only sent when the host recognized none of
/// the requester's revs; otherwise `deltas` extend the requester's history.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncPayload {
	pub registry: Option<Registry>,
	pub deltas: Vec<Delta>,
	pub head: Option<Rev>,
	pub hot_log: Vec<HotOp>,
	pub known_revs: Vec<Rev>,
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
