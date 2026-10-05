use core_types::uuid::NodeId as RuntimeNodeId;
use graph_craft::ProtoNodeIdentifier;
use graph_craft::concrete;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeInput, NodeNetwork};

use crate::InputSlot;
use crate::{Delta, Document, HotOp, Network, NetworkId, NoMetadata, Node, NodeId, PeerId, ROOT_NETWORK, RegistryDelta, Session, TimeStamp, Value};

fn fresh_document(peer: PeerId) -> Document {
	Session::with_peer(peer).document
}

fn remove_node_op(node_id: NodeId) -> RegistryDelta {
	// The snapshot only matters for reverse computation; this op is used to test a no-op removal on an
	// absent node, so a placeholder node is fine.
	let snapshot = Node::dummy();
	RegistryDelta::RemoveNode { id: node_id, snapshot }
}

/// Commit a single op to a document as a retired delta. Mints a fresh timestamp, links to
/// current head, applies, records in history, advances head.
fn commit_op(document: &mut Document, op: RegistryDelta) {
	let reverse = crate::prior::capture(&document.working_registry, &op);
	let timestamp = document.clock.tick();
	let delta = Delta::new(document.head, document.peer, timestamp, op, reverse);
	let rev = delta.id;
	document.apply_delta(delta).expect("apply_retired_delta failed");
	document.head = Some(rev);
}

/// Every applied op must advance the local clock past the op's timestamp, so any subsequent
/// local tick is causally later than what we just observed. Locks in the invariant that
/// `apply_op` calls `clock.observe`, regardless of which apply entry point was used.
#[test]
fn apply_hot_op_advances_clock_past_observed_timestamp() {
	let mut document = fresh_document(PeerId(1));
	assert_eq!(document.clock.counter, 0);

	let observed = TimeStamp { counter: 42, peer: PeerId(2) };
	let hot_op = HotOp {
		op: remove_node_op(NodeId(99)),
		timestamp: observed,
		sequence: crate::HotSequence(1),
	};

	document
		.stage_hot_op(hot_op, crate::document::ApplyMode::Strict)
		.expect("RemoveNode on absent node is a no-op, not an error");

	assert!(
		document.clock.counter >= observed.counter,
		"clock counter {} did not advance past observed counter {}",
		document.clock.counter,
		observed.counter
	);

	let next = document.clock.tick();
	assert!(
		next.counter > observed.counter,
		"next tick {} must be strictly later than the observed timestamp {}",
		next.counter,
		observed.counter
	);
}

/// `next_node_id` must never repeat across successive calls on the same document. The blake3 output
/// space is enormous, so any collision in a small loop is a counter-bumping bug, not a hash
/// collision.
#[test]
fn next_node_id_is_unique_within_a_document() {
	let mut document = fresh_document(PeerId(1));

	let mut seen = std::collections::HashSet::new();
	for _ in 0..1000 {
		let id = document.next_node_id();
		assert!(seen.insert(id), "next_node_id repeated after {} calls", seen.len());
	}
}

/// Two peers reading the same shared counter must produce different `NodeId`s. This is the whole
/// reason the counter can be shared across peers instead of being per-peer.
#[test]
fn next_node_id_differs_across_peers_at_same_counter() {
	let mut document_a = fresh_document(PeerId(1));
	let mut document_b = fresh_document(PeerId(2));

	let id_a = document_a.next_node_id();
	let id_b = document_b.next_node_id();
	assert_ne!(id_a, id_b, "peer-scoping is broken: two peers minted the same NodeId at counter 1");
}

fn tiny_network() -> NodeNetwork {
	NodeNetwork {
		exports: vec![NodeInput::node(RuntimeNodeId(0), 0)],
		nodes: [(
			RuntimeNodeId(0),
			DocumentNode {
				inputs: vec![NodeInput::import(concrete!(u32), 0)],
				implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::identity::IdentityNode")),
				..Default::default()
			},
		)]
		.into_iter()
		.collect(),
		..Default::default()
	}
}

/// `verify_history` passes on a normally built history and flags a delta whose content-addressed
/// `id` no longer matches its identity fields (corrupt or crafted history).
#[test]
fn verify_history_detects_rev_mismatch() {
	let resources = graphene_resource::ResourceRegistry::new();

	let mut session = Session::with_peer(PeerId(1));
	session.stage_from_runtime(&tiny_network(), &NoMetadata, &resources).expect("stage failed");
	let last_timestamp = session.hot_log().last().expect("staged a hot op").timestamp;
	session.retire(last_timestamp).expect("retire failed");

	session.verify_history().expect("a freshly built history must validate");

	// Tamper one delta's stored id so it no longer matches its content hash.
	session.document.history.first_mut().expect("history is non-empty").id = crate::Rev::new(0xdead_beef).unwrap();

	assert!(matches!(session.verify_history(), Err(crate::CrdtError::RevMismatch { .. })), "a tampered delta id must be flagged");
}

/// History iteration emits parents before children and is a pure function of the delta set: two
/// sessions independently built from the same network produce byte-identical history order. (The
/// append-order invariant guarantees this directly, with no separate topological sort.)
#[test]
fn history_is_causal_and_deterministic() {
	let resources = graphene_resource::ResourceRegistry::new();

	let build = || {
		let mut session = Session::with_peer(PeerId(1));
		session.stage_from_runtime(&tiny_network(), &NoMetadata, &resources).expect("stage failed");
		let last_timestamp = session.hot_log().last().expect("staged at least one hot op").timestamp;
		session.retire(last_timestamp).expect("retire failed");
		session
	};

	let session_a = build();
	let session_b = build();

	let order_a: Vec<crate::Rev> = session_a.history().map(|delta| delta.id).collect();
	let order_b: Vec<crate::Rev> = session_b.history().map(|delta| delta.id).collect();

	assert!(order_a.len() > 1, "expected a multi-delta history to make ordering meaningful");
	assert_eq!(order_a, order_b, "same delta set must serialize in the same order");

	// Every parent that's part of this history precedes its child.
	let position: std::collections::HashMap<crate::Rev, usize> = order_a.iter().enumerate().map(|(i, rev)| (*rev, i)).collect();
	for delta in session_a.history() {
		for parent in delta.all_parents() {
			if let Some(parent_pos) = position.get(&parent) {
				assert!(*parent_pos < position[&delta.id], "parent {parent} must precede child {} in order", delta.id);
			}
		}
	}
}

fn set_document_attribute(key: &str, value: u32) -> RegistryDelta {
	RegistryDelta::ChangeDocumentAttribute {
		delta: crate::AttributeDelta {
			key: key.to_string(),
			value: Some(Value::Int(value.into())),
		},
	}
}

/// Two peers that each integrate the other's concurrent branch converge to byte-identical history:
/// the merge commit is parent-set-addressed (same `Rev` on both) and the canonical sort erases the
/// arrival-order difference. Exercises `Session::merge`, the `Merge` variant, and `canonical_sort`.
#[test]
fn merge_converges_to_identical_history() {
	// Shared base commit, then a concurrent edit on each peer's own clone of that base.
	let mut session_a = Session::with_peer(PeerId(1));
	session_a.commit_op_for_test(set_document_attribute("compute::base", 0)).expect("base commit");
	let mut session_b = session_a.clone();

	session_a.commit_op_for_test(set_document_attribute("compute::a", 1)).expect("A edit");
	session_b.commit_op_for_test(set_document_attribute("compute::b", 2)).expect("B edit");

	// Cross-merge: feed each peer the other's full delta set. The shared base dedups by `Rev`.
	let deltas_a = session_a.cloned_deltas();
	let deltas_b = session_b.cloned_deltas();
	let merge_a = session_a.merge(deltas_b).expect("merge into A failed").expect("A produced a merge");
	let merge_b = session_b.merge(deltas_a).expect("merge into B failed").expect("B produced a merge");

	assert_eq!(merge_a, merge_b, "same tips must mint the identical parent-set-addressed merge commit");

	let order_a: Vec<crate::Rev> = session_a.history().map(|d| d.id).collect();
	let order_b: Vec<crate::Rev> = session_b.history().map(|d| d.id).collect();
	assert_eq!(order_a, order_b, "both peers must converge to byte-identical history order");
	assert_eq!(session_a.head_rev(), session_b.head_rev(), "both peers land on the same merge head");
}

