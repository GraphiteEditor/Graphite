use document_graph_storage::{Delta, HeadMove, HotOp, HotOpId, PeerId, Registry, ResourceHash, RetiredHotOps, Rev, UserId};
use serde::{Deserialize, Serialize};

/// The host is the single peer that retires hot ops.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
	Host,
	Guest,
}

/// How far one peer's broadcasts had got. A peer keeps its `PeerId` across a reconnect but restarts
/// `seq`, so `epoch` names the incarnation that produced it and counters from different epochs are
/// never compared.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerSeq {
	pub peer: PeerId,
	pub epoch: u64,
	pub seq: u64,
}

/// MessagePack-encoded on the wire.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SyncPacket {
	/// Sent first on every new connection. `epoch` and `seq` anchor the receiver's counter for this
	/// sender, since broadcasts made before the connection were never sent to it.
	Hello {
		peer: PeerId,
		user: UserId,
		role: Role,
		epoch: u64,
		seq: u64,
	},
	/// `known_revs` come from `Session::known_revs`, so the host can send only what's missing.
	SyncRequest {
		known_revs: Vec<Rev>,
	},
	Sync(Box<SyncPayload>),
	Broadcast(Broadcast),
	/// A guest asks the host to undo (`restore: false`) or redo (`restore: true`) one of its retired
	/// interactions, named by its last delta. The host answers the room with a `HeadMove` or new deltas.
	UndoRequest {
		rev: Rev,
		restore: bool,
	},
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
	pub seen: Vec<PeerSeq>,
	/// Which hot ops the history being handed over already covers, so the requester can recognize one
	/// it is holding rather than re-entering it as live work.
	pub retired: RetiredHotOps,
	/// Which hot ops their authors took back, so the requester drops any it still holds.
	#[serde(default)]
	pub retracted: RetiredHotOps,
}

/// Causal broadcast envelope. `seq` numbers the sender's broadcasts from 1 within `epoch`, and `seen`
/// is the sender's delivery vector; a receiver holds the packet until it has delivered everything in
/// `seen`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Broadcast {
	pub epoch: u64,
	pub seq: u64,
	pub seen: Vec<PeerSeq>,
	pub body: BroadcastBody,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BroadcastBody {
	HotOps(Vec<HotOp>),
	/// `deltas` are in causal order. `retires` names the hot ops they replace (empty for a plain history transfer).
	Deltas {
		deltas: Vec<Delta>,
		retires: Vec<HotOpId>,
	},
	/// Hot ops their author took back: they leave every hot log and never retire.
	Retract(Vec<HotOpId>),
	/// Everything a peer knows to have been taken back, re-announced on a membership change the way hot
	/// ops are, so a peer that joined while a retraction was in flight still hears of it.
	RetractedMarks(RetiredHotOps),
	/// The host dropped a retired interaction out of the line: every peer walks back and follows the head.
	HeadMove(HeadMove),
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
