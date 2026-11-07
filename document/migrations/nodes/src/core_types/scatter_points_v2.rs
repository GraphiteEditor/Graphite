use migration_core::{Inputs, Node, literal, literal_of, migration};
use serde::{Deserialize, Serialize};

const SEPARATION: usize = 1;

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

/// The separation is a length, which keeps the number of pixels it held. A wired number still feeds it, read as pixels.
#[migration("core_types::vector::ScatterPointsNode"@2 => @3)]
pub fn scatter_points_v2(node: &Node, inputs: &mut Inputs) {
	let Some(slot) = node.inputs().get(SEPARATION) else { return };
	let Some(Tagged::Number(pixels)) = literal_of::<Tagged>(&slot.input) else { return };

	inputs.set(
		SEPARATION as u32,
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
		SourceContract::new(3, |random, slot| if slot == 0 { literal(Tagged::None) } else { literal(Tagged::Number(random.below(100) as f64)) })
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&ScatterPointsV2, &contract());
		check_undo(&ScatterPointsV2, &contract());
	}

	#[test]
	fn a_number_of_pixels_becomes_a_length_in_pixels() {
		let mut fixture = Fixture::new(&ScatterPointsV2);
		let number = |number: f64| literal(Tagged::Number(number));
		let scatter_points = fixture.add(decl!("core_types::vector::ScatterPointsNode"@2), vec![literal(Tagged::None), number(10.), number(0.)]);
		let migrated = fixture.migrate(&ScatterPointsV2).unwrap();

		let separation = literal(Tagged::Length(TypedLength { value: 10., unit: LengthUnit::Pixel }));
		assert_eq!(fixture.inputs(&migrated, scatter_points), [literal(Tagged::None), separation, number(0.)]);
	}
}
