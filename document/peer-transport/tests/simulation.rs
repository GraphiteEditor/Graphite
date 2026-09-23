//! Deterministic simulation over the mock transport: random edits on every peer, random delivery
//! order, random host retirement. After quiescence every peer must hold the same retired state.
//!
//! Inspect one run with `SEED=<n> GUESTS=<n> cargo test -p peer-transport --test simulation inspect_seed -- --ignored --nocapture`.

use document_graph_storage::{
	AttributeDelta, Delta, HotOp, HotOpId, Implementation, Network, NetworkId, Node, NodeId, NodeInput, PeerId, Priority, Registry, RegistryDelta, ResourceHash, ResourceId, RetiredHotOps, Rev,
	Session, SourceKey, TimeStamp, UserId,
};
use peer_transport::mock::{MockEndpoint, MockNetwork};
use peer_transport::{Event, Replica, Role, SyncTarget, TargetError, TransportPeerId};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};

static TOTAL_NODES: AtomicUsize = AtomicUsize::new(0);
static TOTAL_WIRED: AtomicUsize = AtomicUsize::new(0);
static TOTAL_EXPORTS: AtomicUsize = AtomicUsize::new(0);
static TOTAL_INPUT_ATTRIBUTES: AtomicUsize = AtomicUsize::new(0);
static TOTAL_SOURCES: AtomicUsize = AtomicUsize::new(0);

/// A peer's document plus the byte store the editor keeps application-wide. Bare `Session` takes the
/// trait's no-op resource defaults, which would leave the whole request path unexercised.
struct SimTarget {
	session: Session,
	resources: HashMap<ResourceHash, Vec<u8>>,
	/// History as of the last `flush`, to check the sync path never leaves applied state undurable.
	durable_history: Vec<Rev>,
	flushes: usize,
	/// Every rev this peer has ever held, to check none is later dropped. See [`SimTarget::flush`].
	seen_revs: HashSet<Rev>,
	seed: u64,
}

