//! What every migration's tests need: fixtures for one anchor, and the property that rewriting a generated history gives `M` of it at every step.
//! Panics on failure, since it only runs in tests.

use crate::node::{apply_migration, rewrite_history};
use crate::{DeclarationTable, RunnerError, references, registry_at};
use document_graph_storage::{Delta, ExportSlot, Implementation, Network, Node, NodeId, NodeInput, PeerId, ROOT_NETWORK, Registry, RegistryDelta, Session, UserId, apply_retired_op};
use migration_core::{Declaration, DeclarationResolver, NodeMigration, decl, literal, registered_migrations};

/// A root network built node by node, for running one migration on.
pub struct Fixture {
	pub registry: Registry,
	table: DeclarationTable,
	next_id: u64,
}

impl Fixture {
	pub fn new(migration: &dyn NodeMigration) -> Self {
		let mut registry = Registry::default();
		registry.networks.insert(ROOT_NETWORK, Network::default());

		let mut table = DeclarationTable::default();
		table.register_migration(migration);

		Self { registry, table, next_id: 0 }
	}

	pub fn add(&mut self, declaration: Declaration, inputs: Vec<NodeInput>) -> NodeId {
		self.next_id += 1;
		let id = NodeId(self.next_id);

		let resource = self.table.register(declaration);
		let mut node = Node::new(ROOT_NETWORK, Implementation::ProtoNode(resource), inputs.len());
		for (slot, input) in node.inputs_mut().iter_mut().zip(inputs) {
			slot.input = input;
		}
		self.registry.node_instances.insert(id, node);

		id
	}

	pub fn export(&mut self, input: NodeInput) {
		let network = self.registry.networks.get_mut(&ROOT_NETWORK).expect("the fixture creates the root network");
		network.exports.push(ExportSlot {
			target: Some(input),
			..Default::default()
		});
	}

	pub fn migrate(&self, migration: &dyn NodeMigration) -> Result<Registry, RunnerError> {
		apply_migration(migration, &self.registry, &self.table)
	}

	pub fn declaration_of(&self, registry: &Registry, node: NodeId) -> Option<Declaration> {
		let Implementation::ProtoNode(resource) = registry.node_instances.get(&node)?.implementation() else {
			return None;
		};
		self.table.declaration(*resource)
	}

	/// The input slots of `node` in `registry`.
	pub fn inputs(&self, registry: &Registry, node: NodeId) -> Vec<NodeInput> {
		registry.node_instances[&node].inputs().iter().map(|slot| slot.input.clone()).collect()
	}
}

/// The registered migration starting from `source`.
pub fn registered(source: Declaration) -> &'static dyn NodeMigration {
	let migration = registered_migrations().into_iter().find(|migration| migration.source() == source);
	migration.unwrap_or_else(|| panic!("no migration from {source} is registered"))
}

/// Small deterministic generator, so a failing seed reproduces.
pub struct Random(u64);

impl Random {
	fn next(&mut self) -> u64 {
		// Xorshift64
		self.0 ^= self.0 << 13;
		self.0 ^= self.0 >> 7;
		self.0 ^= self.0 << 17;
		self.0
	}

	pub fn below(&mut self, bound: usize) -> usize {
		(self.next() % bound as u64) as usize
	}

	pub fn pick<T: Copy>(&mut self, items: &[T]) -> T {
		items[self.below(items.len())]
	}
}

/// What the source declaration allows, so generated histories only hold states a real document could.
pub struct SourceContract {
	pub input_count: usize,
	/// A literal the declaration allows in a slot.
	pub literal: fn(&mut Random, usize) -> NodeInput,
	/// Whether a slot may read an output of another node of the source declaration.
	pub reads_own_output: fn(usize) -> bool,
}

impl SourceContract {
	pub fn new(input_count: usize, literal: fn(&mut Random, usize) -> NodeInput) -> Self {
		Self {
			input_count,
			literal,
			reads_own_output: |_| true,
		}
	}
}

