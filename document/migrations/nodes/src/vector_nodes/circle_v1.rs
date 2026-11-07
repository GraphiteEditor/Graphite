use migration_core::{Inputs, Node, literal, literal_of, migration};
use serde::{Deserialize, Serialize};

const RADIUS: usize = 1;

/// Frozen mirror of the `TaggedValue` variants this migration reads and writes.
#[derive(Serialize, Deserialize)]
enum Tagged {
	None,
	Number(f64),
	Length(TypedLength),
}

#[derive(Serialize, Deserialize)]
struct TypedLength {
	value: f64,
	unit: LengthUnit,
}

#[derive(Serialize, Deserialize)]
enum LengthUnit {
	Pixel,
}

/// The radius is a length, which keeps the number of pixels it held. A wired number still feeds it, read as pixels.
#[migration("vector_nodes::generator_nodes::CircleNode"@1 => @2)]
pub fn circle_v1(node: &Node, inputs: &mut Inputs) {
	let Some(slot) = node.inputs().get(RADIUS) else { return };
	let Some(Tagged::Number(pixels)) = literal_of::<Tagged>(&slot.input) else { return };

	inputs.set(
		RADIUS as u32,
		literal(Tagged::Length(TypedLength {
			value: pixels,
			unit: LengthUnit::Pixel,
		})),
	);
}

#[cfg(test)]
mod tests {
	use super::*;
	use migration_core::decl;
	use migration_runner::testing::{Fixture, SourceContract, check_commutation, check_undo};

	fn contract() -> SourceContract {
		SourceContract::new(2, |random, slot| if slot == 0 { literal(Tagged::None) } else { literal(Tagged::Number(random.below(100) as f64)) })
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&CircleV1, &contract());
		check_undo(&CircleV1, &contract());
	}

	#[test]
	fn a_number_of_pixels_becomes_a_length_in_pixels() {
		let mut fixture = Fixture::new(&CircleV1);
		let circle = fixture.add(decl!("vector_nodes::generator_nodes::CircleNode"@1), vec![literal(Tagged::None), literal(Tagged::Number(50.))]);
		let migrated = fixture.migrate(&CircleV1).unwrap();

		let radius = literal(Tagged::Length(TypedLength { value: 50., unit: LengthUnit::Pixel }));
		assert_eq!(fixture.inputs(&migrated, circle), [literal(Tagged::None), radius]);
	}
}
