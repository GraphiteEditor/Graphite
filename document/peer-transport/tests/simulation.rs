//! Deterministic simulation over the mock transport: random edits on every peer, random delivery
//! order, random host retirement. After quiescence every peer must hold the same retired state.
//!
//! Inspect one run with `SEED=<n> GUESTS=<n> cargo test -p peer-transport --test simulation inspect_seed -- --ignored --nocapture`.

use document_graph_storage::{AttributeDelta, Network, NetworkId, PeerId, RegistryDelta, Session, UserId};
use peer_transport::mock::{MockEndpoint, MockNetwork};
use peer_transport::{Event, Replica, Role};

struct Peer {
	session: Session,
	replica: Replica,
}

impl Peer {
	fn new(endpoint: MockEndpoint, role: Role, index: u64) -> Self {
		let (peer, user) = (PeerId(index), UserId(index));
		let replica = match role {
			Role::Host => Replica::host(endpoint, peer, user),
			Role::Guest => Replica::guest(endpoint, peer, user),
		};
		Self {
			session: Session::with_peer(peer),
			replica,
		}
	}

	fn stage(&mut self, op: RegistryDelta) {
		let hot_ops = self.session.stage_ops([op]).expect("stage");
		self.replica.broadcast_hot_ops(&hot_ops).expect("broadcast");
	}

	fn retire(&mut self) {
		let Some(up_to) = self.session.hot_log().iter().map(|hot_op| hot_op.timestamp).max() else {
			return;
		};
		let retired_hot_ops = self.session.hot_ops_up_to(up_to);
		let revs = self.session.retire(up_to).expect("retire");
		let deltas: Vec<_> = revs.iter().filter_map(|&rev| self.session.delta(rev).cloned()).collect();
		self.replica.broadcast_retired(&deltas, &retired_hot_ops).expect("broadcast retired");
	}

	fn poll(&mut self) -> Vec<Event> {
		self.replica.poll(&mut self.session)
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

fn quiesce(network: &mut MockNetwork, peers: &mut [Peer]) {
	loop {
		network.deliver_all();
		let events: usize = peers.iter_mut().map(|peer| peer.poll().len()).sum();
		if events == 0 && network.pending() == 0 {
			return;
		}
	}
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
		match network.random_below(6) {
			// A guest edits only once synced; anything staged earlier would be invisible to its peers.
			0 | 1 if peers[index].replica.is_synced() => {
				let op = random_op(&mut network, &peers[index].session);
				peers[index].stage(op);
			}
			0 | 1 => {}
			2 => peers[0].retire(),
			3 | 4 => {
				network.step();
			}
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
	let host = &peers[0];
	let host_history: Vec<_> = host.session.history().map(|delta| delta.id).collect();
	for (index, guest) in peers.iter().enumerate().skip(1) {
		let guest_history: Vec<_> = guest.session.history().map(|delta| delta.id).collect();
		assert_eq!(guest_history, host_history, "seed {seed}: guest {index} history diverged");
		assert_eq!(guest.session.head_rev(), host.session.head_rev(), "seed {seed}: guest {index} head diverged");
		assert_eq!(guest.session.retired_registry(), host.session.retired_registry(), "seed {seed}: guest {index} registry diverged");
		assert!(guest.session.hot_log().is_empty(), "seed {seed}: guest {index} still holds hot ops");
	}
}

#[test]
fn peers_converge_under_random_interleavings() {
	for seed in 0..1000 {
		let (_, peers) = simulate(seed, 1 + (seed % 3) as usize, 200);
		assert_converged(seed, &peers);
	}
}

#[test]
#[ignore = "manual inspection of one seed"]
fn inspect_seed() {
	let _ = env_logger::builder().is_test(true).try_init();
	let seed = std::env::var("SEED").ok().and_then(|value| value.parse().ok()).unwrap_or(0);
	let guests = std::env::var("GUESTS").ok().and_then(|value| value.parse().ok()).unwrap_or(1);

	let (_, peers) = simulate(seed, guests, 200);
	for (index, peer) in peers.iter().enumerate() {
		eprintln!("=== peer {index} head {:?}", peer.session.head_rev());
		eprintln!("registry {}", serde_json::to_string(peer.session.retired_registry()).unwrap());
		eprintln!("hot log {:?}", peer.session.hot_log());
		for delta in peer.session.history() {
			eprintln!("{:?} parent {:?} author {:?} ts {} {:?}", delta.id, delta.parent, delta.author, delta.timestamp.counter, delta.kind);
		}
	}
	assert_converged(seed, &peers);
}
