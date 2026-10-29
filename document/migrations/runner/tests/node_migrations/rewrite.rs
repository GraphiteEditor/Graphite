//! The commutation property on generated linear histories: `replay(rewrite(H)) == M(replay(H))`.

use crate::examples::{AngleBetweenDegrees, Axis, ExtractXyToSplit, OpacityToggles, Tagged, TransformDegrees};
use crate::registered;
use document_graph_storage::{Delta, Implementation, Network, Node, NodeId, NodeInput, PeerId, ROOT_NETWORK, Registry, RegistryDelta, ResourceId, Session, UserId, apply_retired_op};
use migration_core::{Declaration, NodeMigration, decl, literal};
use migration_runner::{DeclarationTable, apply_migration, rewrite_history};

/// Small deterministic generator, so a failing seed reproduces.
struct Random(u64);

impl Random {
	fn next(&mut self) -> u64 {
		// Xorshift64
		self.0 ^= self.0 << 13;
		self.0 ^= self.0 >> 7;
		self.0 ^= self.0 << 17;
		self.0
	}

	fn below(&mut self, bound: usize) -> usize {
		(self.next() % bound as u64) as usize
	}

	fn pick<T: Copy>(&mut self, items: &[T]) -> T {
		items[self.below(items.len())]
	}
}

/// The declarations a generated graph uses: the migration's source plus plain consumers, with how many inputs each has.
fn palette(migration: &dyn NodeMigration) -> Vec<(Declaration, usize)> {
	let source_inputs = match migration.source().identifier {
		"transform_nodes::transform" => 3,
		_ => 2,
	};
	vec![(migration.source(), source_inputs), (decl!("math_nodes::add"@1), 2), (decl!("math_nodes::add"@1), 2)]
}

/// A literal the declaration's contract allows in `slot`.
fn literal_for(random: &mut Random, declaration: Declaration, slot: usize) -> NodeInput {
	match (declaration.identifier, slot) {
		("vector_nodes::extract_xy", 1) => literal(Tagged::XY(random.pick(&[Axis::X, Axis::Y]))),
		_ => literal(Tagged::Number(random.below(10) as f64)),
	}
}

struct Generator<'a> {
	random: Random,
	session: Session,
	table: &'a mut DeclarationTable,
	palette: Vec<(Declaration, usize)>,
}

impl Generator<'_> {
	fn commit(&mut self, op: RegistryDelta) {
		self.session.stage_computed_ops(vec![op]).expect("generated ops are valid");
		let revs = self.session.retire_all().expect("staged ops retire");
		if let Some(last) = revs.last() {
			self.session.mark_interaction_end(*last);
		}
	}

	fn live_nodes(&self) -> Vec<NodeId> {
		let mut nodes: Vec<NodeId> = self.session.registry().node_instances.keys().copied().collect();
		nodes.sort_unstable();
		nodes
	}

	fn declaration_of(&self, node: &Node) -> Option<Declaration> {
		let Implementation::ProtoNode(resource) = node.implementation() else { return None };
		self.palette
			.iter()
			.map(|(declaration, _)| *declaration)
			.find(|declaration| DeclarationTable::resource_id(*declaration) == *resource)
	}

	/// A literal or a wire to another live node.
	fn input_for(&mut self, declaration: Declaration, slot: usize, exclude: Option<NodeId>) -> NodeInput {
		// The XY axis only accepts what produces an axis, which no extract XY output does
		let axis_slot = declaration.identifier == "vector_nodes::extract_xy" && slot == 1;
		let registry = self.session.registry();
		let produces_axis = |node: &NodeId| !axis_slot || self.declaration_of(&registry.node_instances[node]) != Some(declaration);

		let candidates: Vec<NodeId> = self.live_nodes().into_iter().filter(|node| Some(*node) != exclude && produces_axis(node)).collect();

		if !candidates.is_empty() && self.random.below(2) == 0 {
			let node = self.random.pick(&candidates);
			return NodeInput::Node { id: node, index: 0 };
		}
		literal_for(&mut self.random, declaration, slot)
	}

	fn step(&mut self) {
		let nodes = self.live_nodes();

		match self.random.below(10) {
			0..=3 => {
				let (declaration, input_count) = self.random.pick(&self.palette);
				let resource = self.table.register(declaration);

				let mut node = Node::new(ROOT_NETWORK, Implementation::ProtoNode(resource), input_count);
				for slot in 0..input_count {
					node.inputs_mut()[slot].input = self.input_for(declaration, slot, None);
				}
				let id = self.session.next_node_id();
				self.commit(RegistryDelta::AddNode { id, node });
			}
			4..=6 if !nodes.is_empty() => {
				let id = self.random.pick(&nodes);
				let node = self.session.registry().node_instances[&id].clone();
				let Some(declaration) = self.declaration_of(&node) else { return };

				let index = self.random.below(node.inputs().len());
				let new_input = self.input_for(declaration, index, Some(id));
				if node.inputs()[index].input == new_input {
					return;
				}
				self.commit(RegistryDelta::ChangeNodeInput { id, index: index as u32, new_input });
			}
			7 if !nodes.is_empty() => {
				let id = self.random.pick(&nodes);
				let snapshot = self.session.registry().node_instances[&id].clone();
				self.commit(RegistryDelta::RemoveNode { id, snapshot });
			}
			8 | 9 if !nodes.is_empty() => {
				let id = self.random.pick(&nodes);
				let index = self.random.below(2) as u32;
				let export = Some(NodeInput::Node { id, index: 0 });

				let exports = &self.session.registry().networks[&ROOT_NETWORK].exports;
				if exports.get(index as usize).is_some_and(|slot| slot.target == export) {
					return;
				}
				self.commit(RegistryDelta::SetNetworkExport { id: ROOT_NETWORK, index, export });
			}
			_ => {}
		}
	}
}

