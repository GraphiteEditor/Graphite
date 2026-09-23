//! Deterministic simulation over the mock transport: random edits on every peer, random delivery
//! order, random host retirement. After quiescence every peer must hold the same retired state.
//!
//! Inspect one run with `SEED=<n> GUESTS=<n> cargo test -p peer-transport --test simulation inspect_seed -- --ignored --nocapture`.

use document_graph_storage::{AttributeDelta, Delta, HotOp, Network, NetworkId, PeerId, Registry, RegistryDelta, ResourceHash, ResourceId, Rev, Session, TimeStamp, UserId};
use peer_transport::mock::{MockEndpoint, MockNetwork};
use peer_transport::{Event, Replica, Role, SyncTarget, TargetError, TransportPeerId};
use std::collections::{HashMap, HashSet};

/// A peer's document plus the byte store the editor keeps application-wide. Bare `Session` takes the
/// trait's no-op resource defaults, which would leave the whole request path unexercised.
struct SimTarget {
	session: Session,
	resources: HashMap<ResourceHash, Vec<u8>>,
	/// History as of the last `flush`, to check the sync path never leaves applied state undurable.
	durable_history: Vec<Rev>,
	flushes: usize,
}

impl SimTarget {
	fn new(peer: PeerId) -> Self {
		Self {
			session: Session::with_peer(peer),
			resources: HashMap::new(),
			durable_history: Vec::new(),
			flushes: 0,
		}
	}

	fn history_revs(&self) -> Vec<Rev> {
		self.session.history().map(|delta| delta.id).collect()
	}
}

impl SyncTarget for SimTarget {
	fn peer(&self) -> PeerId {
		SyncTarget::peer(&self.session)
	}

	fn head(&self) -> Option<Rev> {
		SyncTarget::head(&self.session)
	}

	fn retired_registry(&self) -> Registry {
		SyncTarget::retired_registry(&self.session)
	}

	fn hot_log(&self) -> Vec<HotOp> {
		SyncTarget::hot_log(&self.session)
	}

	fn known_revs(&self) -> Vec<Rev> {
		SyncTarget::known_revs(&self.session)
	}

	fn contains_rev(&self, rev: Rev) -> bool {
		SyncTarget::contains_rev(&self.session, rev)
	}

	fn deltas_unknown_to(&self, known: &[Rev]) -> Vec<Delta> {
		SyncTarget::deltas_unknown_to(&self.session, known)
	}

	fn load(&mut self, registry: Registry, history: Vec<Delta>, head: Option<Rev>) -> Result<(), TargetError> {
		SyncTarget::load(&mut self.session, registry, history, head)
	}

	fn apply_remote_hot_ops(&mut self, ops: Vec<HotOp>) -> Result<(), TargetError> {
		SyncTarget::apply_remote_hot_ops(&mut self.session, ops)
	}

	fn merge_remote(&mut self, deltas: Vec<Delta>, retires: &[TimeStamp]) -> Result<(), TargetError> {
		SyncTarget::merge_remote(&mut self.session, deltas, retires)
	}

	fn retired_through(&self) -> HashMap<PeerId, u64> {
		SyncTarget::retired_through(&self.session)
	}

	fn absorb_retired_through(&mut self, remote: &HashMap<PeerId, u64>) -> Result<(), TargetError> {
		SyncTarget::absorb_retired_through(&mut self.session, remote)
	}

	fn flush(&mut self) -> Result<(), TargetError> {
		self.durable_history = self.history_revs();
		self.flushes += 1;
		Ok(())
	}

	fn missing_resources(&self) -> HashSet<ResourceHash> {
		self.session.all_referenced_resource_hashes().into_iter().filter(|hash| !self.resources.contains_key(hash)).collect()
	}

	fn resource_bytes(&self, hash: ResourceHash) -> Option<Vec<u8>> {
		self.resources.get(&hash).cloned()
	}

