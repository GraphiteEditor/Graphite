//! Deterministic simulation over the mock transport: random edits on every peer, random delivery
//! order, random host retirement. After quiescence every peer must hold the same retired state.
//!
//! Inspect one run with `SEED=<n> GUESTS=<n> cargo test -p peer-transport --test simulation inspect_seed -- --ignored --nocapture`.

use document_graph_storage::{
	AttributeDelta, Delta, HeadMove, HotOp, HotOpId, HotSequence, Implementation, Network, NetworkId, Node, NodeId, NodeInput, PeerId, Priority, Registry, RegistryDelta, ResourceHash, ResourceId,
	RetiredMarks, Rev, Session, SourceKey, UserId, Value,
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
static TOTAL_COLLECTED: AtomicUsize = AtomicUsize::new(0);
static TOTAL_REDONE_RETRACTIONS: AtomicUsize = AtomicUsize::new(0);

/// A peer's document plus a byte store, since bare `Session` has no-op resource defaults.
struct SimTarget {
	session: Session,
	resources: HashMap<ResourceHash, Vec<u8>>,
	/// History as of the last `flush`.
	durable_history: Vec<Rev>,
	flushes: usize,
	/// Every rev this peer has ever held.
	seen_revs: HashSet<Rev>,
	/// Hashes this peer collected while it or another present peer lacking the bytes named them: a copy the room
	/// needed. Other drops are fair game, even if a peer that held the bytes too later leaves unserved.
	collected_while_needed: HashSet<ResourceHash>,
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
			collected_while_needed: HashSet::new(),
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

	fn deltas_unknown_to(&mut self, known: &[Rev]) -> Vec<Delta> {
		SyncTarget::deltas_unknown_to(&mut self.session, known)
	}

	fn load(&mut self, registry: Registry, history: Vec<Delta>, head: Option<Rev>) -> Result<(), TargetError> {
		if self.session.history().next().is_some() {
			log::debug!(
				"seed {}: peer {:?} takes a full sync over {} retired deltas of its own (head {:?}), {} incoming",
				self.seed,
				self.session.peer(),
				self.session.history().count(),
				self.session.head_rev(),
				history.len()
			);
		}
		SyncTarget::load(&mut self.session, registry, history, head)
	}

	fn apply_remote_hot_ops(&mut self, ops: Vec<HotOp>) -> Result<Vec<HotOp>, TargetError> {
		SyncTarget::apply_remote_hot_ops(&mut self.session, ops)
	}

	fn merge_remote(&mut self, deltas: Vec<Delta>, retires: &[HotOpId], head: Option<Rev>) -> Result<(), TargetError> {
		let before = self.session.history().count();
		let delta_count = deltas.len();
		let result = SyncTarget::merge_remote(&mut self.session, deltas, retires, head);
		log::debug!(
			"seed {}: peer {:?} merged {} deltas (head {head:?}, {} retires): history {before} -> {}, head {:?}, {result:?}",
			self.seed,
			self.session.peer(),
			delta_count,
			retires.len(),
			self.session.history().count(),
			self.session.head_rev()
		);
		result
	}

	fn merge_divergent(&mut self, deltas: Vec<Delta>, retires: &[HotOpId]) -> Result<Vec<Delta>, TargetError> {
		let before = self.session.history().count();
		let result = SyncTarget::merge_divergent(&mut self.session, deltas, retires);
		log::debug!(
			"seed {}: host {:?} merged a divergent line: history {before} -> {}, head {:?}, minted {:?}",
			self.seed,
			self.session.peer(),
			self.session.history().count(),
			self.session.head_rev(),
			result.as_ref().map(|deltas| deltas.len())
		);
		result
	}

	fn retired_marks(&self) -> RetiredMarks {
		SyncTarget::retired_marks(&self.session)
	}

	fn absorb_retired_marks(&mut self, remote: &RetiredMarks) -> Result<(), TargetError> {
		SyncTarget::absorb_retired_marks(&mut self.session, remote)
	}

	fn retract_hot_ops(&mut self, ops: &[HotOpId]) -> Result<(), TargetError> {
		SyncTarget::retract_hot_ops(&mut self.session, ops)
	}

	fn drop_interaction(&mut self, rev: Rev) -> Result<HeadMove, TargetError> {
		SyncTarget::drop_interaction(&mut self.session, rev)
	}

	fn restore_interaction(&mut self, rev: Rev) -> Result<Vec<Delta>, TargetError> {
		SyncTarget::restore_interaction(&mut self.session, rev)
	}

	fn apply_head_move(&mut self, moved: &HeadMove) -> Result<(), TargetError> {
		SyncTarget::apply_head_move(&mut self.session, moved)
	}

	fn flush(&mut self) -> Result<(), TargetError> {
		// Losing a retired delta is data loss no convergence check would see.
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
	/// Gone for good, unlike a rejoin; no longer asserted on.
	departed: bool,
	/// The marker closing this peer's latest interaction, or its latest retracted op if later. Nothing past it may
	/// retire.
	last_closed: HotSequence,
	/// Retired interactions this peer undid, newest last.
	dropped: Vec<Rev>,
	/// The ops of hot steps this peer took back, newest last.
	retracted: Vec<Vec<RegistryDelta>>,
}

impl Peer {
	fn new(endpoint: MockEndpoint, role: Role, index: u64, seed: u64) -> Self {
		let (peer, user) = (PeerId(index), UserId(index));
		let transport = endpoint.id();
		let replica = match role {
			Role::Host => Replica::host(endpoint, peer, user),
			Role::Guest => Replica::connect(endpoint, peer, user),
			Role::Undecided => Replica::connect(endpoint, peer, user),
		};
		Self {
			target: SimTarget::new(peer, seed),
			replica,
			peer,
			user,
			transport,
			departed: false,
			last_closed: HotSequence::NONE,
			dropped: Vec::new(),
			retracted: Vec::new(),
		}
	}

	/// Reconnect with the same `PeerId` and document but a fresh, undecided `Replica`, as the editor does.
	fn rejoin(&mut self, network: &mut MockNetwork) {
		network.disconnect(self.transport);

		let endpoint = network.endpoint();
		self.transport = endpoint.id();
		self.replica = Replica::connect(endpoint, self.peer, self.user);
		network.connect(self.transport);
	}

	fn depart(&mut self, network: &mut MockNetwork) {
		network.disconnect(self.transport);
		self.departed = true;
	}

	fn session(&self) -> &Session {
		&self.target.session
	}

	fn stage(&mut self, op: RegistryDelta) {
		self.retracted.clear();
		let hot_ops = self.target.session.stage_computed_ops(vec![op]).expect("stage");

		self.replica.broadcast_hot_ops(&hot_ops).expect("broadcast");
	}

	/// Introduce a resource only this peer holds the bytes for, so every other peer has to fetch them.
	fn stage_resource(&mut self, bytes: Vec<u8>) {
		let hash = ResourceHash::from(bytes.as_slice());
		self.retracted.clear();
		self.target.resources.insert(hash, bytes);

		// Per-peer, as the editor gives each resource a fresh id; a content-derived id would collide across peers.
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
		self.replica.broadcast_retired(&self.deltas(&revs), &retired_hot_ops).expect("broadcast retired");
	}

	fn deltas(&self, revs: &[Rev]) -> Vec<Delta> {
		revs.iter().filter_map(|&rev| self.target.session.delta(rev).cloned()).collect()
	}

	/// Close this peer's open interaction, as the editor does at an undo-step boundary.
	fn end_interaction(&mut self) {
		let staged = self.target.session.end_interaction().expect("end interaction");
		if let Some(marker) = staged.last() {
			self.last_closed = marker.sequence;
			self.replica.broadcast_hot_ops(&staged).expect("broadcast");
		}
	}

	/// Undo this peer's latest interaction while hot: the ops leave every hot log and never become history.
	fn retract(&mut self) {
		if let Some(retraction) = self.target.session.retract_interaction().expect("retract") {
			self.last_closed = retraction.ids.iter().map(|id| id.sequence).fold(self.last_closed, HotSequence::max);
			self.replica.broadcast_retraction(&retraction.ids).expect("broadcast retraction");
			if !retraction.ops.is_empty() {
				self.retracted.push(retraction.ops);
			}
		}
	}

	/// Redo the hot step this peer took back last, staging its ops afresh. A redo that no longer fits is given up.
	fn redo_retracted(&mut self) {
		let Some(ops) = self.retracted.pop() else { return };
		let Ok(hot_ops) = self.target.session.stage_computed_ops(ops) else { return };
		self.replica.broadcast_hot_ops(&hot_ops).expect("broadcast");
		self.end_interaction();
		TOTAL_REDONE_RETRACTIONS.fetch_add(1, Ordering::Relaxed);
	}

	/// Drop every held byte outside the GC root set. `needed` is what present peers name and lack.
	fn collect_garbage(&mut self, needed: &HashSet<ResourceHash>) {
		let roots = self.target.session.resource_gc_roots();
		let names = self.names();
		let dropped = self.target.resources.keys().filter(|hash| !roots.contains(hash) && (needed.contains(hash) || names.contains(hash)));
		self.target.collected_while_needed.extend(dropped);
		let before = self.target.resources.len();
		self.target.resources.retain(|hash, _| roots.contains(hash));
		TOTAL_COLLECTED.fetch_add(before - self.target.resources.len(), Ordering::Relaxed);
	}

	/// Every hash this peer's registry, history, hot log or retracted steps name, gathered independently of the GC
	/// root rule it checks.
	fn names(&self) -> HashSet<ResourceHash> {
		let session = self.session();
		let ops = session.history().map(|delta| &delta.kind).chain(session.hot_log().iter().map(|hot_op| &hot_op.op));
		let mut names: HashSet<ResourceHash> = ops.chain(self.retracted.iter().flatten()).filter_map(RegistryDelta::resource_hash).collect();
		names.extend(session.registry().resources.values().filter_map(|entry| entry.hash));
		names
	}

	/// Undo this peer's latest retired step: the host drops it out of the line itself, a guest asks the host to.
	fn undo_retired(&mut self) {
		let Some(rev) = self.target.session.latest_own_interaction() else { return };
		if self.replica.role() == Role::Host {
			match self.target.session.drop_interaction(rev) {
				Ok((moved, _)) => self.replica.broadcast_head_move(moved).expect("broadcast head move"),
				Err(document_graph_storage::CrdtError::NotUndoable(_)) => return,
				Err(error) => panic!("drop: {error:?}"),
			}
		} else {
			self.replica.request_undo(rev, false).expect("request undo");
		}
		self.dropped.push(rev);
	}

	/// Redo the step this peer undid last, as a copy on top of the line.
	fn redo_retired(&mut self) {
		let Some(rev) = self.dropped.pop() else { return };
		if self.replica.role() == Role::Host {
			let revs = match self.target.session.restore_interaction(rev) {
				Ok(revs) => revs,
				Err(document_graph_storage::CrdtError::NotUndoable(_)) => return,
				Err(error) => panic!("restore: {error:?}"),
			};
			self.replica.broadcast_retired(&self.deltas(&revs), &[]).expect("broadcast restored");
		} else {
			self.replica.request_undo(rev, true).expect("request redo");
		}
	}

	/// Retire up to `count` closed interactions, earliest closed first. Open ones stay hot, so one author's later
	/// interaction can retire while another's earlier one is in progress.
	fn retire_closed(&mut self, count: usize) {
		let closed: Vec<_> = self.target.session.closed_interactions().into_iter().filter(|interaction| interaction.contiguous).take(count).collect();
		let mut revs = Vec::new();
		let mut retired_hot_ops = Vec::new();
		for interaction in &closed {
			revs.extend(self.target.session.retire_interaction(interaction).expect("retire"));
			retired_hot_ops.extend(interaction.ops.iter().copied());
		}
		if retired_hot_ops.is_empty() {
			return;
		}
		self.replica.broadcast_retired(&self.deltas(&revs), &retired_hot_ops).expect("broadcast retired");
	}

	fn poll(&mut self) -> Vec<Event> {
		let flushes_before = self.target.flushes;
		let state = |peer: &Self| (peer.replica.role(), peer.replica.is_synced(), peer.session().history().count(), describe_hot_log(peer.session()));
		let before = log::log_enabled!(log::Level::Debug).then(|| state(self));
		let events = self.replica.poll(&mut self.target);
		if let Some(before) = before {
			let after = state(self);
			if before != after {
				log::debug!("peer {:?}: (role, synced, history, hot) {before:?} -> {after:?}", self.peer);
			}
		}
		assert_eq!(self.target.flushes, flushes_before + 1, "poll must flush the target exactly once");
		events
	}
}

fn describe_hot_log(session: &Session) -> Vec<String> {
	session
		.hot_log()
		.iter()
		.map(|op| format!("{}:{}#{}", op.timestamp.peer.0, op.timestamp.counter, op.sequence.0))
		.collect()
}

fn random_value(network: &mut MockNetwork) -> Value {
	Value::from(serde_json::json!(network.random_below(100)))
}

fn add_network(id: u64) -> RegistryDelta {
	RegistryDelta::AddNetwork {
		id: NetworkId(id),
		network: Network::default(),
	}
}

/// Node-level ops, reaching the input-slot LWW arms and the revival a concurrent remove triggers. `None` when the
/// registry holds nothing the drawn op could target.
fn random_node_op(network: &mut MockNetwork, session: &Session) -> Option<RegistryDelta> {
	let registry = session.registry();
	let node_id = NodeId(1 + network.random_below(4) as u64);
	let live_nodes: Vec<NodeId> = sorted(registry.node_instances.keys().copied());
	let live_networks: Vec<NetworkId> = sorted(registry.networks.keys().copied());

	match network.random_below(8) {
		6 => {
			let node = registry.node_instances.get(&node_id)?;
			let index = network.random_below(node.inputs().len().max(1)) as u32;

			Some(RegistryDelta::ChangeNodeInputAttribute {
				id: node_id,
				index,
				delta: AttributeDelta {
					key: "label".into(),
					value: Some(random_value(network)),
				},
			})
		}
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

			// Wiring to a node live here can land on a peer that concurrently removed it, driving a revival.
			let wire_to = pick(network, &live_nodes).copied();
			let new_input = match wire_to {
				Some(target) if network.random_below(3) > 0 => NodeInput::Node { id: target, index: 0 },
				_ => NodeInput::Value {
					value: random_value(network),
					exposed: false,
				},
			};
			Some(RegistryDelta::ChangeNodeInput { id: node_id, index, new_input })
		}
		_ => pick(network, &live_nodes).copied().map(|node_id| RegistryDelta::ChangeNodeAttribute {
			id: node_id,
			delta: AttributeDelta {
				key: "name".into(),
				value: Some(random_value(network)),
			},
		}),
	}
}

/// Resource ops past `AddResource`: the resolved hash, the source list, and removal. `None` when the registry holds
/// no resource to target.
fn random_resource_op(network: &mut MockNetwork, target: &SimTarget) -> Option<RegistryDelta> {
	let session = &target.session;
	let registry = session.registry();
	let live: Vec<ResourceId> = sorted(registry.resources.keys().copied());
	let id = *pick(network, &live)?;

	// A small pool of priorities, so concurrent adds sometimes collide on a key and sometimes stack up.
	let key = SourceKey {
		priority: Priority::new(network.random_below(3) as f64).expect("a whole number is finite"),
		peer: session.peer(),
	};

	Some(match network.random_below(5) {
		// Only a hash whose bytes this peer holds, as in the editor the resolving peer is the one that fetched it.
		0 => {
			let held: Vec<ResourceHash> = sorted(target.resources.keys().copied());
			RegistryDelta::SetResourceHash {
				id,
				hash: Some(*pick(network, &held)?),
			}
		}
		1 => RegistryDelta::SetResourceHash { id, hash: None },
		2 => RegistryDelta::AddSource {
			id,
			key,
			source: Value::from(serde_json::json!("Embedded")),
		},
		3 => RegistryDelta::RemoveSource { id, key },
		_ => RegistryDelta::RemoveResource {
			id,
			snapshot: registry.resources.get(&id)?.clone(),
		},
	})
}

/// Collect into a sorted `Vec`, so the seed rather than hash order picks the edit.
fn sorted<T: Ord>(values: impl Iterator<Item = T>) -> Vec<T> {
	let mut values: Vec<T> = values.collect();
	values.sort_unstable();
	values
}

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
		// A cleared export slot's tombstone must compare equal to absence.
		4 | 5 if session.registry().networks.contains_key(&network_id) => {
			let live_nodes: Vec<NodeId> = sorted(session.registry().node_instances.keys().copied());
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
				value: Some(random_value(network)),
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
				value: Some(random_value(network)),
			},
		},
		_ => RegistryDelta::AddNetwork {
			id: network_id,
			network: Network::default(),
		},
	}
}

