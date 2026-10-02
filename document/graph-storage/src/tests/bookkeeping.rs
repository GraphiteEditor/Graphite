use crate::{AttributeDelta, CrdtError, Implementation, Network, NetworkId, Node, NodeId, NodeInput, PeerId, RegistryDelta, Session, UserId, Value};

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
	};
	let (late, own) = (hot(add_network(9), 50, 2), hot(set_attribute("x", 1), 20, 3));
	host.replay_hot_op(late.clone()).expect("the late-stamped op lands first");
	host.replay_hot_op(own.clone()).expect("the earlier-stamped op lands second");

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
		session.retire(hot.last().expect("staged").timestamp).expect("retire");
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
