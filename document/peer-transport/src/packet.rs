use document_graph_storage::{Delta, HeadMove, HistoryMetadata, HotOp, HotOpId, PeerId, Registry, ResourceHash, Rev, SettledMarks, UserId};
use serde::{Deserialize, Serialize};

/// The host is the single peer that retires hot ops.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
	Host,
	Guest,
	/// Not yet greeted by a host. See [`Replica::connect`](crate::Replica::connect).
	Undecided,
}

/// How far one peer's broadcasts had got. A peer keeps its `PeerId` across a reconnect but restarts `seq`,
/// so `epoch` names the incarnation and counters from different epochs are never compared.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerSeq {
	pub peer: PeerId,
	pub epoch: u64,
	pub seq: u64,
}

/// MessagePack-encoded on the wire.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SyncPacket {
	/// Sent first on every connection. `epoch` and `seq` anchor the receiver's counter for this sender,
	/// since earlier broadcasts were never sent to it.
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
	/// Undo (`restore: false`) or redo (`restore: true`) one of the guest's retired interactions, named by its last
	/// delta. The host answers the room with a `HeadMove` or new deltas.
	UndoRequest {
		rev: Rev,
		restore: bool,
	},
	/// Move the shared head back to its ancestor `rev`, leaving the line since as a branch. The host answers the
	/// room with a `HeadMove` without copies.
	MoveRequest {
		rev: Rev,
	},
	ResourceRequest(Vec<ResourceHash>),
	Resource {
		hash: ResourceHash,
		#[serde(with = "serde_bytes")]
		bytes: Vec<u8>,
	},
	/// The sender's display name, sent after the hello and on change; the newest wins. Presence is not history,
	/// so it stays outside the causal broadcast.
	Profile {
		name: String,
	},
	/// The sender's pointer, `None` when it left the viewport. Sent at most once a frame; the link identifies the sender.
	Cursor {
		position: Option<CursorPosition>,
	},
	/// A peer's record of the history's users and their names. Outside the causal order, since it merges in any order.
	Metadata(HistoryMetadata),
}

/// The host's answer to a `SyncRequest`. `registry` is sent only when the host recognized none of the requester's
/// revs; otherwise `deltas` extend its history. `seen` is the host's delivery vector at snapshot time, adopted so
/// later broadcasts line up.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncPayload {
	pub registry: Option<Registry>,
	pub deltas: Vec<Delta>,
	pub head: Option<Rev>,
	pub hot_log: Vec<HotOp>,
	pub known_revs: Vec<Rev>,
	pub seen: Vec<PeerSeq>,
	/// Which hot ops are retired or taken back, so the requester drops any it holds rather than re-entering it as
	/// live work.
	pub settled: SettledMarks,
	/// Adopted by a peer that started empty, so opening its own copy later reconnects to the same room.
	#[serde(default)]
	pub document_id: Option<u64>,
	/// So a joiner can name every author.
	#[serde(default)]
	pub metadata: HistoryMetadata,
}

/// Causal broadcast envelope. `seq` numbers the sender's broadcasts from 1 within `epoch`, and `seen` is the
/// sender's delivery vector; a receiver holds the packet until it has delivered everything in `seen`.
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
	/// `deltas` are in causal order. `retires` names the hot ops they replace, empty for a plain history transfer.
	Deltas {
		deltas: Vec<Delta>,
		retires: Vec<HotOpId>,
	},
	/// Hot ops their author took back: they leave every hot log and never retire.
	Retract(Vec<HotOpId>),
	/// Everything a peer knows is settled, re-announced with hot ops on a membership change so a peer that joined
	/// during a retirement or retraction still hears of it.
	SettledMarks(SettledMarks),
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

/// A peer's pointer, with its space so it is drawn only where it means something.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CursorPosition {
	pub position: [f64; 2],
	pub space: CursorSpace,
	/// Icon name of the peer's tool, shown beside the pointer; `None` from a viewer with no tools.
	pub tool: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CursorSpace {
	/// Document coordinates over the canvas.
	Document,
	/// Node-graph coordinates in the network at this node-id path from the document network.
	Graph { network: Vec<u64> },
}