fn present_peers(peers: &[Peer]) -> usize {
	peers.iter().filter(|peer| !peer.departed).count()
}

fn present_host(peers: &[Peer]) -> Option<usize> {
	peers.iter().position(|peer| !peer.departed && peer.replica.role() == Role::Host)
}

fn expect_host(seed: u64, peers: &[Peer]) -> usize {
	present_host(peers).unwrap_or_else(|| {
		let states: Vec<_> = peers.iter().map(|peer| (peer.peer, peer.departed, peer.replica.role(), peer.replica.is_synced())).collect();
		panic!("seed {seed}: an idle room has no host: {states:?}")
	})
}

fn add_peer(network: &mut MockNetwork, peers: &mut Vec<Peer>, role: Role, seed: u64) {
	let endpoint = network.endpoint();
	let id = endpoint.id();
	peers.push(Peer::new(endpoint, role, peers.len() as u64 + 1, seed));
	network.connect(id);
}

/// A host and `guest_count` guests, all connected.
fn open_room(network: &mut MockNetwork, guest_count: usize, seed: u64) -> Vec<Peer> {
	let mut peers = Vec::new();
	add_peer(network, &mut peers, Role::Host, seed);
	for _ in 0..guest_count {
		add_peer(network, &mut peers, Role::Guest, seed);
	}
	peers
}