/// Resurrection must reach into a merged-in branch: a network added then removed on the other peer's
/// branch lives only under the merge's secondary parent, so a `SetNetworkExport` targeting it after
/// the merge can only restore it by traversing all ancestors (not the primary-parent chain).
#[test]
fn resurrection_reaches_across_a_merge() {
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

	// A SetNetworkExport on 7 must resurrect it by walking into the merged-in branch. Before the
	// all-ancestors fix this failed with NetworkNotInHistory (the primary-parent walk missed B's branch).
	session_a
		.commit_op_for_test(RegistryDelta::SetNetworkExport {
			id: network_id,
			index: 0,
			export: None,
		})
		.expect("resurrection must find the AddNetwork on the merged-in branch");
}

/// Committing the same NodeNetwork twice must produce zero history entries on the second commit.
/// Without value-only diffing in compute_deltas, the second commit would emit spurious
/// ChangeNodeInput / ChangeNodeAttribute ops because self.registry has real timestamps while the
/// freshly-built `to` registry has TimeStamp::ORIGIN.
#[test]
fn stage_from_runtime_is_idempotent_for_unchanged_network() {
	let mut session = Session::with_peer(PeerId(1));
	let network = tiny_network();

	let resources = graphene_resource::ResourceRegistry::new();
	let (first, _) = session.stage_from_runtime(&network, &NoMetadata, &resources).expect("first stage failed");
	assert!(!first.is_empty(), "first stage should produce at least one hot op for the initial network");

	let (second, _) = session.stage_from_runtime(&network, &NoMetadata, &resources).expect("second stage failed");
	assert_eq!(second.len(), 0, "second stage of unchanged network produced {} spurious hot ops: {:?}", second.len(), second);
}

/// The peer's first contribution prepends a `RegisterPeer` op (establishing its `UserId` mapping);
/// later contributions don't re-register, and a no-op batch registers nothing.
#[test]
fn first_contribution_registers_the_peer() {
	let mut session = Session::with_peer(PeerId(7));
	let resources = graphene_resource::ResourceRegistry::new();

	assert!(session.registry().peer_users.is_empty(), "no registration before any contribution");

	let (first, _) = session.stage_from_runtime(&tiny_network(), &NoMetadata, &resources).expect("first stage failed");
	let registrations = first.iter().filter(|hot_op| matches!(hot_op.op, RegistryDelta::RegisterPeer { .. })).count();
	assert_eq!(registrations, 1, "exactly one RegisterPeer on first contribution");
	assert!(matches!(first[0].op, RegistryDelta::RegisterPeer { .. }), "RegisterPeer must precede the edit ops");
	assert_eq!(
		session.registry().peer_users.get(&PeerId(7)).map(|registration| registration.user),
		Some(crate::UserId(7)),
		"peer mapped to its UserId"
	);

	// A second, distinct contribution must not re-register.
	let mut other_network = tiny_network();
	other_network.exports.clear();
	let (second, _) = session.stage_from_runtime(&other_network, &NoMetadata, &resources).expect("second stage failed");
	assert!(
		!second.iter().any(|hot_op| matches!(hot_op.op, RegistryDelta::RegisterPeer { .. })),
		"already-registered peer must not re-register"
	);

	// A no-op batch (re-staging an already-converged network) registers nothing on a fresh peer:
	// registration rides a real edit, never a lone op.
	let mut fresh = Session::with_peer(PeerId(8));
	fresh.stage_from_runtime(&tiny_network(), &NoMetadata, &resources).expect("seed stage failed");
	let peers_before = fresh.registry().peer_users.clone();
	let (empty, _) = fresh.stage_from_runtime(&tiny_network(), &NoMetadata, &resources).expect("no-op stage failed");
	assert!(empty.is_empty(), "an unchanged re-stage must produce no hot ops");
	assert_eq!(fresh.registry().peer_users, peers_before, "a no-op batch must not add a registration");
}

/// A SetExport newer than a network's removal revives the network from its tombstone instead of erroring.
#[test]
fn set_network_export_revives_a_removed_network() {
	let mut document = fresh_document(PeerId(1));
	let network_id = NetworkId(7);

	commit_op(
		&mut document,
		RegistryDelta::AddNetwork {
			id: network_id,
			network: Network::default(),
		},
	);
	commit_op(
		&mut document,
		RegistryDelta::RemoveNetwork {
			id: network_id,
			snapshot: Network::default(),
		},
	);
	assert!(!document.working_registry.networks.contains_key(&network_id), "network should be removed before the revival");

	commit_op(
		&mut document,
		RegistryDelta::SetNetworkExport {
			id: network_id,
			index: 0,
			export: None,
		},
	);

	assert!(document.working_registry.networks.contains_key(&network_id), "SetExport should have revived the network");
}

/// An addition into a removed network is evidence the network exists, so it brings the network back.
#[test]
fn add_node_revives_its_removed_network() {
	use crate::Node;

	let mut document = fresh_document(PeerId(1));
	let network_id = NetworkId(7);
	let node_id = NodeId(42);

	commit_op(
		&mut document,
		RegistryDelta::AddNetwork {
			id: network_id,
			network: Network::default(),
		},
	);
	commit_op(
		&mut document,
		RegistryDelta::RemoveNetwork {
			id: network_id,
			snapshot: Network::default(),
		},
	);

	let node = Node { network: network_id, ..Node::dummy() };
	commit_op(&mut document, RegistryDelta::AddNode { id: node_id, node });

	assert!(document.working_registry.networks.contains_key(&network_id), "AddNode should have revived the owning network");
	assert!(document.working_registry.node_instances.contains_key(&node_id), "the node itself should also be present");
}

/// Two peers reverting one removal: the second revival is not newer than the first, so it lands once.
#[test]
fn the_same_revival_arriving_twice_lands_once() {
	use crate::Node;

	let mut document = fresh_document(PeerId(1));
	let network_id = NetworkId(7);
	let node_id = NodeId(42);

	commit_op(
		&mut document,
		RegistryDelta::AddNetwork {
			id: network_id,
			network: Network::default(),
		},
	);
	let node = Node { network: network_id, ..Node::dummy() };
	commit_op(&mut document, RegistryDelta::AddNode { id: node_id, node: node.clone() });
	commit_op(&mut document, RegistryDelta::RemoveNode { id: node_id, snapshot: node });
	assert!(!document.working_registry.node_instances.contains_key(&node_id), "node should be removed before the revival");

	let revive = RegistryDelta::AddNode {
		id: node_id,
		node: Node { network: network_id, ..Node::dummy() },
	};
	let at = document.clock.tick();
	document.apply_op_idempotent(revive.clone(), at).expect("first revival");
	assert!(document.working_registry.node_instances.contains_key(&node_id), "the first revival brings the node back");

	let second = document.apply_op_idempotent(revive, at);
	assert!(second.is_ok(), "the same revival again is a no-op, got {second:?}");
	assert!(document.working_registry.node_instances.contains_key(&node_id));
}

