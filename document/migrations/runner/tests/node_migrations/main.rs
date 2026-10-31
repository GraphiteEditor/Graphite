mod examples;
mod registration;
mod rewrite;

use document_graph_storage::{NodeId, NodeInput, ROOT_NETWORK, Registry};
use examples::{AngleBetweenDegrees, Axis, ExtractXyToSplit, OpacityToggles, ROTATION, Tagged, TransformDegrees};
use migration_core::{decl, literal, literal_of};
use migration_runner::RunnerError;
use migration_runner::testing::{Fixture, registered};

fn wire(node: NodeId, index: u32) -> NodeInput {
	NodeInput::Node { id: node, index }
}

fn inputs(registry: &Registry, node: NodeId) -> Vec<NodeInput> {
	registry.node_instances[&node].inputs().iter().map(|slot| slot.input.clone()).collect()
}

/// The nodes `migrated` has that `source` doesn't.
fn added_nodes(source: &Registry, migrated: &Registry) -> Vec<NodeId> {
	let mut added: Vec<NodeId> = migrated.node_instances.keys().filter(|id| !source.node_instances.contains_key(id)).copied().collect();
	added.sort_unstable();
	added
}

#[test]
fn rename_switches_declaration_and_keeps_inputs() {
	let migration = registered(decl!("graphene_core::ops::AddNode"@1));
	let mut fixture = Fixture::new(migration);
	let add = fixture.add(decl!("graphene_core::ops::AddNode"@1), vec![literal(Tagged::Number(1.)), literal(Tagged::Number(2.))]);

	let migrated = fixture.migrate(migration).unwrap();

	assert_eq!(fixture.declaration_of(&migrated, add), Some(decl!("math_nodes::add"@2)));
	assert_eq!(inputs(&migrated, add), inputs(&fixture.registry, add));
}

#[test]
fn opacity_toggles_build_the_target_layout() {
	let mut fixture = Fixture::new(&OpacityToggles);
	let content = fixture.add(decl!("math_nodes::add"@1), vec![]);
	let opacity = fixture.add(decl!("blending_nodes::opacity"@1), vec![wire(content, 0), literal(Tagged::Number(50.))]);

	let migrated = fixture.migrate(&OpacityToggles).unwrap();

	let expected = vec![
		wire(content, 0),
		literal(Tagged::Bool(true)),
		literal(Tagged::Number(50.)),
		literal(Tagged::Bool(false)),
		literal(Tagged::Number(100.)),
	];
	assert_eq!(inputs(&migrated, opacity), expected);
	assert_eq!(fixture.declaration_of(&migrated, opacity), Some(decl!("blending_nodes::opacity"@2)));
}

#[test]
fn transform_converts_a_literal_rotation() {
	let mut fixture = Fixture::new(&TransformDegrees);
	let transform = fixture.add(
		decl!("transform_nodes::transform"@1),
		vec![literal(Tagged::Number(0.)), literal(Tagged::Number(0.)), literal(Tagged::Number(std::f64::consts::PI))],
	);

	let migrated = fixture.migrate(&TransformDegrees).unwrap();

	let rotation = &inputs(&migrated, transform)[ROTATION as usize];
	assert_eq!(literal_of::<Tagged>(rotation), Some(Tagged::Number(180.)));
	assert!(added_nodes(&fixture.registry, &migrated).is_empty());
}

#[test]
fn transform_converts_a_wired_rotation_upstream() {
	let mut fixture = Fixture::new(&TransformDegrees);
	let angle = fixture.add(decl!("math_nodes::add"@1), vec![]);
	let transform = fixture.add(decl!("transform_nodes::transform"@1), vec![literal(Tagged::Number(0.)), literal(Tagged::Number(0.)), wire(angle, 0)]);

	let migrated = fixture.migrate(&TransformDegrees).unwrap();

	let [multiply] = added_nodes(&fixture.registry, &migrated)[..] else {
		panic!("expected one inserted node")
	};
	assert_eq!(fixture.declaration_of(&migrated, multiply), Some(decl!("math_nodes::multiply"@1)));
	assert_eq!(inputs(&migrated, multiply)[0], wire(angle, 0));
	assert_eq!(inputs(&migrated, transform)[ROTATION as usize], wire(multiply, 0));
}

#[test]
fn transform_rejects_a_rotation_of_the_wrong_variant() {
	let mut fixture = Fixture::new(&TransformDegrees);
	fixture.add(
		decl!("transform_nodes::transform"@1),
		vec![literal(Tagged::Number(0.)), literal(Tagged::Number(0.)), literal(Tagged::Bool(true))],
	);

	let error = fixture.migrate(&TransformDegrees).unwrap_err();
	assert!(matches!(error, RunnerError::Migration { .. }), "{error}");
}

/// The inserted node's ID derives from the migration, anchor, and declaration, so a different source state yields the same node.
#[test]
fn inserted_node_ids_are_stable_across_states() {
	let mut fixture = Fixture::new(&TransformDegrees);
	let angle = fixture.add(decl!("math_nodes::add"@1), vec![]);
	let other_angle = fixture.add(decl!("math_nodes::add"@1), vec![]);
	let transform = fixture.add(decl!("transform_nodes::transform"@1), vec![literal(Tagged::Number(0.)), literal(Tagged::Number(0.)), wire(angle, 0)]);
	let first = fixture.migrate(&TransformDegrees).unwrap();

	fixture.registry.node_instances.get_mut(&transform).unwrap().inputs_mut()[ROTATION as usize].input = wire(other_angle, 0);
	let second = fixture.migrate(&TransformDegrees).unwrap();

	assert_eq!(added_nodes(&fixture.registry, &first), added_nodes(&fixture.registry, &second));
}

