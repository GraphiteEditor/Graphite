use crate::{AttributeDelta, CrdtError, HotOpId, Implementation, Network, NetworkId, Node, NodeId, NodeInput, PeerId, RegistryDelta, Session, UserId, Value};

pub(super) fn set_attribute(key: &str, value: u32) -> RegistryDelta {
	RegistryDelta::ChangeDocumentAttribute {
		delta: AttributeDelta {
			key: key.to_string(),
			value: Some(Value::from(serde_json::json!(value))),
		},
	}
}

pub(super) fn add_network(id: u64) -> RegistryDelta {
	RegistryDelta::AddNetwork {
		id: NetworkId(id),
		network: Network::default(),
	}
}

/// Retirement takes the ops under the cutoff in any log order: every op commutes, so the snapshot folds to
/// the working registry's values with a later-stamped op left hot.
#[test]
fn retirement_takes_the_ops_under_the_cutoff_in_any_applied_order() {
	let mut host = Session::with_peer(PeerId(1));
	// A guest's late-stamped op sits ahead of an earlier-stamped one: the log is in arrival order, stamps
	// are the authors' clocks.
	let hot = |op, counter, peer| crate::HotOp {
		op,
		timestamp: crate::TimeStamp { counter, peer: PeerId(peer) },
		sequence: crate::HotSequence(1),
	};
	let (late, own) = (hot(add_network(9), 50, 2), hot(set_attribute("x", 1), 20, 3));
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
	assert_eq!(device.registry().peer_users[&PeerId(1)].user, UserId(10));

	// The person at the device changes: the next batch registers again, and the newer registration wins.
	device.document.user = UserId(20);
	stage_and_retire(&mut device, set_attribute("second", 2));
	assert_eq!(device.registry().peer_users[&PeerId(1)].user, UserId(20));
	let registrations = device.history().filter(|delta| matches!(delta.kind, RegistryDelta::RegisterPeer { .. })).count();
	assert_eq!(registrations, 2, "one registration per person the device stood for");

	let mut other = Session::with_peer(PeerId(2));
	for delta in device.cloned_deltas().into_iter().rev() {
		other.document.apply_op_idempotent(delta.kind, delta.timestamp).unwrap();
	}
	assert_eq!(other.user_of(PeerId(1)), Some(UserId(20)), "the newest registration wins in any order");
	assert_eq!(other.user_of(PeerId(3)), None, "a peer never registered has no user");
}

#[test]
fn a_batch_past_the_last_sequence_is_refused_whole() {
	let mut session = Session::with_peer(PeerId(1));
	session.restore_hot_sequence(crate::HotSequence(u64::MAX - 1));
	assert!(matches!(session.stage_ops([set_attribute("k", 1)]), Err(crate::CrdtError::SequencesExhausted)));
	assert!(session.hot_log().is_empty());
}

/// The settled marks carry over a reopen, so a late copy of a retired op is dropped rather than retired again.
#[test]
fn a_reopened_session_drops_a_late_copy_of_a_retired_op() {
	let op = crate::HotOp {
		op: set_attribute("k", 1),
		timestamp: crate::TimeStamp { counter: 3, peer: PeerId(2) },
		sequence: crate::HotSequence(1),
	};
	let mut host = Session::with_peer(PeerId(1));
	host.apply_hot_op(op.clone()).unwrap();
	host.retire(op.timestamp).unwrap();

	let mut reopened = Session::load(PeerId(1), UserId(1), host.retired_registry().clone(), host.cloned_deltas(), host.head_rev(), Vec::new(), 0);
	reopened.absorb_settled_marks(host.settled_marks());
	reopened.apply_hot_op(op.clone()).unwrap();
	assert!(reopened.hot_log().is_empty());
	assert!(reopened.retire(op.timestamp).unwrap().is_empty());
	assert_eq!(reopened.history_len(), host.history_len());
}

/// A local op naming a node never seen is refused before it writes anything, its target included, so the working
/// registry stays the snapshot plus the hot log.
#[test]
fn a_refused_local_op_leaves_the_working_registry_unchanged() {
	let mut s = Session::with_peer(PeerId(1));
	let node = Node::new(NetworkId(1), Implementation::Network(NetworkId(1)), 1);
	s.stage_ops([
		RegistryDelta::AddNetwork {
			id: NetworkId(1),
			network: Network::default(),
		},
		RegistryDelta::AddNode { id: NodeId(1), node },
	])
	.unwrap();
	let before = s.registry().clone();

	let refused = s.stage_ops([RegistryDelta::ChangeNodeInput {
		id: NodeId(1),
		index: 0,
		new_input: NodeInput::Node { id: NodeId(404), index: 0 },
	}]);
	assert!(matches!(refused, Err(CrdtError::TargetNodeDoesNotExist(_))));
	assert_eq!(s.registry(), &before, "the refused op's write to node 1 stayed in the working registry");
}

/// A batch refused partway stages nothing: the caller only gets the error, so ops left staged would never be persisted
/// or sent.
#[test]
fn a_refused_batch_stages_nothing() {
	let mut s = Session::with_peer(PeerId(1));
	s.stage_ops([RegistryDelta::AddNetwork {
		id: NetworkId(1),
		network: Network::default(),
	}])
	.unwrap();
	let hot_before = s.hot_log().len();
	let refused = s.stage_ops([
		set_attribute("lands", 1),
		RegistryDelta::ChangeNodeInput {
			id: NodeId(404),
			index: 0,
			new_input: NodeInput::Value {
				value: Value::from(serde_json::json!(1)),
				exposed: false,
			},
		},
	]);
	assert!(refused.is_err());
	assert_eq!(s.hot_log().len(), hot_before, "a refused batch stages nothing");
}

#[test]
fn settled_marks_round_trip_through_json() {
	let mut marks = crate::SettledHotOps::default();
	let id = |peer: u64, sequence: u64| HotOpId {
		peer: PeerId(peer),
		sequence: crate::HotSequence(sequence),
	};
	marks.extend([id(1, 1), id(1, 2), id(1, 5), id(u64::MAX, 3)]);
	let json = serde_json::to_string(&marks).unwrap();
	assert_eq!(serde_json::from_str::<crate::SettledHotOps>(&json).unwrap(), marks);
}