	fn store_resource(&mut self, hash: ResourceHash, bytes: &[u8]) -> Result<(), TargetError> {
		self.resources.insert(hash, bytes.to_vec());
		Ok(())
	}
}

struct Peer {
	target: SimTarget,
	replica: Replica,
	peer: PeerId,
	user: UserId,
	transport: TransportPeerId,
	/// Gone for good, unlike a rejoin. Its document stops taking part and stops being asserted on.
	departed: bool,
}

impl Peer {
	fn new(endpoint: MockEndpoint, role: Role, index: u64) -> Self {
		let (peer, user) = (PeerId(index), UserId(index));
		let transport = endpoint.id();
		let replica = match role {
			Role::Host => Replica::host(endpoint, peer, user),
			Role::Guest => Replica::guest(endpoint, peer, user),
		};
		Self {
			target: SimTarget::new(peer),
			replica,
			peer,
			user,
			transport,
			departed: false,
		}
	}

	/// Drop off the room, keeping the document. The editor rebuilds the `Replica` on a reconnect but
	/// reuses the `Gdd`'s persisted `PeerId`, so protocol state resets while document state does not.
	fn rejoin(&mut self, network: &mut MockNetwork) {
		network.disconnect(self.transport);

		let endpoint = network.endpoint();
		self.transport = endpoint.id();
		self.replica = Replica::guest(endpoint, self.peer, self.user);
		network.connect(self.transport);
	}

	/// Close the tab. The peer is never heard from again, so the room has to settle without whatever
	/// it alone knew.
	fn depart(&mut self, network: &mut MockNetwork) {
		network.disconnect(self.transport);
		self.departed = true;
	}

	fn session(&self) -> &Session {
		&self.target.session
	}

	fn stage(&mut self, op: RegistryDelta) {
		let hot_ops = self.target.session.stage_ops([op]).expect("stage");
		self.replica.broadcast_hot_ops(&hot_ops).expect("broadcast");
	}

	/// Introduce a resource only this peer holds the bytes for, so every other peer has to fetch them.
	fn stage_resource(&mut self, bytes: Vec<u8>) {
		let hash = ResourceHash::from(bytes.as_slice());
		self.target.resources.insert(hash, bytes);

		// Each peer names its resources itself, the way live resources get a fresh `ResourceId`. Deriving
		// the id from the content instead would collide across peers, which `AddResource` resolves by
		// replay order rather than by merging.
		let id = ResourceId::from(u64::from(ResourceId::from_hash(&hash)) ^ self.peer.0);
		let hot_ops = self.target.session.stage_embedded_resource(id, hash).expect("stage resource");
		self.replica.broadcast_hot_ops(&hot_ops).expect("broadcast");
	}

	fn retire(&mut self) {
		let Some(up_to) = self.target.session.hot_log().iter().map(|hot_op| hot_op.timestamp).max() else {
			return;
		};
		let retired_hot_ops = self.target.session.hot_ops_up_to(up_to);
		let revs = self.target.session.retire(up_to).expect("retire");
		let deltas: Vec<_> = revs.iter().filter_map(|&rev| self.target.session.delta(rev).cloned()).collect();
		self.replica.broadcast_retired(&deltas, &retired_hot_ops).expect("broadcast retired");
	}

	fn poll(&mut self) -> Vec<Event> {
		let flushes_before = self.target.flushes;
		let events = self.replica.poll(&mut self.target);
		assert_eq!(self.target.flushes, flushes_before + 1, "poll must flush the target exactly once");
		events
	}
}