#[test]
fn angle_between_routes_consumers_through_a_conversion() {
	let mut fixture = Fixture::new(&AngleBetweenDegrees);
	let angle = fixture.add(decl!("vector_nodes::angle_between"@1), vec![literal(Tagged::Number(0.)), literal(Tagged::Number(0.))]);
	let consumer = fixture.add(decl!("math_nodes::add"@1), vec![literal(Tagged::Number(1.)), wire(angle, 0)]);
	fixture.export(wire(angle, 0));

	let migrated = fixture.migrate(&AngleBetweenDegrees).unwrap();

	let [conversion] = added_nodes(&fixture.registry, &migrated)[..] else {
		panic!("expected one inserted node")
	};
	assert_eq!(fixture.declaration_of(&migrated, conversion), Some(decl!("math_nodes::to_radians"@1)));
	assert_eq!(inputs(&migrated, conversion), vec![wire(angle, 0)]);
	assert_eq!(inputs(&migrated, consumer)[1], wire(conversion, 0));
	assert_eq!(migrated.networks[&ROOT_NETWORK].exports[0].target, Some(wire(conversion, 0)));
}

#[test]
fn angle_between_without_consumers_adds_nothing() {
	let mut fixture = Fixture::new(&AngleBetweenDegrees);
	let angle = fixture.add(decl!("vector_nodes::angle_between"@1), vec![]);

	let migrated = fixture.migrate(&AngleBetweenDegrees).unwrap();

	assert!(added_nodes(&fixture.registry, &migrated).is_empty());
	assert_eq!(fixture.declaration_of(&migrated, angle), Some(decl!("vector_nodes::angle_between"@2)));
}

#[test]
fn extract_xy_with_a_literal_axis_picks_an_output() {
	let mut fixture = Fixture::new(&ExtractXyToSplit);
	let extract = fixture.add(decl!("vector_nodes::extract_xy"@1), vec![literal(Tagged::Number(0.)), literal(Tagged::XY(Axis::Y))]);
	let consumer = fixture.add(decl!("math_nodes::add"@1), vec![wire(extract, 0)]);

	let migrated = fixture.migrate(&ExtractXyToSplit).unwrap();

	assert_eq!(inputs(&migrated, consumer), vec![wire(extract, 1)]);
	assert_eq!(inputs(&migrated, extract), vec![literal(Tagged::Number(0.))]);
	assert_eq!(fixture.declaration_of(&migrated, extract), Some(decl!("vector_nodes::split_vec2"@2)));
}

#[test]
fn extract_xy_with_a_wired_axis_selects_at_runtime() {
	let mut fixture = Fixture::new(&ExtractXyToSplit);
	let axis = fixture.add(decl!("math_nodes::add"@1), vec![]);
	let extract = fixture.add(decl!("vector_nodes::extract_xy"@1), vec![literal(Tagged::Number(0.)), wire(axis, 0)]);
	let consumer = fixture.add(decl!("math_nodes::add"@1), vec![wire(extract, 0)]);

	let migrated = fixture.migrate(&ExtractXyToSplit).unwrap();

	let [select] = added_nodes(&fixture.registry, &migrated)[..] else {
		panic!("expected one inserted node")
	};
	assert_eq!(inputs(&migrated, select), vec![wire(axis, 0), wire(extract, 0), wire(extract, 1)]);
	assert_eq!(inputs(&migrated, consumer), vec![wire(select, 0)]);
}

#[test]
fn extract_xy_with_a_wired_axis_and_no_consumers_adds_nothing() {
	let mut fixture = Fixture::new(&ExtractXyToSplit);
	let axis = fixture.add(decl!("math_nodes::add"@1), vec![]);
	let extract = fixture.add(decl!("vector_nodes::extract_xy"@1), vec![literal(Tagged::Number(0.)), wire(axis, 0)]);

	let migrated = fixture.migrate(&ExtractXyToSplit).unwrap();

	assert!(added_nodes(&fixture.registry, &migrated).is_empty());
	assert_eq!(inputs(&migrated, extract), vec![literal(Tagged::Number(0.))]);
}

/// One anchor rewires a slot that another anchor's own migration removes. Both runs read the source state, so neither sees the
/// other's edit, and the runner has to reject the combination rather than pick one.
#[test]
fn chained_anchors_conflict_on_a_shared_slot() {
	let mut fixture = Fixture::new(&ExtractXyToSplit);
	let upstream = fixture.add(decl!("vector_nodes::extract_xy"@1), vec![literal(Tagged::Number(0.)), literal(Tagged::XY(Axis::Y))]);
	fixture.add(decl!("vector_nodes::extract_xy"@1), vec![literal(Tagged::Number(0.)), wire(upstream, 0)]);

	let error = fixture.migrate(&ExtractXyToSplit).unwrap_err();
	assert!(matches!(error, RunnerError::ConflictingSlot(_)), "{error}");
}
