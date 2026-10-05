use crate::packet::{Broadcast, BroadcastBody, CursorPosition, PacketError, PeerSeq, Role, SyncPacket, SyncPayload};
use crate::target::{SyncTarget, TargetError};
use crate::transport::{Transport, TransportEvent, TransportPeerId};
use document_graph_storage::{Delta, HeadMove, HistoryMetadata, HotOp, HotOpId, PeerId, ResourceHash, Rev, UserId};
use std::collections::{HashMap, HashSet};

pub enum Event {
	PeerJoined {
		peer: PeerId,
		user: UserId,
	},
	PeerLeft {
		peer: PeerId,
	},
	/// This peer's own role changed: it took over from a departed host, stepped aside unsynced, or yielded to a lower id.
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
	/// Something on record about the history's users changed, from a peer or a sync.
	MetadataChanged,
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
	/// Empty until its profile arrives.
	pub name: String,
	/// In document space; `None` when not over the viewport.
	pub cursor: Option<CursorPosition>,
}

/// How far one peer's broadcasts have been delivered here.
#[derive(Clone, Copy)]
struct PeerProgress {
	epoch: u64,
	seq: u64,
}

/// Broadcasts arriving before the host's `Sync` are held back, since they target state the guest doesn't have yet.
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
	/// Sent with every hello and on change.
	name: String,
	peers: HashMap<TransportPeerId, RemotePeer>,
	/// The role each link was last greeted with, so a link greeted before a role change is greeted again.
	greeted: HashMap<TransportPeerId, Role>,
	sync: SyncState,
	/// Drawn fresh so a reconnect or a reload never reuses one. See [`PeerSeq`].
	epoch: u64,
	seq: u64,
	delivered: HashMap<PeerId, PeerProgress>,
	held: Vec<(PeerId, Broadcast)>,
	/// Hot ops whose referents had not arrived, retried as later ops fill the gaps. A missing entity is
	/// indistinguishable from a concurrently removed one, so the op waits rather than applying against state it does not fit.
	deferred: Vec<HotOp>,
	/// Asked for and not yet received, so a standing request is not resent every poll.
	requested_resources: HashSet<ResourceHash>,
	/// Requests that arrived before the bytes did, answered once they turn up.
	owed_resources: HashMap<ResourceHash, HashSet<TransportPeerId>>,
	/// Whether the target's resource references may have moved since last examined.
	resources_stale: bool,
	/// The unanswered sync request's link, so it is asked again only if that peer stops hosting first.
	sync_requested_from: Option<TransportPeerId>,
	/// The host whose line this peer holds: the one it last synced from, or itself. Any other host calls for a
	/// sync from it, since what it retired may never have been broadcast here. See [`Self::follow_current_host`].
	synced_from: Option<PeerId>,
}

impl Replica {
	pub fn host(transport: impl Transport + 'static, peer: PeerId, user: UserId) -> Self {
		Self::new(Box::new(transport), Role::Host, peer, user, SyncState::Synced)
	}

	/// Join through a link, expecting a host. The role stays undecided until that host greets, so a link into a room
	/// whose host is gone elects like anyone else rather than waiting for a greeting that never comes.
	pub fn guest(transport: impl Transport + 'static, peer: PeerId, user: UserId) -> Self {
		Self::connect(transport, peer, user)
	}

	/// Connect to the document's room in whichever role it calls for: a host's hello makes this peer a guest, and
	/// [`decide_role`](Self::decide_role) makes it the host if none greets in time.
	pub fn connect(transport: impl Transport + 'static, peer: PeerId, user: UserId) -> Self {
		Self::new(Box::new(transport), Role::Undecided, peer, user, SyncState::AwaitingSync { pending: Vec::new() })
	}

