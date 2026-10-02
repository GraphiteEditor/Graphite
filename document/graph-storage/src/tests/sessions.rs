//! Peers in a session: syncing and merging lines, settled marks, undo against what other peers hold, reopening
//! with ops discarded, and the session-only operations on the shared head.

use core_types::uuid::NodeId as RuntimeNodeId;
use graph_craft::ProtoNodeIdentifier;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeInput as RuntimeInput, NodeNetwork};

use crate::{
	AttributeDelta, Delta, History, HotOp, HotOpId, Implementation, InputSlot, MergeOutcome, Network, NetworkId, NoMetadata, Node, NodeId, NodeInput, PeerId, Registry, RegistryDelta, ResourceEntry,
	ResourceHash, ResourceId, Rev, Session, TimeStamp, UserId, Value,
};
use std::collections::HashSet;

use super::bookkeeping::{add_network, set_attribute};
use super::crdt::{change_node_attribute, commit_retired, set_document_attribute};

/// Commits `op` as one interaction, the working registry following as it does when nothing is hot.
fn step(session: &mut Session, op: RegistryDelta) -> Rev {
	session.commit_op_for_test(op).expect("commit");
	session.document.working_registry = session.document.retired_snapshot.clone();
	let rev = session.head_rev().expect("committed");
	session.mark_interaction_end(rev);
	rev
}

fn sync(from: &mut Session, to: &mut Session) -> MergeOutcome {
	let deltas = from.deltas_unknown_to(to.known_revs());
	to.merge(deltas).expect("merge")
}

fn sync_pair(peers: &mut [Session], from: usize, to: usize) -> MergeOutcome {
	let known = peers[to].known_revs();
	let deltas = peers[from].deltas_unknown_to(known);
	peers[to].merge(deltas).expect("merge")
}

fn document_attribute(session: &Session, key: &str) -> Option<Value> {
	session.retired_registry().attributes.get(key).filter(|value| !value.deleted).map(|value| value.value.clone())
}

/// The working registry as the invariant says it must be: the retired snapshot with the hot log folded on top.
fn rebuilt_working(session: &Session) -> Registry {
	let mut copy = session.clone();
	copy.document.rebuild_working();
	copy.document.working_registry
}

/// A merge bounds an interaction, so this user's own step after one undoes on its own.
#[test]
fn an_own_step_after_a_merge_further_back_can_be_undone() {
	let mut a = Session::with_peer(PeerId(1));
	step(&mut a, set_attribute("base", 0));
	step(&mut a, set_attribute("a0", 0));
	let mut b = Session::with_peer(PeerId(2));
	sync(&mut a, &mut b);

	step(&mut a, set_attribute("a", 1));
	step(&mut b, set_attribute("b", 1));
	assert!(matches!(sync(&mut b, &mut a), MergeOutcome::Merged(_)));

	step(&mut a, set_attribute("mine", 1));
	assert!(a.can_undo(), "the head interaction is this user's own single step; the merge is further back");
	a.undo().expect("undo");
	assert_eq!(document_attribute(&a, "mine"), None);
	assert!(document_attribute(&a, "b").is_some(), "undo stopped at the merge");
	assert_eq!(a.retired_registry(), &a.snapshot_from_history().unwrap());
}

/// Another user's delta bounds an interaction too, marked as an end or not, so the walk stops there instead of running
/// through every unmarked delta to the last marker.
#[test]
fn an_own_step_after_a_fast_forward_onto_unmarked_remote_deltas_can_be_undone() {
	let mut a = Session::with_peer(PeerId(1));
	step(&mut a, set_attribute("base", 0));
	step(&mut a, set_attribute("a0", 0));
	let mut b = Session::with_peer(PeerId(2));
	sync(&mut a, &mut b);

	for i in 0..3 {
		let staged = b.stage_ops([set_attribute("remote", i)]).unwrap();
		b.retire(staged.last().unwrap().timestamp).unwrap();
	}
	assert!(matches!(sync(&mut b, &mut a), MergeOutcome::FastForward(_)));

	step(&mut a, set_attribute("mine", 1));
	assert!(a.can_undo(), "the head interaction is this user's own single step");
}

fn runtime_network(values: [u32; 2]) -> NodeNetwork {
	NodeNetwork {
		exports: vec![RuntimeInput::node(RuntimeNodeId(0), 0)],
		nodes: [(
			RuntimeNodeId(0),
			DocumentNode {
				inputs: values.iter().map(|&value| RuntimeInput::value(TaggedValue::Integer(value as i64), false)).collect(),
				implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::identity::IdentityNode")),
				..Default::default()
			},
		)]
		.into_iter()
		.collect(),
		..Default::default()
	}
}

/// A whole-document stage diffs against what the runtime held, so after a peer shrank a node's input list it writes a
/// slot past the new end. It lands as a peer's write would, rather than failing and failing again on every later stage.
#[test]
fn a_local_stage_after_a_peers_input_list_change_still_lands() {
	let resources = graphene_resource::ResourceRegistry::new();
	let mut a = Session::with_peer(PeerId(1));
	a.stage_from_runtime(&runtime_network([1, 2]), &NoMetadata, &resources).expect("first stage");
	let up_to = a.hot_log().last().unwrap().timestamp;
	a.retire(up_to).unwrap();

	let mut b = Session::with_peer(PeerId(2));
	sync(&mut a, &mut b);
	let (&id, node) = b.registry().node_instances.iter().find(|(_, node)| node.inputs().len() == 2).expect("the converted node");
	let first = node.inputs()[0].clone();
	let shrink = b.stage_ops([RegistryDelta::SetNodeInputs { id, inputs: vec![first] }]).unwrap();
	for hot_op in shrink {
		a.replay_hot_op(hot_op).unwrap();
	}

	// The runtime on A still shows two inputs and edits the second one.
	let edit = a.stage_from_runtime(&runtime_network([1, 5]), &NoMetadata, &resources);
	// Even if that concurrent edit were refused, an edit of the first input alone must still land.
	let later = a.stage_from_runtime(&runtime_network([9, 5]), &NoMetadata, &resources);
	assert!(edit.is_ok() || later.is_ok(), "a later stage still lands: first {:?}, later {:?}", edit.err(), later.err());
}

struct Lcg(u64);
impl Lcg {
	fn below(&mut self, bound: u64) -> u64 {
		self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
		(self.0 >> 33) % bound
	}
}

