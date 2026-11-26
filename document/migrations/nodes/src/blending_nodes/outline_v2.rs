use migration_core::{Inputs, literal, migration};
use serde::{Deserialize, Serialize};

/// Frozen mirror of the `TaggedValue` variant this migration writes.
#[derive(Serialize, Deserialize)]
enum Tagged {
	Bool(bool),
}

/// 'Outline' takes its color as any paint now, and gained a Contour Gradient option, set off to keep painting by the paint's own placement.
#[migration("blending_nodes::effects::OutlineNode"@2 => @3)]
pub fn outline_v2(inputs: &mut Inputs) {
	inputs.push(literal(Tagged::Bool(false)));
}

#[cfg(test)]
mod tests {
	use super::*;
	use migration_core::decl;
	use migration_runner::testing::{Fixture, SourceContract, check_commutation, check_undo};

	fn contract() -> SourceContract {
		SourceContract::new(5, |random, _slot| literal(random.below(10) as f64))
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&OutlineV2, &contract());
		check_undo(&OutlineV2, &contract());
	}

	#[test]
	fn keeps_the_inputs_and_adds_contour_gradient_off() {
		let inputs: Vec<_> = (0..5).map(|index| literal(index as f64)).collect();

		let mut fixture = Fixture::new(&OutlineV2);
		let outline = fixture.add(decl!("blending_nodes::effects::OutlineNode"@2), inputs.clone());
		let migrated = fixture.migrate(&OutlineV2).unwrap();

		let mut expected = inputs;
		expected.push(literal(Tagged::Bool(false)));
		assert_eq!(fixture.declaration_of(&migrated, outline), Some(decl!("blending_nodes::effects::OutlineNode"@3)));
		assert_eq!(fixture.inputs(&migrated, outline), expected);
	}
}
