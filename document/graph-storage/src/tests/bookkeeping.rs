use crate::{AttributeDelta, CrdtError, HotOpId, Implementation, Network, NetworkId, Node, NodeId, NodeInput, PeerId, RegistryDelta, ResourceId, Session, UserId, Value};

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

/// `hot_op` flagged as the end of its author's transaction, as a peer's `end_transaction` stages it.
fn closing(mut hot_op: crate::HotOp) -> crate::HotOp {
	hot_op.attributes.set(crate::attr::hot_op::TRANSACTION_END, Value::Bool(true), crate::TimeStamp::ORIGIN);
	hot_op
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

/// Only closed transactions retire: an author's ops past its last marker stay hot however old, while a
/// later transaction from someone else retires around them.
#[test]
fn only_closed_transactions_retire_and_an_open_one_stays_hot() {
	let mut host = Session::with_peer(PeerId(1));
	let guest_op = |counter: u64, sequence: u64, op: RegistryDelta| crate::HotOp {
		op,
		timestamp: crate::TimeStamp { counter, peer: PeerId(2) },
		sequence: crate::HotSequence(sequence),
		attributes: Default::default(),
	};

	assert!(host.end_transaction().expect("nothing to close").is_empty(), "no op of this peer is open");

	// The guest opens a transaction and leaves it open, stamped earlier than everything the host does.
	host.replay_hot_op(guest_op(10, 1, add_network(9))).expect("guest op");
	host.replay_hot_op(guest_op(11, 2, set_attribute("guest", 1))).expect("guest op");

	host.stage_ops([set_attribute("host", 1)]).expect("host op");
	let marker = host.end_transaction().expect("close").pop().expect("the host's transaction was open");
	assert!(matches!(marker.op, RegistryDelta::Meta) && marker.ends_transaction());
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
	host.replay_hot_op(closing(guest_op(12, 3, RegistryDelta::Meta))).expect("guest marker");
	let closed = host.closed_transactions();
	assert_eq!(closed.len(), 1);
	assert_eq!(closed[0].author, PeerId(2));
	host.retire_transaction(&closed[0]).expect("retire");
	assert!(host.hot_log().is_empty());
	assert!(host.registry().value_equal(host.retired_registry()));
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
		attributes: Default::default(),
	};
	host.replay_hot_op(guest_op(10, 1, add_network(9))).expect("guest op");
	host.replay_hot_op(closing(guest_op(12, 3, RegistryDelta::Meta))).expect("guest marker, op 2 still in flight");

	let closed = host.closed_transactions();
	assert_eq!(closed.len(), 1);
	assert!(!closed[0].contiguous);

	host.replay_hot_op(guest_op(11, 2, set_attribute("guest", 1))).expect("the late op");
	let closed = host.closed_transactions();
	assert!(closed[0].contiguous);
	assert_eq!(closed[0].ops.len(), 3);
}

/// Retracting a hot transaction removes its ops for good and re-derives what they touched from the snapshot
/// and the other hot ops, so a concurrent write to the same entity survives. The ops never retire, and a late copy is dropped.
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
				value: Some(Value::from(serde_json::json!(1))),
			},
		},
		timestamp: crate::TimeStamp { counter: 40, peer: PeerId(2) },
		sequence: crate::HotSequence(1),
		attributes: Default::default(),
	};
	host.stage_ops([RegistryDelta::ChangeNetworkAttribute {
		id: NetworkId(3),
		delta: AttributeDelta {
			key: "host".into(),
			value: Some(Value::from(serde_json::json!(1))),
		},
	}])
	.expect("host op");
	host.replay_hot_op(guest_write.clone()).expect("guest op");
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
		attributes: Default::default(),
	};
	host.replay_hot_op(late.clone()).expect("dropped, not an error");
	assert_eq!(host.hot_log().len(), 1);
	assert!(host.closed_transactions().is_empty());

	// Another peer learns of the retraction from the marks alone.
	let mut guest = Session::with_peer(PeerId(2));
	guest.commit_op_for_test(add_network(3)).expect("base");
	guest.document.working_registry = guest.document.retired_snapshot.clone();
	guest.replay_hot_op(late).expect("the host's op reached the guest");
	assert!(guest.registry().networks.contains_key(&NetworkId(9)));
	let touched = guest.absorb_settled_marks(host.settled_marks());
	assert!(touched.networks.contains(&NetworkId(9)));
	assert!(!guest.registry().networks.contains_key(&NetworkId(9)));
	assert!(guest.hot_log().is_empty());
}