/// A local op valid under `Live` against `registry`: it only names entities this peer has seen and indices in range.
fn local_op(rng: &mut Lcg, registry: &Registry) -> Option<RegistryDelta> {
	let pick = |rng: &mut Lcg, ids: Vec<u64>| (!ids.is_empty()).then(|| ids[rng.below(ids.len() as u64) as usize]);
	let mut live_networks: Vec<u64> = registry.networks.keys().map(|id| id.0).collect();
	live_networks.sort_unstable();
	let mut seen_networks: Vec<u64> = registry
		.networks
		.keys()
		.chain(registry.removed_networks.iter().filter(|(_, m)| !m.placeholder).map(|(id, _)| id))
		.map(|id| id.0)
		.collect();
	seen_networks.sort_unstable();
	let mut live_nodes: Vec<u64> = registry.node_instances.keys().map(|id| id.0).collect();
	live_nodes.sort_unstable();
	let mut seen_nodes: Vec<u64> = registry
		.node_instances
		.keys()
		.chain(registry.removed_nodes.iter().filter(|(_, m)| !m.placeholder).map(|(id, _)| id))
		.map(|id| id.0)
		.collect();
	seen_nodes.sort_unstable();
	let content = |id: u64| registry.node_or_removed(NodeId(id)).expect("seen");
	let value = |rng: &mut Lcg| NodeInput::Value {
		value: Value::from(serde_json::json!(rng.below(50))),
		exposed: false,
	};
	let input = |rng: &mut Lcg| match pick(rng, live_nodes.clone()) {
		Some(id) if rng.below(2) == 0 => NodeInput::Node { id: NodeId(id), index: 0 },
		_ => value(rng),
	};
	let attribute = |rng: &mut Lcg, key: &str| AttributeDelta {
		key: key.into(),
		value: (rng.below(4) > 0).then(|| Value::from(serde_json::json!(rng.below(50)))),
	};
	let source_key = |rng: &mut Lcg| crate::SourceKey {
		priority: crate::Priority::new(rng.below(2) as f64).expect("finite"),
		peer: PeerId(1 + rng.below(2)),
	};
	Some(match rng.below(17) {
		0 | 1 => RegistryDelta::ChangeDocumentAttribute {
			delta: {
				let key = ["x", "y"][rng.below(2) as usize];
				attribute(rng, key)
			},
		},
		2 => {
			let id = 1 + rng.below(3);
			if registry.networks.contains_key(&NetworkId(id)) {
				return None;
			}
			RegistryDelta::AddNetwork {
				id: NetworkId(id),
				network: Network::default(),
			}
		}
		3 => {
			let id = pick(rng, live_networks.clone())?;
			RegistryDelta::RemoveNetwork {
				id: NetworkId(id),
				snapshot: registry.networks[&NetworkId(id)].clone(),
			}
		}
		4 => {
			let id = 1 + rng.below(4);
			if registry.node_instances.contains_key(&NodeId(id)) {
				return None;
			}
			let network = pick(rng, seen_networks.clone())?;
			RegistryDelta::AddNode {
				id: NodeId(id),
				node: Node::new(NetworkId(network), Implementation::Network(NetworkId(network)), 1 + rng.below(2) as usize),
			}
		}
		5 => {
			let id = pick(rng, live_nodes.clone())?;
			RegistryDelta::RemoveNode {
				id: NodeId(id),
				snapshot: registry.node_instances[&NodeId(id)].clone(),
			}
		}
		6 => RegistryDelta::ChangeNodeAttribute {
			id: NodeId(pick(rng, seen_nodes.clone())?),
			delta: {
				let key = ["name", "lock"][rng.below(2) as usize];
				attribute(rng, key)
			},
		},
		7 => {
			let id = pick(rng, seen_nodes.clone())?;
			let len = content(id).inputs().len() as u64;
			if len == 0 {
				return None;
			}
			RegistryDelta::ChangeNodeInput {
				id: NodeId(id),
				index: rng.below(len) as u32,
				new_input: input(rng),
			}
		}
		8 => {
			let id = pick(rng, seen_nodes.clone())?;
			let inputs = (0..1 + rng.below(3))
				.map(|_| InputSlot {
					input: input(rng),
					timestamp: TimeStamp::ORIGIN,
					attributes: Default::default(),
					attributes_timestamp: TimeStamp::ORIGIN,
				})
				.collect();
			RegistryDelta::SetNodeInputs { id: NodeId(id), inputs }
		}
		9 => RegistryDelta::SetNetworkExport {
			id: NetworkId(pick(rng, seen_networks.clone())?),
			index: rng.below(2) as u32,
			export: (rng.below(3) > 0).then(|| input(rng)),
		},
		10 => RegistryDelta::ChangeNetworkAttribute {
			id: NetworkId(pick(rng, seen_networks.clone())?),
			delta: attribute(rng, "name"),
		},
		11 => RegistryDelta::SetNodeImplementation {
			id: NodeId(pick(rng, seen_nodes.clone())?),
			implementation: Implementation::Network(NetworkId(pick(rng, seen_networks.clone())?)),
		},
		12 => {
			let mut entry = crate::ResourceEntry {
				hash: Some(crate::ResourceHash::from([rng.below(2) as u8; 32])),
				..Default::default()
			};
			entry.force_set_source(
				source_key(rng),
				crate::SourceValue {
					source: Value::from(serde_json::json!(rng.below(9))),
					timestamp: TimeStamp::ORIGIN,
					deleted: false,
				},
			);
			RegistryDelta::AddResource {
				id: crate::ResourceId::from(1 + rng.below(2)),
				entry,
			}
		}
		13 => {
			let id = crate::ResourceId::from(1 + rng.below(2));
			let snapshot = registry.resources.get(&id)?.clone();
			RegistryDelta::RemoveResource { id, snapshot }
		}
		14 => RegistryDelta::SetResourceHash {
			id: crate::ResourceId::from(1 + rng.below(2)),
			hash: Some(crate::ResourceHash::from([rng.below(2) as u8; 32])),
		},
		15 => RegistryDelta::AddSource {
			id: crate::ResourceId::from(1 + rng.below(2)),
			key: source_key(rng),
			source: Value::from(serde_json::json!(rng.below(9))),
		},
		_ => RegistryDelta::RemoveSource {
			id: crate::ResourceId::from(1 + rng.below(2)),
			key: source_key(rng),
		},
	})
}

