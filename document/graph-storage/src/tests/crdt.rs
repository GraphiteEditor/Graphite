use core_types::uuid::NodeId as RuntimeNodeId;
use graph_craft::ProtoNodeIdentifier;
use graph_craft::concrete;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeInput, NodeNetwork};

use crate::InputSlot;
use crate::{Delta, Document, HotOp, Network, NetworkId, NoMetadata, Node, NodeId, PeerId, ROOT_NETWORK, RegistryDelta, RegistryTarget, Session, TimeStamp};

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
	let reverse = document.compute_reverse_delta(RegistryTarget::Working, &op).expect("compute_reverse_delta failed");
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

	document.apply_hot_op(hot_op).expect("RemoveNode on absent node is a no-op, not an error");

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
			value: Some(serde_json::json!(value)),
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
	let merge_a = session_a.merge(deltas_b).expect("merge into A failed");
	let merge_b = session_b.merge(deltas_a).expect("merge into B failed");

	assert!(matches!(merge_a, crate::MergeOutcome::Merged(_)), "divergent branches must produce a merge delta");
	assert_eq!(merge_a, merge_b, "same tips must mint the identical parent-set-addressed merge commit");

	let order_a: Vec<crate::Rev> = session_a.history().map(|d| d.id).collect();
	let order_b: Vec<crate::Rev> = session_b.history().map(|d| d.id).collect();
	assert_eq!(order_a, order_b, "both peers must converge to byte-identical history order");
	assert_eq!(session_a.head_rev(), session_b.head_rev(), "both peers land on the same merge head");
}

/// A peer whose history is a prefix of the incoming one moves its head forward without minting a
/// merge delta, and the negotiated transfer sends only the deltas past the sampled known revs.
#[test]
fn merge_fast_forwards_a_prefix_history() {
	let mut session_a = Session::with_peer(PeerId(1));
	session_a.commit_op_for_test(set_document_attribute("compute::base", 0)).expect("base commit");
	let mut session_b = session_a.clone();

	for value in 1..=5 {
		session_b.commit_op_for_test(set_document_attribute("compute::b", value)).expect("B edit");
	}

	let known = session_a.known_revs();
	let missing: Vec<_> = session_b.deltas_unknown_to(known).into_iter().cloned().collect();
	assert_eq!(missing.len(), 5, "only B's new commits are unknown to A");

	let outcome = session_a.merge(missing).expect("merge failed");
	assert_eq!(outcome, crate::MergeOutcome::FastForward(session_b.head_rev().unwrap()));
	assert_eq!(session_a.history().count(), session_b.history().count(), "no merge delta was added");

	assert_eq!(session_b.merge(session_a.cloned_deltas()).expect("merge failed"), crate::MergeOutcome::NoOp);
}

/// A write to a network removed on a merged-in branch revives it: the removal keeps the network as a
/// tombstone, and the write is newer than the removal.
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
	assert_eq!(session.registry().peer_users.get(&PeerId(7)), Some(&crate::UserId(7)), "peer mapped to its UserId");

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

/// A SetExport newer than a network.s removal revives the network from its tombstone rather than error.
#[test]
fn set_export_resurrects_absent_network() {
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
	assert!(!document.working_registry.networks.contains_key(&network_id), "network should be removed before the resurrection test");

	commit_op(
		&mut document,
		RegistryDelta::SetNetworkExport {
			id: network_id,
			index: 0,
			export: None,
		},
	);

	assert!(document.working_registry.networks.contains_key(&network_id), "SetExport should have resurrected the network");
}

/// An addition into a removed network is evidence the network exists, so it brings the network back.
#[test]
fn add_node_resurrects_owning_network() {
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

	assert!(
		document.working_registry.networks.contains_key(&network_id),
		"AddNode should have cascaded a resurrection of the owning network"
	);
	assert!(document.working_registry.node_instances.contains_key(&node_id), "the node itself should also be present");
}

