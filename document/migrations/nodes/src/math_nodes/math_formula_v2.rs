use migration_core::{Inputs, NodeInput, migration};

/// Reads the document's resolution from the scope, which sizes the physical lengths an expression measures.
#[migration("math_nodes::MathFormulaNode"@2 => @3)]
pub fn math_formula_v2(inputs: &mut Inputs) {
	inputs.push(NodeInput::Scope("pixels-per-inch".into()));
}

#[cfg(test)]
mod tests {
	use super::*;
	use migration_core::{decl, literal};
	use migration_runner::testing::{Fixture, SourceContract, check_commutation, check_undo};
	use serde::{Deserialize, Serialize};

	/// Frozen mirror of the `TaggedValue` variants the node's inputs hold in version 2.
	#[derive(Serialize, Deserialize)]
	enum Tagged {
		Number(f64),
		String(String),
	}

	fn contract() -> SourceContract {
		SourceContract::new(3, |random, slot| match slot {
			1 => literal(Tagged::String("a + b".to_string())),
			_ => literal(Tagged::Number(random.below(10) as f64)),
		})
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&MathFormulaV2, &contract());
		check_undo(&MathFormulaV2, &contract());
	}

	#[test]
	fn the_resolution_is_read_from_the_scope() {
		let inputs = vec![literal(Tagged::Number(2.)), literal(Tagged::String("a * 2".to_string())), literal(Tagged::Number(0.))];

		let mut fixture = Fixture::new(&MathFormulaV2);
		let formula = fixture.add(decl!("math_nodes::MathFormulaNode"@2), inputs.clone());
		let migrated = fixture.migrate(&MathFormulaV2).unwrap();

		let expected: Vec<NodeInput> = inputs.into_iter().chain([NodeInput::Scope("pixels-per-inch".into())]).collect();
		assert_eq!(fixture.inputs(&migrated, formula), expected);
	}
}