fn check_invariants(session: &Session, context: &str) {
	assert_eq!(
		session.retired_registry(),
		&session.snapshot_from_history().unwrap(),
		"{context}: retired snapshot is not the fold of the head's line"
	);
	assert_eq!(session.registry(), &rebuilt_working(session), "{context}: working registry is not snapshot + hot log");
}

/// A session as a reopen rebuilds it (what `Gdd::open_in` does): the retired snapshot, history, head, redo stack and
/// node counter, then the hot sequence, clock and settled marks from `session.json`, then the hot log replayed.
fn reopened(s: &Session) -> Session {
	let mut r = Session::load(
		s.peer(),
		s.user(),
		s.retired_registry().clone(),
		s.cloned_deltas(),
		s.head_rev(),
		s.redo_stack().to_vec(),
		s.next_node_counter(),
	);
	r.restore_hot_sequence(s.last_hot_sequence());
	r.absorb_settled_marks(s.settled_marks());
	for hot_op in s.hot_log() {
		r.replay_hot_op(hot_op.clone()).unwrap();
	}
	r.restore_clock_counter(s.clock_counter());
	r
}

/// Undo `session` only if every delta it would take off the line is unknown to all `others` (the silent zone).
fn undo_is_silent(session: &Session, others: &[&Session]) -> bool {
	let mut probe = session.clone();
	let before = probe.document.history.ancestors(probe.head_rev());
	probe.undo().unwrap();
	let after = probe.document.history.ancestors(probe.head_rev());
	before.difference(&after).all(|rev| others.iter().all(|other| !other.document.history.contains(*rev)))
}

/// Three peers of different users stage random valid batches, broadcast hot ops (some delayed), retire up to a
/// random cutoff (the others discarding what was retired), undo (silent zone only), redo, sync pairwise and reopen,
/// in random order. After every action the retired snapshot must be the fold of the head's line and the working
/// registry snapshot + hot log, and a reopen must restore both exactly; at the end, after flushing and syncing to
/// quiescence, all retired registries must be identical (stamps included).
#[test]
fn peers_keep_the_invariants_and_converge_under_random_undo_redo_sync_and_reopen() {
	const PEERS: usize = 3;
	for seed in 0..300u64 {
		let mut rng = Lcg(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5);
		let mut peers: Vec<Session> = (1..=PEERS as u64).map(|id| Session::with_peer(PeerId(id))).collect();
		// Shared base so every peer has a boundary to undo back to.
		step(&mut peers[0], set_attribute("base", 0));
		for i in 1..PEERS {
			sync_pair(&mut peers, 0, i);
		}
		let mut in_flight: Vec<Vec<HotOp>> = vec![Vec::new(); PEERS]; // hot ops on their way to peer i
		let mut log = Vec::new();

		for action in 0..50 {
			let me = rng.below(PEERS as u64) as usize;
			let kind = rng.below(12);
			log.push(format!("{me}:{kind}"));
			let context = format!("seed {seed}, action {action}, trace {log:?}");
			match kind {
				0..=2 => {
					let ops: Vec<RegistryDelta> = (0..1 + rng.below(3)).filter_map(|_| local_op(&mut rng, peers[me].registry())).collect();
					if let Ok(staged) = peers[me].stage_ops(ops) {
						for hot_op in staged {
							for other in (0..PEERS).filter(|&other| other != me) {
								if rng.below(4) == 0 {
									in_flight[other].push(hot_op.clone());
								} else {
									peers[other].replay_hot_op(hot_op.clone()).unwrap();
								}
							}
						}
					}
				}
				3 | 4 => {
					let stamps: Vec<TimeStamp> = peers[me].hot_log().iter().map(|hot_op| hot_op.timestamp).collect();
					if stamps.is_empty() {
						continue;
					}
					let up_to = stamps[rng.below(stamps.len() as u64) as usize];
					let ids: Vec<HotOpId> = peers[me].hot_ops_up_to(up_to);
					let revs = peers[me].retire(up_to).unwrap();
					if let Some(&last) = revs.last()
						&& rng.below(4) > 0
					{
						peers[me].mark_interaction_end(last);
					}
					for other in (0..PEERS).filter(|&other| other != me) {
						peers[other].discard_hot_ops(&ids);
					}
				}
				5 => {
					if peers[me].can_undo() {
						let others: Vec<&Session> = peers.iter().enumerate().filter(|(i, _)| *i != me).map(|(_, s)| s).collect();
						if undo_is_silent(&peers[me], &others) {
							peers[me].undo().unwrap();
						}
					}
				}
				6 => {
					if peers[me].can_redo() {
						peers[me].redo().unwrap();
					}
				}
				7 | 8 => {
					let from = (me + 1 + rng.below(PEERS as u64 - 1) as usize) % PEERS;
					sync_pair(&mut peers, from, me);
				}
				9 => {
					let fresh = reopened(&peers[me]);
					assert_eq!(fresh.retired_registry(), peers[me].retired_registry(), "reopen snapshot, {context}");
					assert_eq!(fresh.registry(), peers[me].registry(), "reopen working, {context}");
					assert_eq!(fresh.settled_marks(), peers[me].settled_marks(), "reopen marks, {context}");
					peers[me] = fresh;
				}
				_ => {
					for hot_op in std::mem::take(&mut in_flight[me]) {
						peers[me].replay_hot_op(hot_op).unwrap();
					}
				}
			}
			for (i, peer) in peers.iter().enumerate() {
				check_invariants(peer, &format!("peer {i}, {context}"));
			}
		}

		// Flush: deliver everything, each peer retires what it holds (the others discarding), then sync to quiescence.
		for i in 0..PEERS {
			for hot_op in std::mem::take(&mut in_flight[i]) {
				peers[i].replay_hot_op(hot_op).unwrap();
			}
		}
		for me in 0..PEERS {
			if let Some(up_to) = peers[me].hot_log().iter().map(|hot_op| hot_op.timestamp).max() {
				let ids = peers[me].hot_ops_up_to(up_to);
				peers[me].retire(up_to).unwrap();
				for other in (0..PEERS).filter(|&other| other != me) {
					peers[other].discard_hot_ops(&ids);
				}
			}
		}
		for _ in 0..2 * PEERS {
			for from in 0..PEERS {
				for to in 0..PEERS {
					if from != to {
						sync_pair(&mut peers, from, to);
					}
				}
			}
		}
		let context = format!("seed {seed}, trace {log:?}");
		for i in 1..PEERS {
			assert_eq!(peers[0].head_rev(), peers[i].head_rev(), "{context}: heads of 0 and {i} differ after syncing to quiescence");
			assert_eq!(peers[0].retired_registry(), peers[i].retired_registry(), "{context}: retired registries of 0 and {i} diverged");
			assert!(peers[i].hot_log().is_empty(), "{context}: peer {i} still holds hot ops");
		}
	}
}