/// The same revival arriving twice, from two peers reverting one removal, lands once: the second copy
/// is not newer than what the first wrote.
#[test]
fn concurrent_resurrection_via_revert_is_idempotent() {
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
	assert!(!document.working_registry.node_instances.contains_key(&node_id), "node should be removed before the resurrection test");

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
	crate::attributes::live(attributes).any(|(held, _)| held == key)
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
				source: serde_json::json!("embedded"),
			},
			ts(1, 1),
		)
		.unwrap();
	document
		.apply_op(
			RD::AddSource {
				id,
				key: source_key(0.75, 2),
				source: serde_json::json!("url"),
			},
			ts(1, 2),
		)
		.unwrap();

	let entry = document.working_registry.resources.get(&id).expect("resource entry exists");
	assert_eq!(entry.sources.len(), 2, "both concurrent additions survive");
	// The chain iterates in priority order.
	let bodies: Vec<_> = entry.sources.iter().map(|(_, v)| v.source.clone()).collect();
	assert_eq!(bodies, vec![serde_json::json!("embedded"), serde_json::json!("url")]);
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
				source: serde_json::json!("old"),
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
				source: serde_json::json!("stale"),
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
				source: serde_json::json!("new"),
			},
			ts(9, 1),
		)
		.unwrap();

	let entry = document.working_registry.resources.get(&id).unwrap();
	assert_eq!(entry.source(&key).unwrap().source, serde_json::json!("new"));
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

/// The reverse delta of a RemoveSource restores the prior source body, and applying op-then-reverse
/// round-trips the source chain.
#[test]
fn remove_source_reverse_restores_prior() {
	let mut document = fresh_document(PeerId(1));
	let id = ResourceId::new();
	let key = source_key(0.5, 1);

	commit_op(
		&mut document,
		RD::AddSource {
			id,
			key,
			source: serde_json::json!("kept"),
		},
	);

	// Compute the reverse while the body is still present, then apply the removal.
	let reverse = document.compute_reverse_delta(RegistryTarget::Working, &RD::RemoveSource { id, key }).unwrap();
	match &reverse {
		RD::AddSource { source, .. } => assert_eq!(*source, serde_json::json!("kept"), "reverse of removal re-adds the body"),
		other => panic!("expected AddSource reverse, got {other:?}"),
	}

	document.apply_op(RD::RemoveSource { id, key }, ts(5, 1)).unwrap();
	assert!(document.working_registry.resources.get(&id).unwrap().sources.is_empty(), "source removed");

	// Applying the reverse restores the chain.
	document.apply_op(reverse, ts(6, 1)).unwrap();
	assert_eq!(document.working_registry.resources.get(&id).unwrap().source(&key).unwrap().source, serde_json::json!("kept"));
}

/// AddSource on a fresh slot reverses to a RemoveSource; on an occupied slot it restores the prior body.
#[test]
fn add_source_reverse_depends_on_prior_state() {
	let mut document = fresh_document(PeerId(1));
	let id = ResourceId::new();
	let key = source_key(0.5, 1);

	// Fresh slot: reverse removes.
	let reverse_fresh = document
		.compute_reverse_delta(
			RegistryTarget::Working,
			&RD::AddSource {
				id,
				key,
				source: serde_json::json!("first"),
			},
		)
		.unwrap();
	assert!(matches!(reverse_fresh, RD::RemoveSource { .. }), "reverse of add-to-empty is remove, got {reverse_fresh:?}");

	// Occupy the slot, then reverse of a new add restores the existing body.
	document
		.apply_op(
			RD::AddSource {
				id,
				key,
				source: serde_json::json!("existing"),
			},
			ts(1, 1),
		)
		.unwrap();
	let reverse_overwrite = document
		.compute_reverse_delta(
			RegistryTarget::Working,
			&RD::AddSource {
				id,
				key,
				source: serde_json::json!("overwrite"),
			},
		)
		.unwrap();
	match reverse_overwrite {
		RD::AddSource { source, .. } => assert_eq!(source, serde_json::json!("existing"), "reverse restores prior body"),
		other => panic!("expected AddSource reverse, got {other:?}"),
	}
}

// --- compute_deltas resource diffing ---

use crate::{ResourceEntry, ResourceStore, SourceValue};

