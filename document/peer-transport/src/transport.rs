use crate::packet::{PacketError, SyncPacket};

pub use matchbox_socket::PeerId as TransportPeerId;

pub enum TransportEvent {
	PeerConnected(TransportPeerId),
	PeerDisconnected(TransportPeerId),
	Packet(TransportPeerId, SyncPacket),
	Malformed(TransportPeerId, PacketError),
}

/// Packet delivery between the peers of one session. Delivery must be reliable and ordered per
/// sender, which is what lets the `Replica` skip acknowledgements and buffering.
pub trait Transport: Send + Sync {
	fn send(&mut self, to: TransportPeerId, packet: &SyncPacket) -> Result<(), PacketError>;
	fn broadcast_except(&mut self, excluded: Option<TransportPeerId>, packet: &SyncPacket) -> Result<(), PacketError>;
	fn poll(&mut self) -> Vec<TransportEvent>;
	fn close(&mut self);

	fn broadcast(&mut self, packet: &SyncPacket) -> Result<(), PacketError> {
		self.broadcast_except(None, packet)
	}
}
