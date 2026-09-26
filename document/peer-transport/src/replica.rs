use crate::packet::{Broadcast, BroadcastBody, CursorPosition, PacketError, PeerSeq, Role, SyncPacket, SyncPayload};
use crate::target::{SyncTarget, TargetError};
use crate::transport::{Transport, TransportEvent, TransportPeerId};
use document_graph_storage::{Delta, HeadMove, HotOp, HotOpId, PeerId, ResourceHash, Rev, UserId};
use std::collections::{HashMap, HashSet};

pub enum Event {
	PeerJoined {
		peer: PeerId,
		user: UserId,
	},
	PeerLeft {
		peer: PeerId,
	},
	/// This peer's own role changed: it took the host role over from a host that left, stepped aside as
	/// an unsynced guest, or yielded to a host with a lower id.
	RoleChanged {
		role: Role,
	},
	/// A peer's display name arrived or changed.
	ProfileChanged {
		peer: PeerId,
	},
	/// A peer's pointer moved, or left the viewport.
	CursorMoved {
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

/// A peer in the room as its last hello described it.
#[derive(Clone, Debug, PartialEq)]
pub struct RemotePeer {
	pub peer: PeerId,
	pub user: UserId,
	pub role: Role,
	/// The display name it announced; empty until its profile arrives.
	pub name: String,
	/// Its pointer in document space, `None` when it is not over the viewport.
	pub cursor: Option<CursorPosition>,
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
	/// This peer's display name, sent with every hello and on change.
	name: String,
	peers: HashMap<TransportPeerId, RemotePeer>,
	/// The role each link was last greeted with, so a peer whose hello arrives after this one's role
	/// changed is greeted again rather than left believing the old role.
	greeted: HashMap<TransportPeerId, Role>,
	sync: SyncState,
	/// This incarnation, drawn fresh so a reconnect or a reload never reuses one. See [`PeerSeq`].
	epoch: u64,
	seq: u64,
	delivered: HashMap<PeerId, PeerProgress>,
	held: Vec<(PeerId, Broadcast)>,
	/// Hot ops whose referents had not arrived when they did, retried as later ops fill the gaps.
	/// A missing entity is not distinguishable from one that was concurrently removed, so the op is
	/// kept rather than applied against state it does not fit.
	deferred: Vec<HotOp>,
	/// Resources asked for and not yet received, so a standing request is not resent every poll.
	requested_resources: HashSet<ResourceHash>,
	/// Requests that arrived before the bytes did, answered once they turn up here.
	owed_resources: HashMap<ResourceHash, HashSet<TransportPeerId>>,
	/// Whether the target's resource references may have moved since they were last examined.
	resources_stale: bool,
	/// The link a sync was asked of and not yet answered by, so the request is made once, and again only
	/// when that peer stops being the host before answering.
	sync_requested_from: Option<TransportPeerId>,
	/// The host whose line this peer holds: the one it last synced from, or itself. A greeting from a host
	/// other than this one, elected while this peer was away or the winner of a tie, calls for a sync from
	/// it, since what it retired may never have been broadcast here.
	synced_from: Option<PeerId>,
}

impl Replica {
	pub fn host(transport: impl Transport + 'static, peer: PeerId, user: UserId) -> Self {
		Self::new(Box::new(transport), Role::Host, peer, user, SyncState::Synced)
	}

	/// Join through a link, expecting a host. The role is still undecided until that host greets: a guest
	/// is a peer with a host, so a link into a room whose host is gone ends up electing like anyone else
	/// rather than waiting for a greeting that never comes and blocking the room's own election.
	pub fn guest(transport: impl Transport + 'static, peer: PeerId, user: UserId) -> Self {
		Self::connect(transport, peer, user)
	}

	/// Connect to the room every copy of the document shares, taking whichever role the room calls for: a
	/// host's hello makes this peer a guest that syncs, and [`decide_role`](Self::decide_role) makes it the
	/// host once it has waited long enough for one to greet it.
	pub fn connect(transport: impl Transport + 'static, peer: PeerId, user: UserId) -> Self {
		Self::new(Box::new(transport), Role::Undecided, peer, user, SyncState::AwaitingSync { pending: Vec::new() })
	}

	/// For a peer still undecided after the grace period: become the host unless another undecided peer
	/// with a lower id is there to become it, in which case its hello as host is on its way. A room with
	/// guests in it is never seized: they hold the line and elect among themselves when their host leaves.
	/// Returns the role taken, if one was.
	pub fn decide_role(&mut self) -> Option<Role> {
		if self.role != Role::Undecided {
			return None;
		}
		if self.peers.values().any(|remote| remote.role != Role::Undecided) {
			return None;
		}
		if self.peers.values().any(|remote| remote.peer < self.peer) {
			return None;
		}
		log::info!("Join handshake: no host greeted, becoming the host");
		self.become_host();
		Some(Role::Host)
	}

	/// An unsynced peer asks the host it knows for a sync: once, and again only once the peer it asked has
	/// stopped hosting or left. Nothing to do while synced, while an answer is due, or with no host known.
	fn ensure_sync_requested(&mut self, target: &dyn SyncTarget) -> Result<(), PacketError> {
		if self.is_synced() || self.sync_requested_from.is_some() {
			return Ok(());
		}
		let Some((&host, _)) = self.peers.iter().find(|(_, remote)| remote.role == Role::Host) else {
			return Ok(());
		};
		log::info!("Join handshake: sync request sent to the host");
		self.sync_requested_from = Some(host);
		self.transport.send(host, &SyncPacket::SyncRequest { known_revs: target.known_revs() })
	}

	/// A synced guest whose room's host is not the one it synced from, elected while this peer was away, the
	/// winner of a tie, or simply not the host that answered, syncs from it: what it retired may never have
	/// been broadcast here. The sync merges onto what is held.
	fn follow_current_host(&mut self, target: &dyn SyncTarget) -> Result<(), PacketError> {
		if self.role != Role::Guest || !self.is_synced() {
			return Ok(());
		}
		let Some(host) = self.peers.values().find(|remote| remote.role == Role::Host).map(|remote| remote.peer) else {
			return Ok(());
		};
		if self.synced_from == Some(host) {
			return Ok(());
		}
		log::info!("Syncing from {host:?}, a host this peer has not synced from");
		self.sync = SyncState::AwaitingSync { pending: Vec::new() };
		self.sync_requested_from = None;
		self.ensure_sync_requested(target)
	}

	/// Ask the host for its line again, with what is held here so it sends only the rest. Broadcasts that
	/// arrive meanwhile are buffered as on a first sync. Nothing to do while a sync is already on its way
	/// or no host is known to ask.
	fn request_resync(&mut self, target: &dyn SyncTarget) -> Result<(), PacketError> {
		if self.role == Role::Host || !self.is_synced() {
			return Ok(());
		}
		let Some((&host, _)) = self.peers.iter().find(|(_, remote)| remote.role == Role::Host) else {
			return Ok(());
		};
		log::info!("A retired step arrived whose parent is missing here; syncing from the host again");
		self.sync = SyncState::AwaitingSync { pending: Vec::new() };
		self.sync_requested_from = Some(host);
		self.transport.send(host, &SyncPacket::SyncRequest { known_revs: target.known_revs() })
	}

	/// Take the host role and tell the room. Broadcasts buffered against a sync that now never comes are
	/// kept for delivery, or the sender's later ones would wait on them forever.
	fn become_host(&mut self) {
		self.role = Role::Host;
		self.synced_from = Some(self.peer);
		if let SyncState::AwaitingSync { pending } = std::mem::replace(&mut self.sync, SyncState::Synced) {
			let worth_holding: Vec<(PeerId, Broadcast)> = pending.into_iter().filter(|(sender, broadcast)| self.worth_holding(*sender, broadcast)).collect();
			self.held.extend(worth_holding);
		}
		self.announce_role();
	}

	/// Greet every peer again with the current role. A peer already known takes it as a role change only.
	fn announce_role(&mut self) {
		let peers: Vec<TransportPeerId> = self.peers.keys().copied().collect();
		for transport_peer in peers {
			if let Err(error) = self.send_hello(transport_peer) {
				log::error!("Announcing the {:?} role: {error}", self.role);
			}
		}
	}

	/// A guest whose room has no host in it, because the host's link closed or because it yielded the
	/// role: the synced guest with the lowest id takes the role over. Every peer applies the same rule to
	/// the same membership, so they agree. An unsynced guest has nothing to serve and steps aside as
	/// undecided, which takes it out of the candidates, re-runs the rule on the rest through its greeting,
	/// and has it sync from whoever takes over. Nothing to do while a host is present.
	fn settle_without_host(&mut self, events: &mut Vec<Event>) {
		if self.role != Role::Guest || self.peers.values().any(|remote| remote.role == Role::Host) {
			return;
		}
		if !self.is_synced() {
			log::info!("The host went before syncing this peer; waiting for whoever takes over");
			self.role = Role::Undecided;
			self.sync_requested_from = None;
			self.announce_role();
			events.push(Event::RoleChanged { role: self.role });
			return;
		}
		if self.peers.values().any(|remote| remote.role == Role::Guest && remote.peer < self.peer) {
			return;
		}
		log::info!("The host went; taking the host role over as the lowest synced peer");
		self.become_host();
		events.push(Event::RoleChanged { role: self.role });
	}

	fn new(transport: Box<dyn Transport>, role: Role, peer: PeerId, user: UserId, sync: SyncState) -> Self {
		Self {
			transport,
			role,
			peer,
			user,
			name: String::new(),
			peers: HashMap::new(),
			greeted: HashMap::new(),
			sync,
			epoch: core_types::uuid::generate_uuid(),
			seq: 0,
			delivered: HashMap::new(),
			held: Vec::new(),
			deferred: Vec::new(),
			requested_resources: HashSet::new(),
			owed_resources: HashMap::new(),
			resources_stale: false,
			sync_requested_from: None,
			synced_from: (role == Role::Host).then_some(peer),
		}
	}

	pub fn role(&self) -> Role {
		self.role
	}

	pub fn is_synced(&self) -> bool {
		matches!(self.sync, SyncState::Synced)
	}

	/// The display name this peer announces. Sent to everyone in the room when it changes, and with every
	/// hello, so a newcomer learns it at once.
	pub fn set_name(&mut self, name: &str) -> Result<(), PacketError> {
		if self.name == name {
			return Ok(());
		}
		self.name = name.to_string();
		if self.peers.is_empty() {
			return Ok(());
		}
		self.transport.broadcast_except(None, &SyncPacket::Profile { name: self.name.clone() })
	}

	/// Tell the room where this peer's pointer is, `None` once it left the viewport. The
	/// caller coalesces: one call per frame at most, and none when nothing moved.
	pub fn send_cursor(&mut self, position: Option<CursorPosition>) -> Result<(), PacketError> {
		if self.peers.is_empty() {
			return Ok(());
		}
		self.transport.broadcast_except(None, &SyncPacket::Cursor { position })
	}

	/// The other peers in the room, in no particular order.
	pub fn peers(&self) -> impl Iterator<Item = &RemotePeer> + '_ {
		self.peers.values()
	}

	/// Broadcasts waiting on causal dependencies. Non-zero in an idle room means a stuck delivery.
	pub fn held_broadcasts(&self) -> usize {
		self.held.len()
	}

	/// One line per held broadcast saying what it waits on, for inspecting a stuck room.
	#[doc(hidden)]
	pub fn describe_held(&self) -> Vec<String> {
		self.held
			.iter()
			.map(|(sender, broadcast)| {
				let progress = self.delivered.get(sender).map(|progress| (progress.epoch, progress.seq));
				let unmet: Vec<String> = broadcast
					.seen
					.iter()
					.filter(|mark| mark.peer != *sender && !self.has_delivered(**mark))
					.map(|mark| format!("{:?}@{}:{} (here {:?})", mark.peer, mark.epoch, mark.seq, self.delivered.get(&mark.peer).map(|p| (p.epoch, p.seq))))
					.collect();
				format!("from {sender:?} epoch {} seq {} (delivered {progress:?}) unmet {unmet:?}", broadcast.epoch, broadcast.seq)
			})
			.collect()
	}

	/// Ops held back for a referent that had not arrived. Non-zero in an idle room means one never did.
	pub fn deferred_ops(&self) -> usize {
		self.deferred.len()
	}

	/// Resources asked for whose bytes have not come back yet.
	pub fn pending_resource_requests(&self) -> impl Iterator<Item = ResourceHash> + '_ {
		self.requested_resources.iter().copied()
	}

