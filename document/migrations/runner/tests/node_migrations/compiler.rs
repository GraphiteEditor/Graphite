//! Migrations choosing a target per anchor, and asking the compiler, which only run on whole states.

use document_graph_storage::{Implementation, Node, NodeId, NodeInput, PeerId, ROOT_NETWORK, Registry, RegistryDelta, Session, UserId};
use migration_core::{Compiler, DeclarationResolver, Graph, MigrationError, MigrationHost, NodeMigration, Target, ValueType, decl, literal, literal_of, migration};
use migration_runner::testing::{Fixture, SourceContract, check_commutation, check_commutation_with, check_undo, check_undo_with};
use migration_runner::{DeclarationTable, MigrationBlobs, RunnerError, migrate_history, order_migrations};
use std::any::type_name;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Chooses by a literal toggle, at the version each target already has, rather than stepping them all.
#[migration("choice::toggle"@1 => "choice::on"@2)]
pub fn choice_by_toggle(node: &migration_core::Node, target: &mut Target) {
	if literal_of::<bool>(&node.inputs()[0].input) == Some(false) {
		target.set(decl!("choice::off"@5));
	}
}

/// Names a target only at runtime, which the runner can't order against anything.
#[migration("choice::unnamed"@1 => "choice::named"@2)]
pub fn choice_unnamed(target: &mut Target) {
	target.set(migration_core::Declaration::new("choice::elsewhere", 1));
}

/// A fake value type, whose element stands for how many nodes are alive in the state the compiler describes.
struct Crowded(bool);

struct Many;
struct Few;

impl ValueType for Crowded {
	fn element_name(&self) -> Option<&str> {
		Some(if self.0 { type_name::<Many>() } else { type_name::<Few>() })
	}
}

struct FakeCompiler(usize);

impl Compiler for FakeCompiler {
	fn input_type(&self, _node: &migration_core::Node, _slot: usize) -> Option<Box<dyn ValueType>> {
		Some(Box::new(Crowded(self.0 > 3)))
	}
}

/// Chooses by what the compiler says, and only sees whole states.
#[migration("compiler::anchor"@1 => "compiler::few"@2)]
pub fn by_compiler(node: &migration_core::Node, compiler: Option<&dyn Compiler>, target: &mut Target) {
	if compiler.and_then(|compiler| compiler.input_type(node, 0)).is_some_and(|value_type| value_type.element_is::<Many>()) {
		target.set(decl!("compiler::many"@1));
	}
}

/// Asks the compiler, and sorts before `order::static_late` by source, so only the runner's rule can put it after.
#[migration("order::asking"@1 => "order::asked"@2)]
pub fn order_asking(_compiler: Option<&dyn Compiler>) {}

#[migration("order::static_late"@1 => @2)]
pub fn order_static_late() {}

/// Asks the compiler, but `order::inspector` inspects what it produces, so it has to run first after all.
#[migration("order::pinned_asking"@1 => "order::produced"@2)]
pub fn order_pinned_asking(_compiler: Option<&dyn Compiler>) {}

#[migration("order::inspector"@1 => @2)]
pub fn order_inspector() {
	let _ = decl!("order::produced"@2);
}

/// Reads the graph, which doesn't ask the host but still only sees whole states.
#[migration("compiler::graph_reader"@1 => @2)]
pub fn graph_reader(_graph: &Graph) -> Result<(), MigrationError> {
	GRAPH_READS.fetch_add(1, Ordering::Relaxed);
	Ok(())
}

static GRAPH_READS: AtomicUsize = AtomicUsize::new(0);

#[derive(Default)]
pub struct AnswerHost {
	asked: AtomicUsize,
}

impl MigrationHost for AnswerHost {
	fn compiler(&self, state: &Registry, _resolver: &dyn DeclarationResolver) -> Option<Box<dyn Compiler + '_>> {
		self.asked.fetch_add(1, Ordering::Relaxed);
		Some(Box::new(FakeCompiler(state.node_instances.len())))
	}
}