/// Erroring ops still bump the clock: we observed the timestamp on the wire, the fact that the
/// op was rejected locally doesn't unobserve it.
#[test]
fn apply_op_advances_clock_even_when_op_errors() {
	let mut document = fresh_document(PeerId(1));

	let observed = TimeStamp { counter: 17, peer: PeerId(2) };
	let failing_op = RegistryDelta::ChangeNodeInput {
		id: NodeId(7),
		index: 0,
		new_input: crate::NodeInput::Import { index: 0 },
	};

	let result = document.apply_op(failing_op, observed);

	assert!(result.is_err(), "op targeting a nonexistent node should be rejected");
	assert!(document.clock.counter >= observed.counter, "clock should advance on observation even when the op errors");
}

// --- Resource CRDT semantics ---

use crate::{Priority, RegistryDelta as RD, ResourceHash, ResourceId, SourceKey};

fn source_key(priority: f64, peer: u64) -> SourceKey {
	SourceKey {
		priority: Priority::new(priority).expect("test priorities are finite"),
		peer: PeerId(peer),
	}
}

/// Whether the map holds a live value under `key`: a deleted key stays as a tombstone.
fn holds(attributes: &crate::Attributes, key: &str) -> bool {
	attributes.live().any(|(held, _)| held == key)
}

fn ts(counter: u64, peer: u64) -> TimeStamp {
	TimeStamp { counter, peer: PeerId(peer) }
}

/// Two peers concurrently add a source to the same resource at distinct priorities. Both survive
/// (add-wins union), ordered by priority.
#[test]
fn concurrent_source_adds_at_distinct_priorities_both_survive() {
	let mut document = fresh_document(PeerId(1));
	let id = ResourceId::new();

	document
		.apply_op(
			RD::AddSource {
				id,
				key: source_key(0.5, 1),
				source: Value::Str("embedded".into()),
			},
			ts(1, 1),
		)
		.unwrap();
	document
		.apply_op(
			RD::AddSource {
				id,
				key: source_key(0.75, 2),
				source: Value::Str("url".into()),
			},
			ts(1, 2),
		)
		.unwrap();

	let entry = document.working_registry.resources.get(&id).expect("resource entry exists");
	assert_eq!(entry.sources.len(), 2, "both concurrent additions survive");
	// The chain iterates in priority order.
	let bodies: Vec<_> = entry.sources.iter().map(|(_, v)| v.source.clone()).collect();
	assert_eq!(bodies, vec![Value::Str("embedded".into()), Value::Str("url".into())]);
}

/// Re-adding the same source key is LWW on its timestamp: a later write wins, an earlier one is ignored.
#[test]
fn same_source_key_is_last_writer_wins() {
	let mut document = fresh_document(PeerId(1));
	let id = ResourceId::new();
	let key = source_key(0.5, 1);

	document
		.apply_op(
			RD::AddSource {
				id,
				key,
				source: Value::Str("old".into()),
			},
			ts(5, 1),
		)
		.unwrap();
	// Earlier timestamp: ignored.
	document
		.apply_op(
			RD::AddSource {
				id,
				key,
				source: Value::Str("stale".into()),
			},
			ts(2, 1),
		)
		.unwrap();
	// Later timestamp: wins.
	document
		.apply_op(
			RD::AddSource {
				id,
				key,
				source: Value::Str("new".into()),
			},
			ts(9, 1),
		)
		.unwrap();

	let entry = document.working_registry.resources.get(&id).unwrap();
	assert_eq!(entry.source(&key).unwrap().source, Value::Str("new".into()));
}

/// SetResourceHash is LWW on the hash; a later resolve wins, an earlier one is ignored.
#[test]
fn register_resource_hash_is_last_writer_wins() {
	let mut document = fresh_document(PeerId(1));
	let id = ResourceId::new();
	let hash_a = ResourceHash::from(&b"alpha"[..]);
	let hash_b = ResourceHash::from(&b"beta"[..]);

	document.apply_op(RD::SetResourceHash { id, hash: Some(hash_a) }, ts(5, 1)).unwrap();
	document.apply_op(RD::SetResourceHash { id, hash: Some(hash_b) }, ts(2, 1)).unwrap();
	assert_eq!(document.working_registry.resources.get(&id).unwrap().hash, Some(hash_a), "earlier resolve must not clobber later one");

	document.apply_op(RD::SetResourceHash { id, hash: Some(hash_b) }, ts(9, 1)).unwrap();
	assert_eq!(document.working_registry.resources.get(&id).unwrap().hash, Some(hash_b), "later resolve wins");
}

/// For every kind of op, landing on a live entity, a removed one or one never seen.
#[test]
fn restoring_what_an_op_overwrote_gives_back_the_registry_exactly() {
	use crate::{AttributeDelta, Implementation, Network, NodeInput, ROOT_NETWORK};
	let attribute = |key: &str, value: i64| AttributeDelta {
		key: key.into(),
		value: Some(Value::Int(value.into())),
	};
	let reference = |id: u64| NodeInput::Node { id: NodeId(id), index: 0 };
	let (live, removed, unseen, network, resource) = (NodeId(1), NodeId(2), NodeId(3), NetworkId(7), ResourceId::new());
	let node = || Node::new(ROOT_NETWORK, Implementation::ProtoNode(ResourceId::from(1)), 2);

	let mut document = fresh_document(PeerId(1));
	for (op, at) in [
		(
			RD::AddNetwork {
				id: ROOT_NETWORK,
				network: Network::default(),
			},
			1,
		),
		(
			RD::AddNetwork {
				id: network,
				network: Network::default(),
			},
			1,
		),
		(RD::AddNode { id: live, node: node() }, 2),
		(RD::AddNode { id: removed, node: node() }, 2),
		(RD::RemoveNode { id: removed, snapshot: node() }, 3),
		(
			RD::ChangeNodeAttribute {
				id: live,
				delta: attribute("kept", 1),
			},
			4,
		),
		(
			RD::AddSource {
				id: resource,
				key: source_key(0.5, 1),
				source: Value::Str("kept".into()),
			},
			4,
		),
		(RD::ChangeDocumentAttribute { delta: attribute("kept", 1) }, 4),
		(
			RD::SetNetworkExport {
				id: network,
				index: 0,
				export: Some(reference(1)),
			},
			4,
		),
	] {
		document.apply_op_idempotent(op, ts(at, 2)).unwrap();
	}

	let ops = [
		RD::AddNode { id: live, node: node() },
		RD::AddNode { id: unseen, node: node() },
		RD::RemoveNode { id: live, snapshot: node() },
		RD::ChangeNodeAttribute {
			id: live,
			delta: attribute("kept", 2),
		},
		RD::ChangeNodeAttribute { id: live, delta: attribute("new", 2) },
		RD::ChangeNodeAttribute {
			id: removed,
			delta: attribute("new", 2),
		},
		RD::ChangeNodeAttribute {
			id: unseen,
			delta: attribute("new", 2),
		},
		RD::ChangeNodeInput {
			id: live,
			index: 1,
			new_input: reference(2),
		},
		RD::ChangeNodeInput {
			id: live,
			index: 5,
			new_input: reference(3),
		},
		RD::ChangeNodeInputAttribute {
			id: live,
			index: 0,
			delta: attribute("name", 2),
		},
		RD::SetNodeInputs {
			id: live,
			inputs: vec![crate::InputSlot::unset(TimeStamp::ORIGIN); 3],
		},
		RD::SetNodeImplementation {
			id: live,
			implementation: Implementation::Network(NetworkId(8)),
		},
		RD::SetNetworkExport {
			id: network,
			index: 3,
			export: Some(reference(3)),
		},
		RD::ChangeNetworkAttribute {
			id: network,
			delta: attribute("new", 2),
		},
		RD::RemoveNetwork {
			id: network,
			snapshot: Network::default(),
		},
		RD::AddSource {
			id: resource,
			key: source_key(0.5, 1),
			source: Value::Str("new".into()),
		},
		RD::RemoveSource {
			id: resource,
			key: source_key(0.5, 1),
		},
		RD::SetResourceHash { id: ResourceId::new(), hash: None },
		RD::RemoveResource {
			id: resource,
			snapshot: Default::default(),
		},
		RD::ChangeDocumentAttribute { delta: attribute("kept", 2) },
		RD::RegisterPeer {
			peer: PeerId(5),
			user: crate::UserId(5),
		},
	];
	for op in ops {
		let before = document.working_registry.clone();
		let priors = crate::prior::capture(&before, &op);
		let mut after = document.clone();
		after.apply_op_idempotent(op.clone(), ts(10, 2)).unwrap();
		assert_ne!(after.working_registry, before, "{op:?} should change something for this to check anything");
		crate::prior::restore(&mut after.working_registry, &priors);
		assert_eq!(after.working_registry, before, "restoring {op:?}");
	}
}

