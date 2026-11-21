use migration_core::{Node, Target, decl, literal_of, migration};
use serde::{Deserialize, Serialize};

const EXPRESSION: usize = 1;

/// Frozen mirror of the `TaggedValue` variant this migration reads.
#[derive(Serialize, Deserialize)]
enum Tagged {
	String(String),
}

/// The lone operators 'Math f(…)' applied across its items, in every spelling its classifier read when the node was split.
const OPERATORS: &[&str] = &["+", "-", "*", "/", "^", "&&", "∧", "||", "∨", "<", "<=", "≤", ">", ">=", "≥", "==", "!=", "≠"];

/// The variadic functions 'Math f(…)' called with all its items when given a lone name, also spelled with a leading `\`.
const FUNCTIONS: &[&str] = &[
	"hypot",
	"min",
	"max",
	"mean",
	"median",
	"variance",
	"variancepop",
	"stdev",
	"stdevpop",
	"geomean",
	"harmmean",
	"rms",
	"mode",
	"count",
	"xor",
	"gcd",
	"lcm",
];

/// 'Math f(…)' split by the kind of string it held: a lone operator or function name, which it applied across its items, moves to
/// 'Math Aggregate', and an expression stays as 'Math Formula'. A wired string can't be read before it runs, so it stays an expression.
#[migration("math_nodes::MathFNode"@1 => "math_nodes::MathFormulaNode"@2)]
pub fn math_formula_from_math_f(node: &Node, target: &mut Target) {
	let Some(slot) = node.inputs().get(EXPRESSION) else { return };
	let Some(Tagged::String(expression)) = literal_of::<Tagged>(&slot.input) else { return };

	let token = expression.trim();
	let function = token.strip_prefix('\\').unwrap_or(token);
	if OPERATORS.contains(&token) || FUNCTIONS.contains(&function) {
		target.set(decl!("math_nodes::MathAggregateNode"@1));
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use migration_core::{Declaration, NodeInput, literal};
	use migration_runner::testing::{Fixture, SourceContract, check_commutation, check_undo};

	const MATH_F: Declaration = decl!("math_nodes::MathFNode"@1);

	fn contract() -> SourceContract {
		// Only the items can read another 'Math f(…)', since the expression and output type don't take its number
		SourceContract {
			reads_own_output: |slot| slot == 0,
			..SourceContract::new(3, |random, slot| match slot {
				EXPRESSION => literal(Tagged::String(["+", "a * b", "\\max", " ≤ ", "min(a, b)"][random.below(5)].to_string())),
				_ => literal(random.below(4) as f64),
			})
		}
	}

	fn migrated_declaration(expression: NodeInput) -> Option<Declaration> {
		let mut fixture = Fixture::new(&MathFormulaFromMathF);
		let anchor = fixture.add(MATH_F, vec![literal(1.), expression, literal(0.)]);
		let migrated = fixture.migrate(&MathFormulaFromMathF).unwrap();
		fixture.declaration_of(&migrated, anchor)
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&MathFormulaFromMathF, &contract());
		check_undo(&MathFormulaFromMathF, &contract());
	}

	#[test]
	fn lone_operators_and_functions_become_aggregates() {
		let aggregate = Some(decl!("math_nodes::MathAggregateNode"@1));
		let formula = Some(decl!("math_nodes::MathFormulaNode"@2));
		let expression = |text: &str| literal(Tagged::String(text.to_string()));

		assert_eq!(migrated_declaration(expression("+")), aggregate);
		assert_eq!(migrated_declaration(expression(" ≠ ")), aggregate);
		assert_eq!(migrated_declaration(expression("max")), aggregate);
		assert_eq!(migrated_declaration(expression("\\mean")), aggregate);
		assert_eq!(migrated_declaration(expression("a + b")), formula);
		assert_eq!(migrated_declaration(expression("max(a, b)")), formula);
		assert_eq!(migrated_declaration(expression("")), formula);
	}

	#[test]
	fn a_wired_expression_becomes_a_formula() {
		let mut fixture = Fixture::new(&MathFormulaFromMathF);
		let source = fixture.add(MATH_F, vec![literal(1.), literal(Tagged::String("+".to_string())), literal(0.)]);
		let anchor = fixture.add(MATH_F, vec![literal(1.), NodeInput::Node { id: source, index: 0 }, literal(0.)]);
		let migrated = fixture.migrate(&MathFormulaFromMathF).unwrap();

		assert_eq!(fixture.declaration_of(&migrated, anchor), Some(decl!("math_nodes::MathFormulaNode"@2)));
		assert_eq!(fixture.declaration_of(&migrated, source), Some(decl!("math_nodes::MathAggregateNode"@1)));
	}
}
