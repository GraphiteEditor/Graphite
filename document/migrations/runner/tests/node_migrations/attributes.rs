//! Attribute writes on the anchor and the nodes it adds, and the positions added nodes get by default.

use document_graph_storage::attr::node::ui::{DISPLAY_NAME, POSITION};
use document_graph_storage::{NodeId, NodeInput, Position, Value, to_value};
use migration_core::{Attributes, CHAIN_WIDTH, Downstream, FEEDER_ROW_HEIGHT, Inputs, NewNode, Node, Upstream, decl, migration};
use migration_runner::RunnerError;
use migration_runner::testing::Fixture;

/// Clears the anchor's old default name and places a node it adds.
#[migration("attributes::renamed"@1 => @2)]
pub fn renamed(node: &Node, downstream: &mut Downstream, attributes: &mut Attributes) {
	if node.attributes().get_typed::<String>(DISPLAY_NAME).as_deref() == Some("Old name") {
		attributes.remove(node.id(), DISPLAY_NAME);
	}

	let added = downstream.add_node(NewNode {
		declaration: decl!("math_nodes::add"@1),
		inputs: vec![],
	});
	attributes.set(added, POSITION, &Position::Absolute([3, 4]));
}

/// Renames the node its first input reads, which it neither is nor adds.
#[migration("attributes::outside"@1 => @2)]
pub fn outside(node: &Node, attributes: &mut Attributes) {
	if let Some(NodeInput::Node { id, .. }) = node.inputs().first().map(|slot| &slot.input) {
		attributes.set(*id, DISPLAY_NAME, &"Renamed");
	}
}

/// Adds two nodes leading up to the anchor's first input, and one feeding the nearer of them from below.
#[migration("attributes::placed"@1 => @2)]
pub fn placed(node: &Node, inputs: &mut Inputs, upstream: &mut Upstream) {
	let add = decl!("math_nodes::add"@1);
	let source = node.inputs().first().map_or(NodeInput::Import { index: 0 }, |slot| slot.input.clone());

	let far = upstream.add_node_keyed(
		NewNode {
			declaration: add,
			inputs: vec![source],
		},
		"far",
	);
	let below = upstream.add_node_keyed(NewNode { declaration: add, inputs: vec![] }, "below");
	let near = upstream.add_node_keyed(
		NewNode {
			declaration: add,
			inputs: vec![wire(far), wire(below)],
		},
		"near",
	);
	inputs.set(0, wire(near));
}

fn set_name(fixture: &mut Fixture, node: NodeId, name: &str) {
	let name = Value::Str(name.to_string());
	fixture.registry.node_instances.get_mut(&node).unwrap().attributes_mut().set(DISPLAY_NAME, name, Default::default());
}

/// The positions of the far, near, and below nodes `Placed` adds for an anchor stored at `position`.
fn placed_around(position: Position) -> [Option<Position>; 3] {
	let migration = &Placed;
	let mut fixture = Fixture::new(migration);
	let anchor = fixture.add(decl!("attributes::placed"@1), vec![NodeInput::Import { index: 0 }]);
	let position = to_value(&position).unwrap();
	fixture.registry.node_instances.get_mut(&anchor).unwrap().attributes_mut().set(POSITION, position, Default::default());

	let migrated = fixture.migrate(migration).unwrap();
	let position_of = |id: NodeId| migrated.node_instances[&id].attributes().get_typed::<Position>(POSITION);

	let NodeInput::Node { id: near, .. } = fixture.inputs(&migrated, anchor)[0] else {
		panic!("the anchor reads the near node")
	};
	let [NodeInput::Node { id: far, .. }, NodeInput::Node { id: below, .. }] = fixture.inputs(&migrated, near)[..] else {
		panic!("the near node reads the far and below nodes")
	};
	[position_of(far), position_of(near), position_of(below)]
}

fn wire(id: NodeId) -> NodeInput {
	NodeInput::Node { id, index: 0 }
}

#[test]
fn a_run_writes_its_anchor_and_the_nodes_it_adds() {
	let migration = &Renamed;
	let mut fixture = Fixture::new(migration);
	let anchor = fixture.add(decl!("attributes::renamed"@1), vec![]);
	set_name(&mut fixture, anchor, "Old name");
	fixture.export(wire(anchor));

	let migrated = fixture.migrate(migration).unwrap();

	assert!(migrated.node_instances[&anchor].attributes().get(DISPLAY_NAME).is_none());
	let added = migrated.node_instances.iter().find(|(id, _)| **id != anchor).map(|(_, node)| node).expect("the run adds a node");
	assert_eq!(added.attributes().get_typed::<Position>(POSITION), Some(Position::Absolute([3, 4])));
}

#[test]
fn a_name_given_by_the_user_is_kept() {
	let migration = &Renamed;
	let mut fixture = Fixture::new(migration);
	let anchor = fixture.add(decl!("attributes::renamed"@1), vec![]);
	set_name(&mut fixture, anchor, "Mine");

	let migrated = fixture.migrate(migration).unwrap();

	assert_eq!(migrated.node_instances[&anchor].attributes().get_typed::<String>(DISPLAY_NAME).as_deref(), Some("Mine"));
}

#[test]
fn added_nodes_lead_up_to_a_placed_anchor() {
	let [far, near, below] = placed_around(Position::Absolute([20, 10]));

	assert_eq!([far, near], [Some(Position::Relative([-CHAIN_WIDTH, 0])); 2]);
	assert_eq!(below, Some(Position::Relative([-CHAIN_WIDTH, FEEDER_ROW_HEIGHT])));
}

#[test]
fn added_nodes_lead_up_to_a_chained_anchor() {
	let [far, near, below] = placed_around(Position::Relative([-CHAIN_WIDTH, 0]));

	assert_eq!([far, near], [Some(Position::Relative([-CHAIN_WIDTH, 0])); 2]);
	assert_eq!(below, Some(Position::Relative([-CHAIN_WIDTH, FEEDER_ROW_HEIGHT])));
}

#[test]
fn writing_another_nodes_attributes_fails() {
	let migration = &Outside;
	let mut fixture = Fixture::new(migration);
	let other = fixture.add(decl!("math_nodes::add"@1), vec![]);
	fixture.add(decl!("attributes::outside"@1), vec![wire(other)]);

	let error = fixture.migrate(migration).unwrap_err();
	assert!(matches!(error, RunnerError::AttributesOutsideRegion { node, .. } if node == other), "{error}");
}
