use crate::packet::{PacketError, SyncPacket};
use matchbox_socket::{MessageLoopFuture, PeerState, WebRtcSocket};
use std::collections::HashMap;

pub use matchbox_socket::PeerId as TransportPeerId;

const CHANNEL: usize = 0;
/// Largest SCTP message every browser accepts.
const CHUNK_BYTES: usize = 16 * 1024;
const FRAME_FINAL: u8 = 1;
const FRAME_MORE: u8 = 0;

pub enum RoomEvent {
	PeerConnected(TransportPeerId),
	PeerDisconnected(TransportPeerId),
	Packet(TransportPeerId, SyncPacket),
	Malformed(TransportPeerId, PacketError),
}

/// Typed view of one matchbox room. Packets are split into `[flag][bytes]` chunks; the reliable
/// channel is ordered per peer, so the receiver reassembles by appending until the final flag.
/// The returned future must be polled continuously by the caller.
pub struct Room {
	socket: WebRtcSocket,
	partial: HashMap<TransportPeerId, Vec<u8>>,
}

impl Room {
	pub fn connect(signaling_url: &str) -> (Self, MessageLoopFuture) {
		let (socket, driver) = WebRtcSocket::new_reliable(signaling_url);
		(Self { socket, partial: HashMap::new() }, driver)
	}

	pub fn send(&mut self, peer: TransportPeerId, packet: &SyncPacket) -> Result<(), PacketError> {
		let bytes = packet.encode()?;
		self.send_chunks(&bytes, [peer]);
		Ok(())
	}

	pub fn broadcast(&mut self, packet: &SyncPacket) -> Result<(), PacketError> {
		let bytes = packet.encode()?;
		let peers: Vec<_> = self.socket.connected_peers().collect();
		self.send_chunks(&bytes, peers);
		Ok(())
	}

	pub fn broadcast_except(&mut self, excluded: TransportPeerId, packet: &SyncPacket) -> Result<(), PacketError> {
		let bytes = packet.encode()?;
		let peers: Vec<_> = self.socket.connected_peers().filter(|&peer| peer != excluded).collect();
		self.send_chunks(&bytes, peers);
		Ok(())
	}

	fn send_chunks(&mut self, bytes: &[u8], peers: impl IntoIterator<Item = TransportPeerId> + Clone) {
		let channel = self.socket.channel_mut(CHANNEL);
		let chunk_count = bytes.len().div_ceil(CHUNK_BYTES);

		for (index, chunk) in bytes.chunks(CHUNK_BYTES).enumerate() {
			let flag = if index + 1 == chunk_count { FRAME_FINAL } else { FRAME_MORE };
			let frame: Box<[u8]> = std::iter::once(flag).chain(chunk.iter().copied()).collect();
			for peer in peers.clone() {
				channel.send(frame.clone(), peer);
			}
		}
	}

	pub fn connected_peers(&self) -> impl Iterator<Item = TransportPeerId> + '_ {
		self.socket.connected_peers()
	}

	pub fn poll(&mut self) -> Vec<RoomEvent> {
		let mut events = Vec::new();

		for (peer, state) in self.socket.update_peers() {
			events.push(match state {
				PeerState::Connected => RoomEvent::PeerConnected(peer),
				PeerState::Disconnected => {
					self.partial.remove(&peer);
					RoomEvent::PeerDisconnected(peer)
				}
			});
		}

		for (peer, frame) in self.socket.channel_mut(CHANNEL).receive() {
			let Some((&flag, chunk)) = frame.split_first() else { continue };
			let buffer = self.partial.entry(peer).or_default();
			buffer.extend_from_slice(chunk);
			if flag != FRAME_FINAL {
				continue;
			}

			let bytes = std::mem::take(buffer);
			events.push(match SyncPacket::decode(&bytes) {
				Ok(packet) => RoomEvent::Packet(peer, packet),
				Err(error) => RoomEvent::Malformed(peer, error),
			});
		}

		events
	}

	pub fn close(&mut self) {
		self.socket.close();
	}
}
