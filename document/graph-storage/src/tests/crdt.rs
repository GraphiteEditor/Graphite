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

/// A SetExport against a removed network must restore the network from history rather than error.
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

/// Cascading resurrection: bringing a node back must also restore its owning network when absent.
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

/// Reverting the same removal twice (the moral equivalent of two peers concurrently resurrecting
/// the same node) must not error on the second apply. Today the second revert hits
/// `apply_op(AddNode, false)` against a present node and returns `NodeAlreadyExists`.
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

	document.restore_node_from_history(RegistryTarget::Working, node_id).expect("first resurrection should succeed");
	assert!(document.working_registry.node_instances.contains_key(&node_id), "first resurrection should bring the node back");

	let second = document.restore_node_from_history(RegistryTarget::Working, node_id);
	assert!(second.is_ok(), "second resurrection of an already-present node should be a no-op, got {second:?}");
}

/// History-based resurrection must work when the matching delta is the *root* commit. The history
/// walk used to drop the root (its empty parent list short-circuited the iterator before yielding
/// it), so a node removed by the very first commit could not be restored.
#[test]
fn restore_node_from_root_commit() {
	use crate::Node;

	let mut document = fresh_document(PeerId(1));
	let node_id = NodeId(42);

	let node = Node::dummy();

	// Seed the working state so the root commit can remove the node (its reverse is the `AddNode` the
	// resurrection looks for). This `RemoveNode` is the only commit, so the match sits at the root.
	document.working_registry.networks.insert(ROOT_NETWORK, Network::default());
	document.retired_snapshot.networks.insert(ROOT_NETWORK, Network::default());
	document.working_registry.node_instances.insert(node_id, node.clone());
	document.retired_snapshot.node_instances.insert(node_id, node.clone());
	commit_op(&mut document, RegistryDelta::RemoveNode { id: node_id, snapshot: node });
	assert!(!document.working_registry.node_instances.contains_key(&node_id), "node should be removed by the root commit");

	document
		.restore_node_from_history(RegistryTarget::Working, node_id)
		.expect("resurrection from the root commit should succeed");
	assert!(document.working_registry.node_instances.contains_key(&node_id), "node must be restored from the root commit");
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

/// `embed_resource_sources` overwrites the working registry with the snapshot, valid only when no
/// unretired hot ops are present. Called with a non-empty hot log it must error rather than silently
/// drop the hot-zone edits.
#[test]
fn embed_resource_sources_rejects_unretired_hot_ops() {
	let mut session = Session::with_peer(PeerId(1));
	let resources = graphene_resource::ResourceRegistry::new();

	// Stage without retiring, leaving hot ops in the log.
	session.stage_from_runtime(&tiny_network(), &NoMetadata, &resources).expect("stage");
	assert!(!session.hot_log().is_empty(), "staging should leave unretired hot ops");

	let result = session.embed_resource_sources(std::iter::empty::<ResourceId>());
	assert!(matches!(result, Err(crate::CrdtError::HotLogNotEmpty)), "expected HotLogNotEmpty, got {result:?}");
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
		}];

		Node {
			implementation: implementation.clone(),
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
