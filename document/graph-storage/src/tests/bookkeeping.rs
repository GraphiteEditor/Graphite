use crate::{AttributeDelta, CrdtError, HotOpId, Implementation, Network, NetworkId, Node, NodeId, NodeInput, PeerId, RegistryDelta, Session, UserId, Value};

fn set_attribute(key: &str, value: u32) -> RegistryDelta {
	RegistryDelta::ChangeDocumentAttribute {
		delta: AttributeDelta {
			key: key.to_string(),
			value: Some(Value::from(serde_json::json!(value))),
		},
	}
}

fn add_network(id: u64) -> RegistryDelta {
	RegistryDelta::AddNetwork {
		id: NetworkId(id),
		network: Network::default(),
	}
}

/// Every op commutes, so the snapshot folds to the working registry's values whatever the log order.
#[test]
fn retirement_takes_the_ops_under_the_cutoff_in_any_applied_order() {
	let mut host = Session::with_peer(PeerId(1));
	// The log is in arrival order: a guest's later-stamped op sits ahead of an earlier one.
	let hot = |op, counter, peer| crate::HotOp {
		op,
		timestamp: crate::TimeStamp { counter, peer: PeerId(peer) },
		sequence: crate::HotSequence(1),
		attributes: Default::default(),
	};
	let (late, own) = (hot(add_network(9), 50, 2), hot(set_attribute("x", 1), 20, 3));
	host.replay_hot_op(late.clone()).expect("the late-stamped op lands first");
	host.replay_hot_op(own.clone()).expect("the earlier-stamped op lands second");
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

#[test]
fn newest_peer_registration_wins_in_any_order() {
	// Registration rides the staging path, so the steps go through the hot log and retire.
	fn stage_and_retire(session: &mut Session, op: RegistryDelta) {
		let hot = session.stage_ops([op]).expect("stage");
		let ids: Vec<crate::HotOpId> = hot.iter().map(crate::HotOp::id).collect();
		session.retire_hot_ops(&ids).expect("retire");
	}
	let mut device = Session::with_identity(PeerId(1), UserId(10));
	stage_and_retire(&mut device, set_attribute("first", 1));
	assert_eq!(device.registry().peer_users[&PeerId(1)].user, UserId(10));

	// The person at the device changes, so the next batch registers again.
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

#[test]
fn a_reopened_session_drops_a_late_copy_of_a_retired_op() {
	let op = crate::HotOp {
		op: set_attribute("k", 1),
		timestamp: crate::TimeStamp { counter: 3, peer: PeerId(2) },
		sequence: crate::HotSequence(1),
		attributes: Default::default(),
	};
	let mut host = Session::with_peer(PeerId(1));
	host.replay_hot_op(op.clone()).unwrap();
	host.retire(op.timestamp).unwrap();

	let mut reopened = Session::load(PeerId(1), UserId(1), host.retired_registry().clone(), host.cloned_deltas(), host.head_rev(), Vec::new(), 0);
	reopened.absorb_retired_marks(host.retired_marks());
	reopened.replay_hot_op(op.clone()).unwrap();
	assert!(reopened.hot_log().is_empty());
	assert!(reopened.retire(op.timestamp).unwrap().is_empty());
	assert_eq!(reopened.history_len(), host.history_len());
}

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

/// The caller only gets the error, so ops left staged would never be persisted or sent.
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
fn retired_marks_round_trip_through_json() {
	let mut marks = crate::RetiredMarks::default();
	let id = |peer: u64, sequence: u64| HotOpId {
		peer: PeerId(peer),
		sequence: crate::HotSequence(sequence),
	};
	marks.extend([id(1, 1), id(1, 2), id(1, 5), id(u64::MAX, 3)]);
	let json = serde_json::to_string(&marks).unwrap();
	assert_eq!(serde_json::from_str::<crate::RetiredMarks>(&json).unwrap(), marks);
}

/// A late copy of one of this peer's own ops that is already retired still raises the sequence and the clock, so the
/// next op this peer stages takes a fresh sequence and a later stamp.
#[test]
fn a_retired_late_copy_still_advances_the_sequence_and_the_clock() {
	let mut session = Session::with_peer(PeerId(1));
	let own = crate::HotOp {
		op: set_attribute("k", 1),
		timestamp: crate::TimeStamp { counter: 50, peer: PeerId(1) },
		sequence: crate::HotSequence(7),
		attributes: Default::default(),
	};
	let mut marks = crate::RetiredMarks::default();
	marks.extend([own.id()]);
	session.absorb_retired_marks(&marks);
	session.replay_hot_op(own.clone()).expect("a retired copy is dropped");

	let staged = session.stage_ops([set_attribute("k", 2)]).expect("stage");
	let next = staged.last().expect("staged");
	assert!(next.sequence > own.sequence, "a fresh sequence");
	assert!(next.timestamp > own.timestamp, "a later stamp");
}

#[test]
fn only_closed_interactions_are_listed_and_the_marker_commits_nothing() {
	let mut host = Session::with_peer(PeerId(1));
	let guest_op = |counter: u64, sequence: u64, op: RegistryDelta| crate::HotOp {
		op,
		timestamp: crate::TimeStamp { counter, peer: PeerId(2) },
		sequence: crate::HotSequence(sequence),
		attributes: Default::default(),
	};

	assert!(host.end_interaction().expect("nothing to close").is_empty(), "no op of this peer is open");

	// The guest's open interaction is stamped before everything the host does.
	host.replay_hot_op(guest_op(10, 1, add_network(9))).expect("guest op");
	host.replay_hot_op(guest_op(11, 2, set_attribute("guest", 1))).expect("guest op");

	host.stage_ops([set_attribute("host", 1)]).expect("host op");
	let marker = host.end_interaction().expect("close").pop().expect("the host's interaction was open");
	assert!(matches!(marker.op, RegistryDelta::Meta) && marker.ends_interaction());
	assert!(host.end_interaction().expect("nothing more to close").is_empty(), "closing twice stages nothing");

	let closed = host.closed_interactions();
	assert_eq!(closed.len(), 1, "the guest's open interaction is not listed: {closed:?}");
	assert_eq!(closed[0].author, PeerId(1));
	assert!(closed[0].contiguous);
	assert_eq!(closed[0].ops.len(), 3, "RegisterPeer, the attribute write and the marker");

	let revs = host.retire_hot_ops(&closed[0].ops).expect("retire");
	assert_eq!(revs.len(), 2, "the marker commits no delta");
	assert_eq!(host.hot_log().len(), 2, "the guest's ops stay hot");

	host.replay_hot_op(closing(guest_op(12, 3, RegistryDelta::Meta))).expect("guest marker");
	let closed = host.closed_interactions();
	assert_eq!(closed.len(), 1);
	assert_eq!(closed[0].author, PeerId(2));
	assert_eq!(closed[0].ops.len(), 3);
}

#[test]
fn an_interaction_with_a_gap_is_not_contiguous_until_the_gap_fills() {
	let mut host = Session::with_peer(PeerId(1));
	let guest_op = |counter: u64, sequence: u64, op: RegistryDelta| crate::HotOp {
		op,
		timestamp: crate::TimeStamp { counter, peer: PeerId(2) },
		sequence: crate::HotSequence(sequence),
		attributes: Default::default(),
	};
	host.replay_hot_op(guest_op(10, 1, add_network(9))).expect("guest op");
	host.replay_hot_op(closing(guest_op(12, 3, RegistryDelta::Meta))).expect("guest marker, op 2 still in flight");
	host.replay_hot_op(closing(guest_op(13, 4, set_attribute("guest", 2)))).expect("a later interaction");

	let closed = host.closed_interactions();
	assert_eq!(closed.len(), 2);
	assert!(!closed[0].contiguous);
	assert!(!closed[1].contiguous, "the gap is behind the later interaction too");

	host.replay_hot_op(guest_op(11, 2, set_attribute("guest", 1))).expect("the late op");
	let closed = host.closed_interactions();
	assert!(closed.iter().all(|interaction| interaction.contiguous));
	assert_eq!(closed[0].ops.len(), 3);
}

#[test]
fn a_flagged_write_closes_its_interaction() {
	let mut host = Session::with_peer(PeerId(1));
	let guest_op = |counter: u64, sequence: u64, op: RegistryDelta| crate::HotOp {
		op,
		timestamp: crate::TimeStamp { counter, peer: PeerId(2) },
		sequence: crate::HotSequence(sequence),
		attributes: Default::default(),
	};
	host.replay_hot_op(guest_op(10, 1, add_network(9))).expect("guest op");
	assert!(host.closed_interactions().is_empty(), "the interaction is open");

	host.replay_hot_op(closing(guest_op(11, 2, set_attribute("guest", 1)))).expect("flagged write");
	let closed = host.closed_interactions();
	assert_eq!(closed.len(), 1);
	assert_eq!(closed[0].ops.len(), 2, "both writes, and no marker of its own");
}

fn closing(mut hot_op: crate::HotOp) -> crate::HotOp {
	hot_op.attributes.set(crate::attr::delta::INTERACTION_END, Value::Bool(true), crate::TimeStamp::ORIGIN);
	hot_op
}
