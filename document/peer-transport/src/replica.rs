use crate::packet::{PacketError, Role, SyncPacket, SyncPayload};
use crate::room::{Room, RoomEvent, TransportPeerId};
use crate::target::{SyncTarget, TargetError};
use document_graph_storage::{Delta, HotOp, PeerId, ResourceHash, TimeStamp, UserId};
use std::collections::HashMap;

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
	ResourceReceived(ResourceHash),
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

enum SyncState {
	Synced,
	AwaitingSync { pending_hot_ops: Vec<HotOp> },
}

/// Protocol state for one peer in a room. Owns no document; `poll` applies into a `SyncTarget`.
pub struct Replica {
	room: Room,
	role: Role,
	peer: PeerId,
	user: UserId,
	peers: HashMap<TransportPeerId, RemotePeer>,
	sync: SyncState,
}

impl Replica {
	pub fn host(room: Room, peer: PeerId, user: UserId) -> Self {
		Self::new(room, Role::Host, peer, user, SyncState::Synced)
	}

	pub fn guest(room: Room, peer: PeerId, user: UserId) -> Self {
		Self::new(room, Role::Guest, peer, user, SyncState::AwaitingSync { pending_hot_ops: Vec::new() })
	}

	fn new(room: Room, role: Role, peer: PeerId, user: UserId, sync: SyncState) -> Self {
		Self {
			room,
			role,
			peer,
			user,
			peers: HashMap::new(),
			sync,
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
		self.room.broadcast(&SyncPacket::HotOps(ops.to_vec()))
	}

	/// Host only.
	pub fn broadcast_retired(&mut self, deltas: &[Delta], up_to: TimeStamp) -> Result<(), PacketError> {
		debug_assert_eq!(self.role, Role::Host);
		if deltas.is_empty() {
			return Ok(());
		}
		self.room.broadcast(&SyncPacket::Deltas {
			deltas: deltas.to_vec(),
			retires_up_to: Some(up_to),
		})
	}

	pub fn request_resources(&mut self, hashes: Vec<ResourceHash>) -> Result<(), PacketError> {
		if hashes.is_empty() {
			return Ok(());
		}
		self.room.broadcast(&SyncPacket::ResourceRequest(hashes))
	}

	pub fn send_resource(&mut self, to: TransportPeerId, hash: ResourceHash, bytes: Vec<u8>) -> Result<(), PacketError> {
		self.room.send(to, &SyncPacket::Resource { hash, bytes })
	}

	pub fn leave(&mut self) {
		self.room.close();
	}

	pub fn poll(&mut self, target: &mut dyn SyncTarget) -> Vec<Event> {
		let mut events = Vec::new();

		for room_event in self.room.poll() {
			let result = match room_event {
				RoomEvent::PeerConnected(transport_peer) => self.send_hello(transport_peer),
				RoomEvent::PeerDisconnected(transport_peer) => {
					if let Some(remote) = self.peers.remove(&transport_peer) {
						events.push(Event::PeerLeft { peer: remote.peer });
					}
					Ok(())
				}
				RoomEvent::Packet(transport_peer, packet) => self.handle_packet(transport_peer, packet, target, &mut events),
				RoomEvent::Malformed(transport_peer, error) => {
					log::warn!("Dropping malformed packet from {transport_peer}: {error}");
					Ok(())
				}
			};

			if let Err(error) = result {
				log::error!("Sync error: {error}");
			}
		}

		events
	}

	fn send_hello(&mut self, transport_peer: TransportPeerId) -> Result<(), ReplicaError> {
		let hello = SyncPacket::Hello {
			peer: self.peer,
			user: self.user,
			role: self.role,
		};
		self.room.send(transport_peer, &hello)?;
		Ok(())
	}

	fn handle_packet(&mut self, from: TransportPeerId, packet: SyncPacket, target: &mut dyn SyncTarget, events: &mut Vec<Event>) -> Result<(), ReplicaError> {
		match packet {
			SyncPacket::Hello { peer, user, role } => {
				self.peers.insert(from, RemotePeer { peer, user, role });
				events.push(Event::PeerJoined { peer, user });

				if role == Role::Host && !self.is_synced() {
					self.room.send(from, &SyncPacket::SyncRequest { known_revs: target.known_revs() })?;
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
				};
				self.room.send(from, &SyncPacket::Sync(Box::new(sync)))?;
			}
			SyncPacket::Sync(sync) => {
				let SyncState::AwaitingSync { pending_hot_ops } = std::mem::replace(&mut self.sync, SyncState::Synced) else {
					return Ok(());
				};

				match sync.registry {
					Some(registry) => target.load(registry, sync.deltas, sync.head)?,
					None => target.merge_remote(sync.deltas, None)?,
				}
				let pending = Self::hot_ops_missing_from_snapshot(pending_hot_ops, &sync.hot_log);
				target.apply_remote_hot_ops(sync.hot_log)?;
				target.apply_remote_hot_ops(pending)?;

				let host_is_missing = target.deltas_unknown_to(&sync.known_revs);
				if !host_is_missing.is_empty() {
					self.room.send(
						from,
						&SyncPacket::Deltas {
							deltas: host_is_missing,
							retires_up_to: None,
						},
					)?;
				}
				self.room.send(from, &SyncPacket::ResourceRequest(target.missing_resources().into_iter().collect()))?;

				events.push(Event::Synced);
			}
			SyncPacket::HotOps(ops) => match &mut self.sync {
				SyncState::AwaitingSync { pending_hot_ops } => pending_hot_ops.extend(ops),
				SyncState::Synced => {
					target.apply_remote_hot_ops(ops)?;
					events.push(Event::Changed);
				}
			},
			SyncPacket::Deltas { deltas, retires_up_to } => {
				if self.role == Role::Host {
					self.room.broadcast_except(
						from,
						&SyncPacket::Deltas {
							deltas: deltas.clone(),
							retires_up_to,
						},
					)?;
				}
				target.merge_remote(deltas, retires_up_to)?;
				events.push(Event::Changed);
			}
			SyncPacket::ResourceRequest(hashes) => {
				for hash in hashes {
					match target.resource_bytes(hash) {
						Some(bytes) => self.room.send(from, &SyncPacket::Resource { hash, bytes })?,
						None => events.push(Event::ResourceRequested { from, hash }),
					}
				}
			}
			SyncPacket::Resource { hash, bytes } => {
				if ResourceHash::from(bytes.as_slice()) != hash {
					log::warn!("Resource from {from} does not match its hash; dropping");
					return Ok(());
				}
				target.store_resource(hash, bytes)?;
				events.push(Event::ResourceReceived(hash));
			}
		}

		Ok(())
	}

	/// Hot ops that arrived while the sync was in flight may already be included in it.
	fn hot_ops_missing_from_snapshot(mut pending: Vec<HotOp>, snapshot_hot_log: &[HotOp]) -> Vec<HotOp> {
		pending.retain(|rev| !snapshot_hot_log.iter().any(|existing| existing.timestamp == rev.timestamp));
		pending
	}
}