/// Run until nothing is in flight and no peer reports progress. A room idle without a host has its grace period
/// elapse and settles again. Returns how many resources arrived.
fn quiesce(network: &mut MockNetwork, peers: &mut [Peer]) -> usize {
	let mut received = 0;
	for _ in 0..1000 {
		network.deliver_all();
		let events: Vec<Event> = peers.iter_mut().filter(|peer| !peer.departed).flat_map(|peer| peer.poll()).collect();
		received += events.iter().filter(|event| matches!(event, Event::ResourceReceived { .. })).count();
		if events.is_empty() && network.pending() == 0 {
			if present_host(peers).is_none() {
				let decided = peers.iter_mut().filter(|peer| !peer.departed).filter_map(|peer| peer.replica.decide_role()).count();
				if decided > 0 {
					continue;
				}
			}
			return received;
		}
	}
	panic!("the room never went idle");
}

fn simulate(seed: u64, guest_count: usize, steps: usize) -> (MockNetwork, Vec<Peer>) {
	let mut network = MockNetwork::new(seed);
	let mut peers = open_room(&mut network, guest_count, seed);

	for _ in 0..steps {
		let index = network.random_below(peers.len());
		if peers[index].departed {
			continue;
		}

		match network.random_below(18) {
			// Anything staged before syncing would be invisible to the room.
			0..=2 if peers[index].replica.is_synced() => {
				let op = random_op(&mut network, &peers[index].target);
				peers[index].stage(op);
				if network.random_below(3) == 0 {
					peers[index].end_interaction();
				}
			}
			11 if peers[index].replica.is_synced() => peers[index].retract(),
			12 if peers[index].replica.is_synced() => peers[index].undo_retired(),
			13 if peers[index].replica.is_synced() && !peers[index].dropped.is_empty() => peers[index].redo_retired(),
			// About eleven a run, often enough to land between a resource's ops, its retirement and its fetch.
			16 => {
				let needed = needed_anywhere(&peers);
				peers[index].collect_garbage(&needed);
			}
			17 if peers[index].replica.is_synced() => peers[index].redo_retracted(),
			// Bytes nothing names, as an abandoned import leaves behind.
			8 if network.random_below(4) == 0 => {
				let bytes = format!("stray-{}-{}", peers[index].peer.0, network.random_below(1 << 20)).into_bytes();
				peers[index].target.resources.insert(ResourceHash::from(bytes.as_slice()), bytes);
			}
			// A small payload pool, so peers sometimes introduce the same resource concurrently.
			8 if peers[index].replica.is_synced() => {
				let bytes = format!("resource-{}", network.random_below(5)).into_bytes();
				peers[index].stage_resource(bytes);
			}
			0..=2 | 8 => {}
			// A few closed interactions at a time, so the host's history lags its hot log.
			3 => {
				if let Some(host) = present_host(&peers) {
					let count = 1 + network.random_below(4);
					peers[host].retire_closed(count);
					assert_open_interactions_stay_hot(seed, &peers);
				}
			}
			4..=7 => {
				network.step();
			}
			// The host included: the room elects another meanwhile and greets it back as a guest.
			9 => peers[index].rejoin(&mut network),
			// Keep one peer around.
			10 if present_peers(&peers) > 1 => peers[index].depart(&mut network),
			11 => add_peer(&mut network, &mut peers, Role::Guest, seed),
			// The grace period outlasts any hello, so it elapses only once nothing is in flight.
			12 if network.pending() == 0 => {
				peers[index].replica.decide_role();
			}
			_ => {
				peers[index].poll();
			}
		}
	}

	quiesce(&mut network, &mut peers);
	collect_garbage_everywhere(&mut peers);
	for peer in peers.iter_mut().filter(|peer| !peer.departed) {
		peer.end_interaction();
	}
	quiesce(&mut network, &mut peers);
	collect_garbage_everywhere(&mut peers);
	let host = expect_host(seed, &peers);
	peers[host].retire_closed(usize::MAX);
	assert_open_interactions_stay_hot(seed, &peers);
	quiesce(&mut network, &mut peers);
	// What departed peers left open retires too, so the end state is checked in full.
	let host = expect_host(seed, &peers);
	peers[host].retire();
	quiesce(&mut network, &mut peers);

	// At rest every held hash is named or garbage, so collecting must leave nothing to fetch again.
	collect_garbage_everywhere(&mut peers);
	let refetched = quiesce(&mut network, &mut peers);
	assert_eq!(refetched, 0, "seed {seed}: garbage collection at rest dropped bytes the room then fetched again");

	(network, peers)
}