/// A node generated histories wire to and from the anchors, whose literals no migration reads.
const CONSUMER: Declaration = decl!("migration_testing::consumer"@1);
const CONSUMER_INPUTS: usize = 2;

struct Generator<'a> {
	random: Random,
	session: Session,
	table: &'a mut DeclarationTable,
	source: Declaration,
	contract: &'a SourceContract,
	/// How many interactions undo can step back through, leaving the root network in place.
	undoable: usize,
}

impl Generator<'_> {
	fn commit(&mut self, op: RegistryDelta) {
		self.session.stage_computed_ops(vec![op]).expect("generated ops are valid");
		let revs = self.session.retire_all().expect("staged ops retire");
		if let Some(last) = revs.last() {
			self.session.mark_interaction_end(*last);
		}
		self.undoable += 1;
	}

	fn live_nodes(&self) -> Vec<NodeId> {
		let mut nodes: Vec<NodeId> = self.session.registry().node_instances.keys().copied().collect();
		nodes.sort_unstable();
		nodes
	}

	fn declaration_of(&self, node: &Node) -> Option<Declaration> {
		let Implementation::ProtoNode(resource) = node.implementation() else { return None };
		[self.source, CONSUMER].into_iter().find(|declaration| DeclarationTable::resource_id(*declaration) == *resource)
	}

	/// A literal or a wire to another live node.
	fn input_for(&mut self, declaration: Declaration, slot: usize, exclude: Option<NodeId>) -> NodeInput {
		let is_source = declaration == self.source;
		let reads_own_output = !is_source || (self.contract.reads_own_output)(slot);
		let registry = self.session.registry();
		let allowed = |node: &NodeId| reads_own_output || self.declaration_of(&registry.node_instances[node]) != Some(self.source);

		let candidates: Vec<NodeId> = self.live_nodes().into_iter().filter(|node| Some(*node) != exclude && allowed(node)).collect();

		if !candidates.is_empty() && self.random.below(2) == 0 {
			let node = self.random.pick(&candidates);
			return NodeInput::Node { id: node, index: 0 };
		}

		if is_source {
			(self.contract.literal)(&mut self.random, slot)
		} else {
			literal(self.random.below(10) as f64)
		}
	}

	fn step(&mut self) {
		let nodes = self.live_nodes();

		match self.random.below(11) {
			0..=3 => {
				let (declaration, input_count) = if self.random.below(3) == 0 {
					(self.source, self.contract.input_count)
				} else {
					(CONSUMER, CONSUMER_INPUTS)
				};
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
				if node.inputs().is_empty() {
					return;
				}

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
			// The next edit branches off here, which is how undo leaves abandoned deltas in history
			10 if self.undoable > 0 => {
				self.session.undo().expect("an interaction is left to undo");
				self.undoable -= 1;
			}
			_ => {}
		}
	}
}

/// A random linear history over `migration`'s source declaration, and the table resolving it.
fn generate(migration: &dyn NodeMigration, contract: &SourceContract, seed: u64, steps: usize) -> (Session, DeclarationTable) {
	let mut table = DeclarationTable::default();
	table.register_migration(migration);

	let mut generator = Generator {
		random: Random(seed.max(1)),
		session: Session::with_identity(PeerId(1), UserId(1)),
		table: &mut table,
		source: migration.source(),
		contract,
		undoable: 0,
	};
	generator.commit(RegistryDelta::AddNetwork {
		id: ROOT_NETWORK,
		network: Network::default(),
	});
	generator.undoable = 0;
	for _ in 0..steps {
		generator.step();
	}

	(generator.session, table)
}

/// The registry `deltas` build from nothing.
pub fn replay(deltas: &[Delta]) -> Registry {
	let mut registry = Registry::default();
	for delta in deltas {
		apply_retired_op(&mut registry, delta.kind.clone(), delta.timestamp).expect("history replays");
	}
	registry
}