// --- compute_deltas resource diffing ---

use crate::{ResourceEntry, ResourceStore, SourceValue};

fn entry_with_source(priority: f64, peer: u64, body: Value, hash: Option<ResourceHash>) -> ResourceEntry {
	ResourceEntry {
		presence: ts(1, peer),
		sources: vec![(
			source_key(priority, peer),
			SourceValue {
				source: body,
				timestamp: ts(1, peer),
				deleted: false,
			},
		)],
		sources_timestamp: TimeStamp::ORIGIN,
		hash,
		hash_timestamp: ts(1, peer),
	}
}

fn registry_with_resources(resources: ResourceStore) -> crate::Registry {
	crate::Registry { resources, ..Default::default() }
}

/// An unchanged resource store produces zero deltas, even when timestamps differ (value-only diff).
#[test]
fn compute_deltas_ignores_unchanged_resources() {
	let id = ResourceId::new();
	let hash = ResourceHash::from(&b"img"[..]);

	let mut from = ResourceStore::new();
	from.insert(id, entry_with_source(0.0, 1, Value::Str("embedded".into()), Some(hash)));
	// Same value, different timestamps: must not count as a change.
	let mut to = ResourceStore::new();
	let mut to_entry = entry_with_source(0.0, 1, Value::Str("embedded".into()), Some(hash));
	to_entry.hash_timestamp = ts(99, 2);
	to_entry.sources.iter_mut().for_each(|(_, v)| v.timestamp = ts(99, 2));
	to.insert(id, to_entry);

	let deltas = crate::delta::compute_deltas(&registry_with_resources(from), &registry_with_resources(to));
	assert!(deltas.is_empty(), "unchanged resource (value-equal) produced deltas: {deltas:?}");
}

/// Adding, changing, and removing resources each produce the matching delta, and applying the diff
/// transforms `from` into a registry value-equal to `to`.
#[test]
fn compute_deltas_diffs_resources_and_round_trips() {
	let kept = ResourceId::new();
	let removed = ResourceId::new();
	let added = ResourceId::new();
	let hash_old = ResourceHash::from(&b"old"[..]);
	let hash_new = ResourceHash::from(&b"new"[..]);

	let mut from = ResourceStore::new();
	from.insert(kept, entry_with_source(0.0, 1, Value::Str("embedded".into()), Some(hash_old)));
	from.insert(removed, entry_with_source(0.0, 1, Value::Str("gone".into()), None));

	let mut to = ResourceStore::new();
	// `kept`: hash changes and a second source is added.
	let mut kept_entry = entry_with_source(0.0, 1, Value::Str("embedded".into()), Some(hash_new));
	kept_entry.set_source(
		source_key(1.0, 1),
		SourceValue {
			source: Value::Str("url".into()),
			timestamp: ts(1, 1),
			deleted: false,
		},
	);
	to.insert(kept, kept_entry);
	// `added`: brand new resource.
	to.insert(added, entry_with_source(0.0, 1, Value::Str("fresh".into()), None));

	let deltas = crate::delta::compute_deltas(&registry_with_resources(from.clone()), &registry_with_resources(to.clone()));

	// A brand-new resource is a single whole-entry AddResource, never a fan-out of per-source ops.
	let added_deltas: Vec<_> = deltas.iter().filter(|d| matches!(d, RD::AddResource { id, .. } if *id == added)).collect();
	assert_eq!(added_deltas.len(), 1, "adding a resource should produce exactly one AddResource delta, got {added_deltas:?}");
	assert!(
		!deltas.iter().any(|d| matches!(d, RD::AddSource { id, .. } | RD::SetResourceHash { id, .. } if *id == added)),
		"a brand-new resource must not emit per-source or hash ops"
	);
	// The removed resource is a single whole-entry RemoveResource.
	assert_eq!(
		deltas.iter().filter(|d| matches!(d, RD::RemoveResource { id, .. } if *id == removed)).count(),
		1,
		"removing a resource should produce exactly one RemoveResource delta"
	);

	// Apply the diff to a document seeded with `from`, then check it matches `to` by value.
	let mut document = fresh_document(PeerId(1));
	document.working_registry = registry_with_resources(from);
	// A clock is past every stamp in the registry it edits, as a session's persisted clock is.
	document.clock.observe(ts(1, 1));
	for op in deltas {
		let timestamp = document.clock.tick();
		document.apply_op(op, timestamp).expect("apply resource delta");
	}

	assert!(
		document.working_registry.value_equal(&registry_with_resources(to)),
		"applying the resource diff did not reproduce the target registry"
	);
}

/// Resource GC must keep an undone interaction's resources alive: undo removes a interaction's `AddResource`
/// from the working registry, but redo still needs those bytes. `all_referenced_resource_hashes` must
/// therefore report history-referenced resources even after they leave the current registry, so the
/// editor's GC "used" set doesn't evict them between an undo and a redo.
#[test]
fn all_referenced_resource_hashes_survives_undo() {
	use crate::ResourceId;

	let mut session = Session::with_peer(PeerId(1));
	let resources = graphene_resource::ResourceRegistry::new();

	// Base interaction: the first interaction is intentionally not undoable (the mount-base floor), so commit a
	// network first. Undoing the later resource interaction then lands on this base rather than the root.
	session.stage_from_runtime(&tiny_network(), &NoMetadata, &resources).expect("stage base");
	let base_up_to = session.hot_log().last().expect("staged base").timestamp;
	let base_revs = session.retire(base_up_to).expect("retire base");
	session.mark_interaction_end(*base_revs.last().expect("one base delta"));

	// Second interaction: add a resource and mark the retired delta as a interaction boundary.
	let hash = ResourceHash::from(&b"declaration-bytes"[..]);
	let id = ResourceId::new();
	let hot_ops = session.stage_embedded_resource(id, hash).expect("stage resource");
	let up_to = hot_ops.last().expect("staged one op").timestamp;
	let revs = session.retire(up_to).expect("retire");
	session.mark_interaction_end(*revs.last().expect("one retired delta"));

	assert!(session.registry().resources.contains_key(&id), "resource is present after the interaction");
	assert!(session.all_referenced_resource_hashes().contains(&hash));

	// Undo the interaction: the resource leaves the working registry but stays in history.
	session.undo().expect("undo");
	assert!(!session.registry().resources.contains_key(&id), "undo drops the resource from the working registry");
	assert!(
		session.all_referenced_resource_hashes().contains(&hash),
		"the undone interaction's resource must still be reported so GC keeps its bytes for redo"
	);

	// A later hash change, undone, keeps the new hash reported too: redo puts it back.
	session.redo().expect("redo");
	let changed = ResourceHash::from(&b"changed-bytes"[..]);
	let hot_ops = session.stage_ops([crate::RegistryDelta::SetResourceHash { id, hash: Some(changed) }]).expect("stage hash change");
	let revs = session.retire(hot_ops.last().expect("staged").timestamp).expect("retire");
	session.mark_interaction_end(*revs.last().expect("one retired delta"));
	session.undo().expect("undo the hash change");
	assert!(session.all_referenced_resource_hashes().contains(&changed), "the undone hash change keeps its bytes for redo");
}

