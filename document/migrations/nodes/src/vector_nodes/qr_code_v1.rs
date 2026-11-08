use migration_core::{Inputs, Node, NodeInput, literal, literal_of, migration};
use serde::{Deserialize, Serialize};

const SIZE: usize = 3;

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

/// The size is a length, which keeps the number of pixels it held. A wired number still feeds it, read as pixels.
#[migration("vector_nodes::generator_nodes::QrCodeNode"@1 => @2)]
pub fn qr_code_v1(node: &Node, inputs: &mut Inputs) {
	let Some(slot) = node.inputs().get(SIZE) else { return };
	let Some(Tagged::Number(pixels)) = literal_of::<Tagged>(&slot.input) else { return };

	inputs.set(SIZE as u32, length(pixels));
}

fn length(pixels: f64) -> NodeInput {
	literal(Tagged::Length(TypedLength {
		value: pixels,
		unit: LengthUnit::Pixel,
	}))
}

#[cfg(test)]
mod tests {
	use super::*;
	use migration_core::decl;
	use migration_runner::testing::{Fixture, SourceContract, check_commutation, check_undo};

	fn contract() -> SourceContract {
		SourceContract::new(6, |random, slot| if slot == SIZE { literal(Tagged::Number(random.below(100) as f64)) } else { literal(Tagged::None) })
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&QrCodeV1, &contract());
		check_undo(&QrCodeV1, &contract());
	}

	#[test]
	fn a_number_of_pixels_becomes_a_length_in_pixels() {
		let mut fixture = Fixture::new(&QrCodeV1);
		let none = || literal(Tagged::None);
		let qr_code = fixture.add(
			decl!("vector_nodes::generator_nodes::QrCodeNode"@1),
			vec![none(), none(), none(), literal(Tagged::Number(200.)), none(), none()],
		);
		let migrated = fixture.migrate(&QrCodeV1).unwrap();

		assert_eq!(fixture.inputs(&migrated, qr_code), [none(), none(), none(), length(200.), none(), none()]);
	}
}
