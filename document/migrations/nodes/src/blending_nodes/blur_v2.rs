use migration_core::{Inputs, NodeInput, encode, migration};
use serde::{Deserialize, Serialize};

/// Frozen mirror of the `TaggedValue` variant this migration writes.
#[derive(Serialize, Deserialize)]
enum Tagged {
	TypeDefault(Type),
}

/// Frozen mirror of the `Type` an empty list of graphics stores.
#[derive(Serialize, Deserialize)]
enum Type {
	Concrete(TypeDescriptor),
	List(Box<Type>),
}

#[derive(Serialize, Deserialize)]
struct TypeDescriptor {
	name: String,
}

/// 'Blur' gained a radius mask after its radius, left empty to keep blurring the whole content by the radius.
#[migration("blending_nodes::effects::BlurNode"@2 => @3)]
pub fn blur_v2(inputs: &mut Inputs) {
	inputs.insert(2, empty_radius_mask());
}

/// The radius mask input with nothing connected and not exposed, as the node's definition gives it.
fn empty_radius_mask() -> NodeInput {
	let no_graphics = Tagged::TypeDefault(Type::List(Box::new(Type::Concrete(TypeDescriptor {
		name: "graphic_types::graphic::Graphic".to_string(),
	}))));
	NodeInput::Value {
		value: encode(&no_graphics),
		exposed: false,
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use migration_core::{decl, literal};
	use migration_runner::testing::{Fixture, SourceContract, check_commutation, check_undo};

	fn contract() -> SourceContract {
		SourceContract::new(4, |random, _slot| literal(random.below(10) as f64))
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&BlurV2, &contract());
		check_undo(&BlurV2, &contract());
	}

	#[test]
	fn keeps_the_inputs_and_inserts_an_empty_radius_mask_after_the_radius() {
		let inputs: Vec<_> = (0..4).map(|index| literal(index as f64)).collect();

		let mut fixture = Fixture::new(&BlurV2);
		let blur = fixture.add(decl!("blending_nodes::effects::BlurNode"@2), inputs.clone());
		let migrated = fixture.migrate(&BlurV2).unwrap();

		let mut expected = inputs;
		expected.insert(2, empty_radius_mask());
		assert_eq!(fixture.declaration_of(&migrated, blur), Some(decl!("blending_nodes::effects::BlurNode"@3)));
		assert_eq!(fixture.inputs(&migrated, blur), expected);
		assert!(matches!(expected[2], NodeInput::Value { exposed: false, .. }), "the radius mask is not exposed");
	}
}
