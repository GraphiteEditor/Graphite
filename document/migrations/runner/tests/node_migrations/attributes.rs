//! Attribute writes on the anchor and the nodes it adds.

use document_graph_storage::attr::node::ui::{DISPLAY_NAME, POSITION};
use document_graph_storage::{NodeId, NodeInput, Position, Value};
use migration_core::{Attributes, Downstream, NewNode, Node, decl, migration};
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

fn set_name(fixture: &mut Fixture, node: NodeId, name: &str) {
	let name = Value::Str(name.to_string());
	fixture.registry.node_instances.get_mut(&node).unwrap().attributes_mut().set(DISPLAY_NAME, name, Default::default());
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
fn writing_another_nodes_attributes_fails() {
	let migration = &Outside;
	let mut fixture = Fixture::new(migration);
	let other = fixture.add(decl!("math_nodes::add"@1), vec![]);
	fixture.add(decl!("attributes::outside"@1), vec![wire(other)]);

	let error = fixture.migrate(migration).unwrap_err();
	assert!(matches!(error, RunnerError::AttributesOutsideRegion { node, .. } if node == other), "{error}");
}
