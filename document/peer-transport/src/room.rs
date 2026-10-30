use crate::packet::{PacketError, SyncPacket};
use crate::transport::{Transport, TransportEvent, TransportPeerId};
use futures::StreamExt;
use futures::channel::mpsc::UnboundedSender;
use matchbox_socket::{MessageLoopFuture, Packet, PeerState, WebRtcSocket};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

const CHANNEL: usize = 0;
/// Largest SCTP message every browser accepts.
const CHUNK_BYTES: usize = 16 * 1024;
const FRAME_FINAL: u8 = 1;
const FRAME_MORE: u8 = 0;

/// Split a packet into `[flag][bytes]` chunks, the last carrying [`FRAME_FINAL`] so no length prefix is needed.
fn frames(bytes: &[u8]) -> Vec<Box<[u8]>> {
	// Chunking an empty packet would yield no frame at all.
	let chunks: Vec<&[u8]> = if bytes.is_empty() { vec![&[]] } else { bytes.chunks(CHUNK_BYTES).collect() };

	chunks
		.iter()
		.enumerate()
		.map(|(index, chunk)| {
			let flag = if index + 1 == chunks.len() { FRAME_FINAL } else { FRAME_MORE };
			std::iter::once(flag).chain(chunk.iter().copied()).collect()
		})
		.collect()
}

/// Per-peer reassembly of framed packets. The channel is ordered per peer, so a packet ends at its first [`FRAME_FINAL`].
#[derive(Default)]
struct Reassembler {
	partial: HashMap<TransportPeerId, Vec<u8>>,
}

impl Reassembler {
	/// The packet's bytes once its final frame lands.
	fn accept(&mut self, peer: TransportPeerId, frame: &[u8]) -> Option<Vec<u8>> {
		let (&flag, chunk) = frame.split_first()?;
		let buffer = self.partial.entry(peer).or_default();
		buffer.extend_from_slice(chunk);

		(flag == FRAME_FINAL).then(|| std::mem::take(buffer))
	}

	fn forget(&mut self, peer: TransportPeerId) {
		self.partial.remove(&peer);
	}
}

#[derive(Debug, Default)]
struct Inbox {
	packets: VecDeque<(TransportPeerId, Packet)>,
	waker: Option<Waker>,
	/// The socket loop ended, so nothing more will arrive.
	closed: bool,
}

impl Inbox {
	fn wake(&mut self) {
		if let Some(waker) = self.waker.take() {
			waker.wake();
		}
	}
}

fn lock(inbox: &Mutex<Inbox>) -> std::sync::MutexGuard<'_, Inbox> {
	inbox.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A handle onto a room's inbox, so the room can be polled as packets arrive rather than once per frame, which a
/// hidden browser tab gets only about once a second.
#[derive(Clone, Debug)]
pub struct Incoming {
	inbox: Arc<Mutex<Inbox>>,
}

impl Incoming {
	/// Resolves to `true` once a packet is waiting, `false` once the room's loop has ended and nothing more can arrive.
	pub async fn wait(&self) -> bool {
		std::future::poll_fn(|cx| self.poll_ready(cx)).await
	}

	fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<bool> {
		let mut inbox = lock(&self.inbox);
		if !inbox.packets.is_empty() {
			return Poll::Ready(true);
		}
		if inbox.closed {
			return Poll::Ready(false);
		}
		inbox.waker = Some(cx.waker().clone());
		Poll::Pending
	}
}

/// One matchbox room over WebRTC, with packets split into frames and reassembled per peer.
pub struct Room {
	socket: WebRtcSocket,
	outgoing: UnboundedSender<(TransportPeerId, Packet)>,
	inbox: Arc<Mutex<Inbox>>,
	incoming: Reassembler,
}

impl Room {
	/// The returned future must be polled for as long as the room lives.
	pub fn connect(signaling_url: &str) -> (Self, MessageLoopFuture) {
		// WebRTC's DTLS goes through rustls, which needs one provider chosen when several are linked.
		#[cfg(not(target_family = "wasm"))]
		let _ = rustls::crypto::ring::default_provider().install_default();

		let (mut socket, driver) = WebRtcSocket::new_reliable(signaling_url);
		let (outgoing, mut received) = socket.take_channel(CHANNEL).expect("a reliable socket has its one channel").split();
		let inbox = Arc::new(Mutex::new(Inbox::default()));

		// The forwarder ends when the socket loop returns and drops its senders.
		let forwarder = {
			let inbox = Arc::clone(&inbox);
			async move {
				while let Some(arrived) = received.next().await {
					let mut inbox = lock(&inbox);
					inbox.packets.push_back(arrived);
					inbox.wake();
				}
				let mut inbox = lock(&inbox);
				inbox.closed = true;
				inbox.wake();
			}
		};
		let driver: MessageLoopFuture = Box::pin(async move {
			let (result, ()) = futures::join!(driver, forwarder);
			result
		});

		(
			Self {
				socket,
				outgoing,
				inbox,
				incoming: Reassembler::default(),
			},
			driver,
		)
	}

	pub fn incoming(&self) -> Incoming {
		Incoming { inbox: Arc::clone(&self.inbox) }
	}