/// A step another peer holds is published: undoing it silently would leave the peers apart for good, since each holds
/// every delta the other offers.
#[test]
fn a_step_handed_to_a_peer_is_published() {
	let mut a = Session::with_peer(PeerId(1));
	step(&mut a, set_attribute("base", 0));
	let mut b = Session::with_peer(PeerId(2));
	sync(&mut a, &mut b);
	step(&mut b, set_attribute("b", 1));
	step(&mut b, set_attribute("b", 2));
	assert!(matches!(sync(&mut b, &mut a), MergeOutcome::FastForward(_)));

	assert!(!b.can_undo(), "A holds the step, so it is published");
	sync(&mut a, &mut b);
	sync(&mut b, &mut a);
	assert_eq!(a.retired_registry(), b.retired_registry());
}

/// A peer's op that was seen and then discarded before its delta arrived is in neither history nor the hot log, so a
/// reopen takes the clock from `session.json`: a later write here must still be stamped above it.
#[test]
fn a_write_after_a_reopen_beats_a_discarded_op_it_saw() {
	let mut a = Session::with_peer(PeerId(1));
	step(&mut a, set_attribute("base", 0));
	let mut b = Session::with_peer(PeerId(2));
	sync(&mut a, &mut b);
	for _ in 0..10 {
		b.stage_ops([set_attribute("unrelated", 0)]).unwrap();
	}
	let seen = b.stage_ops([set_attribute("k", 1)]).unwrap();
	for hot_op in b.hot_log().to_vec() {
		a.replay_hot_op(hot_op).unwrap();
	}
	let up_to = seen.last().unwrap().timestamp;
	let ids = b.hot_ops_up_to(up_to);
	b.retire(up_to).unwrap();
	a.discard_hot_ops(&ids);

	// A saw k = 1, reopens, then writes k = 2.
	let mut a = reopened(&a);
	let written = a.stage_ops([set_attribute("k", 2)]).unwrap();
	a.retire(written.last().unwrap().timestamp).unwrap();
	sync(&mut b, &mut a);
	assert_eq!(
		a.retired_registry().attributes.get("k").map(|value| value.value.clone()),
		Some(Value::from(serde_json::json!(2))),
		"A's write came after it saw k = 1, but is stamped below it"
	);
}

/// This peer's own op, retired by another and discarded here before the delta arrives: after a reopen, the next op must
/// not get its stamp, as two ops sharing a stamp tie and peers holding both would show different values.
#[test]
fn a_reopen_after_discarding_an_own_op_never_reuses_its_stamp() {
	let mut a = Session::with_peer(PeerId(1));
	let registered = a.stage_ops([set_attribute("base", 0)]).unwrap();
	a.retire(registered.last().unwrap().timestamp).unwrap();
	let mut b = Session::with_peer(PeerId(2));
	sync(&mut a, &mut b);

	let first = a.stage_ops([set_attribute("k", 1)]).unwrap();
	for hot_op in &first {
		b.replay_hot_op(hot_op.clone()).unwrap();
	}
	let up_to = first.last().unwrap().timestamp;
	let ids = b.hot_ops_up_to(up_to);
	b.retire(up_to).unwrap();
	a.discard_hot_ops(&ids);

	let mut a = reopened(&a);
	let second = a.stage_ops([set_attribute("k", 2)]).unwrap();
	let reused = second.iter().any(|new| first.iter().any(|old| old.timestamp == new.timestamp));
	// Both peers now hold the same two ops: B the first as a delta and the second hot, A the same once B's delta lands.
	for hot_op in &second {
		b.replay_hot_op(hot_op.clone()).unwrap();
	}
	assert!(matches!(sync(&mut b, &mut a), MergeOutcome::FastForward(_)));
	let k = |registry: &Registry| registry.attributes.get("k").map(|value| value.value.clone());
	assert_eq!(k(a.registry()), k(b.registry()), "same ops, different live value; reused stamp: {reused}");
	assert_eq!(a.registry(), &rebuilt_working(&a), "A's working registry is not snapshot + hot log");
	assert!(!reused, "a fresh op reused the stamp of the discarded one");
}

/// The tips as a scan finds them, for checking the maintained set against.
fn scanned_tips(history: &History) -> Vec<Rev> {
	let referenced: HashSet<Rev> = history.iter().flat_map(|delta| delta.all_parents()).collect();
	let mut tips: Vec<Rev> = history.iter().map(|delta| delta.id).filter(|rev| !referenced.contains(rev)).collect();
	tips.sort_unstable();
	tips
}

/// Two peers that branch from a shared base, so their histories hold two tips until merged.
fn branched_pair() -> (Session, Session) {
	let mut a = Session::with_peer(PeerId(1));
	a.commit_op_for_test(set_attribute("base", 0)).expect("base");
	let mut b = a.clone();
	b.document.peer = PeerId(2);
	b.document.clock = crate::LamportClock::new(PeerId(2));
	a.commit_op_for_test(set_attribute("a", 1)).expect("a edit");
	b.commit_op_for_test(set_attribute("b", 2)).expect("b edit");
	(a, b)
}

/// The maintained tips and resource hashes follow pushes and merges, keep a removed resource's hash, and a
/// reload rebuilds them from the ordered deltas.
#[test]
fn history_indexes_follow_pushes_merges_and_reloads() {
	let (mut a, b) = branched_pair();
	assert_eq!(a.document.history.tips(), scanned_tips(&a.document.history));
	a.merge(b.cloned_deltas()).expect("merge");
	assert_eq!(a.document.history.tips(), scanned_tips(&a.document.history));
	assert_eq!(a.document.history.tips().len(), 1, "the merge delta is the one tip");

	let (id, hash) = (ResourceId::new(), ResourceHash::from(b"bytes".as_slice()));
	let entry = ResourceEntry::embedded(hash, PeerId(1), crate::TimeStamp::ORIGIN);
	a.commit_op_for_test(RegistryDelta::AddResource { id, entry }).expect("add");
	assert_eq!(a.document.history.tips(), scanned_tips(&a.document.history));
	let snapshot = a.retired_registry().resources[&id].clone();
	a.commit_op_for_test(RegistryDelta::RemoveResource { id, snapshot }).expect("remove");
	assert!(a.all_referenced_resource_hashes().contains(&hash), "a removed resource's hash is still referenced from history");

	let reloaded = History::from_ordered(a.cloned_deltas());
	assert_eq!(reloaded.tips(), a.document.history.tips());
	assert!(reloaded.resource_hashes().contains(&hash));
	assert!(a.history().all(|delta| reloaded.contains(delta.id)));
}