/// A random linear history over `migration`'s source declaration, and the table resolving it.
fn generate(migration: &dyn NodeMigration, seed: u64, steps: usize) -> (Session, DeclarationTable) {
	let mut table = DeclarationTable::default();
	table.register_migration(migration);

	let mut generator = Generator {
		random: Random(seed.max(1)),
		session: Session::with_identity(PeerId(1), UserId(1)),
		table: &mut table,
		palette: palette(migration),
	};
	generator.commit(RegistryDelta::AddNetwork {
		id: ROOT_NETWORK,
		network: Network::default(),
	});
	for _ in 0..steps {
		generator.step();
	}

	(generator.session, table)
}

pub fn replay(deltas: &[Delta]) -> Registry {
	let mut registry = Registry::default();
	for delta in deltas {
		apply_retired_op(&mut registry, delta.kind.clone(), delta.timestamp).expect("history replays");
	}
	registry
}

/// Whether `kind` mentions a node implementing `resource`, so stored history would still hold the old shape.
fn references(kind: &RegistryDelta, resource: ResourceId) -> bool {
	let implements = |node: &Node| *node.implementation() == Implementation::ProtoNode(resource);
	match kind {
		RegistryDelta::AddNode { node, .. } => implements(node),
		RegistryDelta::RemoveNode { snapshot, .. } => implements(snapshot),
		RegistryDelta::SetNodeImplementation { implementation, .. } => *implementation == Implementation::ProtoNode(resource),
		_ => false,
	}
}

fn check_commutation(migration: &dyn NodeMigration, seeds: std::ops::Range<u64>, steps: usize) {
	for seed in seeds {
		let (session, table) = generate(migration, seed, steps);
		let history: Vec<Delta> = session.history().cloned().collect();

		let expected = apply_migration(migration, &replay(&history), &table).unwrap_or_else(|error| panic!("seed {seed}: M failed: {error}"));
		let rewritten = rewrite_history(migration, &history, &table).unwrap_or_else(|error| panic!("seed {seed}: rewrite failed: {error}"));

		assert!(
			replay(&rewritten.deltas).value_equal(&expected),
			"seed {seed}: replaying the rewritten history differs from M of the original"
		);

		let source = DeclarationTable::resource_id(migration.source());
		let stale = rewritten.deltas.iter().find(|delta| references(&delta.kind, source));
		assert!(stale.is_none(), "seed {seed}: rewritten history still holds the source shape: {stale:?}");

		for (index, delta) in rewritten.deltas.iter().enumerate() {
			assert!(delta.has_valid_id(), "seed {seed}: rewritten delta {index} has a stale rev");
			let expected_parent = index.checked_sub(1).map(|previous| rewritten.deltas[previous].id);
			assert_eq!(delta.parent, expected_parent, "seed {seed}: rewritten delta {index} is not chained onto its predecessor");
		}
	}
}

#[test]
fn rename_commutes() {
	check_commutation(registered(decl!("graphene_core::ops::AddNode"@1)), 1..50, 40);
}

#[test]
fn opacity_toggles_commute() {
	check_commutation(&OpacityToggles, 1..50, 40);
}

#[test]
fn transform_degrees_commutes() {
	check_commutation(&TransformDegrees, 1..50, 40);
}

#[test]
fn angle_between_degrees_commutes() {
	check_commutation(&AngleBetweenDegrees, 1..50, 40);
}

#[test]
fn extract_xy_to_split_commutes() {
	check_commutation(&ExtractXyToSplit, 1..50, 40);
}

/// Undo walks the rewritten deltas' reverse priors, so undoing in rewritten history must land on `M` of the undone original.
#[test]
fn undo_through_rewritten_history_matches_migrated_undo() {
	let migration = TransformDegrees;

	for seed in 1..20 {
		let (session, table) = generate(&migration, seed, 30);
		let history: Vec<Delta> = session.history().cloned().collect();
		let rewritten = rewrite_history(&migration, &history, &table).unwrap();

		let mut original = Session::replay_from_history(PeerId(1), UserId(1), history, 0).unwrap();
		let mut migrated = Session::replay_from_history(PeerId(1), UserId(1), rewritten.deltas, 0).unwrap();

		for undo in 0..5 {
			if !original.can_undo() {
				break;
			}
			original.undo().unwrap();
			migrated.undo().unwrap_or_else(|error| panic!("seed {seed}, undo {undo}: {error}"));

			let expected = apply_migration(&migration, original.registry(), &table).unwrap();
			assert!(migrated.registry().value_equal(&expected), "seed {seed}, undo {undo}: undone states diverge");
		}
	}
}