/// `replay(rewrite(H)) == M(replay(H))` at every delta of generated histories, abandoned branches included, which also proves the rewrite
/// leaves none of the old shape behind.
pub fn check_commutation(migration: &dyn NodeMigration, contract: &SourceContract) {
	for seed in 1..50 {
		let (session, table) = generate(migration, contract, seed, 40);
		let history: Vec<Delta> = session.history().cloned().collect();

		let rewritten = rewrite_history(migration, &history, &table).unwrap_or_else(|error| panic!("seed {seed}: rewrite failed: {error}"));

		for delta in &history {
			let original = registry_at(&history, Some(delta.id)).unwrap_or_else(|error| panic!("seed {seed}: {error}"));
			let expected = apply_migration(migration, &original, &table).unwrap_or_else(|error| panic!("seed {seed}: M failed: {error}"));
			let replayed = registry_at(&rewritten.deltas, rewritten.revs[&delta.id]).unwrap_or_else(|error| panic!("seed {seed}: {error}"));
			assert!(
				replayed.value_equal(&expected),
				"seed {seed}: replaying the rewritten history differs from M of the original at {:?}",
				delta.id
			);
		}

		let stale = rewritten.deltas.iter().find(|delta| references(&delta.kind, migration.source(), &table));
		assert!(stale.is_none(), "seed {seed}: rewritten history still holds the source shape: {stale:?}");

		for (index, delta) in rewritten.deltas.iter().enumerate() {
			assert!(delta.has_valid_id(), "seed {seed}: rewritten delta {index} has a stale rev");
			let parent_stored_earlier = delta.parent.is_none_or(|parent| rewritten.deltas[..index].iter().any(|earlier| earlier.id == parent));
			assert!(parent_stored_earlier, "seed {seed}: rewritten delta {index} doesn't follow its parent");
		}
	}
}

/// Undo walks the rewritten deltas' reverse priors, so undoing in rewritten history must land on `M` of the undone original.
pub fn check_undo(migration: &dyn NodeMigration, contract: &SourceContract) {
	for seed in 1..20 {
		let (session, table) = generate(migration, contract, seed, 30);
		let history: Vec<Delta> = session.history().cloned().collect();
		let rewritten = rewrite_history(migration, &history, &table).unwrap_or_else(|error| panic!("seed {seed}: rewrite failed: {error}"));

		let head = session.head_rev();
		let migrated_head = head.and_then(|head| rewritten.revs[&head]);
		let original_registry = registry_at(&history, head).unwrap_or_else(|error| panic!("seed {seed}: {error}"));
		let migrated_registry = registry_at(&rewritten.deltas, migrated_head).unwrap_or_else(|error| panic!("seed {seed}: {error}"));

		let mut original = Session::load(PeerId(1), UserId(1), original_registry, history, head, Vec::new(), 0);
		let mut migrated = Session::load(PeerId(1), UserId(1), migrated_registry, rewritten.deltas, migrated_head, Vec::new(), 0);

		let mut previous = apply_migration(migration, original.registry(), &table).unwrap_or_else(|error| panic!("seed {seed}: M failed: {error}"));
		for undo in 0..5 {
			// An interaction that only touched what `M` drops left no delta in the rewrite, so undo there passes over it
			let mut expected = previous.clone();
			while expected.value_equal(&previous) && original.can_undo() {
				original.undo().expect("can_undo said so");
				expected = apply_migration(migration, original.registry(), &table).unwrap_or_else(|error| panic!("seed {seed}, undo {undo}: M failed: {error}"));
			}
			if expected.value_equal(&previous) {
				break;
			}

			migrated.undo().unwrap_or_else(|error| panic!("seed {seed}, undo {undo}: {error}"));
			assert!(migrated.registry().value_equal(&expected), "seed {seed}, undo {undo}: undone states diverge");

			previous = expected;
		}
	}
}