/// A merge leaves history in canonical order: a chain off the receiver's last delta fast-forwards by
/// appending, sending only what the receiver lacks, and a batch off an earlier delta is sorted into place.
#[test]
fn a_merge_appends_a_chain_and_sorts_a_branch_into_place() {
	let mut host = Session::with_peer(PeerId(1));
	host.commit_op_for_test(set_attribute("base", 0)).expect("base");
	let mut guest = host.clone();
	guest.document.peer = PeerId(2);
	for op in [set_attribute("x", 1), add_network(5), set_attribute("y", 2)] {
		host.commit_op_for_test(op).expect("host step");
	}
	let (a, b) = branched_pair();

	let ids = |history: &History| history.iter().map(|delta| delta.id).collect::<Vec<_>>();
	for (mut receiver, mut sender, missing, fast_forward) in [(guest, host, 3, true), (a, b, 1, false)] {
		let before = receiver.history_len();
		let incoming: Vec<Delta> = sender.deltas_unknown_to(receiver.known_revs());
		assert_eq!(incoming.len(), missing, "only what the receiver lacks is sent");
		let outcome = receiver.merge(incoming).expect("merge");

		let mut sorted = receiver.document.history.clone();
		sorted.canonical_sort();
		assert_eq!(ids(&sorted), ids(&receiver.document.history));
		assert_eq!(receiver.document.history.extends_canonically(before), fast_forward);
		assert_eq!(receiver.document.history.tips(), scanned_tips(&receiver.document.history));
		if fast_forward {
			assert_eq!(outcome, MergeOutcome::FastForward(sender.head_rev().expect("head")));
			assert_eq!(ids(&receiver.document.history), ids(&sender.document.history), "no merge delta");
			assert!(receiver.retired_registry().value_equal(sender.retired_registry()));
			assert_eq!(sender.merge(receiver.cloned_deltas()).expect("merge back"), MergeOutcome::NoOp);
		} else {
			assert!(matches!(outcome, MergeOutcome::Merged(_)), "{outcome:?}");
		}
	}
}

/// A step the cursor walked away from is not sent to a peer, and a merge never joins it: the peer gets the
/// branch the cursor is on, and the undone step's effect stays gone.
#[test]
fn an_undone_branch_is_kept_to_itself() {
	let mut author = Session::with_peer(PeerId(1));
	author.commit_op_for_test(set_attribute("base", 0)).expect("base");
	let base = author.head_rev().expect("base rev");
	author.mark_interaction_end(base);
	let mut peer = author.clone();
	peer.document.peer = PeerId(2);

	author.commit_op_for_test(set_attribute("undone", 1)).expect("the step to undo");
	let undone = author.head_rev().expect("rev");
	author.mark_interaction_end(undone);
	author.undo().expect("undo");
	assert_eq!(author.head_rev(), Some(base));
	author.commit_op_for_test(set_attribute("kept", 2)).expect("a new branch");
	let kept = author.head_rev().expect("rev");

	let sent: Vec<Rev> = author.deltas_unknown_to(peer.known_revs()).into_iter().map(|delta| delta.id).collect();
	assert_eq!(sent, vec![kept], "only the branch the cursor is on goes out, not the undone step");
	assert!(author.delta(undone).is_some(), "the undone step stays in the author's DAG for a history panel");

	let incoming: Vec<Delta> = sent.iter().filter_map(|&rev| author.delta(rev).cloned()).collect();
	let outcome = peer.merge(incoming).expect("merge");
	assert!(matches!(outcome, MergeOutcome::FastForward(rev) if rev == kept), "{outcome:?}");
	assert!(peer.retired_registry().attributes.get("kept").is_some_and(|value| !value.deleted));
	assert!(!peer.retired_registry().attributes.contains_key("undone"), "the undone step never reached the peer");
}

/// Undoing a retired step in a session drops it out of the shared line: later steps are minted again on its
/// parent, every peer following the head move converges, a field a later step wrote keeps that value, and
/// redo brings the step back as a copy on top.
#[test]
fn a_dropped_interaction_leaves_the_line_and_the_room_follows() {
	fn catch_up(guest: &mut Session, host: &Session) {
		let missing: Vec<Delta> = host.cloned_deltas().into_iter().filter(|delta| guest.delta(delta.id).is_none()).collect();
		guest.merge(missing).expect("merge");
	}
	let mut host = Session::with_peer(PeerId(1));
	host.commit_op_for_test(set_attribute("base", 0)).expect("base");
	let base = host.head_rev().expect("rev");
	host.mark_interaction_end(base);

	// The guest's step, retired by the host, writes its own key and one the host writes after it. The guest has
	// its own session and clock, so its ops carry its authorship.
	let mut guest = Session::load(PeerId(2), UserId(2), host.retired_registry().clone(), host.cloned_deltas(), host.head_rev(), Vec::new(), 0);
	let guest_ops = [set_attribute("guest", 1), set_attribute("shared", 1)];
	let hot = guest.stage_ops(guest_ops).expect("stage");
	for hot_op in &hot {
		host.replay_hot_op(hot_op.clone()).expect("the guest's ops reach the host");
	}
	let guest_ids: Vec<crate::HotOpId> = hot.iter().map(crate::HotOp::id).collect();
	let revs = host.retire_hot_ops(&guest_ids).expect("retire");
	let undone = *revs.last().expect("the guest's step");
	host.mark_interaction_end(undone);
	catch_up(&mut guest, &host);
	guest.discard_hot_ops(&guest_ids);

	// A later step by the host writes the shared key.
	host.commit_op_for_test(set_attribute("shared", 2)).expect("later step");
	let later = host.head_rev().expect("rev");
	host.mark_interaction_end(later);
	host.stamp_retired_at(&[later], 1_234);
	catch_up(&mut guest, &host);
	assert_eq!(guest.head_rev(), host.head_rev());
	assert_eq!(guest.latest_own_interaction(), Some(undone));

	let (moved, touched) = host.drop_interaction(undone).expect("drop");
	assert_eq!(moved.from, Some(later));
	assert_eq!(moved.copies.len(), 1, "the later step is minted again");
	assert_eq!(moved.copies[0].parent, Some(base), "the copy hangs off the dropped step's parent");
	assert!(moved.copies[0].is_interaction_end(), "the copy keeps its interaction end");
	assert_eq!(moved.copies[0].retired_at(), Some(1_234), "the copy keeps when the original was retired");
	assert!(touched.nodes.is_empty() && !touched.resources, "document attributes only");
	let snapshot = host.retired_registry();
	assert!(snapshot.attributes.get("guest").is_none_or(|value| value.deleted), "the dropped step's write is gone");
	assert_eq!(
		snapshot.attributes.get("shared").map(|value| &value.value),
		Some(&Value::from(serde_json::json!(2))),
		"the later step's write stands"
	);
	assert!(host.delta(undone).is_some() && host.delta(later).is_some(), "the abandoned branch stays for a history panel");
	assert!(host.registry().value_equal(host.retired_registry()));

	guest.apply_head_move(&moved).expect("follow");
	assert_eq!(guest.head_rev(), host.head_rev());
	assert_eq!(guest.retired_registry(), host.retired_registry());
	assert_eq!(guest.history().map(|delta| delta.id).collect::<Vec<_>>(), host.history().map(|delta| delta.id).collect::<Vec<_>>());
	assert!(
		matches!(guest.apply_head_move(&moved), Err(crate::CrdtError::CursorMismatch { .. })),
		"a move only applies from where it started"
	);

	// Redo: the dropped step returns as a copy on top, its older stamp losing the shared key.
	let restored = host.restore_interaction(undone).expect("restore");
	assert_eq!(restored.len(), 3, "RegisterPeer and the two writes come back");
	assert!(host.retired_registry().attributes.get("guest").is_some_and(|value| value.value == Value::from(serde_json::json!(1))));
	assert_eq!(host.retired_registry().attributes.get("shared").map(|value| &value.value), Some(&Value::from(serde_json::json!(2))));
	catch_up(&mut guest, &host);
	assert_eq!(guest.head_rev(), host.head_rev());
	assert_eq!(guest.retired_registry(), host.retired_registry());
}