/// Every hash a present peer names and lacks the bytes for. See [`Peer::names`].
fn needed_anywhere(peers: &[Peer]) -> HashSet<ResourceHash> {
	let present = peers.iter().filter(|peer| !peer.departed);
	present.flat_map(|peer| peer.names().into_iter().filter(|hash| !peer.target.resources.contains_key(hash))).collect()
}

fn collect_garbage_everywhere(peers: &mut [Peer]) {
	let needed = needed_anywhere(peers);
	for peer in peers.iter_mut().filter(|peer| !peer.departed) {
		peer.collect_garbage(&needed);
	}
}

/// Dump every peer's state for one seed: `SIM_DUMP=<seed> cargo test ... -- --nocapture`.
fn dump_if_requested(seed: u64, peers: &[Peer]) {
	if std::env::var("SIM_DUMP").ok().and_then(|value| value.parse::<u64>().ok()) != Some(seed) {
		return;
	}
	for (index, peer) in peers.iter().enumerate() {
		for line in peer.replica.describe_held() {
			eprintln!("HELD {index} {line}");
		}
		eprintln!(
			"DUMP {index} {:?} departed {} synced {} held {} pending_resources {} history {} hot {:?} retired {:?}",
			peer.peer,
			peer.departed,
			peer.replica.is_synced(),
			peer.replica.held_broadcasts(),
			peer.replica.pending_resource_requests().count(),
			peer.session().history().count(),
			describe_hot_log(peer.session()),
			{
				let retired = peer.session().retired_marks();
				let mut through: Vec<_> = retired.retired_up_to.iter().map(|(peer, sequence)| (peer.0, sequence.0)).collect();
				through.sort();
				let mut above: Vec<_> = retired.retired_runs.iter().map(|(peer, runs)| (peer.0, runs.len())).collect();
				above.sort();
				format!("{through:?} above {above:?}")
			}
		);
	}
}