fn random_op(network: &mut MockNetwork, session: &Session) -> RegistryDelta {
	let network_id = NetworkId(1 + network.random_below(3) as u64);
	match network.random_below(4) {
		0 | 1 => RegistryDelta::ChangeDocumentAttribute {
			delta: AttributeDelta {
				key: format!("key{}", network.random_below(3)),
				value: Some(serde_json::json!(network.random_below(100))),
			},
		},
		2 if let Some(network) = session.registry().networks.get(&network_id) => RegistryDelta::RemoveNetwork {
			id: network_id,
			snapshot: network.clone(),
		},
		_ if session.registry().networks.contains_key(&network_id) => RegistryDelta::ChangeNetworkAttribute {
			id: network_id,
			delta: AttributeDelta {
				key: "name".into(),
				value: Some(serde_json::json!(network.random_below(100))),
			},
		},
		_ => RegistryDelta::AddNetwork {
			id: network_id,
			network: Network::default(),
		},
	}
}

fn present_guests(peers: &[Peer]) -> usize {
	peers.iter().skip(1).filter(|peer| !peer.departed).count()
}

/// Run until nothing is in flight and no peer reports progress. Resource transfers need several
/// rounds (request out, bytes back), so this is not a fixed number of passes.
fn quiesce(network: &mut MockNetwork, peers: &mut [Peer]) {
	for _ in 0..1000 {
		network.deliver_all();
		let events: usize = peers.iter_mut().filter(|peer| !peer.departed).map(|peer| peer.poll().len()).sum();
		if events == 0 && network.pending() == 0 {
			return;
		}
	}
	panic!("the room never went idle");
}

fn simulate(seed: u64, guest_count: usize, steps: usize) -> (MockNetwork, Vec<Peer>) {
	let mut network = MockNetwork::new(seed);

	let mut peers: Vec<Peer> = (0..=guest_count)
		.map(|index| {
			let endpoint = network.endpoint();
			let id = endpoint.id();
			let role = if index == 0 { Role::Host } else { Role::Guest };
			let peer = Peer::new(endpoint, role, index as u64 + 1);
			network.connect(id);
			peer
		})
		.collect();

	for _ in 0..steps {
		let index = network.random_below(peers.len());
		if peers[index].departed {
			continue;
		}

		match network.random_below(16) {
			// A guest edits only once synced; anything staged earlier would be invisible to its peers.
			0..=2 if peers[index].replica.is_synced() => {
				let op = random_op(&mut network, peers[index].session());
				peers[index].stage(op);
			}
			// A small pool of distinct payloads, so peers sometimes introduce the same resource
			// concurrently and sometimes one nobody else can serve.
			8 if peers[index].replica.is_synced() => {
				let bytes = format!("resource-{}", network.random_below(5)).into_bytes();
				peers[index].stage_resource(bytes);
			}
			0..=2 | 8 => {}
			3 => peers[0].retire(),
			4..=7 => {
				network.step();
			}
			// The host cannot hand over, so only guests drop and come back.
			9 if index > 0 => peers[index].rejoin(&mut network),
			9 => {
				let endpoint = network.endpoint();
				let id = endpoint.id();
				peers.push(Peer::new(endpoint, Role::Guest, peers.len() as u64 + 1));
				network.connect(id);
			}
			// Keep one guest around, so the room stays a room.
			10 if index > 0 && present_guests(&peers) > 1 => peers[index].depart(&mut network),
			_ => {
				peers[index].poll();
			}
		}
	}

	quiesce(&mut network, &mut peers);
	peers[0].retire();
	quiesce(&mut network, &mut peers);

	(network, peers)
}

