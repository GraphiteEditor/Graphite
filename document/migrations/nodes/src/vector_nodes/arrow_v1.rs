use migration_core::{Inputs, Node, NodeInput, literal, literal_of, migration};
use serde::{Deserialize, Serialize};

const SHAFT_WIDTH: usize = 2;
const HEAD_WIDTH: usize = 3;
const HEAD_LENGTH: usize = 4;

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

/// The shaft and head sizes are lengths, which keep the numbers of pixels they held. Wired numbers still feed them, read as pixels.
#[migration("vector_nodes::generator_nodes::ArrowNode"@1 => @2)]
pub fn arrow_v1(node: &Node, inputs: &mut Inputs) {
	let number_at = |slot: usize| match node.inputs().get(slot).and_then(|slot| literal_of::<Tagged>(&slot.input)) {
		Some(Tagged::Number(number)) => Some(number),
		_ => None,
	};

	for slot in [SHAFT_WIDTH, HEAD_WIDTH, HEAD_LENGTH] {
		if let Some(pixels) = number_at(slot) {
			inputs.set(slot as u32, length(pixels));
		}
	}
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
		SourceContract::new(5, |random, slot| if slot == 0 { literal(Tagged::None) } else { literal(Tagged::Number(random.below(100) as f64)) })
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&ArrowV1, &contract());
		check_undo(&ArrowV1, &contract());
	}

	#[test]
	fn numbers_of_pixels_become_lengths_in_pixels() {
		let mut fixture = Fixture::new(&ArrowV1);
		let number = |number: f64| literal(Tagged::Number(number));
		let arrow = fixture.add(
			decl!("vector_nodes::generator_nodes::ArrowNode"@1),
			vec![literal(Tagged::None), number(0.), number(10.), number(30.), number(20.)],
		);
		let migrated = fixture.migrate(&ArrowV1).unwrap();

		assert_eq!(fixture.inputs(&migrated, arrow), [literal(Tagged::None), number(0.), length(10.), length(30.), length(20.)]);
	}
}