/// Nothing past an author's last closing marker or retraction is in any peer's retired marks: an open interaction
/// stays hot however many later ones retired around it.
fn assert_open_interactions_stay_hot(seed: u64, peers: &[Peer]) {
	for (index, peer) in peers.iter().enumerate().filter(|(_, peer)| !peer.departed) {
		let marks = peer.session().retired_marks();
		for author in peers {
			let through = marks.retired_up_to.get(&author.peer).copied().unwrap_or(HotSequence::NONE);
			let beyond = marks
				.retired_runs
				.get(&author.peer)
				.and_then(|runs| runs.iter().map(|&(_, end)| end).max())
				.unwrap_or(HotSequence::NONE);
			assert!(
				through <= author.last_closed && beyond <= author.last_closed,
				"seed {seed}: peer {index} retired op {:?} of {:?} past its last closed or retracted interaction {:?}",
				through.max(beyond),
				author.peer,
				author.last_closed
			);
		}
	}
}

/// The revs on the head's ancestry, in file order.
fn reachable_history(session: &Session) -> Vec<Rev> {
	let mut reachable = HashSet::new();
	let mut stack: Vec<Rev> = session.head_rev().into_iter().collect();
	while let Some(rev) = stack.pop() {
		if reachable.insert(rev)
			&& let Some(delta) = session.delta(rev)
		{
			stack.extend(delta.all_parents());
		}
	}
	session.history().map(|delta| delta.id).filter(|rev| reachable.contains(rev)).collect()
}