fn wire(id: NodeId) -> NodeInput {
	NodeInput::Node { id, index: 0 }
}

/// A history of one interaction per entry in `interactions`, each adding that many nodes implementing the `source` of `migration`.
fn history_of(migration: &dyn NodeMigration, table: &mut DeclarationTable, interactions: &[usize]) -> Session {
	table.register_migration(migration);
	let resource = table.register(migration.source());

	let mut session = Session::with_identity(PeerId(1), UserId(1));
	session
		.stage_computed_ops(vec![RegistryDelta::AddNetwork {
			id: ROOT_NETWORK,
			network: Default::default(),
		}])
		.unwrap();
	session.retire_all().unwrap();

	for nodes in interactions {
		let mut last = None;
		for _ in 0..*nodes {
			let id = session.next_node_id();
			let node = Node::new(ROOT_NETWORK, Implementation::ProtoNode(resource), 0);
			session.stage_computed_ops(vec![RegistryDelta::AddNode { id, node }]).unwrap();
			last = session.retire_all().unwrap().last().copied();
		}
		session.mark_interaction_end(last.expect("an interaction adds nodes"));
	}
	session
}

#[test]
fn each_anchor_chooses_its_target() {
	let mut fixture = Fixture::new(&ChoiceByToggle);
	let on = fixture.add(ChoiceByToggle.source(), vec![literal(true)]);
	let off = fixture.add(ChoiceByToggle.source(), vec![literal(false)]);

	let migrated = fixture.migrate(&ChoiceByToggle).unwrap();

	assert_eq!(fixture.declaration_of(&migrated, on), Some(decl!("choice::on"@2)));
	assert_eq!(fixture.declaration_of(&migrated, off), Some(decl!("choice::off"@5)));
}

#[test]
fn a_choice_changing_across_history_commutes() {
	let contract = SourceContract::new(1, |random, _| literal(random.below(2) == 0));
	check_commutation(&ChoiceByToggle, &contract);
	check_undo(&ChoiceByToggle, &contract);
}

#[test]
fn a_target_the_body_does_not_name_fails() {
	let mut fixture = Fixture::new(&ChoiceUnnamed);
	fixture.add(ChoiceUnnamed.source(), vec![]);

	assert!(matches!(fixture.migrate(&ChoiceUnnamed), Err(RunnerError::UnnamedTarget { .. })));
}

#[test]
fn the_compiler_decides_per_state() {
	let host = AnswerHost::default();

	let mut few = Fixture::new(&ByCompiler);
	let anchor = few.add(ByCompiler.source(), vec![]);
	let migrated = few.migrate_with(&ByCompiler, Some(&host)).unwrap();
	assert_eq!(few.declaration_of(&migrated, anchor), Some(decl!("compiler::few"@2)));

	let mut many = Fixture::new(&ByCompiler);
	let anchor = many.add(ByCompiler.source(), vec![]);
	for _ in 0..3 {
		many.add(decl!("compiler::other"@1), vec![]);
	}
	let migrated = many.migrate_with(&ByCompiler, Some(&host)).unwrap();
	assert_eq!(many.declaration_of(&migrated, anchor), Some(decl!("compiler::many"@1)));
}

#[test]
fn without_a_host_it_takes_the_fallback() {
	let mut fixture = Fixture::new(&ByCompiler);
	let anchor = fixture.add(ByCompiler.source(), vec![]);
	for _ in 0..3 {
		fixture.add(decl!("compiler::other"@1), vec![]);
	}

	let migrated = fixture.migrate(&ByCompiler).unwrap();

	assert_eq!(fixture.declaration_of(&migrated, anchor), Some(decl!("compiler::few"@2)));
}

