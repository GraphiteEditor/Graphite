use std::collections::HashSet;

use crate::{AttributeDelta, Delta, History, MergeOutcome, Network, NetworkId, PeerId, RegistryDelta, ResourceEntry, ResourceHash, ResourceId, Rev, Session};

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

	let refolds = guest.refolds();
	let outcome = guest.merge(incoming).expect("merge");

	assert!(matches!(outcome, MergeOutcome::FastForward(rev) if Some(rev) == host.head_rev()), "{outcome:?}");
	assert_eq!(guest.refolds(), refolds, "a fast-forward folds nothing again");
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

/// The in-place fold leaves everything but the snapshot alone and agrees with the cloning oracle.
#[test]
fn the_in_place_fold_matches_the_oracle_and_touches_nothing_else() {
	let mut session = Session::with_peer(PeerId(1));
	session.commit_op_for_test(add_network(3)).expect("network");
	session.commit_op_for_test(set_attribute("x", 1)).expect("x");
	session.stage_ops(vec![set_attribute("hot", 9)]).expect("a hot op on top");

	let oracle = session.snapshot_from_history().expect("oracle");
	let working_before = session.registry().clone();
	let hot_before = session.hot_log().to_vec();
	let head_before = session.head_rev();

	let folded = session.document.fold_snapshot_from_history().expect("fold");

	assert!(folded.value_equal(&oracle));
	assert!(session.registry().value_equal(&working_before), "the working registry is untouched");
	assert_eq!(session.hot_log().len(), hot_before.len());
	assert_eq!(session.head_rev(), head_before);
	assert!(session.document.fold.is_none(), "the scope is cleared on the way out");
}
