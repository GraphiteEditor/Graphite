//! The step to version 5 stores every node another node reads relative to its readers, appending the conversion to history as one interaction.

use document_graph_storage::{
	AttributeDelta, Delta, Implementation, Network, Node, NodeId, NodeInput, PeerId, ROOT_NETWORK, Registry, RegistryDelta, ResourceId, Session, SessionState, UserId, rehash_deltas, to_value,
};
use migration_core::{DocumentPayloads, Payload, PayloadCodec};
use serde::{Deserialize, Serialize};

// Format steps register when linked
use migration_formats as _;

const POSITION: &str = "ui::position";
const PEER: PeerId = PeerId(1);
const USER: UserId = UserId(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Position {
	Absolute([i32; 2]),
	Chain,
	Stack(u32),
	Relative([i32; 2]),
}

fn postcard_payload(value: &impl Serialize) -> Payload {
	Payload::encode(value, PayloadCodec::Postcard).unwrap()
}

fn position_of(registry: &Registry, id: NodeId) -> Option<Position> {
	registry.node_instances.get(&id)?.attributes().get_typed(POSITION)
}

fn set_position(id: NodeId, position: Position) -> RegistryDelta {
	RegistryDelta::ChangeNodeAttribute {
		id,
		delta: AttributeDelta {
			key: POSITION.to_string(),
			value: Some(to_value(&position).unwrap()),
		},
	}
}

/// A layer at (30, 10) reading a node stored at the absolute position (23, 10), each added and placed as one interaction.
fn version_four_session(layer: NodeId, read: NodeId) -> Session {
	let mut session = Session::with_identity(PEER, USER);
	let mut reader = Node::new(ROOT_NETWORK, Implementation::ProtoNode(ResourceId::from(1)), 2);
	reader.inputs_mut()[1].input = NodeInput::Node { id: read, index: 0 };
	let network = RegistryDelta::AddNetwork {
		id: ROOT_NETWORK,
		network: Network::default(),
	};

	let source = Node::new(ROOT_NETWORK, Implementation::ProtoNode(ResourceId::from(2)), 1);
	let interactions = [
		vec![network, RegistryDelta::AddNode { id: read, node: source }, set_position(read, Position::Absolute([23, 10]))],
		vec![RegistryDelta::AddNode { id: layer, node: reader }, set_position(layer, Position::Absolute([30, 10]))],
	];
	for operations in interactions {
		session.stage_computed_ops(operations).unwrap();
		let retired = session.retire_all().unwrap();
		session.mark_interaction_end(*retired.last().unwrap());
	}
	session
}

/// Runs the step into version 5 on `session` saved at version 4, then opens the result the way the load path does.
fn open_at_version_five(session: &Session) -> (Session, Vec<Delta>) {
	let state = SessionState {
		peer_id: PEER,
		user_id: USER,
		head_rev: session.head_rev(),
		clock_counter: session.clock_counter(),
		..Default::default()
	};
	let mut document = DocumentPayloads {
		format_version: 4,
		registry: Some(postcard_payload(session.retired_registry())),
		history: session.history().map(postcard_payload).collect(),
		session: Some(postcard_payload(&state)),
		..Default::default()
	};

	let steps = migration_core::registered_format_migrations();
	let step = steps.iter().find(|step| step.migrates_from() == 4).expect("the step into version 5");
	document.registry = Some(step.migrate_registry(document.registry.as_ref().unwrap()).unwrap());
	document.history = document.history.iter().map(|record| step.migrate_delta(record).unwrap()).collect();
	step.migrate_document(&mut document).unwrap();

	let mut history: Vec<Delta> = document.history.iter().map(|record| record.decode().unwrap()).collect();
	let revs = rehash_deltas(&mut history);
	let state: SessionState = document.session.unwrap().decode().unwrap();
	let head = state.head_rev.map(|rev| revs.get(&rev).copied().unwrap_or(rev));
	let registry: Registry = document.registry.unwrap().decode().unwrap();

	(Session::load(PEER, USER, registry, history.clone(), head, state.redo_stack, 0), history)
}

#[test]
fn a_read_node_becomes_relative_to_its_reader_where_it_was_drawn() {
	let (layer, read) = (NodeId(1), NodeId(2));
	let (migrated, _) = open_at_version_five(&version_four_session(layer, read));

	assert_eq!(position_of(migrated.retired_registry(), read), Some(Position::Relative([-7, 0])));
	assert_eq!(position_of(migrated.retired_registry(), layer), Some(Position::Absolute([30, 10])));
}

#[test]
fn replaying_the_migrated_history_reaches_the_migrated_registry() {
	let (layer, read) = (NodeId(1), NodeId(2));
	let (migrated, history) = open_at_version_five(&version_four_session(layer, read));

	let replayed = Session::replay_from_history(PEER, USER, history, 0).unwrap();
	for id in [layer, read] {
		assert_eq!(position_of(replayed.retired_registry(), id), position_of(migrated.retired_registry(), id));
	}
}

#[test]
fn undo_takes_back_the_conversion_as_one_interaction() {
	let (layer, read) = (NodeId(1), NodeId(2));
	let (mut migrated, _) = open_at_version_five(&version_four_session(layer, read));

	migrated.undo().unwrap();

	assert_eq!(position_of(migrated.retired_registry(), read), Some(Position::Absolute([23, 10])));
	assert!(migrated.retired_registry().node_instances.contains_key(&layer), "the undo should stop at the conversion");
}

