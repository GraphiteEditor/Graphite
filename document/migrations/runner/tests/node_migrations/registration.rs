//! What `#[migration]` and `rename!` generate and register, and how the runner orders and chains the registry.

use crate::examples::{ExtractXyToSplit, OpacityToggles, ROTATION, Tagged, TransformDegrees};
use document_graph_storage::{Delta, Implementation, Network, Node, NodeId, NodeInput, PeerId, ROOT_NETWORK, RegistryDelta, Session, UserId};
use migration_core::{AnchorContext, Declaration, DeclarationResolver, Edits, MigrationError, NodeMigration, Region, decl, literal, registered_migrations};
use migration_runner::testing::replay;
use migration_runner::{DeclarationTable, RunnerError, migrate_history, order_migrations};

#[test]
fn every_migration_registers_once() {
	let mut sources: Vec<Declaration> = registered_migrations().iter().map(|migration| migration.source()).collect();
	sources.sort();

	let mut expected = vec![
		decl!("blending_nodes::opacity"@1),
		decl!("graphene_core::ops::AddNode"@1),
		decl!("math_nodes::multiply"@1),
		decl!("transform_nodes::transform"@1),
		decl!("vector_nodes::angle_between"@1),
		decl!("vector_nodes::extract_xy"@1),
		decl!("vector_nodes::split_vec2"@2),
	];
	expected.sort();
	assert_eq!(sources, expected);
}

#[test]
fn generated_migrations_describe_their_parameters() {
	assert_eq!(TransformDegrees.target(), decl!("transform_nodes::transform"@2));
	assert_eq!(TransformDegrees.pins(), &[decl!("math_nodes::multiply"@1)]);
	assert_eq!(TransformDegrees.footprint(), Region::ANCHOR.union(Region::INPUTS).union(Region::UPSTREAM));

	assert_eq!(ExtractXyToSplit.target(), decl!("vector_nodes::split_vec2"@2));
	assert_eq!(ExtractXyToSplit.pins(), &[decl!("logic_nodes::select_axis"@1)]);

	assert!(OpacityToggles.pins().is_empty());
	assert_eq!(OpacityToggles.footprint(), Region::ANCHOR.union(Region::INPUTS));
}

fn position(ordered: &[&'static dyn NodeMigration], source: Declaration) -> usize {
	ordered.iter().position(|migration| migration.source() == source).unwrap_or_else(|| panic!("{source} is not ordered"))
}

#[test]
fn order_follows_chains_and_pins() {
	let ordered = order_migrations(&registered_migrations()).unwrap();

	// Transform inserts `multiply@1`, so it must run before `multiply@1` is renamed
	assert!(position(&ordered, decl!("transform_nodes::transform"@1)) < position(&ordered, decl!("math_nodes::multiply"@1)));
	// Extract XY produces `split_vec2@2`, which the next step in its lineage consumes
	assert!(position(&ordered, decl!("vector_nodes::extract_xy"@1)) < position(&ordered, decl!("vector_nodes::split_vec2"@2)));
}

#[test]
fn order_ignores_registration_order() {
	let mut reversed = registered_migrations();
	reversed.reverse();

	let sources = |ordered: Vec<&'static dyn NodeMigration>| ordered.iter().map(|migration| migration.source()).collect::<Vec<_>>();
	assert_eq!(sources(order_migrations(&registered_migrations()).unwrap()), sources(order_migrations(&reversed).unwrap()));
}

/// A migration that changes nothing, for ordering cases the examples don't cover.
struct Step {
	source: Declaration,
	target: Declaration,
	pins: &'static [Declaration],
}

impl NodeMigration for Step {
	fn source(&self) -> Declaration {
		self.source
	}
	fn target(&self) -> Declaration {
		self.target
	}
	fn pins(&self) -> &'static [Declaration] {
		self.pins
	}
	fn footprint(&self) -> Region {
		Region::ANCHOR
	}
	fn run(&self, _context: &AnchorContext) -> Result<Edits, MigrationError> {
		Ok(Edits::default())
	}
}

