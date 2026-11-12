//! Network definitions as migration sources: a built-in subgraph collapsing into a proto node, and a version step of one.

use crate::{Tagged, inputs, wire};
use document_graph_storage::attributes::attr::node::ui::REFERENCE;
use document_graph_storage::{AttributeDelta, Implementation, NetworkId, NetworkReference, NodeId, NodeInput, RegistryDelta};
use migration_core::{decl, literal, rename};
use migration_runner::testing::{Fixture, SourceContract, check_commutation, check_undo, registered};
use migration_runner::{DeclarationTable, references};

// The Merge subgraph became the `merge` proto node, which takes the instance's inputs as they are. Inner edits are dropped with the subgraph
rename!(network "Merge"@1 => "graphic_nodes::merge"@2);

rename!(network "Artboard"@1 => @2);

const MERGE: migration_core::Declaration = decl!(network "Merge"@1);

/// A Merge instance in the root network whose subgraph holds the frozen shape, plus `extra_inner_nodes` the user added.
fn merge_fixture(extra_inner_nodes: usize) -> (Fixture, NodeId, NetworkId, NodeId) {
	let mut fixture = Fixture::new(registered(MERGE));
	let (merge, network) = fixture.add_network(MERGE, vec![literal(Tagged::Number(1.)), literal(Tagged::Number(2.))]);

	let into_group = fixture.add_in(network, decl!("graphic_nodes::into_group"@1), vec![NodeInput::Import { index: 0 }]);
	fixture.export_from(network, wire(into_group, 0));
	for _ in 0..extra_inner_nodes {
		fixture.add_in(network, decl!("graphic_nodes::into_group"@1), vec![NodeInput::Import { index: 1 }]);
	}

	(fixture, merge, network, into_group)
}

#[test]
fn network_anchor_collapses_into_proto_node() {
	let (fixture, merge, network, into_group) = merge_fixture(0);

	let migrated = fixture.migrate(registered(MERGE)).unwrap();

	assert_eq!(fixture.declaration_of(&migrated, merge), Some(decl!("graphic_nodes::merge"@2)));
	assert_eq!(inputs(&migrated, merge), inputs(&fixture.registry, merge));
	assert!(!migrated.networks.contains_key(&network));
	assert!(!migrated.node_instances.contains_key(&into_group));
}

#[test]
fn edited_subgraph_collapses_too() {
	let (fixture, merge, network, _) = merge_fixture(1);

	let migrated = fixture.migrate(registered(MERGE)).unwrap();

	assert_eq!(fixture.declaration_of(&migrated, merge), Some(decl!("graphic_nodes::merge"@2)));
	assert!(!migrated.networks.contains_key(&network));
	assert!(migrated.node_instances.values().all(|node| node.network() != network));
}

#[test]
fn network_of_another_version_is_no_anchor() {
	let mut fixture = Fixture::new(registered(MERGE));
	fixture.add_network(decl!(network "Merge"@2), Vec::new());

	let migrated = fixture.migrate(registered(MERGE)).unwrap();

	assert_eq!(migrated, fixture.registry);
}

#[test]
fn network_version_step_names_the_new_version() {
	let migration = registered(decl!(network "Artboard"@1));
	let mut fixture = Fixture::new(migration);
	let (artboard, network) = fixture.add_network(decl!(network "Artboard"@1), vec![literal(Tagged::Number(1.))]);

	let migrated = fixture.migrate(migration).unwrap();

	assert_eq!(NetworkReference::read(&migrated.networks[&network].attributes), Some(decl!(network "Artboard"@2).network_reference()));
	assert_eq!(migrated.node_instances[&artboard].implementation(), &Implementation::Network(network));
}

#[test]
fn reference_write_is_pending_for_network_definition() {
	let delta = RegistryDelta::ChangeNetworkAttribute {
		id: NetworkId(1),
		delta: AttributeDelta {
			key: REFERENCE.to_string(),
			value: Some(MERGE.network_reference().to_value()),
		},
	};

	assert!(references(&delta, MERGE, &DeclarationTable::default()));
	assert!(!references(&delta, decl!(network "Merge"@2), &DeclarationTable::default()));
	assert!(!references(&delta, decl!(network "Artboard"@1), &DeclarationTable::default()));
}

fn numbers(input_count: usize) -> SourceContract {
	SourceContract::new(input_count, |random, _| literal(Tagged::Number(random.below(10) as f64)))
}

#[test]
fn collapse_commutes() {
	check_commutation(registered(MERGE), &numbers(2));
	check_undo(registered(MERGE), &numbers(2));
}

#[test]
fn network_version_step_commutes() {
	check_commutation(registered(decl!(network "Artboard"@1)), &numbers(1));
	check_undo(registered(decl!(network "Artboard"@1)), &numbers(1));
}