/// With the hot log empty, the working registry holds exactly what the snapshot holds. Compared by value, since a
/// refold re-stamps what it replays.
fn assert_zones_agree(seed: u64, peers: &[Peer]) {
	for (index, peer) in peers.iter().enumerate().filter(|(_, peer)| !peer.departed) {
		if !peer.session().hot_log().is_empty() {
			continue;
		}

		assert!(
			peer.session().registry().value_equal(peer.session().retired_registry()),
			"seed {seed}: peer {index} working registry drifted from its own snapshot\nworking minus snapshot: {:#?}",
			document_graph_storage::delta::compute_deltas(peer.session().retired_registry(), peer.session().registry())
		);
		assert!(
			peer.session().registry().removal_marks_equal(peer.session().retired_registry()),
			"seed {seed}: peer {index} working registry removed different things than its snapshot"
		);
	}
}

/// The retired snapshot is exactly what history produces, which convergence alone does not show.
fn assert_snapshot_matches_history(seed: u64, peers: &[Peer]) {
	for (index, peer) in peers.iter().enumerate().filter(|(_, peer)| !peer.departed) {
		let replayed = peer
			.session()
			.snapshot_from_history()
			.unwrap_or_else(|error| panic!("seed {seed}: peer {index} cannot replay its history: {error:?}"));

		assert_eq!(peer.session().retired_registry(), &replayed, "seed {seed}: peer {index} snapshot does not match its history");
	}
}

/// A hot op is identified by its timestamp, so holding one twice means a copy would replay as new work.
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
	assert_zones_agree(seed, peers);

	let present = || peers.iter().enumerate().filter(|(_, peer)| !peer.departed);

	// A sparse `known_revs` sample assumes parent-complete history; a hole makes the host under-send on resync.
	for (index, peer) in present() {
		let present_revs: HashSet<Rev> = peer.target.history_revs().into_iter().collect();
		for delta in peer.session().history() {
			for parent in delta.all_parents() {
				assert!(present_revs.contains(&parent), "seed {seed}: peer {index} has {:?} without its parent {parent:?}", delta.id);
			}
		}
	}

	let hosts: Vec<usize> = present().filter(|(_, peer)| peer.replica.role() == Role::Host).map(|(index, _)| index).collect();
	assert_eq!(hosts.len(), 1, "seed {seed}: the room has hosts {hosts:?}");
	for (index, peer) in present() {
		assert_ne!(peer.replica.role(), Role::Undecided, "seed {seed}: peer {index} is still undecided");
		assert!(peer.replica.is_synced(), "seed {seed}: peer {index} never synced");
	}
	let host = &peers[hosts[0]];
	// Compared by what the head reaches, since an abandoned branch only reached peers present while it was live.
	let host_history = reachable_history(host.session());
	for (index, guest) in present().filter(|(index, _)| *index != hosts[0]) {
		let guest_history = reachable_history(guest.session());
		assert_eq!(guest_history, host_history, "seed {seed}: guest {index} history diverged");
		assert_eq!(guest.session().head_rev(), host.session().head_rev(), "seed {seed}: guest {index} head diverged");
		assert_eq!(guest.session().retired_registry(), host.session().retired_registry(), "seed {seed}: guest {index} registry diverged");
		assert!(guest.session().hot_log().is_empty(), "seed {seed}: guest {index} still holds hot ops");
	}

	// Bytes that left with a departed peer are gone, so only those a present peer holds, or collected while needed,
	// must be served.
	let servable: HashSet<ResourceHash> = present()
		.flat_map(|(_, peer)| peer.target.resources.keys().chain(&peer.target.collected_while_needed).copied())
		.collect();

	for (index, peer) in present() {
		assert_eq!(peer.replica.held_broadcasts(), 0, "seed {seed}: peer {index} still holds undelivered broadcasts");
		assert_eq!(peer.replica.deferred_ops(), 0, "seed {seed}: peer {index} still holds ops waiting on a referent");
		assert_eq!(peer.target.durable_history, peer.target.history_revs(), "seed {seed}: peer {index} has unflushed history");

		let missing = SyncTarget::missing_resources(&peer.target);
		let unserved: Vec<_> = missing.iter().filter(|hash| servable.contains(hash)).collect();
		assert!(
			unserved.is_empty(),
			"seed {seed}: peer {index} is missing resource bytes a present peer holds or collected while another needed them: {unserved:?}"
		);

		// A request outliving its need would suppress the ask once the hash is named again.
		let stale: Vec<_> = peer.replica.pending_resource_requests().filter(|hash| !missing.contains(hash)).collect();
		assert!(stale.is_empty(), "seed {seed}: peer {index} still requests resources it no longer misses: {stale:?}");
	}
}