/// An author's ops can reach the retirer out of order through a re-announcement after a lapsed link.
/// Coarsening keeps the newest write by stamp, not log position, so the retired delta matches every working registry.
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
			value: Value::from(serde_json::json!(value)),
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
fn a_retraction_settles_its_sequences_so_later_transactions_stay_contiguous() {
	let mut session = Session::with_peer(PeerId(1));
	session.stage_ops([set_attribute("a", 1)]).unwrap();
	session.end_transaction().unwrap();
	let up_to = session.hot_log().last().unwrap().timestamp;
	session.retire(up_to).unwrap();
	session.stage_ops([set_attribute("b", 1)]).unwrap();
	session.retract_transaction().unwrap();

	session.stage_ops([set_attribute("c", 1)]).unwrap();
	session.end_transaction().unwrap();
	let closed = session.closed_transactions();
	assert!(closed[0].contiguous, "the retracted sequences are not a gap");
	session.retire_transaction(&closed[0]).unwrap();
	let marks = session.settled_marks();
	assert!(marks.settled_runs.is_empty(), "retired and retracted together leave one prefix");
	assert_eq!(marks.settled_up_to[&PeerId(1)], session.last_hot_sequence());
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
	reopened.absorb_settled_marks(host.settled_marks());
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
fn settled_marks_round_trip_through_json() {
	let mut marks = crate::SettledMarks::default();
	let id = |peer: u64, sequence: u64| HotOpId {
		peer: PeerId(peer),
		sequence: crate::HotSequence(sequence),
	};
	marks.extend([id(1, 1), id(1, 2), id(1, 5), id(u64::MAX, 3)]);
	let json = serde_json::to_string(&marks).unwrap();
	assert_eq!(serde_json::from_str::<crate::SettledMarks>(&json).unwrap(), marks);
}

/// A late copy of one of this peer's own ops that is already settled still raises the sequence and the clock, so the
/// next op this peer stages takes a fresh sequence and a later stamp.
#[test]
fn a_settled_late_copy_still_advances_the_sequence_and_the_clock() {
	let mut session = Session::with_peer(PeerId(1));
	let own = crate::HotOp {
		op: set_attribute("k", 1),
		timestamp: crate::TimeStamp { counter: 50, peer: PeerId(1) },
		sequence: crate::HotSequence(7),
		attributes: Default::default(),
	};
	let mut marks = crate::SettledMarks::default();
	marks.extend([own.id()]);
	session.absorb_settled_marks(&marks);
	session.replay_hot_op(own.clone()).expect("a settled copy is dropped");

	let staged = session.stage_ops([set_attribute("k", 2)]).expect("stage");
	let next = staged.last().expect("staged");
	assert!(next.sequence > own.sequence, "a fresh sequence");
	assert!(next.timestamp > own.timestamp, "a later stamp");
}

/// The end of a transaction can ride on its last write: a flagged write closes it as a `Meta` op does.
#[test]
fn a_flagged_write_closes_its_transaction() {
	let mut host = Session::with_peer(PeerId(1));
	let guest_op = |counter: u64, sequence: u64, op: RegistryDelta| crate::HotOp {
		op,
		timestamp: crate::TimeStamp { counter, peer: PeerId(2) },
		sequence: crate::HotSequence(sequence),
		attributes: Default::default(),
	};
	host.replay_hot_op(guest_op(10, 1, add_network(9))).expect("guest op");
	assert!(host.closed_transactions().is_empty(), "the transaction is open");

	host.replay_hot_op(closing(guest_op(11, 2, set_attribute("guest", 1)))).expect("flagged write");
	let closed = host.closed_transactions();
	assert_eq!(closed.len(), 1);
	assert_eq!(closed[0].ops.len(), 2, "both writes, and no marker of its own");
}
