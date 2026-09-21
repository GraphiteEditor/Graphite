pub mod mock;
mod packet;
mod replica;
mod room;
mod target;
mod token;
mod transport;

pub use packet::{PacketError, Role, SyncPacket, SyncPayload};
pub use replica::{Event, Replica, ReplicaError};
pub use room::Room;
pub use target::{SyncTarget, TargetError};
pub use token::{InvalidSessionToken, SessionToken};
pub use transport::{Transport, TransportEvent, TransportPeerId};

pub use matchbox_socket::MessageLoopFuture;

/// Matchbox signaling server for sessions, behind TLS so browsers on HTTPS pages can reach it.
pub const DEFAULT_SIGNALING_SERVER: &str = "wss://graphite.kobert.dev";