	pub fn broadcast_hot_ops(&mut self, ops: &[HotOp]) -> Result<(), PacketError> {
		if ops.is_empty() {
			return Ok(());
		}

		// A local edit can name a resource whose bytes are not here, a pasted node referencing a font for
		// instance, so the references are worth re-examining even though nothing arrived from the wire.
		self.resources_stale = true;

		self.broadcast(BroadcastBody::HotOps(ops.to_vec()))
	}

	/// Ask the host to undo (`restore == false`) or redo (`restore == true`) this peer's retired interaction.
	/// A host undoes its own directly with [`broadcast_head_move`](Self::broadcast_head_move).
	pub fn request_undo(&mut self, rev: Rev, restore: bool) -> Result<(), PacketError> {
		let Some((&host, _)) = self.peers.iter().find(|(_, remote)| remote.role == Role::Host) else {
			return Ok(());
		};
		self.transport.send(host, &SyncPacket::UndoRequest { rev, restore })
	}

	/// Host only: tell the room the head moved, with the steps minted again under it.
	pub fn broadcast_head_move(&mut self, moved: HeadMove) -> Result<(), PacketError> {
		debug_assert_eq!(self.role, Role::Host);
		self.resources_stale = true;
		self.broadcast(BroadcastBody::HeadMove(moved))
	}

