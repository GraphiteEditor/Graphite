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
#[migration("blending_nodes::effects::BlurNode"@2 => @3)]
pub fn blur_v2(node: &Node, inputs: &mut Inputs) {
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
		SourceContract::new(4, |random, slot| if slot == 0 { literal(Tagged::None) } else { literal(Tagged::Number(random.below(100) as f64)) })
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&BlurV2, &contract());
		check_undo(&BlurV2, &contract());
	}

	#[test]
	fn a_number_of_pixels_becomes_a_length_in_pixels() {
		let mut fixture = Fixture::new(&BlurV2);
		let number = |number: f64| literal(Tagged::Number(number));
		let blur = fixture.add(decl!("blending_nodes::effects::BlurNode"@2), vec![literal(Tagged::None), number(10.), number(0.), number(0.)]);
		let migrated = fixture.migrate(&BlurV2).unwrap();

		let radius = literal(Tagged::Length(TypedLength { value: 10., unit: LengthUnit::Pixel }));
		assert_eq!(fixture.inputs(&migrated, blur), [literal(Tagged::None), radius, number(0.), number(0.)]);
	}
}
