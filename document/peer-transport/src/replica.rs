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
	/// This peer's own role changed.
	RoleChanged {
		role: Role,
	},
	ProfileChanged {
		peer: PeerId,
	},
	/// A peer's pointer moved, or left the viewport.
	CursorMoved {
		peer: PeerId,
	},
	MetadataChanged,
	/// This peer applied the host's state.
	Synced,
	/// Remote ops changed the target.
	Changed,
	ResourceReceived {
		hash: ResourceHash,
		bytes: Vec<u8>,
	},
	/// The target could not serve this synchronously; answer with [`Replica::send_resource`].
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
	pub cursor: Option<CursorPosition>,
}

/// How far one peer's broadcasts have been delivered here.
#[derive(Clone, Copy)]
struct PeerProgress {
	epoch: u64,
	seq: u64,
}

/// Broadcasts arriving before the host's `Sync` wait in `pending`, since they target state this peer lacks.
enum SyncState {
	Synced,
	AwaitingSync { pending: Vec<(PeerId, Broadcast)> },
}

/// Protocol state for one peer in a room. Owns no document; `poll` applies into a `SyncTarget`.
///
/// Broadcasts are delivered in causal order: each is held until everything its sender had delivered is delivered here.
pub struct Replica {
	transport: Box<dyn Transport>,
	role: Role,
	peer: PeerId,
	user: UserId,
	name: String,
	peers: HashMap<TransportPeerId, RemotePeer>,
	/// The role each link was last greeted with.
	greeted: HashMap<TransportPeerId, Role>,
	sync: SyncState,
	/// Drawn fresh per `Replica`, so a reconnect never reuses one. See [`PeerSeq`].
	epoch: u64,
	seq: u64,
	delivered: HashMap<PeerId, PeerProgress>,
	held: Vec<(PeerId, Broadcast)>,
	/// Hot ops whose referents have not arrived. A missing entity looks like a concurrently removed one, so the op
	/// waits rather than applying against state it does not fit.
	deferred: Vec<HotOp>,
	/// Asked for and not received yet.
	requested_resources: HashSet<ResourceHash>,
	/// Requests that arrived before the bytes did.
	owed_resources: HashMap<ResourceHash, HashSet<TransportPeerId>>,
	/// Whether the target's resource references may have changed since last examined.
	resources_stale: bool,
	sync_requested_from: Option<TransportPeerId>,
	/// The host whose line this peer holds: the one it last synced from, or itself.
	synced_from: Option<PeerId>,
}

impl Replica {
	pub fn host(transport: impl Transport + 'static, peer: PeerId, user: UserId) -> Self {
		Self::new(Box::new(transport), Role::Host, peer, user, SyncState::Synced)
	}

	/// Connect undecided: a host's hello makes this peer a guest, and [`decide_role`](Self::decide_role) makes it the
	/// host if none greets in time.
	pub fn connect(transport: impl Transport + 'static, peer: PeerId, user: UserId) -> Self {
		Self::new(Box::new(transport), Role::Undecided, peer, user, SyncState::AwaitingSync { pending: Vec::new() })
	}

	/// For a peer still undecided after the grace period: become the host unless an undecided peer with a lower id
	/// will. A room with a host or guests is never seized. Returns the role taken.
	pub fn decide_role(&mut self) -> Option<Role> {
		if self.role != Role::Undecided || self.peers.values().any(|remote| remote.role != Role::Undecided || remote.peer < self.peer) {
			return None;
		}
		log::debug!("Join handshake: no host greeted, becoming the host");
		self.become_host();
		Some(Role::Host)
	}

	/// The link and id of a peer greeting as host.
	fn current_host(&self) -> Option<(TransportPeerId, PeerId)> {
		self.peers.iter().find(|(_, remote)| remote.role == Role::Host).map(|(&link, remote)| (link, remote.peer))
	}