#[test]
fn pins_that_depend_on_each_other_are_a_cycle() {
	// Each inserts the other's target, so each must run after the other
	static FIRST: Step = Step {
		source: decl!("a"@1),
		target: decl!("a"@2),
		pins: &[decl!("b"@2)],
	};
	static SECOND: Step = Step {
		source: decl!("b"@1),
		target: decl!("b"@2),
		pins: &[decl!("a"@2)],
	};

	let error = order_migrations(&[&FIRST, &SECOND]).err().expect("a cycle can't be ordered");
	assert!(matches!(error, RunnerError::OrderCycle(_)), "{error}");
}

#[test]
fn order_rejects_two_migrations_from_one_declaration() {
	static FIRST: Step = Step {
		source: decl!("a"@1),
		target: decl!("a"@2),
		pins: &[],
	};
	static SECOND: Step = Step {
		source: decl!("a"@1),
		target: decl!("b"@2),
		pins: &[],
	};

	let error = order_migrations(&[&FIRST, &SECOND]).err().expect("two migrations from one declaration are ambiguous");
	assert!(matches!(error, RunnerError::DuplicateMigration(_)), "{error}");
}

#[test]
fn order_rejects_a_step_skipping_versions() {
	static SKIP: Step = Step {
		source: decl!("a"@1),
		target: decl!("a"@3),
		pins: &[],
	};

	let error = order_migrations(&[&SKIP]).err().expect("a migration steps exactly one version");
	assert!(matches!(error, RunnerError::InvalidStep { .. }), "{error}");
}

/// Every registered migration over one history: the transform inserts `multiply@1`, which the later rename turns into `product@2`.
#[test]
fn migrate_history_chains_every_pending_migration() {
	let mut table = DeclarationTable::new([decl!("math_nodes::add"@1)]);
	for migration in registered_migrations() {
		table.register_migration(migration);
	}

	let mut session = Session::with_identity(PeerId(1), UserId(1));
	let mut commit = |op: RegistryDelta| {
		session.stage_computed_ops(vec![op]).unwrap();
		session.retire_all().unwrap();
	};
	let node = |declaration: Declaration, inputs: Vec<NodeInput>| {
		let mut node = Node::new(ROOT_NETWORK, Implementation::ProtoNode(DeclarationTable::resource_id(declaration)), inputs.len());
		for (slot, input) in node.inputs_mut().iter_mut().zip(inputs) {
			slot.input = input;
		}
		node
	};

	let (angle, transform, rename_only) = (NodeId(1), NodeId(2), NodeId(3));
	commit(RegistryDelta::AddNetwork {
		id: ROOT_NETWORK,
		network: Network::default(),
	});
	commit(RegistryDelta::AddNode {
		id: angle,
		node: node(decl!("math_nodes::add"@1), vec![]),
	});
	commit(RegistryDelta::AddNode {
		id: transform,
		node: node(
			decl!("transform_nodes::transform"@1),
			vec![literal(Tagged::Number(0.)), literal(Tagged::Number(0.)), NodeInput::Node { id: angle, index: 0 }],
		),
	});
	commit(RegistryDelta::AddNode {
		id: rename_only,
		node: node(decl!("math_nodes::multiply"@1), vec![]),
	});

	let history: Vec<Delta> = session.history().cloned().collect();
	let migrated = migrate_history(&registered_migrations(), history.clone(), &table).unwrap();
	let registry = replay(&migrated.deltas);

	let declaration_of = |id: NodeId| match registry.node_instances[&id].implementation() {
		Implementation::ProtoNode(resource) => table.declaration(*resource),
		Implementation::Network(_) => None,
	};
	assert_eq!(declaration_of(transform), Some(decl!("transform_nodes::transform"@2)));
	assert_eq!(declaration_of(rename_only), Some(decl!("math_nodes::product"@2)));

	let NodeInput::Node { id: converter, .. } = registry.node_instances[&transform].inputs()[ROTATION as usize].input else {
		panic!("rotation stays wired")
	};
	assert_eq!(declaration_of(converter), Some(decl!("math_nodes::product"@2)));

	// Every original delta maps into the rewritten history
	assert!(history.iter().all(|delta| migrated.revs[&delta.id].is_some()));
}