/// A commit that produces no deltas must not touch the redo stack. Redo is only abandoned by a real
/// new edit; a no-op commit (here `embed_resource_sources` over an empty id set) leaving it cleared
/// would silently disable redo after an undo.
#[test]
fn no_op_commit_preserves_redo_stack() {
	let mut session = Session::with_peer(PeerId(1));
	let resources = graphene_resource::ResourceRegistry::new();

	// Base interaction (the non-undoable mount floor), then a second interaction to undo onto it.
	session.stage_from_runtime(&tiny_network(), &NoMetadata, &resources).expect("stage base");
	let base_up_to = session.hot_log().last().expect("staged base").timestamp;
	let base_revs = session.retire(base_up_to).expect("retire base");
	session.mark_interaction_end(*base_revs.last().expect("one base delta"));

	let hash = ResourceHash::from(&b"declaration-bytes"[..]);
	let id = ResourceId::new();
	let hot_ops = session.stage_embedded_resource(id, hash).expect("stage resource");
	let up_to = hot_ops.last().expect("staged one op").timestamp;
	let revs = session.retire(up_to).expect("retire");
	session.mark_interaction_end(*revs.last().expect("one retired delta"));

	session.undo().expect("undo");
	assert!(session.can_redo(), "undo must populate the redo stack");

	// A commit over no resources produces no deltas; redo must survive it.
	session.embed_resource_sources(std::iter::empty::<ResourceId>()).expect("no-op embed");
	assert!(session.can_redo(), "a no-op commit must not clear the redo stack");
}

/// `embed_resource_sources` commits its `AddSource` deltas as retired, then mirrors them onto the
/// working registry. With unretired hot ops present it must keep the hot-zone edits (export of a
/// mid-interaction document is lossless) rather than clobbering the working registry with the snapshot.
#[test]
fn embed_resource_sources_preserves_unretired_hot_ops() {
	let mut session = Session::with_peer(PeerId(1));
	let resources = graphene_resource::ResourceRegistry::new();

	// Retire a base so the network's nodes live in the retired snapshot.
	session.stage_from_runtime(&tiny_network(), &NoMetadata, &resources).expect("stage base");
	let base_up_to = session.hot_log().last().expect("staged base").timestamp;
	session.retire(base_up_to).expect("retire base");

	// Stage an embedded resource without retiring, leaving it in the hot log (the working registry now
	// holds it, the retired snapshot does not).
	let hash = ResourceHash::from(&b"hot-resource"[..]);
	let id = ResourceId::new();
	session.stage_embedded_resource(id, hash).expect("stage resource");
	assert!(!session.hot_log().is_empty(), "staging should leave unretired hot ops");
	assert!(session.registry().resources.contains_key(&id), "working registry should hold the hot resource");

	session.embed_resource_sources(std::iter::empty::<ResourceId>()).expect("embed tolerates a non-empty hot log");

	// The hot-zone resource survives in the working registry (not reset to the snapshot), and the hot log
	// is untouched so a later retire still promotes it.
	assert!(session.registry().resources.contains_key(&id), "hot resource must survive the embed");
	assert!(!session.hot_log().is_empty(), "embed must not drain the hot log");
}

/// A delta's `Rev` is content-addressed, so two byte-equal deltas must hash identically regardless
/// of the order their attributes were inserted. This guards the `Attributes` map staying canonically
/// ordered (`BTreeMap`): a hash-randomized map would give the same logical delta different `Rev`s.
#[test]
fn add_node_rev_is_independent_of_attribute_insertion_order() {
	use crate::{AttributeValue, Implementation};

	let keys = ["ui::position", "ui::display_name", "ui::locked", "ui::pinned", "call_argument", "context_features"];

	// Fixed implementation so the two nodes differ only in attribute insertion order.
	let implementation = Implementation::ProtoNode(ResourceId::new());

	let make_node = |insertion_order: &[&str]| {
		let mut attributes = crate::Attributes::new();
		for &key in insertion_order {
			attributes.set(key, Value::Str(key.to_string()), TimeStamp::ORIGIN);
		}

		let mut input_attributes = crate::Attributes::new();
		for &key in insertion_order {
			input_attributes.insert(key.to_string(), AttributeValue::new(Value::Str(key.to_string()), TimeStamp::ORIGIN));
		}

		let inputs = vec![InputSlot {
			input: crate::NodeInput::Import { index: 0 },
			timestamp: TimeStamp::ORIGIN,
			attributes: input_attributes,
		}];

		Node {
			presence: Default::default(),
			network_timestamp: Default::default(),
			inputs_timestamp: Default::default(),
			implementation: implementation.clone(),
			implementation_timestamp: Default::default(),
			inputs,
			attributes,
			network: ROOT_NETWORK,
		}
	};

	let forward: Vec<&str> = keys.to_vec();
	let reversed: Vec<&str> = keys.iter().rev().copied().collect();

	let parent = crate::Rev::new(1);
	let author = PeerId(7);
	let timestamp = TimeStamp { counter: 42, peer: PeerId(7) };

	let delta_forward = Delta::new(
		parent,
		author,
		timestamp,
		RegistryDelta::AddNode {
			id: NodeId(9),
			node: make_node(&forward),
		},
		Vec::new(),
	);
	let delta_reversed = Delta::new(
		parent,
		author,
		timestamp,
		RegistryDelta::AddNode {
			id: NodeId(9),
			node: make_node(&reversed),
		},
		Vec::new(),
	);

	assert_eq!(delta_forward.id, delta_reversed.id, "Rev must not depend on attribute insertion order");
}

/// Commit a retired delta and mirror it onto the working registry, which `commit_op_for_test` leaves alone.
fn commit_retired(session: &mut Session, op: RegistryDelta) {
	let before = session.history().count();
	session.commit_op_for_test(op).expect("commit failed");

	for delta in session.cloned_deltas().into_iter().skip(before) {
		session.document.apply_op_idempotent(delta.kind, delta.timestamp).expect("mirroring onto the working registry");
	}
}

fn add_network(id: u64) -> RegistryDelta {
	let network = Network::default();
	RegistryDelta::AddNetwork { id: NetworkId(id), network }
}

fn change_node_attribute(id: NodeId, key: &str, value: serde_json::Value) -> RegistryDelta {
	let delta = crate::AttributeDelta {
		key: key.into(),
		value: Some(Value::from(value)),
	};
	RegistryDelta::ChangeNodeAttribute { id, delta }
}

fn hot_op(op: RegistryDelta, counter: u64, peer: u64, sequence: u64) -> HotOp {
	let sequence = crate::HotSequence(sequence);
	HotOp {
		op,
		timestamp: ts(counter, peer),
		sequence,
	}
}

