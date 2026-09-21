use crate::packet::{PacketError, SyncPacket};
use crate::transport::{Transport, TransportEvent, TransportPeerId};
use matchbox_socket::{MessageLoopFuture, PeerState, WebRtcSocket};
use std::collections::HashMap;

const CHANNEL: usize = 0;
/// Largest SCTP message every browser accepts.
const CHUNK_BYTES: usize = 16 * 1024;
const FRAME_FINAL: u8 = 1;
const FRAME_MORE: u8 = 0;

/// One matchbox room over WebRTC. Packets are split into `[flag][bytes]` chunks; the reliable
/// channel is ordered per peer, so the receiver reassembles by appending until the final flag.
/// The returned future must be polled continuously by the caller.
pub struct Room {
	socket: WebRtcSocket,
	partial: HashMap<TransportPeerId, Vec<u8>>,
}

impl Room {
	pub fn connect(signaling_url: &str) -> (Self, MessageLoopFuture) {
		// WebRTC's DTLS goes through rustls, which needs one provider chosen when several are linked.
		#[cfg(not(target_family = "wasm"))]
		let _ = rustls::crypto::ring::default_provider().install_default();

		let (socket, driver) = WebRtcSocket::new_reliable(signaling_url);
		(Self { socket, partial: HashMap::new() }, driver)
	}

	fn send_chunks(&mut self, bytes: &[u8], peers: &[TransportPeerId]) {
		let channel = self.socket.channel_mut(CHANNEL);
		let chunk_count = bytes.len().div_ceil(CHUNK_BYTES);

		for (index, chunk) in bytes.chunks(CHUNK_BYTES).enumerate() {
			let flag = if index + 1 == chunk_count { FRAME_FINAL } else { FRAME_MORE };
			let frame: Box<[u8]> = std::iter::once(flag).chain(chunk.iter().copied()).collect();
			for &peer in peers {
				channel.send(frame.clone(), peer);
			}
		}
	}
}

impl Transport for Room {
	fn send(&mut self, to: TransportPeerId, packet: &SyncPacket) -> Result<(), PacketError> {
		let bytes = packet.encode()?;
		self.send_chunks(&bytes, &[to]);
		Ok(())
	}

	fn broadcast_except(&mut self, excluded: Option<TransportPeerId>, packet: &SyncPacket) -> Result<(), PacketError> {
		let bytes = packet.encode()?;
		let peers: Vec<_> = self.socket.connected_peers().filter(|&peer| Some(peer) != excluded).collect();
		self.send_chunks(&bytes, &peers);
		Ok(())
	}

	fn poll(&mut self) -> Vec<TransportEvent> {
		let mut events = Vec::new();

		for (peer, state) in self.socket.update_peers() {
			events.push(match state {
				PeerState::Connected => TransportEvent::PeerConnected(peer),
				PeerState::Disconnected => {
					self.partial.remove(&peer);
					TransportEvent::PeerDisconnected(peer)
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
				Ok(packet) => TransportEvent::Packet(peer, packet),
				Err(error) => TransportEvent::Malformed(peer, error),
			});
		}

		events
	}

	fn close(&mut self) {
		self.socket.close();
	}
}