#[test]
fn asking_the_compiler_goes_last_unless_a_dependency_says_otherwise() {
	let position = |ordered: &[&'static dyn NodeMigration], source: migration_core::Declaration| ordered.iter().position(|migration| migration.source() == source).unwrap();

	let ordered = order_migrations(&[&OrderAsking, &OrderStaticLate]).unwrap();
	assert!(position(&ordered, OrderStaticLate.source()) < position(&ordered, OrderAsking.source()));

	let ordered = order_migrations(&[&OrderPinnedAsking, &OrderInspector]).unwrap();
	assert!(position(&ordered, OrderPinnedAsking.source()) < position(&ordered, OrderInspector.source()));
}

#[test]
fn upstream_anchors_run_before_those_reading_them() {
	let host = AnswerHost::default();
	let mut fixture = Fixture::new(&ByCompiler);
	let first = fixture.add(ByCompiler.source(), vec![]);
	let second = fixture.add(ByCompiler.source(), vec![wire(first)]);
	fixture.add(ByCompiler.source(), vec![wire(second)]);

	fixture.migrate_with(&ByCompiler, Some(&host)).unwrap();

	assert_eq!(host.asked.load(Ordering::Relaxed), 3, "one round per level of nesting");
}

#[test]
fn unrelated_anchors_share_one_round() {
	let host = AnswerHost::default();
	let mut fixture = Fixture::new(&ByCompiler);
	for _ in 0..3 {
		fixture.add(ByCompiler.source(), vec![]);
	}

	fixture.migrate_with(&ByCompiler, Some(&host)).unwrap();

	assert_eq!(host.asked.load(Ordering::Relaxed), 1);
}

#[test]
fn the_compiler_is_only_asked_at_interaction_boundaries() {
	let mut table = DeclarationTable::default();
	let session = history_of(&ByCompiler, &mut table, &[4, 3]);
	let history: Vec<_> = session.history().cloned().collect();
	let host = AnswerHost::default();

	let rewritten = migrate_history(&[&ByCompiler], history.clone(), &table, Some(&host), session.head_rev(), &mut MigrationBlobs::default()).unwrap();

	// The root network's interaction has no anchors yet, so it never asks, and each of the other two asks once for its one round of unrelated anchors
	assert_eq!(host.asked.load(Ordering::Relaxed), 2);

	// A delta inside an interaction has no state of its own, and an interaction end does
	let ends: Vec<_> = history.iter().filter(|delta| delta.is_interaction_end()).collect();
	assert_eq!(ends.len(), 2);
	for end in ends {
		assert!(rewritten.revs[&end.id].is_some());
		let rev = rewritten.revs[&end.id].unwrap();
		assert!(rewritten.deltas.iter().any(|delta| delta.id == rev && delta.is_interaction_end()));
	}
}

#[test]
fn a_cursor_inside_an_interaction_keeps_its_state() {
	let mut table = DeclarationTable::default();
	let session = history_of(&ByCompiler, &mut table, &[4]);
	let history: Vec<_> = session.history().cloned().collect();
	let inside = history[history.len() - 2].id;
	let host = AnswerHost::default();

	let rewritten = migrate_history(&[&ByCompiler], history, &table, Some(&host), [inside], &mut MigrationBlobs::default()).unwrap();
	assert_eq!(host.asked.load(Ordering::Relaxed), 2, "the cursor and the interaction's end");
	assert!(rewritten.revs[&inside].is_some());
}

#[test]
fn reading_the_graph_also_only_sees_whole_states() {
	let mut table = DeclarationTable::default();
	let session = history_of(&GraphReader, &mut table, &[5]);
	let history: Vec<_> = session.history().cloned().collect();

	let before = GRAPH_READS.load(Ordering::Relaxed);
	migrate_history(&[&GraphReader], history, &table, None, session.head_rev(), &mut MigrationBlobs::default()).unwrap();

	// All five anchors exist at the one boundary, and the anchorless root network's boundary reads nothing
	assert_eq!(GRAPH_READS.load(Ordering::Relaxed) - before, 5);
}

#[test]
fn asking_the_compiler_commutes() {
	let host = AnswerHost::default();
	let contract = SourceContract::new(1, |_, _| literal(0.));
	check_commutation_with(&ByCompiler, &contract, Some(&host));
	check_undo_with(&ByCompiler, &contract, Some(&host));
}
