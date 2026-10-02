use crate::{AttributeDelta, PeerId, RegistryDelta, Session, UserId, Value};

fn set_attribute(key: &str, value: u32) -> RegistryDelta {
	RegistryDelta::ChangeDocumentAttribute {
		delta: AttributeDelta {
			key: key.to_string(),
			value: Some(Value::from(serde_json::json!(value))),
		},
	}
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