/// A merge delta can join a branch this peer walked away from. Following it as a plain extension would leave
/// that branch's effects out, so the snapshot is folded from the joined head's whole ancestry and matches the host's.
#[test]
fn following_a_merge_refolds_the_branch_it_joins() {
	let mut host = Session::with_peer(PeerId(1));
	host.commit_op_for_test(add_network(1)).expect("root");
	let mut guest = Session::with_peer(PeerId(2));
	let root: Vec<crate::Delta> = host.history().cloned().collect();
	guest.follow(root, host.head_rev()).expect("follow the root");

	// The guest retires a step of its own while apart, then follows the host's line, which leaves it behind.
	guest.commit_op_for_test(add_network(2)).expect("own step");
	let own: Vec<crate::Delta> = guest.history().filter(|delta| delta.author == PeerId(2)).cloned().collect();
	host.commit_op_for_test(add_network(3)).expect("host step");
	let line: Vec<crate::Delta> = host.history().cloned().collect();
	guest.follow(line, host.head_rev()).expect("follow the host");
	assert!(!guest.retired_registry().networks.contains_key(&NetworkId(2)), "the guest's own step is off the line");

	// The host merges the guest's step and the guest follows to the joined head.
	host.merge(own).expect("merge");
	let merged: Vec<crate::Delta> = host.history().cloned().collect();
	guest.follow(merged, host.head_rev()).expect("follow the merge");

	let fold = guest.snapshot_from_history().expect("fold");
	assert_eq!(guest.retired_registry(), &fold, "the snapshot is the fold of the joined head's ancestry");
	assert!(guest.retired_registry().networks.contains_key(&NetworkId(2)), "the branch is back on the line");
	assert_eq!(guest.retired_registry(), host.retired_registry());
	assert!(guest.registry().value_equal(guest.retired_registry()));
}

/// Undo of a retired step in a session names the person's latest step, not the device's: a fresh copy of
/// the document under a new peer id still undoes what its user did from the old one, and nobody else's.
#[test]
fn undo_in_a_session_finds_the_users_step_from_another_peer() {
	let mut host = Session::with_identity(PeerId(1), UserId(1));
	host.commit_op_for_test(set_attribute("base", 0)).expect("base");
	let base = host.head_rev().expect("rev");
	host.mark_interaction_end(base);

	let mut old_device = Session::load(PeerId(2), UserId(7), host.retired_registry().clone(), host.cloned_deltas(), host.head_rev(), Vec::new(), 0);
	let hot = old_device.stage_ops([set_attribute("mine", 1)]).expect("stage");
	for hot_op in &hot {
		host.replay_hot_op(hot_op.clone()).expect("the step reaches the host");
	}
	let ids: Vec<crate::HotOpId> = hot.iter().map(crate::HotOp::id).collect();
	let revs = host.retire_hot_ops(&ids).expect("retire");
	let step = *revs.last().expect("the step");
	host.mark_interaction_end(step);

	let new_device = Session::load(PeerId(3), UserId(7), host.retired_registry().clone(), host.cloned_deltas(), host.head_rev(), Vec::new(), 0);
	assert_eq!(new_device.latest_own_interaction(), Some(step), "the same person under a new peer id owns the step");
	assert!(new_device.is_mine(PeerId(2)));

	let stranger = Session::load(PeerId(4), UserId(8), host.retired_registry().clone(), host.cloned_deltas(), host.head_rev(), Vec::new(), 0);
	assert_eq!(stranger.latest_own_interaction(), None, "someone else has nothing of theirs to undo");
	assert!(!stranger.is_mine(PeerId(2)));
}