/// An op past a gap is covered without the gap, a filled gap joins the prefix, and absorbing is a union.
#[test]
fn settled_marks_keep_a_prefix_and_runs_past_gaps() {
	let author = PeerId(1);
	let id = |sequence: u64| crate::HotOpId {
		peer: author,
		sequence: crate::HotSequence(sequence),
	};
	let marks_of = |batches: &[&[u64]]| {
		let mut marks = crate::SettledMarks::default();
		batches.iter().for_each(|batch| marks.extend(batch.iter().map(|&sequence| id(sequence))));
		marks
	};
	let after_a_lost_op: Vec<u64> = (2..=20).collect();
	let one_by_one: Vec<&[u64]> = after_a_lost_op.chunks(1).collect();

	/// Batches extended in order, batches of a peer's marks absorbed, expected prefix, expected runs past it.
	type Case<'a> = (&'a [&'a [u64]], &'a [&'a [u64]], u64, Option<usize>);
	let cases: [Case; 5] = [
		(&[&[1, 2]], &[], 2, None),
		(&[&[1, 2], &[4]], &[], 2, Some(1)),
		(&[&[1, 2], &[4], &[3]], &[], 4, None),
		(one_by_one.as_slice(), &[], 0, Some(1)),
		(&[&[1, 4]], &[&[1, 2, 3, 5]], 5, None),
	];
	for (batches, absorbed, prefix, runs) in cases {
		let mut marks = marks_of(batches);
		marks.absorb(&marks_of(absorbed));
		let up_to = marks.settled_up_to.get(&author).copied().unwrap_or(crate::HotSequence::NONE);
		assert_eq!(up_to, crate::HotSequence(prefix), "{batches:?}");
		assert_eq!(marks.settled_runs.get(&author).map(Vec::len), runs, "{batches:?}");
		for sequence in 1..=21 {
			let retired = batches.iter().chain(absorbed).any(|batch| batch.contains(&sequence));
			assert_eq!(marks.covers(id(sequence)), retired, "{batches:?}: sequence {sequence}");
		}
	}
}

/// A retirer that never received an author's earlier op still retires the later one, past the author's prefix.
#[test]
fn retiring_over_a_gap_lands_beyond_the_prefix() {
	let mut host = Session::with_peer(PeerId(1));
	// Sequence 1 was lost with the link that carried it.
	let second = hot_op(set_document_attribute("late", 1), 9, 2, 2);
	host.replay_hot_op(second.clone()).expect("apply");
	host.retire(second.timestamp).expect("retire");

	let marks = host.settled_marks();
	assert_eq!(marks.settled_up_to.get(&PeerId(2)).copied().unwrap_or(crate::HotSequence::NONE), crate::HotSequence::NONE);
	assert_eq!(marks.settled_runs.get(&PeerId(2)).map(Vec::len), Some(1));
	assert!(marks.covers(second.id()));
}

/// Undo and redo never promote the hot tail, a fold skips undone deltas, and a published commit is not rewound.
#[test]
fn undo_and_redo_move_only_unpublished_retired_history() {
	let mut session = Session::with_peer(PeerId(1));
	commit_retired(&mut session, set_document_attribute("first", 1));
	let boundary = session.history().last().expect("a committed delta").id;
	session.mark_interaction_end(boundary);
	commit_retired(&mut session, set_document_attribute("second", 2));
	session.stage_ops([set_document_attribute("hot", 3)]).expect("stage");

	let mut published = session.clone();
	published.publish_up_to(published.head_rev().expect("a head"));
	assert!(session.can_undo() && !published.can_undo());

	let retired = |session: &Session, key| holds(&session.retired_registry().attributes, key);
	session.undo().expect("undo");
	assert!(!retired(&session, "second") && !retired(&session, "hot"));
	let folded = session.snapshot_from_history().expect("fold");
	assert!(holds(&folded.attributes, "first") && !holds(&folded.attributes, "second"));

	session.redo().expect("redo");
	assert!(retired(&session, "second") && !retired(&session, "hot"));
	assert!(holds(&session.registry().attributes, "hot"));
}

/// So the snapshot and a replay keep the LWW winner the live view did, whether the straggler retires with it or after.
#[test]
fn retirement_preserves_the_live_lww_winner() {
	for straggler_retires_alone in [false, true] {
		let mut host = Session::with_peer(PeerId(1));
		let winner = hot_op(set_document_attribute("k", 1), 10, 2, 1);
		host.replay_hot_op(winner.clone()).expect("apply winner");
		if straggler_retires_alone {
			host.retire(winner.timestamp).expect("retire winner");
		}
		host.replay_hot_op(hot_op(set_document_attribute("k", 2), 5, 3, 1)).expect("apply straggler");
		host.retire(winner.timestamp).expect("retire");

		let value = |registry: &crate::Registry| registry.attributes.get("k").map(|attribute| attribute.value.clone());
		let replayed = host.snapshot_from_history().expect("refold");
		let values = [value(host.registry()), value(host.retired_registry()), value(&replayed)];
		assert_eq!(values, [(); 3].map(|_| Some(Value::from(serde_json::json!(1)))), "straggler retires alone: {straggler_retires_alone}");
	}
}

/// Random ops over a few colliding ids fold to one registry in any order. Removal snapshots are constant or, as a remover
/// sends them, a fold of some earlier ops; restoring each op's priors gives back the registry it applied to.
#[test]
fn a_set_of_ops_folds_to_one_registry_in_any_order() {
	use crate::{Implementation, Priority, ResourceEntry, SourceKey, SourceValue, UserId};
	use graphene_resource::{ResourceHash, ResourceId};

	struct Lcg(u64);
	impl Lcg {
		fn below(&mut self, bound: u64) -> u64 {
			self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
			(self.0 >> 33) % bound
		}
	}

	/// Folds the ops in order; `check_restore` also checks each op's priors restore the registry it applied to.
	fn fold(ops: &[(RegistryDelta, TimeStamp)], check_restore: bool) -> crate::Registry {
		let mut document = fresh_document(PeerId(9));
		for (op, at) in ops {
			let before = check_restore.then(|| (document.working_registry.clone(), crate::prior::capture(&document.working_registry, op)));
			document.apply_op_idempotent(op.clone(), *at).unwrap_or_else(|error| panic!("{op:?} at {at:?}: {error}"));
			if let Some((registry, priors)) = before {
				let mut restored = document.working_registry.clone();
				crate::prior::restore(&mut restored, &priors);
				assert_eq!(restored, registry, "restoring what {op:?} overwrote");
			}
		}
		document.working_registry
	}

	fn random_op(rng: &mut Lcg, at: TimeStamp, seen: &[(RegistryDelta, TimeStamp)], real_snapshots: bool) -> RegistryDelta {
		let node_id = NodeId(1 + rng.below(4));
		let network_id = NetworkId(1 + rng.below(3));
		let resource_id = ResourceId::from(1 + rng.below(2));
		let input = |rng: &mut Lcg| match rng.below(3) {
			0 => crate::NodeInput::Node {
				id: NodeId(1 + rng.below(4)),
				index: 0,
			},
			_ => crate::NodeInput::Value {
				value: Value::from(serde_json::json!(rng.below(100))),
				exposed: false,
			},
		};
		// A write, or a deletion one time in four.
		let attribute = |rng: &mut Lcg, key: &str| crate::AttributeDelta {
			key: key.into(),
			value: (rng.below(4) > 0).then(|| Value::from(serde_json::json!(rng.below(100)))),
		};
		let key = |rng: &mut Lcg| SourceKey {
			priority: Priority::new(rng.below(2) as f64).expect("finite"),
			peer: PeerId(1 + rng.below(2).min(at.peer.0)),
		};
		// What a remover saw: some of the ops before it. Two snapshots agreeing on a stamp then agree on the value,
		// which a fabricated snapshot only does by being constant.
		let saw = |rng: &mut Lcg| fold(&seen.iter().filter(|_| rng.below(2) == 0).cloned().collect::<Vec<_>>(), false);
		match rng.below(22) {
			0 => {
				// An export list of its own, so additions of different lengths meet.
				let exports = (0..rng.below(3))
					.map(|_| crate::ExportSlot {
						target: Some(input(rng)),
						timestamp: TimeStamp::ORIGIN,
					})
					.collect();
				RegistryDelta::AddNetwork {
					id: network_id,
					network: Network { exports, ..Default::default() },
				}
			}
			1 => {
				let snapshot = if real_snapshots { saw(rng).networks.get(&network_id).cloned() } else { Some(Network::default()) };
				snapshot.map_or(RegistryDelta::Other(Value::None), |snapshot| RegistryDelta::RemoveNetwork { id: network_id, snapshot })
			}
			2 | 3 => RegistryDelta::AddNode {
				id: node_id,
				node: Node::new(network_id, Implementation::ProtoNode(ResourceId::from(7)), 1 + rng.below(2) as usize),
			},
			4 => {
				let fabricated = || Node::new(NetworkId(1), Implementation::ProtoNode(ResourceId::from(7)), 2);
				let snapshot = if real_snapshots { saw(rng).node_instances.get(&node_id).cloned() } else { Some(fabricated()) };
				snapshot.map_or(RegistryDelta::Other(Value::None), |snapshot| RegistryDelta::RemoveNode { id: node_id, snapshot })
			}
			5 | 6 => RegistryDelta::ChangeNodeInput {
				id: node_id,
				index: rng.below(3) as u32,
				new_input: input(rng),
			},
			7 => RegistryDelta::SetNodeInputs {
				id: node_id,
				inputs: (0..1 + rng.below(3))
					.map(|_| {
						let mut slot = InputSlot {
							input: input(rng),
							timestamp: TimeStamp::ORIGIN,
							attributes: Default::default(),
						};
						// Half the time a slot carries attributes over with earlier stamps, as an implementation swap does.
						if rng.below(2) == 0 {
							let floor = TimeStamp {
								counter: 1 + rng.below(at.counter),
								peer: PeerId(1 + rng.below(3)),
							};
							let label = TimeStamp {
								counter: floor.counter + rng.below(at.counter - floor.counter + 1),
								..floor
							};
							slot.attributes
								.insert("label".into(), crate::AttributeValue::new(Value::from(serde_json::json!(rng.below(100))), label));
							slot.attributes.set_floor(floor);
						}
						slot
					})
					.collect(),
			},
			8 => {
				let key = ["name", "lock"][rng.below(2) as usize];
				RegistryDelta::ChangeNodeAttribute {
					id: node_id,
					delta: attribute(rng, key),
				}
			}
			9 => RegistryDelta::SetNodeImplementation {
				id: node_id,
				implementation: match rng.below(2) {
					0 => Implementation::Network(network_id),
					_ => Implementation::ProtoNode(ResourceId::from(rng.below(3))),
				},
			},
			10 => RegistryDelta::SetNetworkExport {
				id: network_id,
				index: rng.below(2) as u32,
				export: match rng.below(2) {
					0 => None,
					_ => Some(input(rng)),
				},
			},
			11 => RegistryDelta::AddResource {
				id: resource_id,
				entry: ResourceEntry::default(),
			},
			12 => {
				let snapshot = if real_snapshots {
					saw(rng).resources.get(&resource_id).cloned()
				} else {
					Some(ResourceEntry::default())
				};
				snapshot.map_or(RegistryDelta::Other(Value::None), |snapshot| RegistryDelta::RemoveResource { id: resource_id, snapshot })
			}
			13 => RegistryDelta::SetResourceHash {
				id: resource_id,
				hash: Some(ResourceHash::from([rng.below(2) as u8; 32])),
			},
			14 => RegistryDelta::AddSource {
				id: resource_id,
				key: key(rng),
				source: Value::from(serde_json::json!(rng.below(100))),
			},
			15 => RegistryDelta::ChangeNodeInputAttribute {
				id: node_id,
				index: rng.below(3) as u32,
				delta: attribute(rng, "label"),
			},
			16 => RegistryDelta::ChangeNetworkAttribute {
				id: network_id,
				delta: attribute(rng, "name"),
			},
			17 => RegistryDelta::RemoveSource { id: resource_id, key: key(rng) },
			18 => RegistryDelta::RegisterPeer {
				peer: PeerId(1 + rng.below(2)),
				user: UserId(rng.below(3)),
			},
			19 => RegistryDelta::ChangeDocumentAttribute { delta: attribute(rng, "doc") },
			_ => {
				// A resource added whole, with content.
				let mut entry = ResourceEntry {
					hash: Some(ResourceHash::from([rng.below(2) as u8; 32])),
					..Default::default()
				};
				let source = SourceValue {
					source: Value::from(serde_json::json!(rng.below(100))),
					timestamp: TimeStamp::ORIGIN,
					deleted: false,
				};
				entry.set_source(key(rng), source);
				RegistryDelta::AddResource { id: resource_id, entry }
			}
		}
	}

	for real_snapshots in [false, true] {
		for seed in 0..300u64 {
			let mut rng = Lcg(seed);
			let mut ops: Vec<(RegistryDelta, TimeStamp)> = Vec::new();
			for i in 0..8 + rng.below(24) {
				let at = TimeStamp {
					counter: 1 + i,
					peer: PeerId(1 + rng.below(3)),
				};
				let op = random_op(&mut rng, at, &ops, real_snapshots);
				ops.push((op, at));
			}

			let reference = fold(&ops, true);
			for _ in 0..6 {
				let mut shuffled = ops.clone();
				for i in (1..shuffled.len()).rev() {
					shuffled.swap(i, rng.below(i as u64 + 1) as usize);
				}
				let order: Vec<u64> = shuffled.iter().map(|(_, at)| at.counter).collect();
				assert_eq!(
					fold(&shuffled, false),
					reference,
					"seed {seed}, real snapshots {real_snapshots}: folded differently in the order {order:?}"
				);
			}
		}
	}
}

/// An entry stamped at its map's floor was written with it, so folding a removal's snapshot keeps it.
#[test]
fn a_removal_snapshot_keeps_the_attributes_the_addition_wrote() {
	let mut document = fresh_document(PeerId(1));
	let id = NodeId(4);
	let mut node = Node::new(ROOT_NETWORK, crate::Implementation::ProtoNode(graphene_resource::ResourceId::from(7)), 1);
	node.attributes.set("ui::name", Value::from(serde_json::json!("Layer")), TimeStamp::ORIGIN);
	node.inputs[0].attributes.set("reflection_metadata", Value::from(serde_json::json!("meta")), TimeStamp::ORIGIN);
	commit_op(&mut document, add_network(ROOT_NETWORK.0));
	commit_op(&mut document, RegistryDelta::AddNode { id, node });

	// A removal and a newer write, which revives the node from the tombstone.
	let removal = RegistryDelta::RemoveNode {
		id,
		snapshot: document.working_registry.node_instances[&id].clone(),
	};
	let removed_at = document.clock.tick();
	document.apply_op_idempotent(removal.clone(), removed_at).expect("removal");
	let written_at = document.clock.tick();
	document.apply_op_idempotent(change_node_attribute(id, "ui::lock", serde_json::json!(true)), written_at).expect("write");

	let revived = &document.working_registry.node_instances[&id];
	assert_eq!(revived.attributes.get_typed::<String>("ui::name").as_deref(), Some("Layer"));
	assert_eq!(revived.inputs()[0].attributes.get_typed::<String>("reflection_metadata").as_deref(), Some("meta"));

	// The same removal replayed into the live node is too old to land and changes nothing.
	let mut replayed = document.clone();
	replayed.apply_op_idempotent(removal, removed_at).expect("a replayed removal");
	assert_eq!(replayed.working_registry, document.working_registry);
}

#[test]
fn a_refused_replayed_op_changes_nothing() {
	use crate::{Implementation, NodeInput};
	let mut document = fresh_document(PeerId(1));
	let node = Node::new(NetworkId(1), Implementation::Network(NetworkId(1)), 1);
	for (op, counter) in [
		(add_network(1), 1),
		(RegistryDelta::AddNode { id: NodeId(1), node: node.clone() }, 2),
		(RegistryDelta::AddNode { id: NodeId(2), node: node.clone() }, 3),
		(RegistryDelta::RemoveNode { id: NodeId(2), snapshot: node }, 4),
	] {
		document.apply_op_idempotent(op, ts(counter, 5)).unwrap();
	}
	let before = document.working_registry.clone();

	let out_of_bounds = RegistryDelta::ChangeNodeInput {
		id: NodeId(1),
		index: 1 << 20,
		new_input: NodeInput::Node { id: NodeId(2), index: 0 },
	};
	assert!(document.apply_op_idempotent(out_of_bounds, ts(5, 5)).is_err());
	assert_eq!(document.working_registry, before);
}

/// An implementation swap carries a slot's `ui::*` attributes over as the editor held them; a concurrent rename and a
/// concurrent first-time attribute on that slot both survive it, whichever lands first.
#[test]
fn an_implementation_swap_keeps_concurrent_slot_attribute_writes() {
	use crate::Implementation;
	use graphene_resource::ResourceId;
	let slot_attribute = |key: &str, value: &str| RegistryDelta::ChangeNodeInputAttribute {
		id: NodeId(1),
		index: 0,
		delta: crate::AttributeDelta {
			key: key.into(),
			value: Some(Value::from(serde_json::json!(value))),
		},
	};
	let mut base = fresh_document(PeerId(1));
	commit_op(&mut base, add_network(1));
	let node = Node::new(NetworkId(1), Implementation::ProtoNode(ResourceId::from(7)), 1);
	commit_op(&mut base, RegistryDelta::AddNode { id: NodeId(1), node });
	commit_op(&mut base, slot_attribute("ui::name", "a"));

	// The swap as the editor builds it: the new slot with the held slot's `ui::*` attributes and floor carried over.
	let held = base.working_registry.node_instances[&NodeId(1)].inputs()[0].clone();
	let mut carried = InputSlot::unset(TimeStamp::ORIGIN);
	carried.attributes.set_floor(held.attributes.floor());
	carried.attributes.extend(held.attributes.into_iter().filter(|(key, value)| key.starts_with("ui::") && !value.deleted));
	let swap = (RegistryDelta::SetNodeInputs { id: NodeId(1), inputs: vec![carried] }, ts(20, 1));
	let rename = (slot_attribute("ui::name", "b"), ts(10, 2));
	let describe = (slot_attribute("ui::description", "d"), ts(11, 2));

	for order in [[&swap, &rename, &describe], [&rename, &describe, &swap]] {
		let mut document = base.clone();
		for (op, at) in order {
			document.apply_op_idempotent(op.clone(), *at).expect("apply");
		}
		let slot = &document.working_registry.node_instances[&NodeId(1)].inputs()[0];
		assert_eq!(
			slot.attributes.get("ui::name").map(|value| value.value.clone()),
			Some(Value::from(serde_json::json!("b"))),
			"the rename survives"
		);
		assert!(slot.attributes.get("ui::description").is_some_and(|value| !value.deleted), "the first-time attribute survives");
	}
}

/// Re-adding a removed network with a shorter export list drops the slots past its end, whichever lands first.
#[test]
fn a_network_re_added_shorter_keeps_only_its_own_exports() {
	use crate::ExportSlot;
	let export = |node: u64| ExportSlot {
		target: Some(crate::NodeInput::Node { id: NodeId(node), index: 0 }),
		timestamp: TimeStamp::ORIGIN,
	};
	let network = |exports: Vec<ExportSlot>| Network { exports, ..Default::default() };
	let added = (
		RegistryDelta::AddNetwork {
			id: NetworkId(1),
			network: network(vec![export(1), export(2)]),
		},
		ts(1, 1),
	);
	let removed = (
		RegistryDelta::RemoveNetwork {
			id: NetworkId(1),
			snapshot: network(vec![export(1), export(2)]),
		},
		ts(2, 1),
	);
	let re_added = (
		RegistryDelta::AddNetwork {
			id: NetworkId(1),
			network: network(vec![export(3)]),
		},
		ts(3, 2),
	);
	for order in [[&added, &removed, &re_added], [&re_added, &removed, &added]] {
		let mut document = fresh_document(PeerId(9));
		for (op, at) in order {
			document.apply_op_idempotent(op.clone(), *at).expect("apply");
		}
		let exports = &document.working_registry.networks[&NetworkId(1)].exports;
		assert_eq!(exports.iter().map(|slot| slot.target.clone()).collect::<Vec<_>>(), vec![export(3).target]);
	}
}

/// Two registries ordering a removal differently against a write are not order-consistent, though their values agree.
#[test]
fn order_consistency_covers_removal_stamps() {
	let with_removal_at = |counter: u64| {
		let mut document = fresh_document(PeerId(1));
		let node = Node::new(NetworkId(1), crate::Implementation::ProtoNode(graphene_resource::ResourceId::from(7)), 0);
		document.apply_op_idempotent(RegistryDelta::AddNode { id: NodeId(2), node: node.clone() }, ts(3, 1)).expect("add");
		document
			.apply_op_idempotent(RegistryDelta::RemoveNode { id: NodeId(1), snapshot: node }, ts(counter, 1))
			.expect("remove");
		document.working_registry
	};
	let (earlier, later) = (with_removal_at(2), with_removal_at(4));
	assert!(earlier.value_equal(&later));
	assert!(!earlier.order_consistent(&later));
}

/// An input naming a node whose addition has not arrived renders as a dangling input, as one naming a removed node does,
/// rather than failing the conversion: the placeholder knows nothing of the node's network.
#[test]
fn an_input_naming_a_node_not_yet_added_renders_dangling() {
	use crate::Implementation;
	let (nested, empty, owner, inner, unseen) = (NetworkId(5), NetworkId(6), NodeId(1), NodeId(2), NodeId(3));
	let mut document = fresh_document(PeerId(1));
	let ops = [
		add_network(ROOT_NETWORK.0),
		add_network(nested.0),
		add_network(empty.0),
		RegistryDelta::AddNode {
			id: owner,
			node: Node::new(ROOT_NETWORK, Implementation::Network(nested), 0),
		},
		RegistryDelta::AddNode {
			id: inner,
			node: Node::new(nested, Implementation::Network(empty), 1),
		},
		RegistryDelta::ChangeNodeInput {
			id: inner,
			index: 0,
			new_input: crate::NodeInput::Node { id: unseen, index: 0 },
		},
	];
	for (counter, op) in (1..).zip(ops) {
		document.apply_op_idempotent(op, ts(counter, 2)).expect("apply");
	}
	assert!(document.working_registry.removed_nodes.get(&unseen).is_some_and(|mark| mark.placeholder));

	document.working_registry.to_runtime_with_metadata(&crate::Declarations::new()).expect("converts with a dangling input");
}