#[test]
fn peers_converge_under_random_interleavings() {
	let seeds = std::env::var("SIM_SEEDS").ok().and_then(|value| value.parse::<u64>().ok()).unwrap_or(5000);

	// Runs every seed and lists the failures, to tell one bug from several.
	let keep_going = std::env::var_os("SIM_KEEP_GOING").is_some();
	let mut failed = Vec::new();
	for seed in 0..seeds {
		let run = || {
			let (_, peers) = simulate(seed, 1 + (seed % 3) as usize, 200);
			count_coverage(peers[expect_host(seed, &peers)].session().retired_registry());
			assert_converged(seed, &peers);
		};
		if !keep_going {
			run();
		} else if std::panic::catch_unwind(run).is_err() {
			failed.push(seed);
		}
	}
	assert!(failed.is_empty(), "{} seeds failed: {failed:?}", failed.len());
	// The generator's ops are guarded on registry state, so a change can quietly stop producing them.
	assert!(TOTAL_NODES.load(Ordering::Relaxed) > 0, "the corpus produced no nodes");
	assert!(TOTAL_WIRED.load(Ordering::Relaxed) > 0, "the corpus produced no node-to-node inputs");
	assert!(TOTAL_EXPORTS.load(Ordering::Relaxed) > 0, "the corpus set no network exports");
	assert!(TOTAL_INPUT_ATTRIBUTES.load(Ordering::Relaxed) > 0, "the corpus wrote no input-slot attributes");
	assert!(TOTAL_SOURCES.load(Ordering::Relaxed) > 0, "the corpus never stacked a second source on a resource");
	assert!(TOTAL_COLLECTED.load(Ordering::Relaxed) > 0, "garbage collection never dropped a resource");
	assert!(TOTAL_REDONE_RETRACTIONS.load(Ordering::Relaxed) > 0, "the corpus never redid a step taken back");
}

fn count_coverage(registry: &Registry) {
	let inputs = || registry.node_instances.values().flat_map(|node| node.inputs());
	TOTAL_NODES.fetch_add(registry.node_instances.len(), Ordering::Relaxed);
	TOTAL_WIRED.fetch_add(inputs().filter(|slot| matches!(slot.input, NodeInput::Node { .. })).count(), Ordering::Relaxed);
	TOTAL_INPUT_ATTRIBUTES.fetch_add(inputs().filter(|slot| !slot.attributes.is_empty()).count(), Ordering::Relaxed);
	let exports = registry.networks.values().flat_map(|network| &network.exports).filter(|slot| slot.target.is_some()).count();
	TOTAL_EXPORTS.fetch_add(exports, Ordering::Relaxed);
	// A second source can only come from `AddSource`.
	TOTAL_SOURCES.fetch_add(registry.resources.values().filter(|entry| entry.sources.len() > 1).count(), Ordering::Relaxed);
}

/// A rejoining guest keeps its `PeerId` but restarts its broadcast sequence, which the room must follow
/// rather than wait on the old connection's.
#[test]
fn a_rejoining_guest_is_still_heard() {
	let mut network = MockNetwork::new(0);
	let mut peers = open_room(&mut network, 1, 0);
	quiesce(&mut network, &mut peers);

	peers[1].stage(add_network(1));
	quiesce(&mut network, &mut peers);
	assert!(peers[0].session().registry().networks.contains_key(&NetworkId(1)), "the host missed the first edit");

	peers[1].rejoin(&mut network);
	quiesce(&mut network, &mut peers);

	peers[1].stage(add_network(2));
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
		eprintln!("working {}", serde_json::to_string(peer.session().registry()).unwrap());
		eprintln!("hot log {:?}", peer.session().hot_log());
		eprintln!("resources {:?}", peer.target.resources.keys().collect::<Vec<_>>());
		for delta in peer.session().history() {
			eprintln!("{:?} parent {:?} author {:?} ts {} {:?}", delta.id, delta.parent, delta.author, delta.timestamp.counter, delta.kind);
		}
	}
	assert_converged(seed, &peers);
}

/// A hot op reaches one guest but not the host, and its author leaves for good; the guest must pass on the
/// only copy. Built by hand, since random delivery reaches this only by luck.
#[test]
fn a_guest_holding_the_only_copy_gets_it_retired() {
	let mut network = MockNetwork::new(0);
	let mut peers = open_room(&mut network, 2, 0);
	quiesce(&mut network, &mut peers);

	peers[1].stage(add_network(9));
	network.deliver_to(peers[2].transport);
	peers[2].poll();

	assert!(peers[2].session().registry().networks.contains_key(&NetworkId(9)), "the witness must have received the op");
	assert!(peers[0].session().hot_log().is_empty(), "the host must not have received it yet");

	peers[1].depart(&mut network);

	quiesce(&mut network, &mut peers);
	peers[0].retire();
	quiesce(&mut network, &mut peers);

	assert!(
		peers[0].session().retired_registry().networks.contains_key(&NetworkId(9)),
		"the op the witness alone held never reached history"
	);
	assert_converged(0, &peers);
}

/// A peer that alone received an author's early ops passes them on when the author rejoins and its resync drops
/// its own copies, so the run reaches the host contiguously.
#[test]
fn a_witness_closes_an_authors_run_before_a_gap_forms() {
	let mut network = MockNetwork::new(0);
	let mut peers = open_room(&mut network, 2, 0);
	quiesce(&mut network, &mut peers);

	peers[1].stage(add_network(9));
	network.deliver_to(peers[2].transport);
	peers[2].poll();
	assert!(peers[2].session().registry().networks.contains_key(&NetworkId(9)), "the witness holds the early op");
	assert!(peers[0].session().hot_log().is_empty(), "the host does not");

	peers[1].rejoin(&mut network);
	quiesce(&mut network, &mut peers);
	peers[1].stage(add_network(10));
	quiesce(&mut network, &mut peers);

	let author = peers[1].peer;
	let held: Vec<u64> = peers[0].session().hot_log().iter().filter(|op| op.timestamp.peer == author).map(|op| op.sequence.0).collect();
	assert_eq!(held, vec![1, 2, 3], "the witness must have supplied the ops the author could no longer send");

	peers[0].retire();
	quiesce(&mut network, &mut peers);

	let retired = peers[0].session().retired_marks();
	assert!(retired.retired_runs.is_empty(), "a contiguous run retires wholly into the prefix");
	assert_converged(0, &peers);
}