	/// For a peer still undecided after the grace period: become the host unless an undecided peer with a lower id
	/// will. A room with guests is never seized; they elect among themselves when their host leaves. Returns the role taken.
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
		log::debug!("Join handshake: no host greeted, becoming the host");
		self.become_host();
		Some(Role::Host)
	}

	/// An unsynced peer asks the known host for a sync, again only if that peer stops hosting or leaves first.
	fn ensure_sync_requested(&mut self, target: &dyn SyncTarget) -> Result<(), PacketError> {
		if self.is_synced() || self.sync_requested_from.is_some() {
			return Ok(());
		}
		let Some((&host, _)) = self.peers.iter().find(|(_, remote)| remote.role == Role::Host) else {
			return Ok(());
		};
		log::debug!("Join handshake: sync request sent to the host");
		self.sync_requested_from = Some(host);
		self.transport.send(host, &SyncPacket::SyncRequest { known_revs: target.known_revs() })
	}

	/// A synced guest whose host is not the one it synced from syncs from it, since what that host retired may never
	/// have been broadcast here. The sync merges onto what is held.
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
		log::debug!("Syncing from {host:?}, a host this peer has not synced from");
		self.sync = SyncState::AwaitingSync { pending: Vec::new() };
		self.sync_requested_from = None;
		self.ensure_sync_requested(target)
	}

	/// Ask the host for its line again, naming what is held so it sends only the rest. Broadcasts arriving meanwhile
	/// are buffered as on a first sync.
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

	/// Take the host role and tell the room. Broadcasts buffered for a sync that now never comes are kept for
	/// delivery, or the sender's later ones would wait on them forever.
	fn become_host(&mut self) {
		self.role = Role::Host;
		self.synced_from = Some(self.peer);
		if let SyncState::AwaitingSync { pending } = std::mem::replace(&mut self.sync, SyncState::Synced) {
			let worth_holding: Vec<(PeerId, Broadcast)> = pending.into_iter().filter(|(sender, broadcast)| self.worth_holding(*sender, broadcast)).collect();
			self.held.extend(worth_holding);
		}
		self.announce_role();
	}

	/// Greet every peer again with the current role; a known peer takes it as a role change only.
	fn announce_role(&mut self) {
		let peers: Vec<TransportPeerId> = self.peers.keys().copied().collect();
		for transport_peer in peers {
			if let Err(error) = self.send_hello(transport_peer) {
				log::error!("Announcing the {:?} role: {error}", self.role);
			}
		}
	}

	/// In a room left without a host, the synced guest with the lowest id takes the role; every peer applies the
	/// same rule to the same membership, so they agree. An unsynced guest has nothing to serve, so it steps aside as
	/// undecided, which re-runs the rule on the rest through its greeting and has it sync from whoever takes over.
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

	/// The display name this peer announces, sent to the room on change and with every hello.
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

	/// Tell the room this peer's history metadata after it changed here.
	pub fn send_metadata(&mut self, metadata: &HistoryMetadata) -> Result<(), PacketError> {
		if self.peers.is_empty() {
			return Ok(());
		}
		self.transport.broadcast_except(None, &SyncPacket::Metadata(metadata.clone()))
	}

	/// Tell the room where this peer's pointer is, `None` once it left the viewport. The caller coalesces to at
	/// most one call per frame.
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

		// A local edit can name a resource whose bytes are not here, such as a pasted node's font.
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

	/// Ask the host to move the shared head to `rev`, an ancestor of it. A host moves its own directly with
	/// [`broadcast_head_move`](Self::broadcast_head_move).
	pub fn request_move(&mut self, rev: Rev) -> Result<(), PacketError> {
		let Some((&host, _)) = self.peers.iter().find(|(_, remote)| remote.role == Role::Host) else {
			return Ok(());
		};
		self.transport.send(host, &SyncPacket::MoveRequest { rev })
	}

	/// Host only: tell the room the head moved, with the steps minted again under it.
	pub fn broadcast_head_move(&mut self, moved: HeadMove) -> Result<(), PacketError> {
		debug_assert_eq!(self.role, Role::Host);
		self.resources_stale = true;
		self.broadcast(BroadcastBody::HeadMove(moved))
	}

	/// Take back this peer's own hot ops, already gone from the local log, so every other peer drops them too.
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

		// References are read from history as well as the registry, so retiring can re-reference a hash the registry overwrote.
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

		// Greet every new peer before handling the batch. The transport adds them to its send set up front, so a
		// broadcast made meanwhile would reach them before a hello whose seq already covers it, and be skipped.
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
					// The departed peer may have owed bytes, so let the rest be asked.
					self.requested_resources.clear();
					self.resources_stale = true;

					match departed {
						Some(remote) => {
							let closed = self.close_epoch(remote.peer, target, &mut events);
							// Any departure can settle the election, since a guest that deferred to a lower id is the
							// candidate once that id is gone. A sync asked of the departed peer goes to whoever hosts.
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

		// A role decision can release broadcasts buffered for a sync that never comes, with no packet arriving.
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

	/// Re-announce every hot op held here that history does not cover, whoever wrote it, since this peer may hold
	/// the only copy of another's op. Covered ops are no-ops on every receiver.
	fn reannounce_hot_ops(&mut self, target: &dyn SyncTarget) -> Result<(), PacketError> {
		let settled = target.settled_marks();
		let unsettled: Vec<HotOp> = target.hot_log().into_iter().filter(|hot_op| !settled.covers(hot_op.id())).collect();

		self.broadcast_hot_ops(&unsettled)?;
		// A retraction in flight when a peer joined never reached it and nothing re-sends it, so the marks go round too.
		if settled.settled_up_to.is_empty() && settled.settled_runs.is_empty() {
			return Ok(());
		}
		self.broadcast(BroadcastBody::SettledMarks(settled))
	}

	/// Retry ops held back for a missing referent, every poll, since any later op can supply it.
	fn retry_deferred(&mut self, target: &mut dyn SyncTarget, events: &mut Vec<Event>) {
		if self.deferred.is_empty() {
			return;
		}

		let pending = std::mem::take(&mut self.deferred);
		let waiting = pending.len();
		match target.apply_remote_hot_ops(pending) {
			Ok(deferred) => {
				// Nothing else in the poll notices that one applied, changing the target and maybe naming a new resource.
				if deferred.len() < waiting {
					self.resources_stale = true;
					events.push(Event::Changed);
				}
				self.deferred = deferred;
			}
			Err(error) => log::error!("Retrying deferred hot ops: {error}"),
		}
	}

	/// Ask the room for referenced bytes not held here, after every batch since any delivered op can reference
	/// a new resource. Hashes with a standing request are skipped.
	fn request_missing_resources(&mut self, target: &dyn SyncTarget) -> Result<(), PacketError> {
		if !self.resources_stale {
			return Ok(());
		}
		self.resources_stale = false;

		let missing = target.missing_resources();
		// A resource that turned up some other way leaves its request behind, which would suppress a later ask.
		self.requested_resources.retain(|hash| missing.contains(hash));

		// Sorted so `HashSet` order does not reach the wire and make a seeded run unreproducible.
		let mut unasked: Vec<ResourceHash> = missing.into_iter().filter(|hash| !self.requested_resources.contains(hash)).collect();
		unasked.sort_unstable();
		if unasked.is_empty() {
			return Ok(());
		}

		self.requested_resources.extend(unasked.iter().copied());
		self.transport.broadcast(&SyncPacket::ResourceRequest(unasked))
	}

	/// Answer requests that arrived before their bytes did. Runs every poll, since bytes can turn up locally and a
	/// requester never asks twice.
	fn serve_owed_resources(&mut self, target: &dyn SyncTarget) -> Result<(), PacketError> {
		if self.owed_resources.is_empty() {
			return Ok(());
		}

		// Both loops run sorted so hash order does not decide send order and make a seeded run unreproducible.
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
		log::debug!("Join handshake: hello sent to {transport_peer:?} as {:?}", self.role);
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
				log::debug!("Join handshake: hello from {peer:?} ({role:?}), synced {}", self.is_synced());
				// A re-greeting over the same link and epoch announces a role; re-anchoring would strand its held broadcasts.
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

					// Can unblock broadcasts held against an earlier link's counter.
					self.deliver_held(target, events)?;
					self.reannounce_hot_ops(&*target)?;
					events.push(Event::PeerJoined { peer, user });
					// A fresh peer may hold bytes nobody else here could serve.
					self.requested_resources.clear();
					self.resources_stale = true;
				}

				if role == Role::Host && self.role == Role::Undecided {
					self.role = Role::Guest;
					events.push(Event::RoleChanged { role: self.role });
				}
				// Two hosts meet briefly when elections ran on differing memberships; the lower id keeps the role.
				if role == Role::Host && self.role == Role::Host && peer < self.peer {
					log::info!("Yielding the host role to {peer:?}, which has the lower id");
					self.role = Role::Guest;
					// Sync from it below, handing over what this peer retired alone, and tell the room so nobody
					// keeps asking this peer for what only a host answers.
					self.sync = SyncState::AwaitingSync { pending: Vec::new() };
					self.sync_requested_from = None;
					self.announce_role();
					events.push(Event::RoleChanged { role: self.role });
				}
				self.follow_current_host(&*target)?;
				// A link greeted with an earlier role before it became a known peer hears the current one, or a
				// guest joining as the host changes would never learn who hosts.
				if self.greeted.get(&from) != Some(&self.role) {
					self.send_hello(from)?;
				}
				// The peer asked for a sync stopped hosting before answering.
				if self.sync_requested_from == Some(from) && role != Role::Host {
					self.sync_requested_from = None;
				}
				self.ensure_sync_requested(&*target)?;
				// A host yielding or a guest stepping aside can leave the room hostless or make this peer the lowest candidate.
				if role != Role::Host {
					self.settle_without_host(events);
				}
			}
			SyncPacket::UndoRequest { rev, restore } => {
				if self.role != Role::Host {
					return Ok(());
				}
				// Best effort: a request naming a step no longer on this host's line is reported and dropped.
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
			SyncPacket::MoveRequest { rev } => {
				if self.role != Role::Host {
					return Ok(());
				}
				let outcome = target.move_head_to(rev).and_then(|moved| self.broadcast_head_move(moved).map_err(TargetError::from));
				match outcome {
					Ok(()) => events.push(Event::Changed),
					Err(error) => log::error!("Move request to {rev:?} failed: {error}"),
				}
			}
			SyncPacket::SyncRequest { known_revs } => {
				if self.role != Role::Host {
					return Ok(());
				}
				let shares_history = known_revs.iter().any(|&rev| target.contains_rev(rev));
				log::debug!("Join handshake: sync request from {from:?}, shares history {shares_history}");
				let sync = SyncPayload {
					registry: (!shares_history).then(|| target.retired_registry()),
					deltas: target.deltas_unknown_to(&known_revs),
					head: target.head(),
					hot_log: target.hot_log(),
					known_revs: target.known_revs(),
					seen: self.seen_vector(),
					settled: target.settled_marks(),
					document_id: target.document_id(),
					metadata: target.metadata(),
				};
				log::debug!("Join handshake: sync answered with {} deltas and {} hot ops", sync.deltas.len(), sync.hot_log.len());
				self.transport.send(from, &SyncPacket::Sync(Box::new(sync)))?;
			}
			SyncPacket::Sync(sync) => {
				log::debug!("Join handshake: sync received with {} deltas, full registry {}", sync.deltas.len(), sync.registry.is_some());
				self.sync_requested_from = None;
				let SyncState::AwaitingSync { pending } = std::mem::replace(&mut self.sync, SyncState::Synced) else {
					return Ok(());
				};
				self.synced_from = self.peers.get(&from).map(|remote| remote.peer);
				// A full registry replaces a fresh copy's session, so its unretired ops, possibly the only copy of
				// what never reached the host, are replayed on top. A copy with retired history is never replaced,
				// or a host over an empty document would wipe it; its line merges and what the host lacks goes back.
				let full = sync.registry.is_some() && target.known_revs().is_empty();
				let held = if full { target.hot_log() } else { Vec::new() };

				match (full, sync.registry) {
					(true, Some(registry)) => target.load(registry, sync.deltas, sync.head)?,
					_ => target.merge_remote(sync.deltas, &[], sync.head)?,
				}
				// A copy that joined by link takes the room's document id, or its own link would name an empty room.
				if let Some(document_id) = sync.document_id
					&& target.document_id() != Some(document_id)
				{
					target.adopt_document_id(document_id)?;
				}
				target.absorb_settled_marks(&sync.settled)?;
				if target.absorb_metadata(&sync.metadata)? {
					events.push(Event::MetadataChanged);
				}
				// Metadata recorded here while apart reaches the room too.
				let own = target.metadata();
				if !own.is_empty() {
					self.transport.broadcast_except(None, &SyncPacket::Metadata(own))?;
				}

				self.deferred.extend(target.apply_remote_hot_ops(held)?);
				self.deferred.extend(target.apply_remote_hot_ops(sync.hot_log)?);
				self.resources_stale = true;

				// This peer may hold the only copy of an op that never reached the host.
				self.reannounce_hot_ops(&*target)?;

				// Broadcasts the host delivered before answering are in the snapshot.
				for mark in sync.seen {
					self.observe_delivered(mark);
				}
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
				// The answering host may have yielded meanwhile.
				self.follow_current_host(&*target)?;

				// Unlike retired work, hot ops exist only in flight, so a drop loses them. Re-announce our own.
				let own_hot_ops: Vec<HotOp> = target.hot_log().into_iter().filter(|hot_op| hot_op.timestamp.peer == self.peer).collect();
				self.broadcast_hot_ops(&own_hot_ops)?;

				events.push(Event::Synced);
			}
			SyncPacket::Broadcast(broadcast) => {
				// Senders greet before broadcasting over a per-pair ordered channel, so this means the transport reordered.
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
				// Presence from a link that has not said hello belongs to nobody yet.
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
			SyncPacket::Metadata(remote) => {
				if target.absorb_metadata(&remote)? {
					events.push(Event::MetadataChanged);
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

	/// Record that a peer's broadcasts up to `mark` need not be waited for. Monotonic, since a hello and a host's
	/// `seen` vector can carry the same fact in either order.
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

	/// Set where a peer's broadcasts resume on a new link. Replaces rather than raises, since nothing before its
	/// hello was sent here.
	fn anchor_delivered(&mut self, peer: PeerId, epoch: u64, seq: u64) {
		if peer == self.peer {
			return;
		}
		self.delivered.insert(peer, PeerProgress { epoch, seq });
	}

	/// Settle up after a peer leaves: its held broadcasts can never fill their gaps, and anything waiting on it is free.
	fn close_epoch(&mut self, peer: PeerId, target: &mut dyn SyncTarget, events: &mut Vec<Event>) -> Result<(), ReplicaError> {
		// Applying its buffered broadcasts would mint orphans no departure is left to clean up, and dropping
		// them breaks no dependency since none applied yet.
		if let SyncState::AwaitingSync { pending } = &mut self.sync {
			pending.retain(|(sender, _)| *sender != peer);
		}
		self.held.retain(|(sender, _)| *sender != peer);

		self.deliver_held(target, events)?;

		// The departed peer's unretired work reached only some peers, and it cannot re-announce it itself.
		self.reannounce_hot_ops(&*target)?;
		Ok(())
	}

	/// Whether a link to this peer is open, so more of its broadcasts may turn up.
	fn is_connected(&self, peer: PeerId) -> bool {
		self.peers.values().any(|remote| remote.peer == peer)
	}

	/// Deliver every held broadcast whose causal dependencies are met, repeating since each can unblock others.
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

			// Best effort: a failure is reported rather than abandoning the bookkeeping and the rest of the queue.
			let applied = match broadcast.body {
				BroadcastBody::HotOps(ops) => target.apply_remote_hot_ops(ops).map(|deferred| self.deferred.extend(deferred)),
				// A retired step whose parent never arrived here cannot apply. A guest resyncs from the host; a host
				// lets it go, since the sender's whole line arrives when the sender syncs from it.
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
				// The host joins a diverged line with a merge delta, and the room follows to the joined head.
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
				BroadcastBody::SettledMarks(marks) => {
					self.deferred.retain(|hot_op| !marks.covers(hot_op.id()));
					target.absorb_settled_marks(&marks)
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

	/// Whether a broadcast can still come up for delivery; a stale epoch or delivered seq never will.
	fn worth_holding(&self, sender: PeerId, broadcast: &Broadcast) -> bool {
		self.delivered.get(&sender).is_none_or(|progress| progress.epoch == broadcast.epoch && broadcast.seq > progress.seq)
	}

	/// Whether a broadcast's dependency is met. One that never can be counts as met rather than stalling its sender.
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
