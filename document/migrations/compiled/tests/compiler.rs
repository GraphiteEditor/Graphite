//! The compiler host, on the real node registry.

use document_graph_storage::{NodeId, NodeInput};
use graphene_std::vector::Vector;
use migration_compiled::CompilerHost;
use migration_core::{Compiler, Declaration, Node, NodeMigration, Target, decl, literal, migration};
use migration_runner::testing::{Fixture, SourceContract, check_commutation_with};
use serde::Serialize;

const STAR: Declaration = decl!("vector_nodes::generator_nodes::StarNode"@1);

/// Frozen mirror of the `TaggedValue` variants these tests store.
#[derive(Serialize)]
enum Tagged {
	None,
	Integer(i64),
	Number(f64),
}

/// Picks by the element type of its first input, with a fallback for what doesn't compile.
#[migration("test_nodes::anchor"@1 => "test_nodes::fallback"@2)]
pub fn by_compiler(node: &Node, compiler: Option<&dyn Compiler>, target: &mut Target) {
	if compiler.and_then(|compiler| compiler.input_type(node, 0)).is_some_and(|value_type| value_type.element_is::<Vector>()) {
		target.set(decl!("test_nodes::vector"@1));
	}
}

fn star_inputs() -> Vec<NodeInput> {
	vec![literal(Tagged::None), literal(Tagged::Integer(5)), literal(Tagged::Number(50.)), literal(Tagged::Number(25.))]
}

fn wire(id: NodeId) -> NodeInput {
	NodeInput::Node { id, index: 0 }
}

#[test]
fn a_wire_from_a_real_node_gives_its_type() {
	let mut fixture = Fixture::new(&ByCompiler);
	let star = fixture.add(STAR, star_inputs());
	let anchor = fixture.add(ByCompiler.source(), vec![wire(star)]);

	let migrated = fixture.migrate_with(&ByCompiler, Some(&CompilerHost::default())).unwrap();

	assert_eq!(fixture.declaration_of(&migrated, anchor), Some(decl!("test_nodes::vector"@1)));
}

#[test]
fn no_host_takes_the_fallback() {
	let mut fixture = Fixture::new(&ByCompiler);
	let star = fixture.add(STAR, star_inputs());
	let anchor = fixture.add(ByCompiler.source(), vec![wire(star)]);

	let migrated = fixture.migrate(&ByCompiler).unwrap();

	assert_eq!(fixture.declaration_of(&migrated, anchor), Some(decl!("test_nodes::fallback"@2)));
}

#[test]
fn what_does_not_compile_takes_the_fallback() {
	let mut fixture = Fixture::new(&ByCompiler);
	let missing = fixture.add(decl!("test_nodes::not_a_node"@1), vec![]);
	let anchor = fixture.add(ByCompiler.source(), vec![wire(missing)]);

	let migrated = fixture.migrate_with(&ByCompiler, Some(&CompilerHost::default())).unwrap();

	assert_eq!(fixture.declaration_of(&migrated, anchor), Some(decl!("test_nodes::fallback"@2)));
}

#[test]
fn a_literal_takes_the_fallback() {
	let mut fixture = Fixture::new(&ByCompiler);
	let anchor = fixture.add(ByCompiler.source(), vec![literal(Tagged::Number(1.))]);

	let migrated = fixture.migrate_with(&ByCompiler, Some(&CompilerHost::default())).unwrap();

	assert_eq!(fixture.declaration_of(&migrated, anchor), Some(decl!("test_nodes::fallback"@2)));
}

#[test]
fn asking_the_compiler_commutes() {
	let host = CompilerHost::default();
	let contract = SourceContract::new(1, |_, _| literal(Tagged::Number(0.)));
	check_commutation_with(&ByCompiler, &contract, Some(&host));
}
