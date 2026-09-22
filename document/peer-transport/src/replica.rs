use crate::packet::{Broadcast, BroadcastBody, PacketError, Role, SyncPacket, SyncPayload};
use crate::target::{SyncTarget, TargetError};
use crate::transport::{Transport, TransportEvent, TransportPeerId};
use document_graph_storage::{Delta, HotOp, PeerId, ResourceHash, TimeStamp, UserId};
use std::collections::{HashMap, HashSet};

pub enum Event {
	PeerJoined {
		peer: PeerId,
		user: UserId,
	},
	PeerLeft {
		peer: PeerId,
	},
	/// The guest has applied the host's state.
	Synced,
	/// Remote ops changed the target.
	Changed,
	ResourceReceived {
		hash: ResourceHash,
		bytes: Vec<u8>,
	},
	/// The target couldn't serve this synchronously; answer with `send_resource`.
	ResourceRequested {
		from: TransportPeerId,
		hash: ResourceHash,
	},
}

#[derive(Debug, thiserror::Error)]
pub enum ReplicaError {
	#[error(transparent)]
	Packet(#[from] PacketError),
	#[error(transparent)]
	Target(#[from] TargetError),
	#[error("packet from {0} before its hello")]
	UnknownPeer(TransportPeerId),
}

struct RemotePeer {
	peer: PeerId,
	#[expect(dead_code, reason = "Read once peers are surfaced in the UI")]
	user: UserId,
	#[expect(dead_code, reason = "Read once host handover is implemented")]
	role: Role,
}

/// Broadcasts that arrive before the host's `Sync` are held back, since they target state the guest
/// doesn't have yet.
enum SyncState {
	Synced,
	AwaitingSync { pending: Vec<(PeerId, Broadcast)> },
}

/// Protocol state for one peer in a room. Owns no document; `poll` applies into a `SyncTarget`.
///
/// Broadcasts are delivered in causal order: each carries the sender's sequence number and delivery
/// vector, and is held until everything the sender had delivered has been delivered here too.
pub struct Replica {
	transport: Box<dyn Transport>,
	role: Role,
	peer: PeerId,
	user: UserId,
	peers: HashMap<TransportPeerId, RemotePeer>,
	sync: SyncState,
	seq: u64,
	delivered: HashMap<PeerId, u64>,
	held: Vec<(PeerId, Broadcast)>,
	/// Resources asked for and not yet received, so a standing request is not resent every poll.
	requested_resources: HashSet<ResourceHash>,
	/// Whether the target's resource references may have moved since they were last examined.
	resources_stale: bool,
}

impl Replica {
	pub fn host(transport: impl Transport + 'static, peer: PeerId, user: UserId) -> Self {
		Self::new(Box::new(transport), Role::Host, peer, user, SyncState::Synced)
	}

	pub fn guest(transport: impl Transport + 'static, peer: PeerId, user: UserId) -> Self {
		Self::new(Box::new(transport), Role::Guest, peer, user, SyncState::AwaitingSync { pending: Vec::new() })
	}

	fn new(transport: Box<dyn Transport>, role: Role, peer: PeerId, user: UserId, sync: SyncState) -> Self {
		Self {
			transport,
			role,
			peer,
			user,
			peers: HashMap::new(),
			sync,
			seq: 0,
			delivered: HashMap::new(),
			held: Vec::new(),
			requested_resources: HashSet::new(),
			resources_stale: false,
		}
	}

	pub fn role(&self) -> Role {
		self.role
	}

	pub fn is_synced(&self) -> bool {
		matches!(self.sync, SyncState::Synced)
	}

	pub fn broadcast_hot_ops(&mut self, ops: &[HotOp]) -> Result<(), PacketError> {
		if ops.is_empty() {
			return Ok(());
		}
		self.broadcast(BroadcastBody::HotOps(ops.to_vec()))
	}

	/// Host only.
	pub fn broadcast_retired(&mut self, deltas: &[Delta], retires: &[TimeStamp]) -> Result<(), PacketError> {
		debug_assert_eq!(self.role, Role::Host);
		if deltas.is_empty() {
			return Ok(());
		}
		self.broadcast(BroadcastBody::Deltas {
			deltas: deltas.to_vec(),
			retires: retires.to_vec(),
		})
	}

	fn broadcast(&mut self, body: BroadcastBody) -> Result<(), PacketError> {
		self.seq += 1;
		let broadcast = Broadcast {
			seq: self.seq,
			seen: self.seen_vector(),
			body,
		};
		self.transport.broadcast(&SyncPacket::Broadcast(broadcast))
	}

	/// Everything delivered here, including this peer's own broadcasts.
	fn seen_vector(&self) -> Vec<(PeerId, u64)> {
		self.delivered.iter().map(|(&peer, &seq)| (peer, seq)).chain([(self.peer, self.seq)]).collect()
	}

	pub fn send_resource(&mut self, to: TransportPeerId, hash: ResourceHash, bytes: Vec<u8>) -> Result<(), PacketError> {
		self.transport.send(to, &SyncPacket::Resource { hash, bytes })
	}

	pub fn leave(&mut self) {
		self.transport.close();
	}

	pub fn poll(&mut self, target: &mut dyn SyncTarget) -> Vec<Event> {
		let mut events = Vec::new();

		for transport_event in self.transport.poll() {
			let result = match transport_event {
				TransportEvent::PeerConnected(transport_peer) => self.send_hello(transport_peer),
				TransportEvent::PeerDisconnected(transport_peer) => {
					if let Some(remote) = self.peers.remove(&transport_peer) {
						events.push(Event::PeerLeft { peer: remote.peer });
					}
					// A peer that left may have been the one still owing bytes, so let the rest be asked again.
					self.requested_resources.clear();
					self.resources_stale = true;
					Ok(())
				}
				TransportEvent::Packet(transport_peer, packet) => self.handle_packet(transport_peer, packet, target, &mut events),
				TransportEvent::Malformed(transport_peer, error) => {
					log::warn!("Dropping malformed packet from {transport_peer}: {error}");
					Ok(())
				}
			};

			if let Err(error) = result {
				log::error!("Sync error: {error}");
			}
		}

		if let Err(error) = target.flush() {
			log::error!("Sync error: {error}");
		}
		if let Err(error) = self.request_missing_resources(target) {
			log::error!("Sync error: {error}");
		}

		events
	}

	/// Ask the room for referenced bytes nobody here has yet. Runs after every batch of packets rather
	/// than once at sync, since an op delivered later can reference a resource this peer has never seen.
	/// Only the hashes without a standing request go out, so a slow transfer is not re-requested.
	fn request_missing_resources(&mut self, target: &dyn SyncTarget) -> Result<(), PacketError> {
		if !self.resources_stale {
			return Ok(());
		}
		self.resources_stale = false;

		let missing: Vec<ResourceHash> = target.missing_resources().into_iter().filter(|hash| !self.requested_resources.contains(hash)).collect();
		if missing.is_empty() {
			return Ok(());
		}

		self.requested_resources.extend(missing.iter().copied());
		self.transport.broadcast(&SyncPacket::ResourceRequest(missing))
	}

	fn send_hello(&mut self, transport_peer: TransportPeerId) -> Result<(), ReplicaError> {
		let hello = SyncPacket::Hello {
			peer: self.peer,
			user: self.user,
			role: self.role,
			seq: self.seq,
		};
		self.transport.send(transport_peer, &hello)?;
		Ok(())
	}

	fn handle_packet(&mut self, from: TransportPeerId, packet: SyncPacket, target: &mut dyn SyncTarget, events: &mut Vec<Event>) -> Result<(), ReplicaError> {
		match packet {
			SyncPacket::Hello { peer, user, role, seq } => {
				self.peers.insert(from, RemotePeer { peer, user, role });
				self.observe_delivered(peer, seq);
				events.push(Event::PeerJoined { peer, user });
				// A fresh peer may hold bytes nobody else here could serve.
				self.requested_resources.clear();
				self.resources_stale = true;

				if role == Role::Host && !self.is_synced() {
					self.transport.send(from, &SyncPacket::SyncRequest { known_revs: target.known_revs() })?;
				}
			}
			SyncPacket::SyncRequest { known_revs } => {
				if self.role != Role::Host {
					return Ok(());
				}
				let shares_history = known_revs.iter().any(|&rev| target.contains_rev(rev));
				let sync = SyncPayload {
					registry: (!shares_history).then(|| target.retired_registry()),
					deltas: target.deltas_unknown_to(&known_revs),
					head: target.head(),
					hot_log: target.hot_log(),
					known_revs: target.known_revs(),
					seen: self.seen_vector(),
				};
				self.transport.send(from, &SyncPacket::Sync(Box::new(sync)))?;
			}
			SyncPacket::Sync(sync) => {
				let SyncState::AwaitingSync { pending } = std::mem::replace(&mut self.sync, SyncState::Synced) else {
					return Ok(());
				};

				match sync.registry {
					Some(registry) => target.load(registry, sync.deltas, sync.head)?,
					None => target.merge_remote(sync.deltas, &[])?,
				}
				target.apply_remote_hot_ops(sync.hot_log)?;
				self.resources_stale = true;

				// Broadcasts the host had delivered before answering are already reflected in the snapshot.
				for (peer, seq) in sync.seen {
					self.observe_delivered(peer, seq);
				}
				let already_reflected = |sender: PeerId, broadcast: &Broadcast| broadcast.seq <= self.delivered.get(&sender).copied().unwrap_or(0);
				self.held.extend(pending.into_iter().filter(|(sender, broadcast)| !already_reflected(*sender, broadcast)));
				self.deliver_held(target, events)?;

				let host_is_missing = target.deltas_unknown_to(&sync.known_revs);
				if !host_is_missing.is_empty() {
					self.broadcast(BroadcastBody::Deltas {
						deltas: host_is_missing,
						retires: Vec::new(),
					})?;
				}

				events.push(Event::Synced);
			}
			SyncPacket::Broadcast(broadcast) => {
				let sender = self.peers.get(&from).ok_or(ReplicaError::UnknownPeer(from))?.peer;
				match &mut self.sync {
					SyncState::AwaitingSync { pending } => pending.push((sender, broadcast)),
					SyncState::Synced => {
						self.held.push((sender, broadcast));
						self.deliver_held(target, events)?;
					}
				}
			}
			SyncPacket::ResourceRequest(hashes) => {
				for hash in hashes {
					match target.resource_bytes(hash) {
						Some(bytes) => self.transport.send(from, &SyncPacket::Resource { hash, bytes })?,
						None => events.push(Event::ResourceRequested { from, hash }),
					}
				}
			}
			SyncPacket::Resource { hash, bytes } => {
				if ResourceHash::from(bytes.as_slice()) != hash {
					log::warn!("Resource from {from} does not match its hash; dropping");
					return Ok(());
				}
				target.store_resource(hash, &bytes)?;
				self.requested_resources.remove(&hash);
				events.push(Event::ResourceReceived { hash, bytes });
			}
		}

		Ok(())
	}

	fn observe_delivered(&mut self, peer: PeerId, seq: u64) {
		if peer != self.peer {
			let delivered = self.delivered.entry(peer).or_default();
			*delivered = (*delivered).max(seq);
		}
	}

	/// Deliver every held broadcast whose causal dependencies are met, repeating since each delivery
	/// can unblock others.
	fn deliver_held(&mut self, target: &mut dyn SyncTarget, events: &mut Vec<Event>) -> Result<(), ReplicaError> {
		while let Some(index) = self.held.iter().position(|(sender, broadcast)| self.is_deliverable(*sender, broadcast)) {
			let (sender, broadcast) = self.held.remove(index);
			self.delivered.insert(sender, broadcast.seq);

			match broadcast.body {
				BroadcastBody::HotOps(ops) => target.apply_remote_hot_ops(ops)?,
				BroadcastBody::Deltas { deltas, retires } => target.merge_remote(deltas, &retires)?,
			}
			self.resources_stale = true;
			events.push(Event::Changed);
		}
		Ok(())
	}

	fn is_deliverable(&self, sender: PeerId, broadcast: &Broadcast) -> bool {
		let next_from_sender = self.delivered.get(&sender).copied().unwrap_or(0) + 1;
		let has_delivered = |peer: PeerId, seq: u64| {
			if peer == self.peer {
				seq <= self.seq
			} else {
				self.delivered.get(&peer).copied().unwrap_or(0) >= seq
			}
		};

		broadcast.seq == next_from_sender && broadcast.seen.iter().all(|&(peer, seq)| peer == sender || has_delivered(peer, seq))
	}
}