	/// Ask the host for a sync unless synced or already asked.
	fn ensure_sync_requested(&mut self, target: &dyn SyncTarget) -> Result<(), PacketError> {
		if self.is_synced() || self.sync_requested_from.is_some() {
			return Ok(());
		}
		let Some((host, _)) = self.current_host() else { return Ok(()) };
		log::debug!("Join handshake: sync request sent to the host");
		self.sync_requested_from = Some(host);
		self.transport.send(host, &SyncPacket::SyncRequest { known_revs: target.known_revs() })
	}

	/// Drop back to unsynced and ask the host for its line, naming what is held so it sends only the rest.
	fn resync(&mut self, target: &dyn SyncTarget) -> Result<(), PacketError> {
		self.sync = SyncState::AwaitingSync { pending: Vec::new() };
		self.sync_requested_from = None;
		self.ensure_sync_requested(target)
	}

	/// A synced guest syncs from a host it has not synced from, since what that host retired may never have been
	/// broadcast here.
	fn follow_current_host(&mut self, target: &dyn SyncTarget) -> Result<(), PacketError> {
		if self.role != Role::Guest || !self.is_synced() {
			return Ok(());
		}
		let Some((_, host)) = self.current_host() else { return Ok(()) };
		if self.synced_from == Some(host) {
			return Ok(());
		}
		log::debug!("Syncing from {host:?}, a host this peer has not synced from");
		self.resync(target)
	}

	fn resync_for_missing_parent(&mut self, target: &dyn SyncTarget) -> Result<(), PacketError> {
		if self.role == Role::Host || !self.is_synced() || self.current_host().is_none() {
			return Ok(());
		}
		log::info!("A retired step arrived whose parent is missing here; syncing from the host again");
		self.resync(target)
	}

	/// Queue pending broadcasts for causal delivery after those already held, dropping any that can never come up.
	fn hold_pending(&mut self, pending: Vec<(PeerId, Broadcast)>) {
		let held = std::mem::take(&mut self.held);
		for (sender, broadcast) in held.into_iter().chain(pending) {
			if self.worth_holding(sender, &broadcast) {
				self.held.push((sender, broadcast));
			}
		}
	}

	/// Take the host role and tell the room. Broadcasts pending for a sync that now never comes are held for delivery,
	/// or the sender's later ones would wait on them forever.
	fn become_host(&mut self) {
		self.role = Role::Host;
		self.synced_from = Some(self.peer);
		if let SyncState::AwaitingSync { pending } = std::mem::replace(&mut self.sync, SyncState::Synced) {
			self.hold_pending(pending);
		}
		self.announce_role();
	}

