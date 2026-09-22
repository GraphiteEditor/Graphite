use crate::packet::{Broadcast, BroadcastBody, PacketError, PeerSeq, Role, SyncPacket, SyncPayload};
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

/// How far one peer's broadcasts have been delivered here, and which incarnation numbered them.
#[derive(Clone, Copy)]
struct PeerProgress {
	epoch: u64,
	seq: u64,
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
	/// Distinguishes this run of the protocol from an earlier one under the same `PeerId`, which a
	/// reconnect or a page reload creates. Drawn fresh so it does not repeat across either.
	epoch: u64,
	seq: u64,
	delivered: HashMap<PeerId, PeerProgress>,
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
			epoch: core_types::uuid::generate_uuid(),
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

	/// Broadcasts waiting on causal dependencies. Non-zero once the room is idle means a delivery is
	/// stuck behind a dependency that will never arrive.
	pub fn held_broadcasts(&self) -> usize {
		self.held.len()
	}

	/// Resources asked for whose bytes have not come back yet.
	pub fn pending_resource_requests(&self) -> usize {
		self.requested_resources.len()
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
			epoch: self.epoch,
			seq: self.seq,
			seen: self.seen_vector(),
			body,
		};
		self.transport.broadcast(&SyncPacket::Broadcast(broadcast))
	}

	/// Everything delivered here, including this peer's own broadcasts.
	fn seen_vector(&self) -> Vec<PeerSeq> {
		let delivered = self.delivered.iter().map(|(&peer, progress)| PeerSeq {
			peer,
			epoch: progress.epoch,
			seq: progress.seq,
		});
		delivered
			.chain([PeerSeq {
				peer: self.peer,
				epoch: self.epoch,
				seq: self.seq,
			}])
			.collect()
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
			epoch: self.epoch,
			seq: self.seq,
		};
		self.transport.send(transport_peer, &hello)?;
		Ok(())
	}

	fn handle_packet(&mut self, from: TransportPeerId, packet: SyncPacket, target: &mut dyn SyncTarget, events: &mut Vec<Event>) -> Result<(), ReplicaError> {
		match packet {
			SyncPacket::Hello { peer, user, role, epoch, seq } => {
				self.peers.insert(from, RemotePeer { peer, user, role });
				self.anchor_delivered(peer, epoch, seq);
				// A peer that dropped and came back reuses its `PeerId` but restarts its sequence, so the
				// anchor can unblock broadcasts held against the counter its previous connection reached.
				self.deliver_held(target, events)?;
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
				// The host's hot log is authoritative for what is still hot. Anything else held here from
				// another author was retired while this peer was away, so drop it; the retired form comes
				// back through history. Ops authored here are kept and re-announced below instead.
				let still_hot: HashSet<TimeStamp> = sync.hot_log.iter().map(|hot_op| hot_op.timestamp).collect();
				let stale: Vec<TimeStamp> = target
					.hot_log()
					.into_iter()
					.map(|hot_op| hot_op.timestamp)
					.filter(|timestamp| timestamp.peer != self.peer && !still_hot.contains(timestamp))
					.collect();
				if !stale.is_empty() {
					target.merge_remote(Vec::new(), &stale)?;
				}

				target.apply_remote_hot_ops(sync.hot_log)?;
				self.resources_stale = true;

				// Broadcasts the host had delivered before answering are already reflected in the snapshot.
				for mark in sync.seen {
					self.observe_delivered(mark);
				}
				// Keep only what the snapshot does not already reflect and the sender can still follow up
				// on: an earlier seq is baked into the snapshot, and an earlier epoch came over a
				// connection that is gone.
				let worth_holding = |sender: PeerId, broadcast: &Broadcast| self.delivered.get(&sender).is_some_and(|progress| progress.epoch == broadcast.epoch && broadcast.seq > progress.seq);
				self.held.extend(pending.into_iter().filter(|(sender, broadcast)| worth_holding(*sender, broadcast)));
				self.deliver_held(target, events)?;

				let host_is_missing = target.deltas_unknown_to(&sync.known_revs);
				if !host_is_missing.is_empty() {
					self.broadcast(BroadcastBody::Deltas {
						deltas: host_is_missing,
						retires: Vec::new(),
					})?;
				}

				// Retired work survives a dropped connection in the history both sides exchange, but hot
				// ops only ever existed in flight. Re-announce the ones authored here so a reconnect does
				// not silently lose edits made before the drop.
				let own_hot_ops: Vec<HotOp> = target.hot_log().into_iter().filter(|hot_op| hot_op.timestamp.peer == self.peer).collect();
				self.broadcast_hot_ops(&own_hot_ops)?;

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

	/// Record that a peer's broadcasts up to `mark` need not be waited for. Monotonic within an epoch,
	/// since the same fact can arrive from several sources (a hello, then a host's `seen` vector) in
	/// either order. A mark from another epoch says nothing about the incarnation tracked here.
	fn observe_delivered(&mut self, mark: PeerSeq) {
		if mark.peer == self.peer {
			return;
		}
		match self.delivered.get_mut(&mark.peer) {
			Some(progress) if progress.epoch == mark.epoch => progress.seq = progress.seq.max(mark.seq),
			Some(_) => {}
			None => {
				self.delivered.insert(mark.peer, PeerProgress { epoch: mark.epoch, seq: mark.seq });
			}
		}
	}

	/// Set where a peer's broadcasts resume on a newly opened link. A transport only sends to peers it
	/// has been told about, so nothing before a hello was ever sent here; the hello's epoch replaces
	/// whatever was tracked, which is what lets a reconnecting peer restart its sequence.
	fn anchor_delivered(&mut self, peer: PeerId, epoch: u64, seq: u64) {
		if peer == self.peer {
			return;
		}
		self.delivered.insert(peer, PeerProgress { epoch, seq });

		// Whatever is still held from an earlier incarnation of this peer can never be delivered in
		// order. The sender re-announces what is still hot once it resyncs, so dropping it loses nothing.
		self.held.retain(|(sender, broadcast)| *sender != peer || broadcast.epoch == epoch);
	}

	/// Deliver every held broadcast whose causal dependencies are met, repeating since each delivery
	/// can unblock others.
	fn deliver_held(&mut self, target: &mut dyn SyncTarget, events: &mut Vec<Event>) -> Result<(), ReplicaError> {
		while let Some(index) = self.held.iter().position(|(sender, broadcast)| self.is_deliverable(*sender, broadcast)) {
			let (sender, broadcast) = self.held.remove(index);
			self.delivered.insert(
				sender,
				PeerProgress {
					epoch: broadcast.epoch,
					seq: broadcast.seq,
				},
			);

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
		let Some(progress) = self.delivered.get(&sender) else { return false };
		if progress.epoch != broadcast.epoch || broadcast.seq != progress.seq + 1 {
			return false;
		}

		broadcast.seen.iter().all(|&mark| mark.peer == sender || self.has_delivered(mark))
	}

	/// Whether one of a broadcast's dependencies is already satisfied here. A dependency on an epoch
	/// that is no longer tracked can never be satisfied, since its sender will not renumber its old
	/// broadcasts, so it counts as met rather than stalling that sender's delivery for good.
	fn has_delivered(&self, mark: PeerSeq) -> bool {
		if mark.peer == self.peer {
			return mark.epoch != self.epoch || mark.seq <= self.seq;
		}
		match self.delivered.get(&mark.peer) {
			Some(progress) if progress.epoch == mark.epoch => progress.seq >= mark.seq,
			Some(_) => true,
			None => false,
		}
	}
}
