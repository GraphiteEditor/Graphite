use migration_core::{Inputs, Node, literal, literal_of, migration};
use serde::{Deserialize, Serialize};

const TOLERANCE: usize = 1;

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

/// The tolerance is a length, which keeps the number of pixels it held. A wired number still feeds it, read as pixels.
#[migration("core_types::vector::DecimateNode"@1 => @2)]
pub fn decimate_v1(node: &Node, inputs: &mut Inputs) {
	let Some(slot) = node.inputs().get(TOLERANCE) else { return };
	let Some(Tagged::Number(pixels)) = literal_of::<Tagged>(&slot.input) else { return };

	inputs.set(
		TOLERANCE as u32,
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
		check_commutation(&DecimateV1, &contract());
		check_undo(&DecimateV1, &contract());
	}

	#[test]
	fn a_number_of_pixels_becomes_a_length_in_pixels() {
		let mut fixture = Fixture::new(&DecimateV1);
		let number = |number: f64| literal(Tagged::Number(number));
		let decimate = fixture.add(decl!("core_types::vector::DecimateNode"@1), vec![literal(Tagged::None), number(5.)]);
		let migrated = fixture.migrate(&DecimateV1).unwrap();

		let tolerance = literal(Tagged::Length(TypedLength { value: 5., unit: LengthUnit::Pixel }));
		assert_eq!(fixture.inputs(&migrated, decimate), [literal(Tagged::None), tolerance]);
	}
}