fn entry_with_source(priority: f64, peer: u64, body: serde_json::Value, hash: Option<ResourceHash>) -> ResourceEntry {
	ResourceEntry {
		presence: ts(1, peer),
		sources: vec![(source_key(priority, peer), SourceValue { source: body, timestamp: ts(1, peer) })],
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
	from.insert(id, entry_with_source(0.0, 1, serde_json::json!("embedded"), Some(hash)));
	// Same value, different timestamps: must not count as a change.
	let mut to = ResourceStore::new();
	let mut to_entry = entry_with_source(0.0, 1, serde_json::json!("embedded"), Some(hash));
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
	from.insert(kept, entry_with_source(0.0, 1, serde_json::json!("embedded"), Some(hash_old)));
	from.insert(removed, entry_with_source(0.0, 1, serde_json::json!("gone"), None));

	let mut to = ResourceStore::new();
	// `kept`: hash changes and a second source is added.
	let mut kept_entry = entry_with_source(0.0, 1, serde_json::json!("embedded"), Some(hash_new));
	kept_entry.set_source(
		source_key(1.0, 1),
		SourceValue {
			source: serde_json::json!("url"),
			timestamp: ts(1, 1),
		},
	);
	to.insert(kept, kept_entry);
	// `added`: brand new resource.
	to.insert(added, entry_with_source(0.0, 1, serde_json::json!("fresh"), None));

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
	// A clock is past every stamp in the registry it edits, as a session.s persisted clock is.
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
	use crate::{AttributesWrite, Implementation, Value};

	let keys = ["ui::position", "ui::display_name", "ui::locked", "ui::pinned", "call_argument", "context_features"];

	// Fixed implementation so the two nodes differ only in attribute insertion order.
	let implementation = Implementation::ProtoNode(ResourceId::new());

	let make_node = |insertion_order: &[&str]| {
		let mut attributes = crate::Attributes::new();
		for &key in insertion_order {
			attributes.set(key, serde_json::json!(key), TimeStamp::ORIGIN);
		}

		let mut input_attributes = crate::Attributes::new();
		for &key in insertion_order {
			input_attributes.insert(key.to_string(), Value::new(serde_json::json!(key), TimeStamp::ORIGIN));
		}

		let inputs = vec![InputSlot {
			input: crate::NodeInput::Import { index: 0 },
			timestamp: TimeStamp::ORIGIN,
			attributes: input_attributes,
			attributes_timestamp: TimeStamp::ORIGIN,
		}];

		Node {
			presence: Default::default(),
			added: Default::default(),
			inputs_timestamp: Default::default(),
			implementation: implementation.clone(),
			implementation_timestamp: Default::default(),
			inputs,
			attributes,
			attributes_timestamp: Default::default(),
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
		RegistryDelta::AddNode {
			id: NodeId(9),
			node: make_node(&forward),
		},
	);
	let delta_reversed = Delta::new(
		parent,
		author,
		timestamp,
		RegistryDelta::AddNode {
			id: NodeId(9),
			node: make_node(&reversed),
		},
		RegistryDelta::AddNode {
			id: NodeId(9),
			node: make_node(&reversed),
		},
	);

	assert_eq!(delta_forward.id, delta_reversed.id, "Rev must not depend on attribute insertion order");
}

/// Commit a retired delta and mirror it onto the working registry; `commit_op_for_test` alone only
/// advances the snapshot zone.
fn commit_retired(session: &mut Session, op: RegistryDelta) {
	let before = session.history().count();
	session.commit_op_for_test(op).expect("commit failed");

	for delta in session.cloned_deltas().into_iter().skip(before) {
		session.document.apply_op_idempotent(delta.kind, delta.timestamp).expect("mirroring onto the working registry");
	}
}

/// Merge `deltas` into a copy of `base` in the given order, reporting node 7's surviving `tint`.
fn merge_order_outcome(base: &Session, deltas: &[Delta], order: [usize; 2]) -> Option<(serde_json::Value, TimeStamp)> {
	let mut session = base.clone();
	let ordered: Vec<Delta> = order.iter().map(|&index| deltas[index].clone()).collect();
	session.merge(ordered).expect("merge failed");

	session
		.retired_registry()
		.node_instances
		.get(&NodeId(7))
		.and_then(|node| node.attributes.get("tint"))
		.map(|value| (value.value.clone(), value.timestamp))
}

/// The raw ops are arrival-order dependent: change-first leaves the node gone, remove-first resurrects
/// it from the removal's snapshot. Merge has to fold both orders onto the same retired registry.
#[test]
fn a_concurrent_remove_and_attribute_change_commute() {
	let network_id = NetworkId(5);
	let node = Node::new(network_id, crate::Implementation::Network(network_id), 0);

	// Shared base holding node 7, then the two peers diverge.
	let mut base = Session::with_peer(PeerId(1));
	commit_retired(
		&mut base,
		RegistryDelta::AddNetwork {
			id: network_id,
			network: Network::default(),
		},
	);
	commit_retired(&mut base, RegistryDelta::AddNode { id: NodeId(7), node: node.clone() });

	let mut remover = base.clone();
	commit_retired(&mut remover, RegistryDelta::RemoveNode { id: NodeId(7), snapshot: node });

	let mut changer = Session::with_peer(PeerId(2));
	changer.merge(base.cloned_deltas()).expect("changer adopts the base");
	commit_retired(
		&mut changer,
		RegistryDelta::ChangeNodeAttribute {
			id: NodeId(7),
			delta: crate::AttributeDelta {
				key: "tint".to_string(),
				value: Some(serde_json::json!(75)),
			},
		},
	);

	// One delta from each branch, neither an ancestor of the other.
	let removal = remover.cloned_deltas().into_iter().last().expect("removal delta");
	let change = changer.cloned_deltas().into_iter().last().expect("change delta");
	let deltas = [removal, change];

	let removal_first = merge_order_outcome(&base, &deltas, [0, 1]);
	let change_first = merge_order_outcome(&base, &deltas, [1, 0]);

	assert_eq!(removal_first, change_first, "the retired registry must not depend on delta arrival order");
}

/// An op can retire before an earlier one from its author arrives, so the marks cover it out of order.
/// The prefix absorbs it once the gap fills, which bounds the exception set.
#[test]
fn retired_marks_cover_a_gap_and_compact_once_it_fills() {
	let author = PeerId(1);
	let id = |sequence: u64| crate::HotOpId {
		peer: author,
		sequence: crate::HotSequence(sequence),
	};

	let mut marks = crate::RetiredHotOps::default();
	marks.extend([id(1), id(2)]);
	assert_eq!(marks.retired_up_to.get(&author), Some(&crate::HotSequence(2)), "a contiguous run folds straight into the prefix");
	assert!(marks.retired_beyond.is_empty());

	// Sequence 3 never arrived, so 4 retires above the prefix rather than extending it.
	marks.extend([id(4)]);
	assert_eq!(marks.retired_up_to.get(&author), Some(&crate::HotSequence(2)));
	assert!(marks.covers(id(4)), "an op past the gap is still recognized as retired");
	assert!(!marks.covers(id(3)), "the missing op is not claimed");

	// The gap fills, so the prefix swallows both it and the exception behind it.
	marks.extend([id(3)]);
	assert_eq!(marks.retired_up_to.get(&author), Some(&crate::HotSequence(4)));
	assert!(marks.retired_beyond.is_empty(), "the exception set empties once the prefix reaches it");
}

/// Marks merge by union, so adopting a peer's wholesale must not lose local coverage.
#[test]
fn absorbing_marks_keeps_both_sides_coverage() {
	let author = PeerId(1);
	let id = |sequence: u64| crate::HotOpId {
		peer: author,
		sequence: crate::HotSequence(sequence),
	};

	let mut local = crate::RetiredHotOps::default();
	local.extend([id(1), id(4)]);

	let mut remote = crate::RetiredHotOps::default();
	remote.extend([id(1), id(2), id(3), id(5)]);

	local.absorb(&remote);

	// The union is 1..=5 contiguous, so it all collapses into the prefix.
	assert_eq!(local.retired_up_to.get(&author), Some(&crate::HotSequence(5)));
	assert!(local.retired_beyond.is_empty());
	for sequence in 1..=5 {
		assert!(local.covers(id(sequence)), "sequence {sequence} must stay covered");
	}
}

/// Undo rewinds retired history and must leave the hot tail alone: copying the working registry onto
/// the snapshot promotes unretired work. Hot ops exist between staging and retirement even solo.
#[test]
fn undo_does_not_promote_hot_ops_into_the_retired_snapshot() {
	let mut session = Session::with_peer(PeerId(1));

	// Two retired interactions, so the second is undoable.
	commit_retired(&mut session, set_document_attribute("first", 1));
	let boundary = session.history().last().expect("a committed delta").id;
	session.mark_interaction_end(boundary);
	commit_retired(&mut session, set_document_attribute("second", 2));

	// Unretired live work sitting on top, on a key no retired delta touches.
	session.stage_ops([set_document_attribute("hot", 3)]).expect("stage");
	assert!(holds(&session.registry().attributes, "hot"), "the hot op must be in the working registry");
	assert!(!holds(&session.retired_registry().attributes, "hot"), "and must not be in the snapshot");

	session.undo().expect("undo");

	assert!(!holds(&session.retired_registry().attributes, "second"), "undo must rewind the retired snapshot");
	assert!(!holds(&session.retired_registry().attributes, "hot"), "undo promoted an unretired hot op into the retired snapshot");
}

/// Redo puts the interaction back on both zones, so the pair does not drift the other way.
#[test]
fn redo_restores_the_retired_snapshot_without_the_hot_tail() {
	let mut session = Session::with_peer(PeerId(1));

	commit_retired(&mut session, set_document_attribute("first", 1));
	let boundary = session.history().last().expect("a committed delta").id;
	session.mark_interaction_end(boundary);
	commit_retired(&mut session, set_document_attribute("second", 2));

	session.stage_ops([set_document_attribute("hot", 3)]).expect("stage");
	session.undo().expect("undo");
	session.redo().expect("redo");

	assert!(holds(&session.retired_registry().attributes, "second"), "redo must restore the retired delta");
	assert!(!holds(&session.retired_registry().attributes, "hot"), "redo promoted an unretired hot op into the retired snapshot");
	assert!(holds(&session.registry().attributes, "hot"), "the hot op must survive an undo/redo round trip");
}

/// Silent undo emits nothing, so it is only legal while a commit is unpublished. Once peers hold it, a
/// rewind would diverge from them for good.
#[test]
fn publishing_a_commit_disables_silent_undo() {
	let mut session = Session::with_peer(PeerId(1));

	commit_retired(&mut session, set_document_attribute("first", 1));
	let boundary = session.history().last().expect("a committed delta").id;
	session.mark_interaction_end(boundary);
	commit_retired(&mut session, set_document_attribute("second", 2));

	let head = session.head_rev().expect("a head");
	assert!(session.can_undo(), "an unpublished interaction is undoable");

	session.publish_up_to(head);

	assert!(!session.can_undo(), "a published interaction must not be silently rewound");
}

/// An undone delta stays in the DAG for redo, outside `head`'s ancestry. A fold that ignores that
/// restores work the user undid.
#[test]
fn snapshot_from_history_ignores_undone_deltas() {
	let mut session = Session::with_peer(PeerId(1));

	commit_retired(&mut session, set_document_attribute("first", 1));
	let boundary = session.history().last().expect("a committed delta").id;
	session.mark_interaction_end(boundary);
	commit_retired(&mut session, set_document_attribute("second", 2));

	session.undo().expect("undo");
	assert!(!holds(&session.retired_registry().attributes, "second"), "undo rewound the snapshot");

	let folded = session.snapshot_from_history().expect("fold");

	assert!(holds(&folded.attributes, "first"), "history still holds the kept interaction");
	assert!(!holds(&folded.attributes, "second"), "a fold restored an undone delta");
}

/// A node removed, revived by a reference and removed again with a snapshot placing it elsewhere folds
/// to the same snapshot whether the deltas land one by one or all at once (simulation seed 3588534,
/// which once caught a revival reading the wrong removal).
#[test]
fn snapshot_from_history_reproduces_a_revival_between_two_removals() {
	let node_id = NodeId(3);
	let first_network = NetworkId(1);
	let later_network = NetworkId(3);
	let node_in = |network: NetworkId| Node { network, ..Node::dummy() };

	let mut session = Session::with_peer(PeerId(1));
	commit_retired(
		&mut session,
		RegistryDelta::AddNetwork {
			id: first_network,
			network: Network::default(),
		},
	);
	commit_retired(
		&mut session,
		RegistryDelta::AddNode {
			id: node_id,
			node: node_in(first_network),
		},
	);
	commit_retired(
		&mut session,
		RegistryDelta::RemoveNode {
			id: node_id,
			snapshot: node_in(first_network),
		},
	);
	// Resurrects the node from the removal above, the only one so far.
	commit_retired(
		&mut session,
		RegistryDelta::SetNetworkExport {
			id: first_network,
			index: 0,
			export: Some(crate::NodeInput::Node { id: node_id, index: 0 }),
		},
	);
	commit_retired(
		&mut session,
		RegistryDelta::AddNetwork {
			id: later_network,
			network: Network::default(),
		},
	);
	// A later removal whose snapshot places the node in the network created after the export.
	commit_retired(
		&mut session,
		RegistryDelta::RemoveNode {
			id: node_id,
			snapshot: node_in(later_network),
		},
	);

	let folded = session.snapshot_from_history().expect("fold");

	assert_eq!(&folded, session.retired_registry(), "the fold must reproduce the snapshot the deltas built as they landed");
}

/// The newtype is `#[serde(transparent)]`, so persisted state and the wire carry a bare number and the
/// field's encoding is unchanged.
#[test]
fn hot_sequence_serializes_as_a_bare_number() {
	let encoded = serde_json::to_string(&crate::HotSequence(7)).expect("serialize");

	assert_eq!(encoded, "7");
	assert_eq!(serde_json::from_str::<crate::HotSequence>("7").expect("deserialize"), crate::HotSequence(7));
}

/// Retirement drains the hot log, so a retirer that never received an author's earlier op cannot
/// retire it. The later op still retires, landing beyond the author's prefix instead of extending it.
#[test]
fn retiring_over_a_gap_lands_beyond_the_prefix() {
	let author = PeerId(2);
	let mut host = Session::with_peer(PeerId(1));

	// Only the author's second op reaches this peer; the first was lost with the link that carried it.
	let second = HotOp {
		op: set_document_attribute("late", 1),
		timestamp: TimeStamp { counter: 9, peer: author },
		sequence: crate::HotSequence(2),
	};
	host.apply_hot_op(second.clone()).expect("apply");

	host.retire(second.timestamp).expect("retire");

	let retired = host.retired_marks();
	let prefix = retired.retired_up_to.get(&author).copied().unwrap_or(crate::HotSequence::NONE);
	assert_eq!(prefix, crate::HotSequence::NONE, "sequence 1 never arrived, so the prefix cannot move");
	assert!(retired.retired_beyond.contains_key(&author), "the retired op has to be recorded past the gap");
	assert!(retired.covers(second.id()), "and still count as retired");
}

/// A gap that never fills pins the prefix, so every later op from that author lands past it. Stored as
/// runs, that stays one entry however many ops follow.
#[test]
fn a_permanent_gap_accumulates_every_later_op() {
	let author = PeerId(2);
	let id = |sequence: u64| crate::HotOpId {
		peer: author,
		sequence: crate::HotSequence(sequence),
	};

	let mut marks = crate::RetiredHotOps::default();

	// Sequence 1 is lost for good; everything this author writes afterwards still retires.
	for sequence in 2..=20 {
		marks.extend([id(sequence)]);
	}

	let runs = marks.retired_beyond.get(&author).map(Vec::len).unwrap_or_default();
	assert_eq!(runs, 1, "the ops after the gap coalesce into a single run");
	for sequence in 2..=20 {
		assert!(marks.covers(id(sequence)), "sequence {sequence} must stay covered");
	}
	assert!(!marks.covers(id(1)), "the lost op is not claimed");
}

/// A retired delta keeps its op's authoring timestamp, so the snapshot resolves LWW on the same stamps
/// the live view did rather than on retirement order.
#[test]
fn retirement_preserves_the_live_lww_winner() {
	let mut host = Session::with_peer(PeerId(1));

	// Same key from two authors, the newer op arriving first, so LWW keeps it and discards the older.
	let newer = HotOp {
		op: set_document_attribute("k", 1),
		timestamp: TimeStamp { counter: 10, peer: PeerId(2) },
		sequence: crate::HotSequence(1),
	};
	let older = HotOp {
		op: set_document_attribute("k", 2),
		timestamp: TimeStamp { counter: 5, peer: PeerId(3) },
		sequence: crate::HotSequence(1),
	};
	host.apply_hot_op(newer.clone()).expect("apply newer");
	host.apply_hot_op(older).expect("apply older");

	let live = host.registry().attributes.get("k").cloned();
	host.retire(newer.timestamp).expect("retire");

	assert_eq!(host.registry().attributes.get("k"), live.as_ref(), "retirement must not move the live value");
	assert_eq!(
		host.retired_registry().attributes.get("k").map(|attribute| &attribute.value),
		live.as_ref().map(|attribute| &attribute.value),
		"the snapshot has to record the value the live view settled on"
	);
}

/// A straggler LWW discarded still retires, and must not return by doing so: stamped at retirement it
/// would outrank the op that beat it, putting the discarded value into history itself.
#[test]
fn retiring_a_straggler_leaves_the_discarded_value_behind() {
	let mut host = Session::with_peer(PeerId(1));

	let winner = HotOp {
		op: set_document_attribute("k", 1),
		timestamp: TimeStamp { counter: 10, peer: PeerId(2) },
		sequence: crate::HotSequence(1),
	};
	host.apply_hot_op(winner.clone()).expect("apply winner");
	host.retire(winner.timestamp).expect("retire winner");

	// Arriving behind the winner, this op loses in the live view and its value is dropped.
	let straggler = HotOp {
		op: set_document_attribute("k", 2),
		timestamp: TimeStamp { counter: 5, peer: PeerId(3) },
		sequence: crate::HotSequence(1),
	};
	host.apply_hot_op(straggler.clone()).expect("apply straggler");
	host.retire(straggler.timestamp).expect("retire straggler");

	let value = |registry: &crate::Registry| registry.attributes.get("k").map(|attribute| attribute.value.clone());
	assert_eq!(value(host.registry()), Some(serde_json::json!(1)), "the live view keeps the winner");
	assert_eq!(value(host.retired_registry()), Some(serde_json::json!(1)), "and retiring the straggler must not overwrite it");

	let replayed = host.snapshot_from_history().expect("refold");
	assert_eq!(value(&replayed), Some(serde_json::json!(1)), "nor may history replay to the discarded value");
}

/// Every field of the registry is last-writer-wins on a timestamp, whether an entity exists included, so
/// a set of ops folds to one registry whatever order it lands in. Random sets of structural and field
/// ops, attribute deletions included, over a few ids so additions, removals and writes collide, folded
/// in random orders with an op naming an entity not yet seen retried after the rest.
#[test]
fn a_set_of_ops_folds_to_one_registry_in_any_order() {
	use crate::Priority;
	use crate::{Implementation, ResourceEntry, SourceKey};
	use graphene_resource::ResourceId;

	struct Lcg(u64);
	impl Lcg {
		fn below(&mut self, bound: u64) -> u64 {
			self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
			(self.0 >> 33) % bound
		}
	}

	fn random_op(rng: &mut Lcg, at: TimeStamp) -> RegistryDelta {
		let node_id = NodeId(1 + rng.below(4));
		let network_id = NetworkId(1 + rng.below(3));
		let resource_id = ResourceId::from(1 + rng.below(2));
		let input = |rng: &mut Lcg| match rng.below(3) {
			0 => crate::NodeInput::Node {
				id: NodeId(1 + rng.below(4)),
				index: 0,
			},
			_ => crate::NodeInput::Value {
				value: serde_json::json!(rng.below(100)),
				exposed: false,
			},
		};
		match rng.below(18) {
			0 => RegistryDelta::AddNetwork {
				id: network_id,
				network: Network::default(),
			},
			1 => RegistryDelta::RemoveNetwork {
				id: network_id,
				snapshot: Network::default(),
			},
			2 | 3 => RegistryDelta::AddNode {
				id: node_id,
				node: Node::new(network_id, Implementation::ProtoNode(ResourceId::from(7)), 1 + rng.below(2) as usize),
			},
			// A real snapshot is a fold of ops the remover saw, so two snapshots agreeing on a stamp agree on the
			// value; a fabricated one has to be constant for that to hold.
			4 => RegistryDelta::RemoveNode {
				id: node_id,
				snapshot: Node::new(NetworkId(1), Implementation::ProtoNode(ResourceId::from(7)), 2),
			},
			5 | 6 => RegistryDelta::ChangeNodeInput {
				id: node_id,
				index: rng.below(2) as u32,
				new_input: input(rng),
			},
			7 => RegistryDelta::SetNodeInputs {
				id: node_id,
				inputs: (0..1 + rng.below(3))
					.map(|_| InputSlot {
						input: input(rng),
						timestamp: TimeStamp::ORIGIN,
						attributes: Default::default(),
						attributes_timestamp: TimeStamp::ORIGIN,
					})
					.collect(),
			},
			8 => RegistryDelta::ChangeNodeAttribute {
				id: node_id,
				delta: crate::AttributeDelta {
					key: ["name", "lock"][rng.below(2) as usize].into(),
					value: (rng.below(4) > 0).then(|| serde_json::json!(rng.below(100))),
				},
			},
			15 => RegistryDelta::ChangeNodeInputAttribute {
				id: node_id,
				index: rng.below(2) as u32,
				delta: crate::AttributeDelta {
					key: "label".into(),
					value: (rng.below(4) > 0).then(|| serde_json::json!(rng.below(100))),
				},
			},
			16 => RegistryDelta::ChangeNetworkAttribute {
				id: network_id,
				delta: crate::AttributeDelta {
					key: "name".into(),
					value: (rng.below(4) > 0).then(|| serde_json::json!(rng.below(100))),
				},
			},
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
			12 => RegistryDelta::RemoveResource {
				id: resource_id,
				snapshot: ResourceEntry::default(),
			},
			13 => RegistryDelta::SetResourceHash {
				id: resource_id,
				hash: Some(graphene_resource::ResourceHash::from([rng.below(2) as u8; 32])),
			},
			_ => RegistryDelta::AddSource {
				id: resource_id,
				key: SourceKey {
					priority: Priority::new(rng.below(2) as f64).expect("finite"),
					peer: at.peer,
				},
				source: serde_json::json!(rng.below(100)),
			},
		}
	}

	/// Folds the ops in the order given, retrying the ones that name an entity not yet seen until no
	/// retry lands, and returns the working registry.
	fn fold(ops: &[(RegistryDelta, TimeStamp)]) -> crate::Registry {
		let mut document = fresh_document(PeerId(9));
		let mut pending: Vec<&(RegistryDelta, TimeStamp)> = ops.iter().collect();
		loop {
			let before = pending.len();
			pending.retain(|(op, at)| document.apply_op_idempotent(op.clone(), *at).is_err());
			if pending.is_empty() || pending.len() == before {
				return document.working_registry;
			}
		}
	}

	for seed in 0..300u64 {
		let mut rng = Lcg(seed);
		let ops: Vec<(RegistryDelta, TimeStamp)> = (0..8 + rng.below(24))
			.map(|i| {
				let at = TimeStamp {
					counter: 1 + i,
					peer: PeerId(1 + rng.below(3)),
				};
				(random_op(&mut rng, at), at)
			})
			.collect();

		let reference = fold(&ops);
		for _ in 0..6 {
			let mut shuffled = ops.clone();
			for i in (1..shuffled.len()).rev() {
				shuffled.swap(i, rng.below(i as u64 + 1) as usize);
			}
			let folded = fold(&shuffled);
			assert_eq!(
				folded,
				reference,
				"seed {seed}: the ops {:?} folded differently in the order {:?}",
				ops.iter().map(|(_, at)| at.counter).collect::<Vec<_>>(),
				shuffled.iter().map(|(_, at)| at.counter).collect::<Vec<_>>()
			);
		}
	}
}