/// Hash order differs per map instance, so running the same seeds twice in one process catches any that leaks into
/// an edit or the wire.
#[test]
fn a_seed_fixes_the_run() {
	let run = |seeds: u64| (0..seeds).map(|seed| simulate(seed, 1 + (seed % 3) as usize, 200).1).collect::<Vec<_>>();

	let (first, second) = (run(100), run(100));

	for (seed, (before, after)) in first.iter().zip(&second).enumerate() {
		assert_eq!(before.len(), after.len(), "seed {seed}: peer count differs between runs");

		for (index, (before, after)) in before.iter().zip(after).enumerate() {
			assert_eq!(before.departed, after.departed, "seed {seed}: peer {index} departed in only one run");
			if before.departed {
				continue;
			}

			assert_eq!(before.target.history_revs(), after.target.history_revs(), "seed {seed}: peer {index} built a different history");
			assert!(
				before.session().retired_registry().value_equal(after.session().retired_registry()),
				"seed {seed}: peer {index} reached a different snapshot"
			);
			assert!(
				before.session().registry().value_equal(after.session().registry()),
				"seed {seed}: peer {index} reached a different working registry"
			);
		}
	}
}

/// Two undecided copies of a document connect: after the grace period the lower id hosts and the other syncs
/// as its guest. A third copy connecting once a host is there is a guest at once.
#[test]
fn undecided_peers_settle_on_a_host_and_the_rest_sync() {
	let mut network = MockNetwork::new(0);
	let mut peers = Vec::new();
	add_peer(&mut network, &mut peers, Role::Undecided, 0);
	add_peer(&mut network, &mut peers, Role::Undecided, 0);
	// `quiesce` elapses the grace period once the room is idle without a host.
	quiesce(&mut network, &mut peers);
	assert_eq!(peers[0].replica.role(), Role::Host);
	assert_eq!(peers[1].replica.role(), Role::Guest);
	assert!(peers[1].replica.is_synced(), "the host's hello prompted a sync");

	peers[0].stage(add_network(9));
	quiesce(&mut network, &mut peers);
	assert!(peers[1].session().registry().networks.contains_key(&NetworkId(9)));

	add_peer(&mut network, &mut peers, Role::Undecided, 0);
	quiesce(&mut network, &mut peers);
	assert_eq!(peers[2].replica.role(), Role::Guest, "a host is there, so the newcomer is its guest");
	assert!(peers[2].replica.is_synced());
	assert!(peers[2].session().registry().networks.contains_key(&NetworkId(9)));
}

/// The host leaves two synced guests and one greeted but unsynced. The lowest synced id takes over and keeps
/// retiring; the unsynced one syncs from it; the old host returns as its guest.
#[test]
fn the_host_leaving_hands_the_role_to_the_lowest_synced_guest() {
	let mut network = MockNetwork::new(0);
	let mut peers = open_room(&mut network, 2, 0);
	quiesce(&mut network, &mut peers);

	peers[0].stage(add_network(1));
	peers[0].end_interaction();
	quiesce(&mut network, &mut peers);
	peers[0].retire_closed(usize::MAX);
	quiesce(&mut network, &mut peers);
	let first = peers[0].session().history().count();
	assert!(first > 0, "the host retired the step");
	assert!(peers.iter().all(|peer| peer.session().history().count() == first), "the room shares the retired step");

	// A fourth peer asks the host for a sync, and the host leaves before answering.
	add_peer(&mut network, &mut peers, Role::Undecided, 0);
	for peer in peers.iter_mut() {
		peer.poll();
	}
	network.deliver_to(peers[3].transport);
	peers[3].poll();
	assert_eq!(peers[3].replica.role(), Role::Guest, "greeted by the host");
	assert!(!peers[3].replica.is_synced(), "not synced yet");
	peers[0].depart(&mut network);
	quiesce(&mut network, &mut peers);

	assert_eq!(peers[1].replica.role(), Role::Host, "the lowest synced guest takes over");
	assert_eq!(peers[2].replica.role(), Role::Guest);
	assert_eq!(peers[3].replica.role(), Role::Guest, "the unsynced guest stepped aside and was greeted by the new host");
	assert!(peers[3].replica.is_synced(), "and synced from it");
	assert_eq!(peers[3].session().history().count(), first);

	peers[2].stage(add_network(2));
	peers[2].end_interaction();
	quiesce(&mut network, &mut peers);
	peers[1].retire_closed(usize::MAX);
	quiesce(&mut network, &mut peers);
	let second = peers[1].session().history().count();
	assert!(second > first, "the new host retired the guest's step");
	assert!(peers.iter().skip(1).all(|peer| peer.session().history().count() == second), "and everyone present has it");

	peers[0].departed = false;
	peers[0].rejoin(&mut network);
	quiesce(&mut network, &mut peers);
	assert_eq!(peers[0].replica.role(), Role::Guest, "a returning host is a guest of whoever hosts now");
	assert_eq!(peers[1].replica.role(), Role::Host, "the host role stayed put");
	assert_eq!(peers[0].session().history().count(), second, "with the step it missed");
	assert_converged(0, &peers);
}
