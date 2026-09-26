use std::collections::HashSet;

use crate::{AttributeDelta, Delta, History, MergeOutcome, Network, NetworkId, NodeId, PeerId, RegistryDelta, ResourceEntry, ResourceHash, ResourceId, Rev, Session, UserId};

fn set_attribute(key: &str, value: u32) -> RegistryDelta {
	RegistryDelta::ChangeDocumentAttribute {
		delta: AttributeDelta {
			key: key.to_string(),
			value: Some(serde_json::json!(value)),
		},
	}
}

fn add_network(id: u64) -> RegistryDelta {
	RegistryDelta::AddNetwork {
		id: NetworkId(id),
		network: Network::default(),
	}
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

#[test]
fn the_tip_set_follows_pushes_merges_and_sorts() {
	let (mut a, b) = branched_pair();
	assert_eq!(a.document.history.tips(), scanned_tips(&a.document.history));

	a.merge(b.cloned_deltas()).expect("merge");
	assert_eq!(a.document.history.tips(), scanned_tips(&a.document.history), "after a merge joining two branches");
	assert_eq!(a.document.history.tips().len(), 1, "the merge delta is the one tip");

	a.commit_op_for_test(set_attribute("after", 3)).expect("edit after the merge");
	assert_eq!(a.document.history.tips(), scanned_tips(&a.document.history));

	// A reload builds the same indexes from the ordered deltas.
	let reloaded = History::from_ordered(a.cloned_deltas());
	assert_eq!(reloaded.tips(), a.document.history.tips());
	for delta in a.history() {
		assert!(reloaded.contains_timestamp(delta.timestamp));
	}
}

#[test]
fn history_names_the_resource_hashes_it_carries() {
	let hash = ResourceHash::from(b"bytes".as_slice());
	let id = ResourceId::new();
	let mut session = Session::with_peer(PeerId(1));
	session
		.commit_op_for_test(RegistryDelta::AddResource {
			id,
			entry: ResourceEntry::embedded(hash, PeerId(1), crate::TimeStamp::ORIGIN),
		})
		.expect("add");
	assert!(session.document.history.resource_hashes().contains(&hash));

	let entry = session.retired_registry().resources[&id].clone();
	session.commit_op_for_test(RegistryDelta::RemoveResource { id, snapshot: entry }).expect("remove");
	assert!(session.all_referenced_resource_hashes().contains(&hash), "a removed resource's hash is still referenced from history");
}

/// A retirement from the host chains off the guest's last delta, so it is placed without a sort and
/// without walking ancestry, and the result is exactly what the sort would have produced.
#[test]
fn a_chain_off_the_last_delta_fast_forwards_without_sorting() {
	let mut host = Session::with_peer(PeerId(1));
	host.commit_op_for_test(set_attribute("base", 0)).expect("base");
	let mut guest = host.clone();
	guest.document.peer = PeerId(2);

	host.commit_op_for_test(set_attribute("x", 1)).expect("x");
	host.commit_op_for_test(add_network(5)).expect("network");
	host.commit_op_for_test(set_attribute("y", 2)).expect("y");
	let incoming: Vec<Delta> = host.cloned_deltas().into_iter().filter(|delta| guest.delta(delta.id).is_none()).collect();
	assert!(guest.document.history.extends_canonically(guest.history_len()), "an empty tail extends trivially");

	let outcome = guest.merge(incoming).expect("merge");

	assert!(matches!(outcome, MergeOutcome::FastForward(rev) if Some(rev) == host.head_rev()), "{outcome:?}");
	assert_eq!(guest.history().map(|delta| delta.id).collect::<Vec<_>>(), host.history().map(|delta| delta.id).collect::<Vec<_>>());

	let mut sorted = guest.document.history.clone();
	sorted.canonical_sort();
	assert_eq!(
		sorted.iter().map(|delta| delta.id).collect::<Vec<_>>(),
		guest.history().map(|delta| delta.id).collect::<Vec<_>>(),
		"the appended order was already canonical"
	);
	assert!(guest.retired_registry().value_equal(host.retired_registry()));
}

#[test]
fn a_batch_off_an_earlier_delta_is_sorted_into_place() {
	let (mut a, b) = branched_pair();
	let before = a.history_len();
	let incoming: Vec<Delta> = b.cloned_deltas().into_iter().filter(|delta| a.delta(delta.id).is_none()).collect();
	a.merge(incoming).expect("merge");

	assert!(!a.document.history.extends_canonically(before), "b's delta hangs off the base, not off a's last delta");
	let mut sorted = a.document.history.clone();
	sorted.canonical_sort();
	assert_eq!(sorted.iter().map(|delta| delta.id).collect::<Vec<_>>(), a.history().map(|delta| delta.id).collect::<Vec<_>>());
	assert_eq!(a.document.history.tips(), scanned_tips(&a.document.history));
}

/// Retirement takes the ops under the cutoff whatever order the log applied them in: every op commutes,
/// so the snapshot folds to the working registry's values with a later-stamped op left hot.
#[test]
fn retirement_takes_the_ops_under_the_cutoff_in_any_applied_order() {
	let mut host = Session::with_peer(PeerId(1));
	// A guest's op stamped late sits first in the log, ahead of an op stamped earlier: the log is in
	// arrival order, and stamps are the authors' clocks, not the arrival order.
	let late = crate::HotOp {
		op: add_network(9),
		timestamp: crate::TimeStamp { counter: 50, peer: PeerId(2) },
		sequence: crate::HotSequence(1),
	};
	let own = crate::HotOp {
		op: set_attribute("x", 1),
		timestamp: crate::TimeStamp { counter: 20, peer: PeerId(3) },
		sequence: crate::HotSequence(1),
	};
	host.apply_hot_op(late.clone()).expect("the late-stamped op lands first");
	host.apply_hot_op(own.clone()).expect("the earlier-stamped op lands second");

	assert_eq!(host.hot_ops_up_to(own.timestamp).len(), 1, "only the op under the cutoff retires");

	host.retire(own.timestamp).expect("retire");
	assert_eq!(host.hot_log().len(), 1, "the later-stamped op stays hot");
	assert!(!host.retired_registry().networks.contains_key(&NetworkId(9)));
	assert!(host.registry().networks.contains_key(&NetworkId(9)));
	assert!(host.retired_registry().attributes.get("x").is_some_and(|value| !value.deleted));

	host.retire(late.timestamp).expect("retire the rest");
	assert!(host.hot_log().is_empty());
	assert!(host.registry().value_equal(host.retired_registry()));
}

/// A transaction closes with a marker its author stages, and only closed transactions retire: an author's
/// ops past its last marker stay hot however old they are, while a later transaction from someone else
/// retires around them.
#[test]
fn only_closed_transactions_retire_and_an_open_one_stays_hot() {
	let mut host = Session::with_peer(PeerId(1));
	let guest_op = |counter: u64, sequence: u64, op: RegistryDelta| crate::HotOp {
		op,
		timestamp: crate::TimeStamp { counter, peer: PeerId(2) },
		sequence: crate::HotSequence(sequence),
	};

	assert!(host.end_transaction().expect("nothing to close").is_empty(), "no op of this peer is open");

	// The guest opens a transaction and leaves it open, stamped earlier than everything the host does.
	host.apply_hot_op(guest_op(10, 1, add_network(9))).expect("guest op");
	host.apply_hot_op(guest_op(11, 2, set_attribute("guest", 1))).expect("guest op");

	host.stage_ops([set_attribute("host", 1)]).expect("host op");
	let marker = host.end_transaction().expect("close").pop().expect("the host's transaction was open");
	assert!(matches!(marker.op, RegistryDelta::EndTransaction));
	assert!(host.end_transaction().expect("nothing more to close").is_empty(), "closing twice stages nothing");

	let closed = host.closed_transactions();
	assert_eq!(closed.len(), 1, "the guest's open transaction is not listed: {closed:?}");
	assert_eq!(closed[0].author, PeerId(1));
	assert!(closed[0].contiguous);
	assert_eq!(closed[0].ops.len(), 3, "RegisterPeer, the attribute write and the marker");

	let revs = host.retire_transaction(&closed[0]).expect("retire");
	assert_eq!(revs.len(), 2, "the marker commits no delta");
	assert!(host.history().any(|delta| delta.id == revs[1] && delta.is_interaction_end()), "the transaction is one interaction");
	assert_eq!(host.hot_log().len(), 2, "the guest's ops stay hot");
	assert!(host.hot_log().iter().all(|hot_op| hot_op.timestamp.peer == PeerId(2)));
	assert!(!host.retired_registry().networks.contains_key(&NetworkId(9)));
	assert!(host.registry().networks.contains_key(&NetworkId(9)));

	// The guest closes: its transaction retires after the host's although it is stamped before it.
	host.apply_hot_op(guest_op(12, 3, RegistryDelta::EndTransaction)).expect("guest marker");
	let closed = host.closed_transactions();
	assert_eq!(closed.len(), 1);
	assert_eq!(closed[0].author, PeerId(2));
	host.retire_transaction(&closed[0]).expect("retire");
	assert!(host.hot_log().is_empty());
	assert!(host.registry().value_equal(host.retired_registry()));
	assert!(host.document.history.extends_canonically(0), "retirement order is the parent chain, so history stays append-only");
}

/// A transaction with a gap in its author's run is listed as not contiguous, so the retirer can wait for
/// the re-announcement, and closes normally once the gap fills.
#[test]
fn a_transaction_with_a_gap_is_not_contiguous_until_the_gap_fills() {
	let mut host = Session::with_peer(PeerId(1));
	let guest_op = |counter: u64, sequence: u64, op: RegistryDelta| crate::HotOp {
		op,
		timestamp: crate::TimeStamp { counter, peer: PeerId(2) },
		sequence: crate::HotSequence(sequence),
	};
	host.apply_hot_op(guest_op(10, 1, add_network(9))).expect("guest op");
	host.apply_hot_op(guest_op(12, 3, RegistryDelta::EndTransaction)).expect("guest marker, op 2 still in flight");

	let closed = host.closed_transactions();
	assert_eq!(closed.len(), 1);
	assert!(!closed[0].contiguous);

	host.apply_hot_op(guest_op(11, 2, set_attribute("guest", 1))).expect("the late op");
	let closed = host.closed_transactions();
	assert!(closed[0].contiguous);
	assert_eq!(closed[0].ops.len(), 3);
}

/// Taking a hot transaction back removes its ops for good and re-derives what they touched from the
/// snapshot and the other hot ops, so a concurrent write to the same entity survives; the ops never
/// retire, and a late copy is dropped.
#[test]
fn a_retracted_transaction_leaves_no_trace_and_keeps_what_others_wrote() {
	let mut host = Session::with_peer(PeerId(1));
	host.commit_op_for_test(add_network(3)).expect("base");
	host.document.working_registry = host.document.retired_snapshot.clone();

	// A guest's write to the network and the host's own gesture on the same network, interleaved.
	let guest_write = crate::HotOp {
		op: RegistryDelta::ChangeNetworkAttribute {
			id: NetworkId(3),
			delta: AttributeDelta {
				key: "guest".into(),
				value: Some(serde_json::json!(1)),
			},
		},
		timestamp: crate::TimeStamp { counter: 40, peer: PeerId(2) },
		sequence: crate::HotSequence(1),
	};
	host.stage_ops([RegistryDelta::ChangeNetworkAttribute {
		id: NetworkId(3),
		delta: AttributeDelta {
			key: "host".into(),
			value: Some(serde_json::json!(1)),
		},
	}])
	.expect("host op");
	host.apply_hot_op(guest_write.clone()).expect("guest op");
	host.stage_ops([add_network(9)]).expect("host op");
	let own: Vec<crate::HotOpId> = host.hot_log().iter().filter(|hot_op| hot_op.timestamp.peer == PeerId(1)).map(crate::HotOp::id).collect();

	let crate::Retraction { ids, touched, ops } = host.retract_transaction().expect("retract").expect("the host's transaction is hot");
	assert_eq!(ops.len(), 3, "RegisterPeer and the two writes come back for a redo, the marker does not");
	assert_eq!(ids, own, "the whole open transaction is taken back");
	assert!(touched.networks.contains(&NetworkId(3)) && touched.networks.contains(&NetworkId(9)));
	assert_eq!(host.hot_log().len(), 1, "only the guest's op stays");
	assert!(!host.registry().networks.contains_key(&NetworkId(9)));
	let network = &host.registry().networks[&NetworkId(3)];
	assert!(!network.attributes.contains_key("host"), "the host's write is gone");
	assert!(network.attributes.contains_key("guest"), "the guest's concurrent write survives");
	assert!(host.retract_transaction().expect("nothing left").is_none());

	// A late copy of a retracted op is dropped, and nothing of it ever retires.
	let late = crate::HotOp {
		op: add_network(9),
		timestamp: crate::TimeStamp { counter: 2, peer: PeerId(1) },
		sequence: own[1].sequence,
	};
	host.replay_hot_op(late).expect("dropped, not an error");
	assert_eq!(host.hot_log().len(), 1);
	assert!(host.closed_transactions().is_empty());

	// Another peer learns of the retraction from the marks alone.
	let mut guest = Session::with_peer(PeerId(2));
	guest.commit_op_for_test(add_network(3)).expect("base");
	guest.document.working_registry = guest.document.retired_snapshot.clone();
	guest
		.apply_hot_op(crate::HotOp {
			op: add_network(9),
			timestamp: crate::TimeStamp { counter: 2, peer: PeerId(1) },
			sequence: own[1].sequence,
		})
		.expect("the host's op reached the guest");
	assert!(guest.registry().networks.contains_key(&NetworkId(9)));
	let touched = guest.absorb_retracted_marks(host.retracted_marks());
	assert!(touched.networks.contains(&NetworkId(9)));
	assert!(!guest.registry().networks.contains_key(&NetworkId(9)));
	assert!(guest.hot_log().is_empty());
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

/// Undoing a retired step in a session drops it out of the shared line: the later steps are minted again
/// on its parent, the head moves, every peer that follows the move holds the same history and registry,
/// a field a later step wrote keeps that value, and the dropped step comes back on redo as a copy on top.
#[test]
fn a_dropped_interaction_leaves_the_line_and_the_room_follows() {
	let mut host = Session::with_peer(PeerId(1));
	host.commit_op_for_test(set_attribute("base", 0)).expect("base");
	let base = host.head_rev().expect("rev");
	host.mark_interaction_end(base);

	// The guest's step, retired by the host: it writes its own key and one the host writes after it. A
	// guest of its own, clock included, so its ops carry its authorship.
	let mut guest = Session::load(PeerId(2), UserId(2), host.retired_registry().clone(), host.cloned_deltas(), host.head_rev(), Vec::new(), 0);
	let guest_ops = [set_attribute("guest", 1), set_attribute("shared", 1)];
	let hot = guest.stage_ops(guest_ops).expect("stage");
	for hot_op in &hot {
		host.apply_hot_op(hot_op.clone()).expect("the guest's ops reach the host");
	}
	let guest_ids: Vec<crate::HotOpId> = hot.iter().map(crate::HotOp::id).collect();
	let revs = host.retire_hot_ops(&guest_ids).expect("retire");
	let undone = *revs.last().expect("the guest's step");
	host.mark_interaction_end(undone);
	guest
		.merge(host.cloned_deltas().into_iter().filter(|delta| guest.delta(delta.id).is_none()).collect::<Vec<Delta>>())
		.expect("merge");
	guest.discard_hot_ops(&guest_ids).expect("discard");

	// A later step by the host writes the shared key.
	host.commit_op_for_test(set_attribute("shared", 2)).expect("later step");
	let later = host.head_rev().expect("rev");
	host.mark_interaction_end(later);
	guest
		.merge(host.cloned_deltas().into_iter().filter(|delta| guest.delta(delta.id).is_none()).collect::<Vec<Delta>>())
		.expect("merge");
	assert_eq!(guest.head_rev(), host.head_rev());
	assert_eq!(guest.latest_own_interaction(), Some(undone));

	let (moved, touched) = host.drop_interaction(undone).expect("drop");
	assert_eq!(moved.from, Some(later));
	assert_eq!(moved.copies.len(), 1, "the later step is minted again");
	assert_eq!(moved.copies[0].parent, Some(base), "the copy hangs off the dropped step's parent");
	assert!(moved.copies[0].is_interaction_end(), "the copy keeps its interaction end");
	assert!(touched.nodes.is_empty() && !touched.resources, "document attributes only");
	let snapshot = host.retired_registry();
	assert!(snapshot.attributes.get("guest").is_none_or(|value| value.deleted), "the dropped step's write is gone");
	assert_eq!(
		snapshot.attributes.get("shared").map(|value| &value.value),
		Some(&serde_json::json!(2)),
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
	assert!(host.retired_registry().attributes.get("guest").is_some_and(|value| value.value == serde_json::json!(1)));
	assert_eq!(host.retired_registry().attributes.get("shared").map(|value| &value.value), Some(&serde_json::json!(2)));
	guest
		.merge(host.cloned_deltas().into_iter().filter(|delta| guest.delta(delta.id).is_none()).collect::<Vec<Delta>>())
		.expect("merge the copies");
	assert_eq!(guest.head_rev(), host.head_rev());
	assert_eq!(guest.retired_registry(), host.retired_registry());
}

/// An author's ops can reach the retirer out of order, the later ones first through a re-announcement
/// after a lapsed link. Coarsening keeps the newest write by stamp, not by position in the log, so the
/// retired delta carries the value every working registry shows.
#[test]
fn a_transaction_that_arrived_out_of_order_retires_to_its_newest_write() {
	let mut author = Session::with_peer(PeerId(2));
	author.stage_ops(vec![set_attribute("name", 1), set_attribute("name", 2)]).expect("stage");
	author.end_transaction().expect("close");
	let ops = author.hot_log().to_vec();

	let mut in_order = Session::with_peer(PeerId(1));
	let mut reversed = Session::with_peer(PeerId(1));
	for hot_op in &ops {
		in_order.replay_hot_op(hot_op.clone()).expect("replay");
	}
	for hot_op in ops.iter().rev() {
		reversed.replay_hot_op(hot_op.clone()).expect("replay");
	}
	for host in [&mut in_order, &mut reversed] {
		let closed = host.closed_transactions();
		assert_eq!(closed.len(), 1);
		host.retire_transaction(&closed[0]).expect("retire");
		assert!(host.registry().value_equal(host.retired_registry()), "the retired value is the one the working registry showed");
	}
	assert_eq!(reversed.retired_registry(), in_order.retired_registry(), "arrival order decides nothing");
}

/// A transaction retires coarsened: of the writes to one field only the newest becomes a delta, a
/// whole-list input write supersedes the slot writes before it, a write wiring an input to a node stays
/// while what supersedes it wires elsewhere, and the fold is the one every op would have produced.
#[test]
fn a_transaction_retires_to_one_delta_per_field() {
	let attribute = |value: u32| set_attribute("name", value);
	let wire = |target: u64| RegistryDelta::ChangeNodeInput {
		id: NodeId(1),
		index: 0,
		new_input: crate::NodeInput::Node { id: NodeId(target), index: 0 },
	};
	let value = |value: u32| RegistryDelta::ChangeNodeInput {
		id: NodeId(1),
		index: 1,
		new_input: crate::NodeInput::Value {
			value: serde_json::json!(value),
			exposed: false,
		},
	};
	let mut host = Session::with_peer(PeerId(1));
	host.commit_op_for_test(add_network(3)).expect("network");
	host.document.working_registry = host.document.retired_snapshot.clone();
	let node = |id: u64| RegistryDelta::AddNode {
		id: NodeId(id),
		node: crate::Node::new(NetworkId(3), crate::Implementation::ProtoNode(ResourceId::from(7)), 2),
	};
	let ops = vec![node(1), node(2), node(4), attribute(1), value(1), attribute(2), wire(2), value(2), wire(4), attribute(3), value(3)];
	let mut raw = host.clone();
	raw.stage_ops(ops.clone()).expect("stage");
	host.stage_ops(ops).expect("stage");
	host.end_transaction().expect("close");
	raw.end_transaction().expect("close");

	let closed = host.closed_transactions();
	assert_eq!(closed.len(), 1);
	let coarsened = host.retire_transaction(&closed[0]).expect("retire");
	let raw_ids: Vec<crate::HotOpId> = raw.hot_log().iter().map(crate::HotOp::id).collect();
	let uncoarsened = raw.retire_hot_ops(&raw_ids).expect("retire raw");

	assert_eq!(uncoarsened.len(), 12, "RegisterPeer, three additions and eight writes");
	assert_eq!(
		coarsened.len(),
		8,
		"RegisterPeer, three additions, the newest attribute, the newest value, and both wires: {coarsened:?}"
	);
	assert!(host.hot_log().is_empty());
	assert!(host.retired_registry().value_equal(raw.retired_registry()), "the fold is unchanged");
	assert!(host.registry().value_equal(host.retired_registry()), "the working registry, built from every op, agrees");
	assert_eq!(host.retired_registry(), raw.retired_registry(), "stamps included: the survivor keeps its own");
	let kinds: Vec<&RegistryDelta> = host.history().map(|delta| &delta.kind).collect();
	assert!(kinds.iter().filter(|kind| matches!(kind, RegistryDelta::ChangeDocumentAttribute { .. })).count() == 1);
	assert!(
		kinds.iter().filter(|kind| matches!(kind, RegistryDelta::ChangeNodeInput { index: 0, .. })).count() == 2,
		"the wire to node 2 stays as evidence node 2 exists"
	);
	assert!(kinds.iter().filter(|kind| matches!(kind, RegistryDelta::ChangeNodeInput { index: 1, .. })).count() == 1);
	assert!(host.history().last().is_some_and(|delta| delta.is_interaction_end()));
}

/// A merge delta joins a line this peer holds as a branch it walked away from. Following it as a plain
/// extension of the line would leave that branch's effects out of the snapshot, so the snapshot is
/// folded from the joined head's whole ancestry instead, and matches the host's.
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

/// A device's registration to a person is a write like any other: a later one replaces it, and a peer that
/// receives the two in the other order ends up with the same mapping.
#[test]
fn a_peers_registration_is_the_newest_by_stamp_whatever_order_it_lands() {
	// Registration rides the staging path, so the steps go through the hot log and retire.
	fn stage_and_retire(session: &mut Session, op: RegistryDelta) {
		let hot = session.stage_ops([op]).expect("stage");
		let ids: Vec<crate::HotOpId> = hot.iter().map(crate::HotOp::id).collect();
		session.retire_hot_ops(&ids).expect("retire");
	}
	let mut device = Session::with_identity(PeerId(1), UserId(10));
	stage_and_retire(&mut device, set_attribute("first", 1));
	assert_eq!(device.user_of(PeerId(1)), Some(UserId(10)));
	assert_eq!(device.registry().peer_users[&PeerId(1)].user, UserId(10));

	// The person at the device changes: the next batch registers again, and the newer registration wins.
	device.set_user(UserId(20));
	stage_and_retire(&mut device, set_attribute("second", 2));
	assert_eq!(device.registry().peer_users[&PeerId(1)].user, UserId(20));
	let registrations = device.history().filter(|delta| matches!(delta.kind, RegistryDelta::RegisterPeer { .. })).count();
	assert_eq!(registrations, 2, "one registration per person the device stood for");

	let mut reversed = Session::with_peer(PeerId(2));
	let deltas: Vec<Delta> = device.cloned_deltas().into_iter().rev().collect();
	reversed.merge(deltas).expect("merge");
	assert_eq!(reversed.user_of(PeerId(1)), Some(UserId(20)), "the newest registration wins in any order");
	assert_eq!(reversed.user_of(PeerId(2)), Some(UserId(2)), "a peer without a stored identity stands for itself");
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
		host.apply_hot_op(hot_op.clone()).expect("the step reaches the host");
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