	fn send_chunks(&mut self, bytes: &[u8], peers: &[TransportPeerId]) {
		for frame in frames(bytes) {
			for &peer in peers {
				// Fails only once the socket loop ended, which its future reports.
				if self.outgoing.unbounded_send((peer, frame.clone())).is_err() {
					return;
				}
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
					self.incoming.forget(peer);
					TransportEvent::PeerDisconnected(peer)
				}
			});
		}

		let received: Vec<_> = lock(&self.inbox).packets.drain(..).collect();
		for (peer, frame) in received {
			let Some(bytes) = self.incoming.accept(peer, &frame) else { continue };

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

#[cfg(test)]
mod tests {
	use super::*;
	use futures::FutureExt;

	fn peer(byte: u128) -> TransportPeerId {
		TransportPeerId(uuid::Uuid::from_u128(byte))
	}

	#[test]
	fn the_inbox_wakes_on_arrival_and_reports_the_end() {
		let inbox = Arc::new(Mutex::new(Inbox::default()));
		let incoming = Incoming { inbox: Arc::clone(&inbox) };

		assert_eq!(incoming.wait().now_or_never(), None, "nothing arrived yet");

		lock(&inbox).packets.push_back((peer(1), Box::new([FRAME_FINAL, 7])));
		assert_eq!(incoming.wait().now_or_never(), Some(true), "an arrival resolves the wait");
		assert_eq!(incoming.wait().now_or_never(), Some(true), "and keeps it resolved until the inbox is drained");

		lock(&inbox).packets.clear();
		assert_eq!(incoming.wait().now_or_never(), None, "drained: pending again");

		lock(&inbox).closed = true;
		assert_eq!(incoming.wait().now_or_never(), Some(false), "the end of the loop resolves the wait for good");
	}

	fn round_trip(bytes: &[u8]) -> Vec<Vec<u8>> {
		let mut incoming = Reassembler::default();

		frames(bytes).into_iter().filter_map(|frame| incoming.accept(peer(1), &frame)).collect()
	}

	#[test]
	fn a_small_packet_is_one_frame() {
		let bytes = b"hello".to_vec();
		let framed = frames(&bytes);

		assert_eq!(framed.len(), 1);
		assert_eq!(framed[0][0], FRAME_FINAL);
		assert_eq!(round_trip(&bytes), vec![bytes]);
	}

	#[test]
	fn an_empty_packet_still_produces_one_frame() {
		let framed = frames(&[]);

		assert_eq!(framed.len(), 1, "an empty packet must still be framed");
		assert_eq!(framed[0][0], FRAME_FINAL);
		assert_eq!(round_trip(&[]), vec![Vec::<u8>::new()]);
	}

	#[test]
	fn packets_around_the_chunk_boundary_round_trip() {
		for length in [CHUNK_BYTES - 1, CHUNK_BYTES, CHUNK_BYTES + 1, CHUNK_BYTES * 3 + 7] {
			let bytes: Vec<u8> = (0..length).map(|index| (index % 251) as u8).collect();
			let framed = frames(&bytes);

			assert_eq!(framed.len(), length.div_ceil(CHUNK_BYTES), "wrong frame count for {length} bytes");
			assert!(framed.iter().all(|frame| frame.len() <= CHUNK_BYTES + 1), "a frame exceeded the SCTP limit");
			assert!(framed.iter().rev().skip(1).all(|frame| frame[0] == FRAME_MORE), "only the last frame is final");
			assert_eq!(framed.last().expect("at least one frame")[0], FRAME_FINAL);

			assert_eq!(round_trip(&bytes), vec![bytes], "{length} bytes did not round trip");
		}
	}

	#[test]
	fn interleaved_peers_reassemble_separately() {
		let (first, second) = (peer(1), peer(2));
		let mut incoming = Reassembler::default();

		let long: Vec<u8> = (0..CHUNK_BYTES + 32).map(|index| (index % 97) as u8).collect();
		let short = b"short".to_vec();

		let long_frames = frames(&long);
		let short_frames = frames(&short);
		assert_eq!(long_frames.len(), 2, "the long packet must span two frames to interleave");

		assert_eq!(incoming.accept(first, &long_frames[0]), None);
		assert_eq!(incoming.accept(second, &short_frames[0]), Some(short));
		assert_eq!(incoming.accept(first, &long_frames[1]), Some(long));
	}

	#[test]
	fn forgetting_a_peer_discards_its_partial_packet() {
		let sender = peer(1);
		let mut incoming = Reassembler::default();

		let long: Vec<u8> = (0..CHUNK_BYTES + 8).map(|index| (index % 89) as u8).collect();
		assert_eq!(incoming.accept(sender, &frames(&long)[0]), None);

		incoming.forget(sender);

		let fresh = b"fresh".to_vec();
		assert_eq!(incoming.accept(sender, &frames(&fresh)[0]), Some(fresh), "the abandoned prefix must be gone");
	}

	#[test]
	fn an_empty_frame_is_ignored() {
		let mut incoming = Reassembler::default();

		assert_eq!(incoming.accept(peer(1), &[]), None);
	}
}
