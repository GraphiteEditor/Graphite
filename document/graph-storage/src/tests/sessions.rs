//! Peers exchanging retired history: syncing, following and merging lines, and undo against what other peers hold.

use crate::{Delta, History, Implementation, MergeOutcome, Network, NetworkId, Node, NodeId, PeerId, RegistryDelta, ResourceEntry, ResourceHash, ResourceId, Rev, Session, Value};
use std::collections::HashSet;

use super::bookkeeping::{add_network, set_attribute};
use super::crdt::{change_node_attribute, commit_retired};

/// Commits `op` as one interaction, with nothing hot.
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

fn document_attribute(session: &Session, key: &str) -> Option<Value> {
	session.retired_registry().attributes.get(key).filter(|value| !value.deleted).map(|value| value.value.clone())
}

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

#[test]
fn following_a_merge_refolds_the_branch_it_joins() {
	let mut host = Session::with_peer(PeerId(1));
	host.commit_op_for_test(add_network(1)).expect("root");
	let mut guest = Session::with_peer(PeerId(2));
	let root: Vec<Delta> = host.history().cloned().collect();
	guest.follow(root, host.head_rev()).expect("follow the root");

	// The guest retires a step of its own while apart, then follows the host's line, which leaves it behind.
	guest.commit_op_for_test(add_network(2)).expect("own step");
	let own: Vec<Delta> = guest.history().filter(|delta| delta.author == PeerId(2)).cloned().collect();
	host.commit_op_for_test(add_network(3)).expect("host step");
	let line: Vec<Delta> = host.history().cloned().collect();
	guest.follow(line, host.head_rev()).expect("follow the host");
	assert!(!guest.retired_registry().networks.contains_key(&NetworkId(2)), "the guest's own step is off the line");

	// The host merges the guest's step and the guest follows to the joined head.
	host.merge(own).expect("merge");
	let merged: Vec<Delta> = host.history().cloned().collect();
	guest.follow(merged, host.head_rev()).expect("follow the merge");

	let fold = guest.snapshot_from_history().expect("fold");
	assert_eq!(guest.retired_registry(), &fold, "the snapshot is the fold of the joined head's ancestry");
	assert!(guest.retired_registry().networks.contains_key(&NetworkId(2)), "the branch is back on the line");
	assert_eq!(guest.retired_registry(), host.retired_registry());
	assert!(guest.registry().value_equal(guest.retired_registry()));
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

#[test]
fn merge_converges_to_identical_history() {
	let mut session_a = Session::with_peer(PeerId(1));
	session_a.commit_op_for_test(set_attribute("compute::base", 0)).expect("base commit");
	let mut session_b = session_a.clone();

	session_a.commit_op_for_test(set_attribute("compute::a", 1)).expect("A edit");
	session_b.commit_op_for_test(set_attribute("compute::b", 2)).expect("B edit");

	let deltas_a = session_a.cloned_deltas();
	let deltas_b = session_b.cloned_deltas();
	let merge_a = session_a.merge(deltas_b).expect("merge into A failed");
	let merge_b = session_b.merge(deltas_a).expect("merge into B failed");

	assert!(matches!(merge_a, MergeOutcome::Merged(_)), "divergent branches must produce a merge delta");
	assert_eq!(merge_a, merge_b, "same tips must mint the identical parent-set-addressed merge commit");

	let order_a: Vec<Rev> = session_a.history().map(|d| d.id).collect();
	let order_b: Vec<Rev> = session_b.history().map(|d| d.id).collect();
	assert_eq!(order_a, order_b, "both peers must converge to byte-identical history order");
	assert_eq!(session_a.head_rev(), session_b.head_rev(), "both peers land on the same merge head");
}

#[test]
fn a_write_revives_a_network_removed_on_a_merged_branch() {
	let network_id = NetworkId(7);

	// Shared base, then peer B adds and removes network 7 on its own branch.
	let mut session_a = Session::with_peer(PeerId(1));
	session_a.commit_op_for_test(set_attribute("compute::base", 0)).expect("base commit");
	let mut session_b = session_a.clone();

	session_a.commit_op_for_test(set_attribute("compute::a", 1)).expect("A edit");
	session_b.commit_op_for_test(add_network(network_id.0)).expect("B AddNetwork");
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

#[test]
fn a_concurrent_remove_and_attribute_change_commute() {
	let node = Node::new(NetworkId(5), Implementation::Network(NetworkId(5)), 0);
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
