use migration_core::{Inputs, Node, NodeInput, literal, literal_of, migration};
use serde::{Deserialize, Serialize};

const WIDTH: usize = 1;
const HEIGHT: usize = 2;

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

/// The width and height are lengths, which keep the numbers of pixels they held. Wired numbers still feed them, read as pixels.
#[migration("vector_nodes::generator_nodes::RectangleNode"@1 => @2)]
pub fn rectangle_v1(node: &Node, inputs: &mut Inputs) {
	let number_at = |slot: usize| match node.inputs().get(slot).and_then(|slot| literal_of::<Tagged>(&slot.input)) {
		Some(Tagged::Number(number)) => Some(number),
		_ => None,
	};

	for slot in [WIDTH, HEIGHT] {
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
		SourceContract::new(6, |random, slot| if slot == 0 { literal(Tagged::None) } else { literal(Tagged::Number(random.below(100) as f64)) })
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&RectangleV1, &contract());
		check_undo(&RectangleV1, &contract());
	}

	#[test]
	fn numbers_of_pixels_become_lengths_in_pixels() {
		let mut fixture = Fixture::new(&RectangleV1);
		let number = |number: f64| literal(Tagged::Number(number));
		let rectangle = fixture.add(
			decl!("vector_nodes::generator_nodes::RectangleNode"@1),
			vec![literal(Tagged::None), number(100.), number(80.), number(0.), number(1.), number(0.)],
		);
		let migrated = fixture.migrate(&RectangleV1).unwrap();

		assert_eq!(
			fixture.inputs(&migrated, rectangle),
			[literal(Tagged::None), length(100.), length(80.), number(0.), number(1.), number(0.)]
		);
	}
}