	/// Take back hot ops of this peer's own, so every peer drops them. The ops have already left the
	/// local log; this is what makes them leave the others.
	pub fn broadcast_retraction(&mut self, ops: &[HotOpId]) -> Result<(), PacketError> {
		if ops.is_empty() {
			return Ok(());
		}
		self.resources_stale = true;
		self.broadcast(BroadcastBody::Retract(ops.to_vec()))
	}

	/// Host only.
	pub fn broadcast_retired(&mut self, deltas: &[Delta], retires: &[HotOpId]) -> Result<(), PacketError> {
		debug_assert_eq!(self.role, Role::Host);
		// A transaction of nothing but its marker retires with no delta, and the marker still has to go.
		if deltas.is_empty() && retires.is_empty() {
			return Ok(());
		}

		// Retirement moves ops into history, which referenced resources are read from as well as from the
		// registry, so a hash the registry has since overwritten becomes referenced again from here.
		self.resources_stale = true;

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
					self.greeted.remove(&transport_peer);
					if self.sync_requested_from == Some(transport_peer) {
						self.sync_requested_from = None;
					}
					let departed = self.peers.remove(&transport_peer);
					log::debug!("Link {transport_peer:?} closed: {:?}", departed.as_ref().map(|remote| (remote.peer, remote.role)));
					if let Some(remote) = &departed {
						events.push(Event::PeerLeft { peer: remote.peer });
					}
					// A peer that left may have been the one still owing bytes, so let the rest be asked again.
					self.requested_resources.clear();
					self.resources_stale = true;

					match departed {
						Some(remote) => {
							let closed = self.close_epoch(remote.peer, target, &mut events);
							// The line needs a retirer; the guests it left settle on one. Any departure can be the one
							// that decides it: a guest that deferred to a lower id is the candidate once that id is
							// gone. A peer that had asked the departed one for a sync asks whoever else hosts.
							self.settle_without_host(&mut events);
							closed.and_then(|()| self.ensure_sync_requested(&*target).map_err(ReplicaError::from))
						}
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

		// Broadcasts can become deliverable without a packet arriving, when a role decision releases the
		// ones buffered for a sync that never comes.
		if !self.held.is_empty()
			&& let Err(error) = self.deliver_held(target, &mut events)
		{
			log::error!("Sync error: {error}");
		}
		self.retry_deferred(target, &mut events);

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

	/// Re-announce every hot op held here that history does not yet cover, whoever wrote it. A peer can
	/// hold the only copy of another's op, so replaying only our own would leave that one to die with the
	/// connection it arrived on. Ops the marks cover replay as no-ops on every receiver.
	fn reannounce_hot_ops(&mut self, target: &dyn SyncTarget) -> Result<(), PacketError> {
		let retired = target.retired_marks();
		let unretired: Vec<HotOp> = target.hot_log().into_iter().filter(|hot_op| !retired.covers(hot_op.id())).collect();

		self.broadcast_hot_ops(&unretired)?;
		// A retraction in flight when a peer joined never reached it, and nothing holds it to re-send, so the
		// marks that record it go round again with the ops.
		let retracted = target.retracted_marks();
		if retracted.retired_up_to.is_empty() && retracted.retired_beyond.is_empty() {
			return Ok(());
		}
		self.broadcast(BroadcastBody::RetractedMarks(retracted))
	}

	/// Retry ops held back for a missing referent. A later op can supply the entity an earlier one
	/// named, so this runs once per poll rather than only where the op arrived.
	fn retry_deferred(&mut self, target: &mut dyn SyncTarget, events: &mut Vec<Event>) {
		if self.deferred.is_empty() {
			return;
		}

		let pending = std::mem::take(&mut self.deferred);
		let waiting = pending.len();
		match target.apply_remote_hot_ops(pending) {
			Ok(deferred) => {
				// One that finally applied can name a resource nobody here has seen, and leaves the target
				// changed, neither of which anything else in the poll would notice.
				if deferred.len() < waiting {
					self.resources_stale = true;
					events.push(Event::Changed);
				}
				self.deferred = deferred;
			}
			Err(error) => log::error!("Retrying deferred hot ops: {error}"),
		}
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

		// Sorted because `missing_resources` answers with a `HashSet`, whose order varies per process and
		// would otherwise reach the wire, making a seeded run unreproducible.
		let mut unasked: Vec<ResourceHash> = missing.into_iter().filter(|hash| !self.requested_resources.contains(hash)).collect();
		unasked.sort_unstable();
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

		// Both loops run in sorted order: the collections are hashed, so their iteration order varies per
		// process and would decide send order, leaving a seeded run unreproducible.
		let mut ready: Vec<(ResourceHash, Vec<u8>)> = self.owed_resources.keys().filter_map(|&hash| target.resource_bytes(hash).map(|bytes| (hash, bytes))).collect();
		ready.sort_unstable_by_key(|(hash, _)| *hash);

		for (hash, bytes) in ready {
			let mut recipients: Vec<TransportPeerId> = self.owed_resources.remove(&hash).unwrap_or_default().into_iter().collect();
			recipients.sort_unstable();

			for to in recipients {
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
		log::info!("Join handshake: hello sent to {transport_peer:?} as {:?}", self.role);
		self.transport.send(transport_peer, &hello)?;
		self.greeted.insert(transport_peer, self.role);
		if !self.name.is_empty() {
			self.transport.send(transport_peer, &SyncPacket::Profile { name: self.name.clone() })?;
		}
		Ok(())
	}

	fn handle_packet(&mut self, from: TransportPeerId, packet: SyncPacket, target: &mut dyn SyncTarget, events: &mut Vec<Event>) -> Result<(), ReplicaError> {
		match packet {
			SyncPacket::Hello { peer, user, role, epoch, seq } => {
				log::info!("Join handshake: hello from {peer:?} ({role:?}), synced {}", self.is_synced());
				// A peer greeting again over the same link and incarnation is announcing a role, not arriving:
				// anchoring its progress again would strand broadcasts of its still waiting on their dependencies.
				let known = self.peers.contains_key(&from) && self.delivered.get(&peer).is_some_and(|progress| progress.epoch == epoch);
				// A re-greeting keeps the presence the link already announced.
				let presence = self.peers.remove(&from);
				self.peers.insert(
					from,
					RemotePeer {
						peer,
						user,
						role,
						name: presence.as_ref().map(|remote| remote.name.clone()).unwrap_or_default(),
						cursor: presence.and_then(|remote| remote.cursor),
					},
				);
				if !known {
					self.anchor_delivered(peer, epoch, seq);

					// The anchor can unblock broadcasts held against the counter an earlier link reached.
					self.deliver_held(target, events)?;
					self.reannounce_hot_ops(&*target)?;
					events.push(Event::PeerJoined { peer, user });
					// A fresh peer may hold bytes nobody else here could serve.
					self.requested_resources.clear();
					self.resources_stale = true;
				}

				// A host greeting an undecided peer settles it: it is a guest of that host.
				if role == Role::Host && self.role == Role::Undecided {
					self.role = Role::Guest;
					events.push(Event::RoleChanged { role: self.role });
				}
				// Two hosts can meet for an instant when an election ran on differing memberships. The lower id
				// keeps the role; the other becomes its guest.
				if role == Role::Host && self.role == Role::Host && peer < self.peer {
					log::info!("Yielding the host role to {peer:?}, which has the lower id");
					self.role = Role::Guest;
					// Its line is the room's now: sync from it below, as any guest does, and hand it whatever
					// this peer retired on its own in the meantime. The room hears of the change, so nobody keeps
					// asking this peer for what only a host answers.
					self.sync = SyncState::AwaitingSync { pending: Vec::new() };
					self.sync_requested_from = None;
					self.announce_role();
					events.push(Event::RoleChanged { role: self.role });
				}
				self.follow_current_host(&*target)?;
				// A link greeted with an earlier role, before its own hello arrived and made it a known peer to
				// announce to, hears the current one now, or a guest joining a room whose host just changed
				// would never learn who hosts.
				if self.greeted.get(&from) != Some(&self.role) {
					self.send_hello(from)?;
				}
				// The peer asked for a sync stopped hosting before answering: ask whoever hosts now.
				if self.sync_requested_from == Some(from) && role != Role::Host {
					self.sync_requested_from = None;
				}
				self.ensure_sync_requested(&*target)?;
				// A host yielding or a guest stepping aside can leave this peer in a room without a host, or make
				// it the lowest candidate for one.
				if role != Role::Host {
					self.settle_without_host(events);
				}
			}
			SyncPacket::UndoRequest { rev, restore } => {
				if self.role != Role::Host {
					return Ok(());
				}
				// Applying is best effort, as for any op: a request naming a step this host no longer has on its
				// line is reported and dropped.
				let outcome: Result<(), TargetError> = if restore {
					target.restore_interaction(rev).and_then(|deltas| self.broadcast_retired(&deltas, &[]).map_err(TargetError::from))
				} else {
					target.drop_interaction(rev).and_then(|moved| self.broadcast_head_move(moved).map_err(TargetError::from))
				};
				match outcome {
					Ok(()) => events.push(Event::Changed),
					Err(error) => log::error!("Undo request for {rev:?} failed: {error}"),
				}
			}
			SyncPacket::SyncRequest { known_revs } => {
				if self.role != Role::Host {
					return Ok(());
				}
				let shares_history = known_revs.iter().any(|&rev| target.contains_rev(rev));
				log::info!("Join handshake: sync request from {from:?}, shares history {shares_history}");
				let sync = SyncPayload {
					registry: (!shares_history).then(|| target.retired_registry()),
					deltas: target.deltas_unknown_to(&known_revs),
					head: target.head(),
					hot_log: target.hot_log(),
					known_revs: target.known_revs(),
					seen: self.seen_vector(),
					retired: target.retired_marks(),
					retracted: target.retracted_marks(),
					document_id: target.document_id(),
				};
				log::info!("Join handshake: sync answered with {} deltas and {} hot ops", sync.deltas.len(), sync.hot_log.len());
				self.transport.send(from, &SyncPacket::Sync(Box::new(sync)))?;
			}
			SyncPacket::Sync(sync) => {
				log::info!("Join handshake: sync received with {} deltas, full registry {}", sync.deltas.len(), sync.registry.is_some());
				self.sync_requested_from = None;
				let SyncState::AwaitingSync { pending } = std::mem::replace(&mut self.sync, SyncState::Synced) else {
					return Ok(());
				};
				self.synced_from = self.peers.get(&from).map(|remote| remote.peer);
				// A full registry replaces the session of a fresh copy, hot log included, so the unretired ops
				// held here are kept and replayed back on top. They are the only copy of whatever never reached
				// the host. A copy with retired history of its own is never replaced, whatever the host sent:
				// its line merges with the host's, and what the host lacks goes back below. A host that took
				// the role over an empty document, say, would otherwise wipe a returning member's history.
				let full = sync.registry.is_some() && target.known_revs().is_empty();
				let held = if full { target.hot_log() } else { Vec::new() };

				match (full, sync.registry) {
					(true, Some(registry)) => target.load(registry, sync.deltas, sync.head)?,
					_ => target.merge_remote(sync.deltas, &[], sync.head)?,
				}
				// The room is the document's: a copy that joined by link takes the id whatever state it brought, or its
				// own link would name a room nobody else is in.
				if let Some(document_id) = sync.document_id
					&& target.document_id() != Some(document_id)
				{
					target.adopt_document_id(document_id)?;
				}
				target.absorb_retired_marks(&sync.retired)?;
				target.absorb_retracted_marks(&sync.retracted)?;

				self.deferred.extend(target.apply_remote_hot_ops(held)?);
				self.deferred.extend(target.apply_remote_hot_ops(sync.hot_log)?);
				self.resources_stale = true;

				// The host's hot log is not a superset of the room's: this peer may hold the only copy of an
				// op that never reached the host, and history is the only way it survives.
				self.reannounce_hot_ops(&*target)?;

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
				// The answer may be from a host that yielded while it was on its way; the line is the current host's.
				self.follow_current_host(&*target)?;

				// Hot ops only ever existed in flight, so a drop loses them where retired work survives in
				// history. Re-announce the ones authored here.
				let own_hot_ops: Vec<HotOp> = target.hot_log().into_iter().filter(|hot_op| hot_op.timestamp.peer == self.peer).collect();
				self.broadcast_hot_ops(&own_hot_ops)?;

				events.push(Event::Synced);
			}
			SyncPacket::Broadcast(broadcast) => {
				// A sender greets every peer before it broadcasts and the channel is ordered per pair, so a
				// hello always lands first. Reaching this means the transport stopped being ordered.
				let Some(remote) = self.peers.get(&from) else {
					log::error!("Dropping a broadcast from a peer that never said hello");
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
			SyncPacket::Profile { name } => {
				// Presence from a link that has not said hello has nobody to belong to yet.
				if let Some(remote) = self.peers.get_mut(&from)
					&& remote.name != name
				{
					remote.name = name;
					events.push(Event::ProfileChanged { peer: remote.peer });
				}
			}
			SyncPacket::Cursor { position } => {
				if let Some(remote) = self.peers.get_mut(&from)
					&& remote.cursor != position
				{
					remote.cursor = position;
					events.push(Event::CursorMoved { peer: remote.peer });
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

		// The departed peer's unretired work reached some peers and not others, and it cannot re-announce
		// the difference itself. Passing on the whole hot tail covers it and anything depending on it.
		self.reannounce_hot_ops(&*target)?;
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

			// Applying is best effort: the ops that did land still have to be accounted for, so a failure
			// is reported rather than abandoning the bookkeeping and the rest of the queue.
			let applied = match broadcast.body {
				BroadcastBody::HotOps(ops) => target.apply_remote_hot_ops(ops).map(|deferred| self.deferred.extend(deferred)),
				// A retired step built on one this peer never got, because its sync came from a host that
				// had no history yet or from one that has since yielded, cannot be applied. A guest asks the
				// host for its line again rather than dropping the step. The host lacks part of the sender's
				// line instead, which arrives whole when the sender syncs from it, as a yielded host or a
				// returning guest does, so the host lets this copy go.
				BroadcastBody::Deltas { deltas, .. }
					if deltas
						.iter()
						.any(|delta| delta.all_parents().any(|parent| !target.contains_rev(parent) && !deltas.iter().any(|held| held.id == parent))) =>
				{
					if self.role == Role::Host {
						log::info!("A retired step arrived whose parent is missing here; its author's line comes with its sync");
						Ok(())
					} else {
						self.request_resync(&*target).map_err(TargetError::from)
					}
				}
				// The host joins a line that diverged from its own, a guest's retired while apart or a yielded
				// host's, with a merge delta, and the room follows to the joined head. A guest follows.
				BroadcastBody::Deltas { deltas, retires } if self.role == Role::Host => target.merge_divergent(deltas, &retires).and_then(|minted| {
					if minted.is_empty() {
						return Ok(());
					}
					self.broadcast(BroadcastBody::Deltas { deltas: minted, retires: Vec::new() }).map_err(TargetError::from)
				}),
				BroadcastBody::Deltas { deltas, retires } => target.merge_remote(deltas, &retires, None),
				BroadcastBody::Retract(ops) => {
					// A copy still waiting on a referent is taken back with the rest.
					self.deferred.retain(|hot_op| !ops.contains(&hot_op.id()));
					target.retract_hot_ops(&ops)
				}
				BroadcastBody::RetractedMarks(marks) => {
					self.deferred.retain(|hot_op| !marks.covers(hot_op.id()));
					target.absorb_retracted_marks(&marks)
				}
				BroadcastBody::HeadMove(moved) => target.apply_head_move(&moved),
			};
			if let Err(error) = applied {
				log::error!("Applying a delivered broadcast: {error}");
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
