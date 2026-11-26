use migration_core::{Inputs, literal, migration};
use serde::{Deserialize, Serialize};

/// Frozen mirror of the `TaggedValue` variant this migration writes.
#[derive(Serialize, Deserialize)]
enum Tagged {
	BlendMode(BlendMode),
}

#[derive(Serialize, Deserialize)]
enum BlendMode {
	Normal,
}

/// 'Drop Shadow' gained a blend mode after its color, set to Normal to keep compositing the shadow as before.
#[migration("blending_nodes::effects::DropShadowNode"@1 => @2)]
pub fn drop_shadow_v1(inputs: &mut Inputs) {
	inputs.insert(2, literal(Tagged::BlendMode(BlendMode::Normal)));
}

#[cfg(test)]
mod tests {
	use super::*;
	use migration_core::decl;
	use migration_runner::testing::{Fixture, SourceContract, check_commutation, check_undo};

	fn contract() -> SourceContract {
		SourceContract::new(8, |random, _slot| literal(random.below(10) as f64))
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&DropShadowV1, &contract());
		check_undo(&DropShadowV1, &contract());
	}

	#[test]
	fn keeps_the_inputs_and_inserts_normal_after_the_color() {
		let inputs: Vec<_> = (0..8).map(|index| literal(index as f64)).collect();

		let mut fixture = Fixture::new(&DropShadowV1);
		let drop_shadow = fixture.add(decl!("blending_nodes::effects::DropShadowNode"@1), inputs.clone());
		let migrated = fixture.migrate(&DropShadowV1).unwrap();

		let mut expected = inputs;
		expected.insert(2, literal(Tagged::BlendMode(BlendMode::Normal)));
		assert_eq!(fixture.declaration_of(&migrated, drop_shadow), Some(decl!("blending_nodes::effects::DropShadowNode"@2)));
		assert_eq!(fixture.inputs(&migrated, drop_shadow), expected);
	}
}
