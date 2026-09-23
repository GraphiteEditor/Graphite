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
}

struct RemotePeer {
	peer: PeerId,
	#[expect(dead_code, reason = "Read once peers are surfaced in the UI")]
	user: UserId,
	#[expect(dead_code, reason = "Read once host handover is implemented")]
	role: Role,
}

/// How far one peer's broadcasts have been delivered here.
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
	/// This incarnation, drawn fresh so a reconnect or a reload never reuses one. See [`PeerSeq`].
	epoch: u64,
	seq: u64,
	delivered: HashMap<PeerId, PeerProgress>,
	held: Vec<(PeerId, Broadcast)>,
	/// Broadcasts that arrived before their sender's hello, parked rather than dropped so a reordering
	/// on a fresh link cannot lose ops.
	ungreeted: HashMap<TransportPeerId, Vec<Broadcast>>,
	/// Resources asked for and not yet received, so a standing request is not resent every poll.
	requested_resources: HashSet<ResourceHash>,
	/// Requests that arrived before the bytes did, answered once they turn up here.
	owed_resources: HashMap<ResourceHash, HashSet<TransportPeerId>>,
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
			ungreeted: HashMap::new(),
			requested_resources: HashSet::new(),
			owed_resources: HashMap::new(),
			resources_stale: false,
		}
	}

	pub fn role(&self) -> Role {
		self.role
	}

	pub fn is_synced(&self) -> bool {
		matches!(self.sync, SyncState::Synced)
	}

	/// Broadcasts waiting on causal dependencies. Non-zero in an idle room means a stuck delivery.
	pub fn held_broadcasts(&self) -> usize {
		self.held.len()
	}

	/// Resources asked for whose bytes have not come back yet.
	pub fn pending_resource_requests(&self) -> impl Iterator<Item = ResourceHash> + '_ {
		self.requested_resources.iter().copied()
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
		let transport_events = self.transport.poll();

		// Greet every new peer before handling anything else in the batch. A transport adds them to its
		// send set for the whole batch up front, so a broadcast made while handling an earlier event
		// would reach a peer that has not been greeted; the hello that followed would then carry a
		// sequence number covering it, and the receiver would rightly treat those ops as unneeded.
		for transport_event in &transport_events {
			if let TransportEvent::PeerConnected(transport_peer) = transport_event
				&& let Err(error) = self.send_hello(*transport_peer)
			{
				log::error!("Sync error: {error}");
			}
		}

		for transport_event in transport_events {
			let result = match transport_event {
				TransportEvent::PeerConnected(_) => Ok(()),
				TransportEvent::PeerDisconnected(transport_peer) => {
					let departed = self.peers.remove(&transport_peer);
					if let Some(remote) = &departed {
						events.push(Event::PeerLeft { peer: remote.peer });
					}
					// A peer that left may have been the one still owing bytes, so let the rest be asked again.
					self.requested_resources.clear();
					self.resources_stale = true;
					// The hello those were waiting on is never coming now.
					self.ungreeted.remove(&transport_peer);

					match departed {
						Some(remote) => self.close_epoch(remote.peer, target, &mut events),
						None => Ok(()),
					}
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
		if let Err(error) = self.serve_owed_resources(target) {
			log::error!("Sync error: {error}");
		}

		events
	}

	/// Re-announce hot ops authored here that are not in history yet. A broadcast can be lost with the
	/// link that carried it, and nothing else would resend it. Runs on membership changes, where loss
	/// is plausible; the watermark ends it as soon as the host retires them.
	fn reannounce_own_hot_ops(&mut self, target: &dyn SyncTarget) -> Result<(), PacketError> {
		let retired_through = target.retired_through().get(&self.peer).copied();
		let unretired: Vec<HotOp> = target
			.hot_log()
			.into_iter()
			.filter(|hot_op| hot_op.timestamp.peer == self.peer && retired_through.is_none_or(|counter| hot_op.timestamp.counter > counter))
			.collect();

		self.broadcast_hot_ops(&unretired)
	}

	/// Ask the room for referenced bytes nobody here has yet. Runs after every batch of packets rather
	/// than once at sync, since an op delivered later can reference a resource this peer has never seen.
	/// Only the hashes without a standing request go out, so a slow transfer is not re-requested.
	fn request_missing_resources(&mut self, target: &dyn SyncTarget) -> Result<(), PacketError> {
		if !self.resources_stale {
			return Ok(());
		}
		self.resources_stale = false;

		let missing = target.missing_resources();
		// A resource that turned up some other way, staged here for instance, leaves its request behind.
		// Clearing those keeps a later ask for the same hash from being suppressed.
		self.requested_resources.retain(|hash| missing.contains(hash));

		let unasked: Vec<ResourceHash> = missing.into_iter().filter(|hash| !self.requested_resources.contains(hash)).collect();
		if unasked.is_empty() {
			return Ok(());
		}

		self.requested_resources.extend(unasked.iter().copied());
		self.transport.broadcast(&SyncPacket::ResourceRequest(unasked))
	}

	/// Answer requests that arrived before their bytes did. A resource can turn up locally rather than
	/// over the wire, so this runs every poll; a requester never asks twice, so nothing else would.
	fn serve_owed_resources(&mut self, target: &dyn SyncTarget) -> Result<(), PacketError> {
		if self.owed_resources.is_empty() {
			return Ok(());
		}

		let ready: Vec<(ResourceHash, Vec<u8>)> = self.owed_resources.keys().filter_map(|&hash| target.resource_bytes(hash).map(|bytes| (hash, bytes))).collect();
		for (hash, bytes) in ready {
			for to in self.owed_resources.remove(&hash).unwrap_or_default() {
				self.transport.send(to, &SyncPacket::Resource { hash, bytes: bytes.clone() })?;
			}
		}
		Ok(())
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
				// Now that the sender is known, whatever arrived ahead of its hello can be handled.
				for broadcast in self.ungreeted.remove(&from).unwrap_or_default() {
					self.handle_packet(from, SyncPacket::Broadcast(broadcast), target, events)?;
				}

				// The anchor can unblock broadcasts held against the counter an earlier link reached.
				self.deliver_held(target, events)?;
				self.reannounce_own_hot_ops(&*target)?;
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
					retired_through: target.retired_through(),
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
				target.absorb_retired_through(&sync.retired_through)?;

				// The host's hot log is authoritative, so another author's op missing from it was retired
				// while this peer was away and comes back through history. Own ops are re-announced below.
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
				// An earlier seq is baked into the snapshot already.
				let worth_holding: Vec<(PeerId, Broadcast)> = pending.into_iter().filter(|(sender, broadcast)| self.worth_holding(*sender, broadcast)).collect();
				self.held.extend(worth_holding);
				self.deliver_held(target, events)?;

				let host_is_missing = target.deltas_unknown_to(&sync.known_revs);
				if !host_is_missing.is_empty() {
					self.broadcast(BroadcastBody::Deltas {
						deltas: host_is_missing,
						retires: Vec::new(),
					})?;
				}

				// Hot ops only ever existed in flight, so a drop loses them where retired work survives in
				// history. Re-announce the ones authored here.
				let own_hot_ops: Vec<HotOp> = target.hot_log().into_iter().filter(|hot_op| hot_op.timestamp.peer == self.peer).collect();
				self.broadcast_hot_ops(&own_hot_ops)?;

				events.push(Event::Synced);
			}
			SyncPacket::Broadcast(broadcast) => {
				// Its sender is only known from a hello, so park it rather than drop it.
				let Some(remote) = self.peers.get(&from) else {
					self.ungreeted.entry(from).or_default().push(broadcast);
					return Ok(());
				};
				let sender = remote.peer;
				match &mut self.sync {
					SyncState::AwaitingSync { pending } => pending.push((sender, broadcast)),
					SyncState::Synced => {
						if self.worth_holding(sender, &broadcast) {
							self.held.push((sender, broadcast));
							self.deliver_held(target, events)?;
						}
					}
				}
			}
			SyncPacket::ResourceRequest(hashes) => {
				for hash in hashes {
					match target.resource_bytes(hash) {
						Some(bytes) => self.transport.send(from, &SyncPacket::Resource { hash, bytes })?,
						None => {
							self.owed_resources.entry(hash).or_default().insert(from);
							events.push(Event::ResourceRequested { from, hash });
						}
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

	/// Record that a peer's broadcasts up to `mark` need not be waited for. Monotonic, since a hello
	/// and a host's `seen` vector can carry the same fact in either order.
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

	/// Set where a peer's broadcasts resume on a newly opened link. Nothing before its hello was ever
	/// sent here, so this replaces rather than raises.
	fn anchor_delivered(&mut self, peer: PeerId, epoch: u64, seq: u64) {
		if peer == self.peer {
			return;
		}
		self.delivered.insert(peer, PeerProgress { epoch, seq });

		// Held broadcasts from an earlier incarnation can never be delivered in order, and the sender
		// re-announces what is still hot once it resyncs.
		self.held.retain(|(sender, broadcast)| *sender != peer || broadcast.epoch == epoch);
	}

	/// Settle up after a peer leaves: its own held broadcasts can never fill their gaps, and anything
	/// waiting on it is now free to go.
	fn close_epoch(&mut self, peer: PeerId, target: &mut dyn SyncTarget, events: &mut Vec<Event>) -> Result<(), ReplicaError> {
		// Buffered broadcasts go too, in both queues. Applying them after their author left would mint a
		// fresh orphan with no departure left to clean it up, and nothing has been applied yet, so
		// dropping them breaks no dependency. Anything else waiting on this peer is released instead.
		if let SyncState::AwaitingSync { pending } = &mut self.sync {
			pending.retain(|(sender, _)| *sender != peer);
		}
		self.held.retain(|(sender, _)| *sender != peer);
		self.deliver_held(target, events)?;
		self.reannounce_own_hot_ops(&*target)?;

		// Its unretired work reached some peers and not others, and it can no longer re-announce the
		// difference itself. Pass on what this peer holds so the host can retire it into history, which
		// is the only way it survives; dropping it instead would strand every hot op that depends on it.
		let orphaned: Vec<HotOp> = target.hot_log().into_iter().filter(|hot_op| hot_op.timestamp.peer == peer).collect();
		self.broadcast_hot_ops(&orphaned)?;
		Ok(())
	}

	/// Whether a link to this peer is open, meaning more of its broadcasts may still turn up.
	fn is_connected(&self, peer: PeerId) -> bool {
		self.peers.values().any(|remote| remote.peer == peer)
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

	/// Whether a broadcast can still come up for delivery. A stale epoch or an already-delivered seq
	/// never will, so holding one would park it for good.
	fn worth_holding(&self, sender: PeerId, broadcast: &Broadcast) -> bool {
		self.delivered.get(&sender).is_none_or(|progress| progress.epoch == broadcast.epoch && broadcast.seq > progress.seq)
	}

	/// Whether one of a broadcast's dependencies is met. Dependencies that can never be met count as
	/// met, rather than stalling their sender for good.
	fn has_delivered(&self, mark: PeerSeq) -> bool {
		if mark.peer == self.peer {
			return mark.epoch != self.epoch || mark.seq <= self.seq;
		}
		match self.delivered.get(&mark.peer) {
			Some(progress) if progress.epoch != mark.epoch => true,
			Some(progress) if progress.seq >= mark.seq => true,
			// A peer with no open link sends nothing more, including one whose hello never arrived.
			_ => !self.is_connected(mark.peer),
		}
	}
}