#[test]
fn moving_the_head_back_leaves_the_line_since_as_a_branch_everyone_follows() {
	let mut host = Session::with_identity(PeerId(1), UserId(1));
	let [first, second, third] = [("a", 1), ("b", 2), ("c", 3)].map(|(key, value)| {
		host.commit_op_for_test(set_attribute(key, value)).expect("step");
		let rev = host.head_rev().expect("rev");
		host.mark_interaction_end(rev);
		rev
	});
	let mut guest = Session::load(PeerId(2), UserId(2), host.retired_registry().clone(), host.cloned_deltas(), host.head_rev(), Vec::new(), 0);

	let (moved, touched) = host.move_head_to(first).expect("move");
	assert_eq!(moved.from, Some(third));
	assert_eq!(moved.head, Some(first));
	assert!(moved.copies.is_empty(), "nothing is minted: the line since is left behind");
	assert!(touched.nodes.is_empty() && !touched.resources, "document attributes only");
	let snapshot = host.retired_registry();
	assert!(snapshot.attributes.get("b").is_none_or(|value| value.deleted), "the second step's write is gone");
	assert!(snapshot.attributes.get("c").is_none_or(|value| value.deleted), "the third step's write is gone");
	assert_eq!(snapshot.attributes.get("a").map(|value| &value.value), Some(&Value::from(serde_json::json!(1))));
	assert!(host.delta(second).is_some() && host.delta(third).is_some(), "the steps left behind stay as a branch");
	assert!(host.registry().value_equal(host.retired_registry()));

	guest.apply_head_move(&moved).expect("follow");
	assert_eq!(guest.head_rev(), Some(first));
	assert!(guest.retired_registry().value_equal(host.retired_registry()), "the follower folds to the same state");
}

#[test]
fn a_late_copy_of_a_followed_line_leaves_the_head_but_a_known_step_ahead_moves_it() {
	let mut host = Session::with_peer(PeerId(1));
	host.commit_op_for_test(set_attribute("a", 1)).unwrap();
	host.commit_op_for_test(set_attribute("b", 1)).unwrap();
	let deltas = host.cloned_deltas();
	let (first, second) = (deltas[0].id, deltas[1].id);

	let mut guest = Session::with_peer(PeerId(2));
	guest.follow(deltas.clone(), None).unwrap();
	assert_eq!(guest.head_rev(), Some(second));

	// The first step again, as a broadcast delayed past the second arrives.
	assert!(matches!(guest.follow(vec![deltas[0].clone()], None).unwrap(), MergeOutcome::NoOp));
	assert_eq!(guest.head_rev(), Some(second));

	// The host moves back and then restores the step, which mints the same delta again.
	guest.follow(Vec::new(), Some(first)).unwrap();
	assert_eq!(guest.head_rev(), Some(first));
	guest.follow(vec![deltas[1].clone()], None).unwrap();
	assert_eq!(guest.head_rev(), Some(second));
	assert!(guest.retired_registry().value_equal(&host.snapshot_from_history().unwrap()));
}

/// Undo puts back what the step overwrote rather than writing over it at the step's stamp, so a concurrent write
/// stamped between lands the same here as on a peer that never saw the step.
#[test]
fn an_undone_step_leaves_no_stamp_behind_for_a_concurrent_write_to_lose_to() {
	let mut a = Session::with_peer(PeerId(3));
	step(&mut a, set_attribute("base", 1));
	let mut b = Session::with_peer(PeerId(2));
	sync(&mut a, &mut b);
	let concurrent = b.stage_ops([set_attribute("k", 20)]).unwrap().last().unwrap().timestamp;

	step(&mut a, set_attribute("k", 10));
	a.undo().unwrap();
	step(&mut a, set_attribute("other", 1));
	sync(&mut a, &mut b);
	b.retire(concurrent).unwrap();
	sync(&mut b, &mut a);

	assert_eq!(document_attribute(&a, "k"), document_attribute(&b, "k"));
	assert_eq!(a.retired_registry(), &a.snapshot_from_history().unwrap());
}

/// A peer's line built on a step undone here brings that step back into the snapshot when the head moves onto it.
#[test]
fn a_fast_forward_onto_an_undone_step_folds_it_back_in() {
	let mut a = Session::with_peer(PeerId(1));
	let first = step(&mut a, set_attribute("first", 1));
	step(&mut a, set_attribute("second", 2));
	// A copy that left without being recorded, as when a crash comes between sending it and persisting the frontier.
	let mut b = Session::with_peer(PeerId(2));
	sync(&mut a.clone(), &mut b);
	a.undo().unwrap();
	assert_eq!(a.head_rev(), Some(first));

	let third = step(&mut b, set_attribute("third", 3));
	assert!(matches!(sync(&mut b, &mut a), MergeOutcome::FastForward(rev) if rev == third));
	assert!(document_attribute(&a, "second").is_some());
	assert!(a.retired_registry().value_equal(&a.snapshot_from_history().unwrap()));
	assert!(!a.can_redo(), "the undone step is on the line again");
}

/// Undo rewinds this user's own steps only, and never across a merge, since either would rewind another peer's write.
#[test]
fn undo_takes_only_this_users_own_steps_and_stops_at_a_merge() {
	let mut a = Session::with_peer(PeerId(1));
	step(&mut a, set_attribute("first", 1));
	step(&mut a, set_attribute("second", 2));
	let mut b = Session::with_peer(PeerId(2));
	sync(&mut a, &mut b);
	assert!(!b.can_undo(), "the steps are the other user's");
	assert!(matches!(b.undo(), Err(crate::CrdtError::NothingToUndo)));

	step(&mut a, set_attribute("a", 1));
	step(&mut b, set_attribute("b", 1));
	assert!(matches!(sync(&mut b, &mut a), MergeOutcome::Merged(_)));
	assert!(!a.can_undo(), "the head is a merge");
}

/// Dropping hot ops another peer retired re-derives the working registry without them until their deltas arrive.
#[test]
fn discarded_hot_ops_leave_the_working_registry_at_the_snapshot() {
	let op = crate::HotOp {
		op: set_attribute("k", 1),
		timestamp: crate::TimeStamp { counter: 3, peer: PeerId(2) },
		sequence: crate::HotSequence(1),
	};
	let mut session = Session::with_peer(PeerId(1));
	session.replay_hot_op(op.clone()).unwrap();
	session.discard_hot_ops(&[op.id()]);
	assert!(session.hot_log().is_empty());
	assert_eq!(session.registry(), session.retired_registry());
}