impl SimTarget {
	fn new(peer: PeerId, seed: u64) -> Self {
		Self {
			session: Session::with_peer(peer),
			resources: HashMap::new(),
			durable_history: Vec::new(),
			flushes: 0,
			seen_revs: HashSet::new(),
			seed,
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

	fn apply_remote_hot_ops(&mut self, ops: Vec<HotOp>) -> Result<Vec<HotOp>, TargetError> {
		SyncTarget::apply_remote_hot_ops(&mut self.session, ops)
	}

	fn merge_remote(&mut self, deltas: Vec<Delta>, retires: &[HotOpId]) -> Result<(), TargetError> {
		SyncTarget::merge_remote(&mut self.session, deltas, retires)
	}

	fn retired_marks(&self) -> RetiredHotOps {
		SyncTarget::retired_marks(&self.session)
	}

	fn absorb_retired_marks(&mut self, remote: &RetiredHotOps) -> Result<(), TargetError> {
		SyncTarget::absorb_retired_marks(&mut self.session, remote)
	}

	fn flush(&mut self) -> Result<(), TargetError> {
		// History is append-only. Hot ops are explicitly transient and a rejoin may drop the lot, but a
		// retired delta is the durable record, so losing one is data loss no convergence check would see.
		let current: HashSet<Rev> = self.history_revs().into_iter().collect();
		if let Some(dropped) = self.seen_revs.iter().find(|rev| !current.contains(rev)) {
			panic!("seed {}: peer {:?} dropped retired delta {dropped:?} from history", self.seed, self.session.peer());
		}
		self.seen_revs.extend(current);

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
	fn new(endpoint: MockEndpoint, role: Role, index: u64, seed: u64) -> Self {
		let (peer, user) = (PeerId(index), UserId(index));
		let transport = endpoint.id();
		let replica = match role {
			Role::Host => Replica::host(endpoint, peer, user),
			Role::Guest => Replica::guest(endpoint, peer, user),
		};
		Self {
			target: SimTarget::new(peer, seed),
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
		self.retire_up_to(up_to);
	}

	/// Retire only part of the hot log, leaving newer ops live. A later op of one author can then retire
	/// while an earlier one is still in flight, which is what puts entries above the retired prefix.
	fn retire_prefix(&mut self, nth: usize) {
		let mut timestamps: Vec<TimeStamp> = self.target.session.hot_log().iter().map(|hot_op| hot_op.timestamp).collect();
		if timestamps.is_empty() {
			return;
		}
		timestamps.sort();

		self.retire_up_to(timestamps[nth.min(timestamps.len() - 1)]);
	}

	fn retire_up_to(&mut self, up_to: TimeStamp) {
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

/// Node-level ops, which reach the input-slot LWW arms and the resurrection path a concurrent remove
/// triggers. `None` when the registry holds nothing the drawn op could target.
fn random_node_op(network: &mut MockNetwork, session: &Session) -> Option<RegistryDelta> {
	let registry = session.registry();
	let node_id = NodeId(1 + network.random_below(4) as u64);
	let live_nodes: Vec<NodeId> = registry.node_instances.keys().copied().collect();
	let live_networks: Vec<NetworkId> = registry.networks.keys().copied().collect();

	match network.random_below(8) {
		// Per-slot attributes, the one place `InputSlot.attributes` is written.
		6 => {
			let node = registry.node_instances.get(&node_id)?;
			let index = network.random_below(node.inputs().len().max(1)) as u32;

			Some(RegistryDelta::ChangeNodeInputAttribute {
				id: node_id,
				index,
				delta: AttributeDelta {
					key: "label".into(),
					value: Some(serde_json::json!(network.random_below(100))),
				},
			})
		}
		// A node needs a live network to sit in, and `AddNode` errors on one that already exists.
		0 | 1 if !registry.node_instances.contains_key(&node_id) => {
			let network_id = *pick(network, &live_networks)?;
			Some(RegistryDelta::AddNode {
				id: node_id,
				node: Node::new(network_id, Implementation::ProtoNode(ResourceId::from(7)), 2),
			})
		}
		2 => registry.node_instances.get(&node_id).map(|node| RegistryDelta::RemoveNode { id: node_id, snapshot: node.clone() }),
		3 | 4 => {
			let node = registry.node_instances.get(&node_id)?;
			let index = network.random_below(node.inputs().len().max(1)) as u32;

			// Wiring to a node that is live here can still land on a peer that concurrently removed it,
			// which is what drives the resurrection path.
			let wire_to = pick(network, &live_nodes).copied();
			let new_input = match wire_to {
				Some(target) if network.random_below(3) > 0 => NodeInput::Node { id: target, index: 0 },
				_ => NodeInput::Value {
					value: serde_json::json!(network.random_below(100)),
					exposed: false,
				},
			};
			Some(RegistryDelta::ChangeNodeInput { id: node_id, index, new_input })
		}
		_ => pick(network, &live_nodes).copied().map(|node_id| RegistryDelta::ChangeNodeAttribute {
			id: node_id,
			delta: AttributeDelta {
				key: "name".into(),
				value: Some(serde_json::json!(network.random_below(100))),
			},
		}),
	}
}

/// Resource-lifecycle ops past creation. `AddResource` already rides the staged-resource path, so
/// these cover the rest of the chain: the resolved hash, the source fallback list, and removal.
/// `None` when the registry holds no resource to target.
fn random_resource_op(network: &mut MockNetwork, target: &SimTarget) -> Option<RegistryDelta> {
	let session = &target.session;
	let registry = session.registry();
	let live: Vec<ResourceId> = registry.resources.keys().copied().collect();
	let id = *pick(network, &live)?;

	// A small pool of priorities, so concurrent adds sometimes collide on a key and sometimes stack up
	// as distinct entries in the same chain.
	let key = SourceKey {
		priority: Priority::new(network.random_below(3) as f64).expect("a whole number is finite"),
		peer: session.peer(),
	};

	Some(match network.random_below(5) {
		// Only a hash this peer holds the bytes for: a resolved hash is content derived, so in the editor
		// the peer that resolves a resource is the one that fetched it.
		0 => {
			let held: Vec<ResourceHash> = target.resources.keys().copied().collect();
			RegistryDelta::SetResourceHash {
				id,
				hash: Some(*pick(network, &held)?),
			}
		}
		1 => RegistryDelta::SetResourceHash { id, hash: None },
		2 => RegistryDelta::AddSource {
			id,
			key,
			source: serde_json::json!("Embedded"),
		},
		3 => RegistryDelta::RemoveSource { id, key },
		_ => RegistryDelta::RemoveResource {
			id,
			snapshot: registry.resources.get(&id)?.clone(),
		},
	})
}

/// One element at random, or `None` when there is nothing to choose from.
fn pick<'a, T>(network: &mut MockNetwork, options: &'a [T]) -> Option<&'a T> {
	(!options.is_empty()).then(|| &options[network.random_below(options.len())])
}

fn random_op(network: &mut MockNetwork, target: &SimTarget) -> RegistryDelta {
	let session = &target.session;
	let network_id = NetworkId(1 + network.random_below(3) as u64);

	// Half the ops are node-level, falling through to the network-level ones when nothing fits.
	if network.random_below(2) == 0
		&& let Some(op) = random_node_op(network, session)
	{
		return op;
	}

	// Resources exist only once a peer has staged one, so this yields nothing early in a run.
	if network.random_below(5) == 0
		&& let Some(op) = random_resource_op(network, target)
	{
		return op;
	}

	match network.random_below(6) {
		// Export slots resize on demand and LWW per slot, and clearing one leaves a tombstone that must
		// still compare equal to the slot being absent.
		4 | 5 if session.registry().networks.contains_key(&network_id) => {
			let live_nodes: Vec<NodeId> = session.registry().node_instances.keys().copied().collect();
			let export = match pick(network, &live_nodes).copied() {
				Some(id) if network.random_below(4) > 0 => Some(NodeInput::Node { id, index: 0 }),
				_ => None,
			};

			RegistryDelta::SetNetworkExport {
				id: network_id,
				index: network.random_below(3) as u32,
				export,
			}
		}
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
			let peer = Peer::new(endpoint, role, index as u64 + 1, seed);
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
				let op = random_op(&mut network, &peers[index].target);
				peers[index].stage(op);
			}
			// A small pool of distinct payloads, so peers sometimes introduce the same resource
			// concurrently and sometimes one nobody else can serve.
			8 if peers[index].replica.is_synced() => {
				let bytes = format!("resource-{}", network.random_below(5)).into_bytes();
				peers[index].stage_resource(bytes);
			}
			0..=2 | 8 => {}
			// Mostly a partial retire, so the host's history lags its hot log the way a real one does.
			3 => {
				let nth = network.random_below(4);
				peers[0].retire_prefix(nth);
			}
			4..=7 => {
				network.step();
			}
			// The host cannot hand over, so only guests drop and come back.
			9 if index > 0 => peers[index].rejoin(&mut network),
			9 => {
				let endpoint = network.endpoint();
				let id = endpoint.id();
				peers.push(Peer::new(endpoint, Role::Guest, peers.len() as u64 + 1, seed));
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

/// Dump every peer's state for one seed: `SIM_DUMP=<seed> cargo test ... -- --nocapture`.
fn dump_if_requested(seed: u64, peers: &[Peer]) {
	if std::env::var("SIM_DUMP").ok().and_then(|value| value.parse::<u64>().ok()) != Some(seed) {
		return;
	}
	for (index, peer) in peers.iter().enumerate() {
		eprintln!(
			"DUMP {index} {:?} departed {} synced {} held {} pending_resources {} history {} hot {:?} retired {:?}",
			peer.peer,
			peer.departed,
			peer.replica.is_synced(),
			peer.replica.held_broadcasts(),
			peer.replica.pending_resource_requests().count(),
			peer.session().history().count(),
			peer.session()
				.hot_log()
				.iter()
				.map(|h| format!("{}:{}#{}", h.timestamp.peer.0, h.timestamp.counter, h.sequence.0))
				.collect::<Vec<_>>(),
			{
				let retired = peer.session().retired_marks();
				let mut through: Vec<_> = retired.retired_up_to.iter().map(|(peer, sequence)| (peer.0, sequence.0)).collect();
				through.sort();
				let mut above: Vec<_> = retired.retired_beyond.iter().map(|(peer, runs)| (peer.0, runs.len())).collect();
				above.sort();
				format!("{through:?} above {above:?}")
			}
		);
	}
}

/// The retired snapshot has to be exactly what canonical history produces, holding no trace of which
/// order deltas arrived in or which hot ops this peer happens to hold. Convergence alone does not say
/// this: every peer can agree on a snapshot that none of their histories accounts for.
fn assert_snapshot_matches_history(seed: u64, peers: &[Peer]) {
	for (index, peer) in peers.iter().enumerate().filter(|(_, peer)| !peer.departed) {
		let replayed = peer.session().snapshot_from_history().expect("replaying history onto a fresh registry");

		assert_eq!(peer.session().retired_registry(), &replayed, "seed {seed}: peer {index} snapshot does not match its history");
	}
}

/// Whether every `AddNode` in history names a network that history itself creates. Retirement promotes
/// whatever the host managed to apply, so a referent that only ever existed as an unretired hot op
/// could in principle leave a durable hole here.
fn assert_history_self_contained(seed: u64, peers: &[Peer]) {
	for (index, peer) in peers.iter().enumerate().filter(|(_, peer)| !peer.departed) {
		let created: HashSet<NetworkId> = peer
			.session()
			.history()
			.filter_map(|delta| match delta.kind {
				RegistryDelta::AddNetwork { id, .. } => Some(id),
				_ => None,
			})
			.chain(peer.session().history().filter_map(|delta| match delta.reverse {
				RegistryDelta::AddNetwork { id, .. } => Some(id),
				_ => None,
			}))
			.collect();
		for delta in peer.session().history() {
			if let RegistryDelta::AddNode { node, .. } = &delta.kind {
				assert!(
					created.contains(&node.network()),
					"seed {seed}: peer {index} history has AddNode into {:?} that nothing in history creates",
					node.network()
				);
			}
		}
	}
}

/// A hot op is identified by its timestamp, so holding one twice means some path appended a copy of
/// something already there, which then replays and re-broadcasts as if it were new work.
fn assert_no_duplicate_hot_ops(seed: u64, peers: &[Peer]) {
	for (index, peer) in peers.iter().enumerate() {
		let mut seen = HashSet::new();
		for hot_op in peer.session().hot_log() {
			assert!(seen.insert(hot_op.timestamp), "seed {seed}: peer {index} holds {:?} twice", hot_op.timestamp);
		}
	}
}

fn assert_converged(seed: u64, peers: &[Peer]) {
	dump_if_requested(seed, peers);
	assert_no_duplicate_hot_ops(seed, peers);
	assert_snapshot_matches_history(seed, peers);
	assert_history_self_contained(seed, peers);

	let present = || peers.iter().enumerate().filter(|(_, peer)| !peer.departed);

	// A sparse `known_revs` sample only describes a peer's state if history is parent-complete, so a
	// hole makes the host under-send on the next resync.
	for (index, peer) in present() {
		let present_revs: HashSet<Rev> = peer.session().history().map(|delta| delta.id).collect();
		for delta in peer.session().history() {
			for parent in delta.all_parents() {
				assert!(present_revs.contains(&parent), "seed {seed}: peer {index} has {:?} without its parent {parent:?}", delta.id);
			}
		}
	}

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
		assert_eq!(peer.replica.deferred_ops(), 0, "seed {seed}: peer {index} still holds ops waiting on a referent");
		assert_eq!(peer.target.durable_history, peer.target.history_revs(), "seed {seed}: peer {index} has unflushed history");

		let unserved: Vec<_> = SyncTarget::missing_resources(&peer.target).into_iter().filter(|hash| servable.contains(hash)).collect();
		assert!(unserved.is_empty(), "seed {seed}: peer {index} is missing resource bytes another peer holds: {unserved:?}");
	}
}

#[test]
fn peers_converge_under_random_interleavings() {
	// The committed corpus is the first 1000 seeds; `SIM_SEEDS` pushes the frontier further by hand.
	let seeds = std::env::var("SIM_SEEDS").ok().and_then(|value| value.parse::<u64>().ok()).unwrap_or(1000);

	for seed in 0..seeds {
		let (_, peers) = simulate(seed, 1 + (seed % 3) as usize, 200);
		{
			let registry = peers[0].session().retired_registry();
			TOTAL_NODES.fetch_add(registry.node_instances.len(), Ordering::Relaxed);
			let wired = registry
				.node_instances
				.values()
				.flat_map(|node| node.inputs())
				.filter(|slot| matches!(slot.input, NodeInput::Node { .. }))
				.count();
			TOTAL_WIRED.fetch_add(wired, Ordering::Relaxed);

			let exports = registry.networks.values().flat_map(|net| &net.exports).filter(|slot| slot.target.is_some()).count();
			TOTAL_EXPORTS.fetch_add(exports, Ordering::Relaxed);

			let input_attributes = registry.node_instances.values().flat_map(|node| node.inputs()).filter(|slot| !slot.attributes.is_empty()).count();
			TOTAL_INPUT_ATTRIBUTES.fetch_add(input_attributes, Ordering::Relaxed);

			// More than one source on a resource can only come from `AddSource`, which the staging path
			// never emits, so this is what proves the resource-chain ops reach the registry.
			let stacked_sources = registry.resources.values().filter(|entry| entry.sources.len() > 1).count();
			TOTAL_SOURCES.fetch_add(stacked_sources, Ordering::Relaxed);
		}
		assert_converged(seed, &peers);
	}
	// The node ops are guarded on what the registry holds, so a generator change can quietly stop
	// producing them. Without nodes wired to other nodes nothing reaches the resurrection path.
	assert!(TOTAL_NODES.load(Ordering::Relaxed) > 0, "the corpus produced no nodes");
	assert!(TOTAL_WIRED.load(Ordering::Relaxed) > 0, "the corpus produced no node-to-node inputs");
	assert!(TOTAL_EXPORTS.load(Ordering::Relaxed) > 0, "the corpus set no network exports");
	assert!(TOTAL_INPUT_ATTRIBUTES.load(Ordering::Relaxed) > 0, "the corpus wrote no input-slot attributes");
	assert!(TOTAL_SOURCES.load(Ordering::Relaxed) > 0, "the corpus never stacked a second source on a resource");
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
			let peer = Peer::new(endpoint, role, index as u64 + 1, 0);
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

/// A hot op can reach one guest and not the host, and its author can then leave for good. The guest
/// holds the only copy, so unless it passes the op on the work is lost with the peer that made it.
/// Random delivery reaches this only by luck, so it is built by hand.
#[test]
fn a_guest_holding_the_only_copy_gets_it_retired() {
	let mut network = MockNetwork::new(0);

	let mut peers: Vec<Peer> = (0..3)
		.map(|index| {
			let endpoint = network.endpoint();
			let id = endpoint.id();
			let role = if index == 0 { Role::Host } else { Role::Guest };
			let peer = Peer::new(endpoint, role, index as u64 + 1, 0);
			network.connect(id);
			peer
		})
		.collect();

	quiesce(&mut network, &mut peers);

	// The author stages one op, and only the other guest is allowed to receive it.
	let author_transport = peers[1].transport;
	peers[1].stage(RegistryDelta::AddNetwork {
		id: NetworkId(9),
		network: Network::default(),
	});
	let witness_transport = peers[2].transport;
	network.deliver_to(witness_transport);
	peers[2].poll();

	assert!(peers[2].session().registry().networks.contains_key(&NetworkId(9)), "the witness must have received the op");
	assert!(peers[0].session().hot_log().is_empty(), "the host must not have received it yet");

	// The author leaves for good, taking its own copy and its undelivered packets with it.
	network.disconnect(author_transport);
	peers[1].departed = true;

	quiesce(&mut network, &mut peers);
	peers[0].retire();
	quiesce(&mut network, &mut peers);

	assert!(
		peers[0].session().retired_registry().networks.contains_key(&NetworkId(9)),
		"the op the witness alone held never reached history"
	);
	assert_converged(0, &peers);
}