	/// Greet every peer again with the current role.
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
	/// undecided and syncs from whoever takes over.
	fn settle_without_host(&mut self, events: &mut Vec<Event>) {
		if self.role != Role::Guest || self.current_host().is_some() {
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

	/// The display name this peer announces with every hello and on change.
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

	pub fn send_metadata(&mut self, metadata: &HistoryMetadata) -> Result<(), PacketError> {
		if self.peers.is_empty() {
			return Ok(());
		}
		self.transport.broadcast_except(None, &SyncPacket::Metadata(metadata.clone()))
	}

	/// Tell the room where this peer's pointer is, `None` once it left the viewport. Call at most once a frame.
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
		let progress = |peer: &PeerId| self.delivered.get(peer).map(|progress| (progress.epoch, progress.seq));
		self.held
			.iter()
			.map(|(sender, broadcast)| {
				let unmet: Vec<String> = broadcast
					.seen
					.iter()
					.filter(|mark| mark.peer != *sender && !self.has_delivered(**mark))
					.map(|mark| format!("{:?}@{}:{} (here {:?})", mark.peer, mark.epoch, mark.seq, progress(&mark.peer)))
					.collect();
				format!("from {sender:?} epoch {} seq {} (delivered {:?}) unmet {unmet:?}", broadcast.epoch, broadcast.seq, progress(sender))
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
		self.resources_stale = true;
		self.broadcast(BroadcastBody::HotOps(ops.to_vec()))
	}

	/// Ask the host to undo (`restore == false`) or redo (`restore == true`) this peer's retired interaction.
	/// A host undoes its own directly with [`broadcast_head_move`](Self::broadcast_head_move).
	pub fn request_undo(&mut self, rev: Rev, restore: bool) -> Result<(), PacketError> {
		let Some((host, _)) = self.current_host() else { return Ok(()) };
		self.transport.send(host, &SyncPacket::UndoRequest { rev, restore })
	}

	/// Ask the host to move the shared head to `rev`, an ancestor of it. A host moves its own directly with
	/// [`broadcast_head_move`](Self::broadcast_head_move).
	pub fn request_move(&mut self, rev: Rev) -> Result<(), PacketError> {
		let Some((host, _)) = self.current_host() else { return Ok(()) };
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

	/// Host only. `deltas` is empty for an interaction of nothing but its marker, which still has to retire.
	pub fn broadcast_retired(&mut self, deltas: &[Delta], retires: &[HotOpId]) -> Result<(), PacketError> {
		debug_assert_eq!(self.role, Role::Host);
		if deltas.is_empty() && retires.is_empty() {
			return Ok(());
		}
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

		// The transport already sends to new peers, so a broadcast made while handling the batch would reach them
		// before a hello whose seq covers it.
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
				TransportEvent::PeerDisconnected(transport_peer) => self.handle_disconnect(transport_peer, target, &mut events),
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

		// A role decision can release pending broadcasts with no packet arriving.
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
		self.serve_owed_resources(target);

		events
	}

	fn handle_disconnect(&mut self, transport_peer: TransportPeerId, target: &mut dyn SyncTarget, events: &mut Vec<Event>) -> Result<(), ReplicaError> {
		self.greeted.remove(&transport_peer);
		if self.sync_requested_from == Some(transport_peer) {
			self.sync_requested_from = None;
		}
		self.owed_resources.retain(|_, recipients| {
			recipients.remove(&transport_peer);
			!recipients.is_empty()
		});
		// The departed peer may have owed bytes, so let the rest be asked.
		self.requested_resources.clear();
		self.resources_stale = true;

		let departed = self.peers.remove(&transport_peer);
		log::debug!("Link {transport_peer:?} closed: {:?}", departed.as_ref().map(|remote| (remote.peer, remote.role)));
		let Some(remote) = departed else { return Ok(()) };
		events.push(Event::PeerLeft { peer: remote.peer });

		let closed = self.close_epoch(remote.peer, target, events);
		// Any departure can settle the election, since a guest that deferred to a lower id is the candidate once it left.
		self.settle_without_host(events);
		closed?;
		Ok(self.ensure_sync_requested(&*target)?)
	}

	/// Re-announce every unretired hot op held here, whoever wrote it, since this peer may hold the only copy. The
	/// retired marks go too, since a retraction in flight when a peer joined never reaches it otherwise.
	fn reannounce_hot_ops(&mut self, target: &dyn SyncTarget) -> Result<(), PacketError> {
		let retired = target.retired_marks();
		let unretired: Vec<HotOp> = target.hot_log().into_iter().filter(|hot_op| !retired.covers(hot_op.id())).collect();

		self.broadcast_hot_ops(&unretired)?;
		if retired.is_empty() {
			return Ok(());
		}
		self.broadcast(BroadcastBody::RetiredMarks(retired))
	}

	/// Retry ops held back for a missing referent, since any later op can supply it.
	fn retry_deferred(&mut self, target: &mut dyn SyncTarget, events: &mut Vec<Event>) {
		if self.deferred.is_empty() {
			return;
		}

		let pending = std::mem::take(&mut self.deferred);
		let waiting = pending.len();
		match target.apply_remote_hot_ops(pending) {
			Ok(deferred) => {
				if deferred.len() < waiting {
					self.resources_stale = true;
					events.push(Event::Changed);
				}
				self.deferred = deferred;
			}
			Err(error) => log::error!("Retrying deferred hot ops: {error}"),
		}
	}

	/// Ask the room for referenced bytes not held here and not asked for yet.
	fn request_missing_resources(&mut self, target: &dyn SyncTarget) -> Result<(), PacketError> {
		if !self.resources_stale {
			return Ok(());
		}
		self.resources_stale = false;

		let missing = target.missing_resources();
		// A request left behind by bytes that turned up some other way would suppress a later ask.
		self.requested_resources.retain(|hash| missing.contains(hash));

		// Sorted, here and below, so hash order does not reach the wire and a seeded simulation stays reproducible.
		let mut unasked: Vec<ResourceHash> = missing.into_iter().filter(|hash| !self.requested_resources.contains(hash)).collect();
		unasked.sort_unstable();
		if unasked.is_empty() {
			return Ok(());
		}

		self.requested_resources.extend(unasked.iter().copied());
		self.transport.broadcast(&SyncPacket::ResourceRequest(unasked))
	}

	/// Answer requests whose bytes turned up since, as a requester never asks twice.
	fn serve_owed_resources(&mut self, target: &dyn SyncTarget) {
		if self.owed_resources.is_empty() {
			return;
		}

		let mut ready: Vec<(ResourceHash, Vec<u8>)> = self.owed_resources.keys().filter_map(|&hash| target.resource_bytes(hash).map(|bytes| (hash, bytes))).collect();
		ready.sort_unstable_by_key(|(hash, _)| *hash);

		for (hash, bytes) in ready {
			let mut recipients: Vec<TransportPeerId> = self.owed_resources.remove(&hash).unwrap_or_default().into_iter().collect();
			recipients.sort_unstable();

			for to in recipients {
				self.serve_resource(to, hash, bytes.clone());
			}
		}
	}

	/// A resource that fails to send, as one past [`MAX_PACKET_BYTES`](crate::MAX_PACKET_BYTES) does, is logged so the
	/// others still go.
	fn serve_resource(&mut self, to: TransportPeerId, hash: ResourceHash, bytes: Vec<u8>) {
		if let Err(error) = self.send_resource(to, hash, bytes) {
			log::error!("Sending resource {hash:?} to {to}: {error}");
		}
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
				if peer == self.peer {
					log::warn!("Ignoring link {from}, whose hello claims this peer's own id");
					return Ok(());
				}
				log::debug!("Join handshake: hello from {peer:?} ({role:?}), synced {}", self.is_synced());
				// A re-greeting over the same link and epoch only announces a role; re-anchoring would strand held broadcasts.
				let known = self.peers.contains_key(&from) && self.delivered.get(&peer).is_some_and(|progress| progress.epoch == epoch);
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
					self.deliver_held(target, events)?;
					self.reannounce_hot_ops(&*target)?;
					events.push(Event::PeerJoined { peer, user });
					// The new peer may hold bytes nobody else here could serve.
					self.requested_resources.clear();
					self.resources_stale = true;
				}

				if role == Role::Host && self.role == Role::Undecided {
					self.role = Role::Guest;
					events.push(Event::RoleChanged { role: self.role });
				}
				// Two hosts meet when elections ran on differing memberships; the lower id keeps the role, and the other
				// syncs from it, handing over what it retired alone.
				if role == Role::Host && self.role == Role::Host && peer < self.peer {
					log::info!("Yielding the host role to {peer:?}, which has the lower id");
					self.role = Role::Guest;
					self.sync = SyncState::AwaitingSync { pending: Vec::new() };
					self.sync_requested_from = None;
					self.announce_role();
					events.push(Event::RoleChanged { role: self.role });
				}
				self.follow_current_host(&*target)?;
				// A link greeted before this peer's role changed hears the current one.
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
				// A request naming a step no longer on this host's line is logged and dropped.
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
					retired: target.retired_marks(),
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
				// Only a copy without retired history is replaced, or a host over an empty document would wipe it; its
				// unretired ops, possibly the only copy of what never reached the host, are replayed on top.
				let registry = sync.registry.filter(|_| target.known_revs().is_empty());
				let replayed = if registry.is_some() { target.hot_log() } else { Vec::new() };
				match registry {
					Some(registry) => target.load(registry, sync.deltas, sync.head)?,
					None => target.merge_remote(sync.deltas, &[], sync.head)?,
				}
				if let Some(document_id) = sync.document_id
					&& target.document_id() != Some(document_id)
				{
					target.adopt_document_id(document_id)?;
				}
				target.absorb_retired_marks(&sync.retired)?;
				if target.absorb_metadata(&sync.metadata)? {
					events.push(Event::MetadataChanged);
				}
				let own = target.metadata();
				if !own.is_empty() {
					self.transport.broadcast_except(None, &SyncPacket::Metadata(own))?;
				}

				self.deferred.extend(target.apply_remote_hot_ops(replayed)?);
				self.deferred.extend(target.apply_remote_hot_ops(sync.hot_log)?);
				self.resources_stale = true;
				self.reannounce_hot_ops(&*target)?;

				// Broadcasts the host delivered before answering are in the snapshot.
				for mark in sync.seen {
					self.observe_delivered(mark);
				}
				self.hold_pending(pending);
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

				let own_hot_ops: Vec<HotOp> = target.hot_log().into_iter().filter(|hot_op| hot_op.timestamp.peer == self.peer).collect();
				self.broadcast_hot_ops(&own_hot_ops)?;

				events.push(Event::Synced);
			}
			SyncPacket::Broadcast(broadcast) => {
				// Senders greet before broadcasting over an ordered channel, so this means the transport reordered.
				let Some(remote) = self.peers.get(&from) else {
					log::error!("Dropping a broadcast from a peer that never said hello");
					return Ok(());
				};
				let sender = remote.peer;
				if !self.worth_holding(sender, &broadcast) {
					log::warn!("Dropping broadcast {}:{} from {sender:?}, which can never be delivered", broadcast.epoch, broadcast.seq);
					return Ok(());
				}
				match &mut self.sync {
					SyncState::AwaitingSync { pending } => pending.push((sender, broadcast)),
					SyncState::Synced => {
						self.held.push((sender, broadcast));
						self.deliver_held(target, events)?;
					}
				}
			}
			SyncPacket::Profile { name } => {
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
						Some(bytes) => self.serve_resource(from, hash, bytes),
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

	/// Settle up after a peer leaves: its undelivered broadcasts are dropped, since applying them would mint orphans,
	/// anything waiting on it is free, and its unretired work, which may have reached only some peers, is re-announced.
	fn close_epoch(&mut self, peer: PeerId, target: &mut dyn SyncTarget, events: &mut Vec<Event>) -> Result<(), ReplicaError> {
		if let SyncState::AwaitingSync { pending } = &mut self.sync {
			pending.retain(|(sender, _)| *sender != peer);
		}
		self.held.retain(|(sender, _)| *sender != peer);

		self.deliver_held(target, events)?;
		self.reannounce_hot_ops(&*target)?;
		Ok(())
	}

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

			// A failure is logged rather than abandoning the bookkeeping and the rest of the queue.
			let applied = match broadcast.body {
				BroadcastBody::HotOps(ops) => target.apply_remote_hot_ops(ops).map(|deferred| self.deferred.extend(deferred)),
				// A host lets a step with a missing parent go, since its author's whole line arrives when the author syncs.
				BroadcastBody::Deltas { deltas, .. } if has_missing_parent(&deltas, &*target) => {
					if self.role == Role::Host {
						log::info!("A retired step arrived whose parent is missing here; its author's line comes with its sync");
						Ok(())
					} else {
						self.resync_for_missing_parent(&*target).map_err(TargetError::from)
					}
				}
				// The host joins a diverged line with a merge delta, which the room follows.
				BroadcastBody::Deltas { deltas, retires } if self.role == Role::Host => target.merge_divergent(deltas, &retires).and_then(|minted| {
					if minted.is_empty() {
						return Ok(());
					}
					self.broadcast(BroadcastBody::Deltas { deltas: minted, retires: Vec::new() }).map_err(TargetError::from)
				}),
				BroadcastBody::Deltas { deltas, retires } => target.merge_remote(deltas, &retires, None),
				BroadcastBody::Retract(ops) => {
					self.deferred.retain(|hot_op| !ops.contains(&hot_op.id()));
					target.retract_hot_ops(&ops)
				}
				BroadcastBody::RetiredMarks(marks) => {
					self.deferred.retain(|hot_op| !marks.covers(hot_op.id()));
					target.absorb_retired_marks(&marks)
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
		if progress.epoch != broadcast.epoch || progress.seq.checked_add(1) != Some(broadcast.seq) {
			return false;
		}

		broadcast.seen.iter().all(|&mark| mark.peer == sender || self.has_delivered(mark))
	}

	/// Whether a broadcast can still come up for delivery. It must directly follow the sender's last one delivered or
	/// queued here, since its link is ordered and a gap or a repeat never resolves.
	fn worth_holding(&self, sender: PeerId, broadcast: &Broadcast) -> bool {
		let Some(progress) = self.delivered.get(&sender).filter(|progress| progress.epoch == broadcast.epoch) else {
			return false;
		};
		let pending = match &self.sync {
			SyncState::AwaitingSync { pending } => pending.as_slice(),
			SyncState::Synced => &[],
		};
		let last = self
			.held
			.iter()
			.chain(pending)
			.filter(|(queued_sender, queued)| *queued_sender == sender && queued.epoch == broadcast.epoch)
			.fold(progress.seq, |last, (_, queued)| last.max(queued.seq));

		last.checked_add(1) == Some(broadcast.seq)
	}

	/// Whether a broadcast's dependency is met. One that never can be counts as met rather than stalling its sender,
	/// so every mark of this peer's own is met: none can be ahead of what it sent.
	fn has_delivered(&self, mark: PeerSeq) -> bool {
		if mark.peer == self.peer {
			return true;
		}
		match self.delivered.get(&mark.peer) {
			Some(progress) if progress.epoch != mark.epoch => true,
			Some(progress) if progress.seq >= mark.seq => true,
			// A peer with no open link sends nothing more, including one whose hello never arrived.
			_ => !self.is_connected(mark.peer),
		}
	}
}

/// Whether a batch names a parent neither the target nor the batch holds.
fn has_missing_parent(deltas: &[Delta], target: &dyn SyncTarget) -> bool {
	deltas
		.iter()
		.any(|delta| delta.all_parents().any(|parent| !target.contains_rev(parent) && !deltas.iter().any(|other| other.id == parent)))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{MockEndpoint, MockNetwork};
	use document_graph_storage::Session;

	const HOST_PEER: PeerId = PeerId(1);

	/// A bare endpoint that speaks to the host by hand, each knowing the other.
	fn connect_bare_peer(network: &mut MockNetwork, host: &mut Replica, session: &mut Session) -> MockEndpoint {
		let mut bare = network.endpoint();
		network.connect(bare.id());
		bare.poll();
		host.poll(session);
		bare
	}

	fn hello_as(peer: PeerId, role: Role, seq: u64) -> SyncPacket {
		SyncPacket::Hello {
			peer,
			user: UserId(peer.0),
			role,
			epoch: 1,
			seq,
		}
	}

	fn hello(seq: u64) -> SyncPacket {
		hello_as(PeerId(2), Role::Guest, seq)
	}

	fn broadcast(seq: u64, seen: Vec<PeerSeq>) -> SyncPacket {
		SyncPacket::Broadcast(Broadcast {
			epoch: 1,
			seq,
			seen,
			body: BroadcastBody::HotOps(Vec::new()),
		})
	}

	fn hosted_room() -> (MockNetwork, Replica, TransportPeerId, Session) {
		let mut network = MockNetwork::new(0);
		let endpoint = network.endpoint();
		let link = endpoint.id();
		network.connect(link);
		(network, Replica::host(endpoint, HOST_PEER, UserId(1)), link, Session::with_peer(HOST_PEER))
	}

	/// A host holding one broadcast from peer 2 that waits on peer 3's seq 5, with both greeted from their own links.
	fn host_holding_one() -> (MockNetwork, Replica, TransportPeerId, Session, MockEndpoint) {
		let (mut network, mut host, host_link, mut session) = hosted_room();
		let mut sender = connect_bare_peer(&mut network, &mut host, &mut session);
		let mut third = connect_bare_peer(&mut network, &mut host, &mut session);
		third.send(host_link, &hello_as(PeerId(3), Role::Guest, 0)).unwrap();
		sender.send(host_link, &hello(0)).unwrap();
		sender.send(host_link, &broadcast(1, vec![PeerSeq { peer: PeerId(3), epoch: 1, seq: 5 }])).unwrap();
		network.deliver_all();
		host.poll(&mut session);
		assert_eq!(host.held_broadcasts(), 1, "the broadcast waits for peer 3");
		(network, host, host_link, session, sender)
	}

	#[test]
	fn a_hello_at_the_largest_seq_does_not_overflow() {
		let (mut network, mut host, host_link, mut session, _sender) = host_holding_one();

		let mut second_link = connect_bare_peer(&mut network, &mut host, &mut session);
		second_link.send(host_link, &hello(u64::MAX)).unwrap();
		network.deliver_all();
		host.poll(&mut session);
		assert_eq!(host.held_broadcasts(), 1);
	}

	#[test]
	fn a_repeated_or_skipping_broadcast_is_never_held() {
		let (mut network, mut host, host_link, mut session, mut sender) = host_holding_one();

		for seq in [1, 1, 3, 0, u64::MAX] {
			sender.send(host_link, &broadcast(seq, Vec::new())).unwrap();
		}
		network.deliver_all();
		host.poll(&mut session);
		assert_eq!(host.held_broadcasts(), 1, "only the next seq can ever be delivered");

		sender.send(host_link, &broadcast(2, Vec::new())).unwrap();
		network.deliver_all();
		host.poll(&mut session);
		assert_eq!(host.held_broadcasts(), 2, "the next seq waits behind the first");
	}

	#[test]
	fn a_mark_ahead_of_our_own_seq_does_not_hold_a_broadcast() {
		let (mut network, mut host, host_link, mut session) = hosted_room();
		let mut sender = connect_bare_peer(&mut network, &mut host, &mut session);
		let ahead = PeerSeq {
			peer: HOST_PEER,
			epoch: host.epoch,
			seq: u64::MAX,
		};
		sender.send(host_link, &hello(0)).unwrap();
		sender.send(host_link, &broadcast(1, vec![ahead])).unwrap();
		network.deliver_all();
		host.poll(&mut session);
		assert_eq!(host.held_broadcasts(), 0, "nobody can have seen more of the host than it sent");
	}

	#[test]
	fn a_hello_claiming_our_own_id_is_ignored() {
		let (mut network, mut host, host_link, mut session) = hosted_room();
		let mut impostor = connect_bare_peer(&mut network, &mut host, &mut session);
		impostor.send(host_link, &hello_as(HOST_PEER, Role::Guest, 0)).unwrap();
		impostor.send(host_link, &broadcast(1, Vec::new())).unwrap();
		network.deliver_all();
		host.poll(&mut session);

		assert_eq!(host.peers().count(), 0, "the impostor never joins");
		assert_eq!(host.held_broadcasts(), 0, "nothing from it waits forever");
	}

	#[test]
	fn a_departed_peer_is_owed_nothing() {
		let (mut network, mut host, host_link, mut session) = hosted_room();
		let mut bare = connect_bare_peer(&mut network, &mut host, &mut session);

		bare.send(host_link, &hello(0)).unwrap();
		bare.send(host_link, &SyncPacket::ResourceRequest(vec![ResourceHash::from(b"bytes".as_slice())])).unwrap();
		network.deliver_all();
		host.poll(&mut session);
		assert_eq!(host.owed_resources.len(), 1, "the host has no bytes to send yet");

		network.disconnect(bare.id());
		host.poll(&mut session);
		assert!(host.owed_resources.is_empty(), "nothing is owed to a link that closed");
	}
}