/// Two peers that each integrate the other's concurrent branch converge to byte-identical history:
/// the merge commit is parent-set-addressed (same `Rev` on both) and the canonical sort erases the
/// arrival-order difference. Exercises `Session::merge`, the `Merge` variant, and `canonical_sort`.
#[test]
fn merge_converges_to_identical_history() {
	// Shared base commit, then a concurrent edit on each peer's own clone of that base.
	let mut session_a = Session::with_peer(PeerId(1));
	session_a.commit_op_for_test(set_attribute("compute::base", 0)).expect("base commit");
	let mut session_b = session_a.clone();

	session_a.commit_op_for_test(set_attribute("compute::a", 1)).expect("A edit");
	session_b.commit_op_for_test(set_attribute("compute::b", 2)).expect("B edit");

	// Cross-merge: feed each peer the other's full delta set. The shared base dedups by `Rev`.
	let deltas_a = session_a.cloned_deltas();
	let deltas_b = session_b.cloned_deltas();
	let merge_a = session_a.merge(deltas_b).expect("merge into A failed");
	let merge_b = session_b.merge(deltas_a).expect("merge into B failed");

	assert!(matches!(merge_a, crate::MergeOutcome::Merged(_)), "divergent branches must produce a merge delta");
	assert_eq!(merge_a, merge_b, "same tips must mint the identical parent-set-addressed merge commit");

	let order_a: Vec<crate::Rev> = session_a.history().map(|d| d.id).collect();
	let order_b: Vec<crate::Rev> = session_b.history().map(|d| d.id).collect();
	assert_eq!(order_a, order_b, "both peers must converge to byte-identical history order");
	assert_eq!(session_a.head_rev(), session_b.head_rev(), "both peers land on the same merge head");
}

/// A write to a network removed on a merged-in branch revives it: the removal keeps the network as a
/// tombstone, and the write is newer than the removal.
#[test]
fn a_write_revives_a_network_removed_on_a_merged_branch() {
	let network_id = NetworkId(7);

	// Shared base, then peer B adds and removes network 7 on its own branch.
	let mut session_a = Session::with_peer(PeerId(1));
	session_a.commit_op_for_test(set_document_attribute("compute::base", 0)).expect("base commit");
	let mut session_b = session_a.clone();

	session_a.commit_op_for_test(set_document_attribute("compute::a", 1)).expect("A edit");
	session_b
		.commit_op_for_test(RegistryDelta::AddNetwork {
			id: network_id,
			network: Network::default(),
		})
		.expect("B AddNetwork");
	session_b
		.commit_op_for_test(RegistryDelta::RemoveNetwork {
			id: network_id,
			snapshot: Network::default(),
		})
		.expect("B RemoveNetwork");

	// A merges B's branch: 7's AddNetwork now lives only under the merge's secondary parent.
	session_a.merge(session_b.cloned_deltas()).expect("merge failed");

	// A SetNetworkExport on 7 is newer than its removal, so it brings 7 back from its tombstone.
	session_a
		.commit_op_for_test(RegistryDelta::SetNetworkExport {
			id: network_id,
			index: 0,
			export: None,
		})
		.expect("a write newer than the removal revives the network");
	assert!(session_a.retired_registry().networks.contains_key(&network_id));
}

/// A concurrent removal and attribute change fold to one retired registry whichever branch merges first.
#[test]
fn a_concurrent_remove_and_attribute_change_commute() {
	let node = Node::new(NetworkId(5), crate::Implementation::Network(NetworkId(5)), 0);
	let mut base = Session::with_peer(PeerId(1));
	commit_retired(&mut base, add_network(5));
	commit_retired(&mut base, RegistryDelta::AddNode { id: NodeId(7), node: node.clone() });

	let mut remover = base.clone();
	commit_retired(&mut remover, RegistryDelta::RemoveNode { id: NodeId(7), snapshot: node });
	let mut changer = Session::with_peer(PeerId(2));
	changer.merge(base.cloned_deltas()).expect("changer adopts the base");
	commit_retired(&mut changer, change_node_attribute(NodeId(7), "tint", serde_json::json!(75)));

	// One delta from each branch, neither an ancestor of the other.
	let removal = remover.cloned_deltas().pop().expect("removal delta");
	let change = changer.cloned_deltas().pop().expect("change delta");
	let merged = |order: [&Delta; 2]| {
		let mut session = base.clone();
		session.merge(order.map(Delta::clone).to_vec()).expect("merge failed");
		session.retired_registry().clone()
	};
	assert_eq!(merged([&removal, &change]), merged([&change, &removal]));
}

/// A peer removes a node and its nested network while another keeps writing to the node, which the newer write keeps
/// alive. Rebuilding the runtime and staging it back unchanged must not bring the removed network back.
#[test]
fn restaging_an_unchanged_runtime_does_not_revive_a_nested_network_removed_under_a_live_owner() {
	let resources = graphene_resource::ResourceRegistry::new();
	let outer = NodeNetwork {
		exports: vec![RuntimeInput::node(RuntimeNodeId(0), 0)],
		nodes: [(
			RuntimeNodeId(0),
			DocumentNode {
				implementation: DocumentNodeImplementation::Network(runtime_network([1, 2])),
				..Default::default()
			},
		)]
		.into_iter()
		.collect(),
		..Default::default()
	};
	let mut a = Session::with_peer(PeerId(1));
	let (_, conversion) = a.stage_from_runtime(&outer, &NoMetadata, &resources).expect("first stage");
	let up_to = a.hot_log().last().unwrap().timestamp;
	a.retire(up_to).unwrap();
	let mut b = Session::with_peer(PeerId(2));
	sync(&mut a, &mut b);

	let (&owner, owner_node) = b
		.registry()
		.node_instances
		.iter()
		.find(|(_, node)| matches!(node.implementation(), Implementation::Network(_)))
		.expect("the owner");
	let Implementation::Network(nested) = *owner_node.implementation() else { unreachable!() };
	let mut removal = vec![RegistryDelta::RemoveNode {
		id: owner,
		snapshot: owner_node.clone(),
	}];
	for (&id, node) in b.registry().node_instances.iter().filter(|(_, node)| node.network() == nested) {
		removal.push(RegistryDelta::RemoveNode { id, snapshot: node.clone() });
	}
	removal.push(RegistryDelta::RemoveNetwork {
		id: nested,
		snapshot: b.registry().networks[&nested].clone(),
	});
	let removal = b.stage_ops(removal).unwrap();

	// A writes to the owner later than the removal, so the owner stays and only the nested network goes.
	for _ in 0..10 {
		a.document.clock.tick();
	}
	a.stage_ops([change_node_attribute(owner, "tint", serde_json::json!(1))]).unwrap();
	for hot_op in removal {
		a.replay_hot_op(hot_op).unwrap();
	}
	assert!(a.registry().node_instances.contains_key(&owner));
	assert!(!a.registry().networks.contains_key(&nested));

	let (runtime, _) = a.registry().to_runtime_with_metadata(&conversion.declarations).expect("to runtime");
	a.mark_runtime_current();
	a.stage_from_runtime(&runtime, &NoMetadata, &resources).expect("restage");
	assert!(!a.registry().networks.contains_key(&nested), "the removed network stays removed");
}
