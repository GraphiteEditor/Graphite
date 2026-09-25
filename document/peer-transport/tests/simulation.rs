//! Deterministic simulation over the mock transport: random edits on every peer, random delivery
//! order, random host retirement. After quiescence every peer must hold the same retired state.
//!
//! Inspect one run with `SEED=<n> GUESTS=<n> cargo test -p peer-transport --test simulation inspect_seed -- --ignored --nocapture`.

use document_graph_storage::{
	AttributeDelta, Delta, HeadMove, HotOp, HotOpId, HotSequence, Implementation, Network, NetworkId, Node, NodeId, NodeInput, PeerId, Priority, Registry, RegistryDelta, ResourceHash, ResourceId,
	RetiredHotOps, Rev, Session, SourceKey, TimeStamp, UserId,
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
		let result = SyncTarget::merge_remote(&mut self.session, deltas.clone(), retires, head);
		log::debug!(
			"seed {}: peer {:?} merged {} deltas (head {head:?}, {} retires): history {before} -> {}, head {:?}, {result:?}",
			self.seed,
			self.session.peer(),
			deltas.len(),
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

	fn retired_marks(&self) -> RetiredHotOps {
		SyncTarget::retired_marks(&self.session)
	}

	fn absorb_retired_marks(&mut self, remote: &RetiredHotOps) -> Result<(), TargetError> {
		SyncTarget::absorb_retired_marks(&mut self.session, remote)
	}

	fn retracted_marks(&self) -> RetiredHotOps {
		SyncTarget::retracted_marks(&self.session)
	}

	fn absorb_retracted_marks(&mut self, remote: &RetiredHotOps) -> Result<(), TargetError> {
		SyncTarget::absorb_retracted_marks(&mut self.session, remote)
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
	/// The sequence of the marker closing this peer's latest transaction, `NONE` before the first. Nothing
	/// past it may ever retire.
	last_closed: HotSequence,
	/// Retired interactions of this peer's own it undid, newest last, for a redo to name.
	dropped: Vec<Rev>,
}

impl Peer {
	fn new(endpoint: MockEndpoint, role: Role, index: u64, seed: u64) -> Self {
		let (peer, user) = (PeerId(index), UserId(index));
		let transport = endpoint.id();
		let replica = match role {
			Role::Host => Replica::host(endpoint, peer, user),
			Role::Guest => Replica::guest(endpoint, peer, user),
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
		}
	}

	/// Drop off the room, keeping the document. The editor rebuilds the `Replica` on a reconnect but
	/// reuses the `Gdd`'s persisted `PeerId`, so protocol state resets while document state does not.
	/// A reconnect never assumes a role, host included: whoever hosts by now greets it as a guest.
	fn rejoin(&mut self, network: &mut MockNetwork) {
		network.disconnect(self.transport);

		let endpoint = network.endpoint();
		self.transport = endpoint.id();
		self.replica = Replica::connect(endpoint, self.peer, self.user);
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

	/// Close this peer's open transaction, the way the editor does at an undo-step boundary.
	fn end_transaction(&mut self) {
		let staged = self.target.session.end_transaction().expect("end transaction");
		if let Some(marker) = staged.last() {
			self.last_closed = marker.sequence;
			self.replica.broadcast_hot_ops(&staged).expect("broadcast");
		}
	}

	/// Undo this peer's latest transaction while it is hot, the way the editor does for a step not yet
	/// retired: the ops leave every hot log and never become history.
	fn retract(&mut self) {
		if let Some(retraction) = self.target.session.retract_transaction().expect("retract") {
			self.replica.broadcast_retraction(&retraction.ids).expect("broadcast retraction");
		}
	}

	/// Undo this peer's latest retired step, the way the editor does once a step is in history: the host
	/// drops it out of the line itself, a guest asks the host to.
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
			let deltas: Vec<_> = revs.iter().filter_map(|&rev| self.target.session.delta(rev).cloned()).collect();
			self.replica.broadcast_retired(&deltas, &[]).expect("broadcast restored");
		} else {
			self.replica.request_undo(rev, true).expect("request redo");
		}
	}

	/// Retire up to `count` of the closed transactions in the hot log, the earliest closed first, as the
	/// policy does. Open transactions stay hot, so a later transaction of one author retires while an
	/// earlier one of another is still in progress.
	fn retire_closed(&mut self, count: usize) {
		let closed: Vec<_> = self.target.session.closed_transactions().into_iter().filter(|transaction| transaction.contiguous).take(count).collect();
		let mut revs = Vec::new();
		let mut retired_hot_ops = Vec::new();
		for transaction in &closed {
			revs.extend(self.target.session.retire_transaction(transaction).expect("retire"));
			retired_hot_ops.extend(transaction.ops.iter().copied());
		}
		if retired_hot_ops.is_empty() {
			return;
		}
		let deltas: Vec<_> = revs.iter().filter_map(|&rev| self.target.session.delta(rev).cloned()).collect();
		self.replica.broadcast_retired(&deltas, &retired_hot_ops).expect("broadcast retired");
	}

	fn retire_up_to(&mut self, up_to: TimeStamp) {
		let retired_hot_ops = self.target.session.hot_ops_up_to(up_to);

		let revs = self.target.session.retire(up_to).expect("retire");
		let deltas: Vec<_> = revs.iter().filter_map(|&rev| self.target.session.delta(rev).cloned()).collect();
		self.replica.broadcast_retired(&deltas, &retired_hot_ops).expect("broadcast retired");
	}

	fn poll(&mut self) -> Vec<Event> {
		let flushes_before = self.target.flushes;
		let state = |peer: &Self| {
			let hot: Vec<String> = peer
				.target
				.session
				.hot_log()
				.iter()
				.map(|op| format!("{}:{}#{}", op.timestamp.peer.0, op.timestamp.counter, op.sequence.0))
				.collect();
			(peer.replica.role(), peer.replica.is_synced(), peer.target.session.history().count(), hot)
		};
		let before = state(self);
		let events = self.replica.poll(&mut self.target);
		let after = state(self);
		if before != after {
			log::debug!("peer {:?}: (role, synced, history, hot) {before:?} -> {after:?}", self.peer);
		}
		assert_eq!(self.target.flushes, flushes_before + 1, "poll must flush the target exactly once");
		events
	}
}

/// Node-level ops, which reach the input-slot LWW arms and the resurrection path a concurrent remove
/// triggers. `None` when the registry holds nothing the drawn op could target.
fn random_node_op(network: &mut MockNetwork, session: &Session) -> Option<RegistryDelta> {
	let registry = session.registry();
	let node_id = NodeId(1 + network.random_below(4) as u64);
	let live_nodes: Vec<NodeId> = sorted(registry.node_instances.keys().copied());
	let live_networks: Vec<NetworkId> = sorted(registry.networks.keys().copied());

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
	let live: Vec<ResourceId> = sorted(registry.resources.keys().copied());
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
/// Collect into a sorted `Vec`. Every list the generator indexes with the seeded RNG comes from a hashed
/// registry map, whose order varies per process, so without this the seed would not fix the edit.
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
		// Export slots resize on demand and LWW per slot, and clearing one leaves a tombstone that must
		// still compare equal to the slot being absent.
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

fn present_peers(peers: &[Peer]) -> usize {
	peers.iter().filter(|peer| !peer.departed).count()
}

/// The present peer holding the host role, which moves when a host leaves.
fn present_host(peers: &[Peer]) -> Option<usize> {
	peers.iter().position(|peer| !peer.departed && peer.replica.role() == Role::Host)
}

/// Run until nothing is in flight and no peer reports progress. Resource transfers need several
/// rounds (request out, bytes back), so this is not a fixed number of passes. A room that went idle
/// without a host has its grace period elapse, the way the editor's clock does, and settles again.
fn quiesce(network: &mut MockNetwork, peers: &mut [Peer]) {
	for _ in 0..1000 {
		network.deliver_all();
		let events: usize = peers.iter_mut().filter(|peer| !peer.departed).map(|peer| peer.poll().len()).sum();
		if events == 0 && network.pending() == 0 {
			if present_host(peers).is_none() {
				let decided = peers.iter_mut().filter(|peer| !peer.departed).filter_map(|peer| peer.replica.decide_role()).count();
				if decided > 0 {
					continue;
				}
			}
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
				// A gesture is a few ops long; closing it is what lets it retire.
				if network.random_below(3) == 0 {
					peers[index].end_transaction();
				}
			}
			// An undo of a step still hot takes it back everywhere.
			11 if peers[index].replica.is_synced() => peers[index].retract(),
			// An undo of a step already retired drops it out of the shared line, and a redo puts it back on top.
			12 if peers[index].replica.is_synced() => peers[index].undo_retired(),
			13 if peers[index].replica.is_synced() && !peers[index].dropped.is_empty() => peers[index].redo_retired(),
			// A small pool of distinct payloads, so peers sometimes introduce the same resource
			// concurrently and sometimes one nobody else can serve.
			8 if peers[index].replica.is_synced() => {
				let bytes = format!("resource-{}", network.random_below(5)).into_bytes();
				peers[index].stage_resource(bytes);
			}
			0..=2 | 8 => {}
			// A few closed transactions at a time, so the host's history lags its hot log the way a real one
			// does, and never an open one.
			3 => {
				if let Some(host) = present_host(&peers) {
					let count = 1 + network.random_below(4);
					peers[host].retire_closed(count);
					assert_open_transactions_stay_hot(seed, &peers);
				}
			}
			4..=7 => {
				network.step();
			}
			// Anyone drops and comes back, the host included: the room elects a new one while it is away
			// and greets it back as a guest.
			9 => peers[index].rejoin(&mut network),
			// Keep one peer around, so the room stays a room.
			10 if present_peers(&peers) > 1 => peers[index].depart(&mut network),
			11 => {
				let endpoint = network.endpoint();
				let id = endpoint.id();
				peers.push(Peer::new(endpoint, Role::Guest, peers.len() as u64 + 1, seed));
				network.connect(id);
			}
			// The grace period elapses on a peer still undecided. The period outlasts any hello in flight, so
			// it only elapses here once nothing is.
			12 if network.pending() == 0 => {
				peers[index].replica.decide_role();
			}
			_ => {
				peers[index].poll();
			}
		}
	}

	quiesce(&mut network, &mut peers);
	for peer in peers.iter_mut().filter(|peer| !peer.departed) {
		peer.end_transaction();
	}
	quiesce(&mut network, &mut peers);
	let host = present_host(&peers).unwrap_or_else(|| {
		panic!(
			"seed {seed}: an idle room has no host: {:?}",
			peers.iter().map(|peer| (peer.peer, peer.departed, peer.replica.role(), peer.replica.is_synced())).collect::<Vec<_>>()
		)
	});
	peers[host].retire_closed(usize::MAX);
	assert_open_transactions_stay_hot(seed, &peers);
	quiesce(&mut network, &mut peers);
	// What departed peers left open retires with everything else, so the end state is checked in full.
	let host = present_host(&peers).unwrap_or_else(|| {
		panic!(
			"seed {seed}: an idle room has no host: {:?}",
			peers.iter().map(|peer| (peer.peer, peer.departed, peer.replica.role(), peer.replica.is_synced())).collect::<Vec<_>>()
		)
	});
	peers[host].retire();
	quiesce(&mut network, &mut peers);

	(network, peers)
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

/// The working registry is what the editor renders: a peer's snapshot plus its hot tail. With the tail
/// empty it should hold exactly what the snapshot holds. The two zones apply the same ops in different
/// orders, the working registry in arrival order and the snapshot in canonical history order, and
/// structural ops carry no timestamp to arbitrate that, so whatever drops a hot op the snapshot now
/// covers owes a refold. Compared by value, since a refold re-stamps what it replays.
/// Nothing past an author's last closing marker is in any peer's retired marks: an open transaction stays
/// hot however old it is and however many later ones from other authors retired around it.
fn assert_open_transactions_stay_hot(seed: u64, peers: &[Peer]) {
	for (index, peer) in peers.iter().enumerate().filter(|(_, peer)| !peer.departed) {
		let marks = peer.session().retired_marks();
		for author in peers {
			let through = marks.retired_up_to.get(&author.peer).copied().unwrap_or(HotSequence::NONE);
			let beyond = marks
				.retired_beyond
				.get(&author.peer)
				.and_then(|runs| runs.iter().map(|&(_, end)| end).max())
				.unwrap_or(HotSequence::NONE);
			assert!(
				through <= author.last_closed && beyond <= author.last_closed,
				"seed {seed}: peer {index} retired op {:?} of {:?} past its last closed transaction {:?}",
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

/// The retired snapshot has to be exactly what canonical history produces, holding no trace of which
/// order deltas arrived in or which hot ops this peer happens to hold. Convergence alone does not say
/// this: every peer can agree on a snapshot that none of their histories accounts for.
fn assert_snapshot_matches_history(seed: u64, peers: &[Peer]) {
	for (index, peer) in peers.iter().enumerate().filter(|(_, peer)| !peer.departed) {
		let replayed = peer
			.session()
			.snapshot_from_history()
			.unwrap_or_else(|error| panic!("seed {seed}: peer {index} cannot replay its history: {error:?}"));

		assert_eq!(peer.session().retired_registry(), &replayed, "seed {seed}: peer {index} snapshot does not match its history");
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
	assert_zones_agree(seed, peers);

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

	// One host, whoever it is by now, and nobody left undecided once the room is idle.
	let hosts: Vec<usize> = present().filter(|(_, peer)| peer.replica.role() == Role::Host).map(|(index, _)| index).collect();
	assert_eq!(hosts.len(), 1, "seed {seed}: the room has hosts {hosts:?}");
	for (index, peer) in present() {
		assert_ne!(peer.replica.role(), Role::Undecided, "seed {seed}: peer {index} is still undecided");
		assert!(peer.replica.is_synced(), "seed {seed}: peer {index} never synced");
	}
	let host = &peers[hosts[0]];
	// What a peer can reach from its head is what it holds in common with the room: a branch the cursor
	// walked away from is only ever sent to peers that were there when it was live.
	let host_history = reachable_history(host.session());
	for (index, guest) in present().filter(|(index, _)| *index != hosts[0]) {
		let guest_history = reachable_history(guest.session());
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
			let registry = peers[present_host(&peers).expect("an idle room has a host")].session().retired_registry();
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
		eprintln!("working {}", serde_json::to_string(peer.session().registry()).unwrap());
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

/// A peer that alone received an author's early ops passes them on when the author rejoins, so the run
/// reaches the host contiguously even though the author's own copies were dropped by the resync. This
/// is why a gap usually never forms: measured over 200000 seeds, 30 formed and 27 closed.
#[test]
fn a_witness_closes_an_authors_run_before_a_gap_forms() {
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

	// The author's first op reaches only the witness.
	peers[1].stage(RegistryDelta::AddNetwork {
		id: NetworkId(9),
		network: Network::default(),
	});
	let witness_transport = peers[2].transport;
	network.deliver_to(witness_transport);
	peers[2].poll();
	assert!(peers[2].session().registry().networks.contains_key(&NetworkId(9)), "the witness holds the early op");
	assert!(peers[0].session().hot_log().is_empty(), "the host does not");

	// The author rejoins, which resyncs it from the host and drops its own hot log, then writes again.
	peers[1].rejoin(&mut network);
	quiesce(&mut network, &mut peers);
	peers[1].stage(RegistryDelta::AddNetwork {
		id: NetworkId(10),
		network: Network::default(),
	});
	quiesce(&mut network, &mut peers);

	let author = peers[1].peer;
	let held: Vec<u64> = peers[0].session().hot_log().iter().filter(|op| op.timestamp.peer == author).map(|op| op.sequence.0).collect();
	assert_eq!(held, vec![1, 2, 3], "the witness must have supplied the ops the author could no longer send");

	peers[0].retire();
	quiesce(&mut network, &mut peers);

	let retired = peers[0].session().retired_marks();
	assert!(retired.retired_beyond.is_empty(), "a contiguous run retires wholly into the prefix");
	assert_converged(0, &peers);
}

/// A seed has to fix the run. Hashed collections hand out a different iteration order per instance, so
/// anything that lets that order pick an edit target or reach the wire makes a seed unreproducible, and
/// then every seed named in a bug report means nothing. Running the same seeds twice in one process is
/// enough to catch it, since the two runs' maps are keyed differently.
#[test]
fn a_seed_fixes_the_run() {
	let run = |seeds: u64| {
		(0..seeds)
			.map(|seed| {
				let (_, peers) = simulate(seed, 1 + (seed % 3) as usize, 200);
				peers
			})
			.collect::<Vec<_>>()
	};

	let (first, second) = (run(100), run(100));

	for (seed, (before, after)) in first.iter().zip(&second).enumerate() {
		assert_eq!(before.len(), after.len(), "seed {seed}: peer count differs between runs");

		for (index, (before, after)) in before.iter().zip(after).enumerate() {
			assert_eq!(before.departed, after.departed, "seed {seed}: peer {index} departed in only one run");
			if before.departed {
				continue;
			}

			let revs = |peer: &Peer| peer.session().history().map(|delta| delta.id).collect::<Vec<_>>();
			assert_eq!(revs(before), revs(after), "seed {seed}: peer {index} built a different history");
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

/// Two copies of a document connect to their room without knowing who hosts. Neither greets as host,
/// so after the grace period the lower peer id takes the role and greets again; the other, settled
/// as a guest by that hello, syncs. A third copy that connects once a host is there is a guest at once.
#[test]
fn undecided_peers_settle_on_a_host_and_the_rest_sync() {
	let mut network = MockNetwork::new(0);
	let mut peers: Vec<Peer> = (0..2)
		.map(|index| {
			let endpoint = network.endpoint();
			let id = endpoint.id();
			let peer = Peer::new(endpoint, Role::Undecided, index as u64 + 1, 0);
			network.connect(id);
			peer
		})
		.collect();
	// Hellos alone decide nothing; the grace period elapsing on both (which `quiesce` stands in for once
	// the room is idle without a host) has only the lower id take the role.
	quiesce(&mut network, &mut peers);
	assert_eq!(peers[0].replica.role(), Role::Host);
	assert_eq!(peers[1].replica.role(), Role::Guest);
	assert!(peers[1].replica.is_synced(), "the host's hello prompted a sync");

	peers[0].stage(RegistryDelta::AddNetwork {
		id: NetworkId(9),
		network: Network::default(),
	});
	quiesce(&mut network, &mut peers);
	assert!(peers[1].session().registry().networks.contains_key(&NetworkId(9)));

	let endpoint = network.endpoint();
	let id = endpoint.id();
	peers.push(Peer::new(endpoint, Role::Undecided, 3, 0));
	network.connect(id);
	quiesce(&mut network, &mut peers);
	assert_eq!(peers[2].replica.role(), Role::Guest, "a host is there, so the newcomer is its guest");
	assert!(peers[2].replica.is_synced());
	assert!(peers[2].session().registry().networks.contains_key(&NetworkId(9)));
}

/// The host leaves a room of two synced guests and one that was greeted but never synced. The lowest
/// synced id takes the role over and keeps retiring the line; the unsynced one steps aside and syncs
/// from the new host; the old host comes back as a guest of the new one.
#[test]
fn the_host_leaving_hands_the_role_to_the_lowest_synced_guest() {
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

	peers[0].stage(RegistryDelta::AddNetwork {
		id: NetworkId(1),
		network: Network::default(),
	});
	peers[0].end_transaction();
	quiesce(&mut network, &mut peers);
	peers[0].retire_closed(usize::MAX);
	quiesce(&mut network, &mut peers);
	let first = peers[0].session().history().count();
	assert!(first > 0, "the host retired the step");
	assert!(peers.iter().all(|peer| peer.session().history().count() == first), "the room shares the retired step");

	// A fourth peer hears the host's greeting and asks for a sync, and the host leaves before answering.
	let endpoint = network.endpoint();
	let late = endpoint.id();
	peers.push(Peer::new(endpoint, Role::Undecided, 4, 0));
	network.connect(late);
	for peer in peers.iter_mut() {
		peer.poll();
	}
	network.deliver_to(late);
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

	// The new host retires the line on.
	peers[2].stage(RegistryDelta::AddNetwork {
		id: NetworkId(2),
		network: Network::default(),
	});
	peers[2].end_transaction();
	quiesce(&mut network, &mut peers);
	peers[1].retire_closed(usize::MAX);
	quiesce(&mut network, &mut peers);
	let second = peers[1].session().history().count();
	assert!(second > first, "the new host retired the guest's step");
	assert!(peers.iter().skip(1).all(|peer| peer.session().history().count() == second), "and everyone present has it");

	// The old host returns, and is a guest of the new one with the step it missed.
	peers[0].departed = false;
	peers[0].rejoin(&mut network);
	quiesce(&mut network, &mut peers);
	assert_eq!(peers[0].replica.role(), Role::Guest, "a returning host is a guest of whoever hosts now");
	assert_eq!(peers[1].replica.role(), Role::Host, "the host role stayed put");
	assert_eq!(peers[0].session().history().count(), second, "with the step it missed");
	assert_converged(0, &peers);
}
