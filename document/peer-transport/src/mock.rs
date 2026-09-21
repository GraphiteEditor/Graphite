//! In-process transport for deterministic simulation. Every packet sits in a per-(sender, receiver)
//! queue until `step` delivers it, so interleavings across peers are driven by the seed while each
//! pair stays ordered like a reliable channel.

use crate::packet::{PacketError, SyncPacket};
use crate::transport::{Transport, TransportEvent, TransportPeerId};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Shared {
	connected: Vec<TransportPeerId>,
	inboxes: HashMap<TransportPeerId, VecDeque<TransportEvent>>,
	in_flight: BTreeMap<(TransportPeerId, TransportPeerId), VecDeque<SyncPacket>>,
	next_peer: u128,
}

impl Shared {
	fn disconnect(&mut self, id: TransportPeerId) {
		self.connected.retain(|&other| other != id);
		self.in_flight.retain(|&(from, to), _| from != id && to != id);
		for &other in &self.connected.clone() {
			self.inboxes.entry(other).or_default().push_back(TransportEvent::PeerDisconnected(id));
			self.inboxes.entry(id).or_default().push_back(TransportEvent::PeerDisconnected(other));
		}
	}
}

pub struct MockNetwork {
	shared: Arc<Mutex<Shared>>,
	rng: SplitMix64,
}

impl MockNetwork {
	pub fn new(seed: u64) -> Self {
		Self {
			shared: Arc::new(Mutex::new(Shared::default())),
			rng: SplitMix64(seed),
		}
	}

	/// A new, not yet connected peer.
	pub fn endpoint(&mut self) -> MockEndpoint {
		let mut shared = self.shared.lock().unwrap();
		shared.next_peer += 1;
		let id = TransportPeerId(uuid::Uuid::from_u128(shared.next_peer));
		shared.inboxes.insert(id, VecDeque::new());
		MockEndpoint {
			id,
			shared: self.shared.clone(),
			known_peers: Vec::new(),
		}
	}

	pub fn connect(&mut self, id: TransportPeerId) {
		let mut shared = self.shared.lock().unwrap();
		if shared.connected.contains(&id) {
			return;
		}
		for &other in &shared.connected.clone() {
			shared.inboxes.entry(other).or_default().push_back(TransportEvent::PeerConnected(id));
			shared.inboxes.entry(id).or_default().push_back(TransportEvent::PeerConnected(other));
		}
		shared.connected.push(id);
	}

	pub fn disconnect(&mut self, id: TransportPeerId) {
		self.shared.lock().unwrap().disconnect(id);
	}

	pub fn pending(&self) -> usize {
		self.shared.lock().unwrap().in_flight.values().map(VecDeque::len).sum()
	}

	/// Deliver one in-flight packet from a randomly chosen sender/receiver pair.
	pub fn step(&mut self) -> bool {
		let mut shared = self.shared.lock().unwrap();
		let queues: Vec<_> = shared.in_flight.iter().filter(|(_, queue)| !queue.is_empty()).map(|(&key, _)| key).collect();
		if queues.is_empty() {
			return false;
		}

		let (from, to) = queues[self.rng.below(queues.len())];
		let packet = shared.in_flight.get_mut(&(from, to)).and_then(VecDeque::pop_front).expect("queue is non-empty");
		shared.inboxes.entry(to).or_default().push_back(TransportEvent::Packet(from, packet));
		true
	}

	pub fn deliver_all(&mut self) {
		while self.step() {}
	}

	pub fn random_below(&mut self, bound: usize) -> usize {
		self.rng.below(bound)
	}
}

/// Like a real socket, an endpoint only knows the peers whose connect events it has polled, so it
/// can't send to a peer before it had the chance to greet it.
pub struct MockEndpoint {
	id: TransportPeerId,
	shared: Arc<Mutex<Shared>>,
	known_peers: Vec<TransportPeerId>,
}

impl MockEndpoint {
	pub fn id(&self) -> TransportPeerId {
		self.id
	}
}

impl Transport for MockEndpoint {
	fn send(&mut self, to: TransportPeerId, packet: &SyncPacket) -> Result<(), PacketError> {
		if self.known_peers.contains(&to) {
			self.shared.lock().unwrap().in_flight.entry((self.id, to)).or_default().push_back(packet.clone());
		}
		Ok(())
	}

	fn broadcast_except(&mut self, excluded: Option<TransportPeerId>, packet: &SyncPacket) -> Result<(), PacketError> {
		for to in self.known_peers.clone() {
			if Some(to) != excluded {
				self.send(to, packet)?;
			}
		}
		Ok(())
	}

	fn poll(&mut self) -> Vec<TransportEvent> {
		let events: Vec<_> = self.shared.lock().unwrap().inboxes.get_mut(&self.id).map(|inbox| inbox.drain(..).collect()).unwrap_or_default();
		for event in &events {
			match event {
				TransportEvent::PeerConnected(peer) => self.known_peers.push(*peer),
				TransportEvent::PeerDisconnected(peer) => self.known_peers.retain(|known| known != peer),
				_ => {}
			}
		}
		events
	}

	fn close(&mut self) {
		self.shared.lock().unwrap().disconnect(self.id);
	}
}

struct SplitMix64(u64);

impl SplitMix64 {
	fn next(&mut self) -> u64 {
		self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
		let mut z = self.0;
		z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
		z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
		z ^ (z >> 31)
	}

	fn below(&mut self, bound: usize) -> usize {
		(self.next() % bound as u64) as usize
	}
}