fn assert_converged(seed: u64, peers: &[Peer]) {
	let present = || peers.iter().enumerate().filter(|(_, peer)| !peer.departed);

	let host = &peers[0];
	let host_history: Vec<_> = host.session().history().map(|delta| delta.id).collect();
	for (index, guest) in present().skip(1) {
		let guest_history: Vec<_> = guest.session().history().map(|delta| delta.id).collect();
		assert_eq!(guest_history, host_history, "seed {seed}: guest {index} history diverged");
		assert_eq!(guest.session().head_rev(), host.session().head_rev(), "seed {seed}: guest {index} head diverged");
		assert_eq!(guest.session().retired_registry(), host.session().retired_registry(), "seed {seed}: guest {index} registry diverged");
		assert!(guest.session().hot_log().is_empty(), "seed {seed}: guest {index} still holds hot ops");
	}

	// Bytes that left with a departed peer are gone for good, so the room is only answerable for the
	// resources someone still in it could have served.
	let servable: HashSet<ResourceHash> = present().flat_map(|(_, peer)| peer.target.resources.keys().copied()).collect();

	// An idle room with anything outstanding is stuck, which convergence of content alone can miss:
	// a broadcast held forever behind a dependency that will never arrive changes nothing observable.
	for (index, peer) in present() {
		assert_eq!(peer.replica.held_broadcasts(), 0, "seed {seed}: peer {index} still holds undelivered broadcasts");
		assert_eq!(peer.target.durable_history, peer.target.history_revs(), "seed {seed}: peer {index} has unflushed history");

		let unserved: Vec<_> = SyncTarget::missing_resources(&peer.target).into_iter().filter(|hash| servable.contains(hash)).collect();
		assert!(unserved.is_empty(), "seed {seed}: peer {index} is missing resource bytes another peer holds: {unserved:?}");
	}
}

#[test]
fn peers_converge_under_random_interleavings() {
	for seed in 0..1000 {
		let (_, peers) = simulate(seed, 1 + (seed % 3) as usize, 200);
		assert_converged(seed, &peers);
	}
}

/// A guest that drops and comes back keeps its `PeerId` but gets a fresh `Replica`, so its broadcast
/// sequence restarts. The room must follow the restart rather than keep waiting on the sequence the
/// old connection had reached.
#[test]
fn a_rejoining_guest_is_still_heard() {
	let mut network = MockNetwork::new(0);

	let mut peers: Vec<Peer> = (0..2)
		.map(|index| {
			let endpoint = network.endpoint();
			let id = endpoint.id();
			let role = if index == 0 { Role::Host } else { Role::Guest };
			let peer = Peer::new(endpoint, role, index as u64 + 1);
			network.connect(id);
			peer
		})
		.collect();

	quiesce(&mut network, &mut peers);

	peers[1].stage(RegistryDelta::AddNetwork {
		id: NetworkId(1),
		network: Network::default(),
	});
	quiesce(&mut network, &mut peers);
	assert!(peers[0].session().registry().networks.contains_key(&NetworkId(1)), "the host missed the first edit");

	peers[1].rejoin(&mut network);
	quiesce(&mut network, &mut peers);

	peers[1].stage(RegistryDelta::AddNetwork {
		id: NetworkId(2),
		network: Network::default(),
	});
	quiesce(&mut network, &mut peers);

	assert_eq!(peers[0].replica.held_broadcasts(), 0, "the host is holding the rejoined guest's broadcast");
	assert!(peers[0].session().registry().networks.contains_key(&NetworkId(2)), "the host missed the edit made after the rejoin");
}

#[test]
#[ignore = "manual inspection of one seed"]
fn inspect_seed() {
	let _ = env_logger::builder().is_test(true).try_init();
	let seed = std::env::var("SEED").ok().and_then(|value| value.parse().ok()).unwrap_or(0);
	let guests = std::env::var("GUESTS").ok().and_then(|value| value.parse().ok()).unwrap_or(1);

	let (_, peers) = simulate(seed, guests, 200);
	for (index, peer) in peers.iter().enumerate() {
		eprintln!("=== peer {index} head {:?}", peer.session().head_rev());
		eprintln!("registry {}", serde_json::to_string(peer.session().retired_registry()).unwrap());
		eprintln!("hot log {:?}", peer.session().hot_log());
		eprintln!("resources {:?}", peer.target.resources.keys().collect::<Vec<_>>());
		for delta in peer.session().history() {
			eprintln!("{:?} parent {:?} author {:?} ts {} {:?}", delta.id, delta.parent, delta.author, delta.timestamp.counter, delta.kind);
		}
	}
	assert_converged(seed, &peers);
}
