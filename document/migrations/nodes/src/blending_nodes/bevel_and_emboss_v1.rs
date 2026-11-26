use migration_core::{Inputs, NodeInput, literal, migration};
use serde::{Deserialize, Serialize};

/// Frozen mirror of the `TaggedValue` variants this migration writes.
#[derive(Serialize, Deserialize)]
enum Tagged {
	Number(f64),
	Bool(bool),
	TransferCurve(Vec<[f64; 2]>),
	TypeDefault(Type),
}

/// Frozen mirror of the `Type` a paint input's lack of a paint stores.
#[derive(Serialize, Deserialize)]
enum Type {
	Concrete(TypeDescriptor),
	Item(Box<Type>),
}

#[derive(Serialize, Deserialize)]
struct TypeDescriptor {
	name: String,
}

/// 'Bevel & Emboss' gained a texture with its depth, and a gloss contour and a slope contour with its range, set to no texture and straight contours across the whole slope to keep the bevel as it was.
#[migration("blending_nodes::effects::BevelAndEmbossNode"@1 => @2)]
pub fn bevel_and_emboss_v1(inputs: &mut Inputs) {
	for input in added_inputs() {
		inputs.push(input);
	}
}

/// The texture, its depth, the slope contour's range, the gloss and slope contours' antialiasing, and the gloss and slope contours.
fn added_inputs() -> [NodeInput; 7] {
	let straight = || literal(Tagged::TransferCurve(vec![[0., 0.], [1., 1.]]));
	let no_paint = Tagged::TypeDefault(Type::Item(Box::new(Type::Concrete(TypeDescriptor {
		name: "graphic_types::graphic::Graphic".to_string(),
	}))));

	[
		literal(no_paint),
		literal(Tagged::Number(100.)),
		literal(Tagged::Number(100.)),
		literal(Tagged::Bool(false)),
		literal(Tagged::Bool(false)),
		straight(),
		straight(),
	]
}

#[cfg(test)]
mod tests {
	use super::*;
	use migration_core::decl;
	use migration_runner::testing::{Fixture, SourceContract, check_commutation, check_undo};

	fn contract() -> SourceContract {
		SourceContract::new(13, |random, _slot| literal(random.below(10) as f64))
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&BevelAndEmbossV1, &contract());
		check_undo(&BevelAndEmbossV1, &contract());
	}

	#[test]
	fn keeps_the_inputs_and_adds_options_that_leave_the_bevel_as_it_was() {
		let inputs: Vec<_> = (0..13).map(|index| literal(index as f64)).collect();

		let mut fixture = Fixture::new(&BevelAndEmbossV1);
		let bevel = fixture.add(decl!("blending_nodes::effects::BevelAndEmbossNode"@1), inputs.clone());
		let migrated = fixture.migrate(&BevelAndEmbossV1).unwrap();

		let mut expected = inputs;
		expected.extend(added_inputs());
		assert_eq!(fixture.declaration_of(&migrated, bevel), Some(decl!("blending_nodes::effects::BevelAndEmbossNode"@2)));
		assert_eq!(fixture.inputs(&migrated, bevel), expected);
	}
}
