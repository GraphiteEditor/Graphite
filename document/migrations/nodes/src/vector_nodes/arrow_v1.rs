use migration_core::{Inputs, Node, NodeInput, literal, literal_of, migration};
use serde::{Deserialize, Serialize};

const ARROW_TO: usize = 1;
const SHAFT_WIDTH: usize = 2;
const HEAD_WIDTH: usize = 3;
const HEAD_LENGTH: usize = 4;

/// Frozen mirror of the `TaggedValue` variants this migration reads and writes.
#[derive(Serialize, Deserialize)]
enum Tagged {
	None,
	Number(f64),
	DVec2([f64; 2]),
	Length(TypedLength),
	LengthVec2(TypedLengthVec2),
}

#[derive(Serialize, Deserialize)]
struct TypedLength {
	value: f64,
	unit: LengthUnit,
}

#[derive(Serialize, Deserialize)]
struct TypedLengthVec2 {
	value: [f64; 2],
	unit: LengthUnit,
}

#[derive(Serialize, Deserialize)]
enum LengthUnit {
	Pixel,
}

/// The end point is a pair of lengths and the shaft and head sizes are lengths, which keep the pixels they held. Wired values still feed them, read as pixels.
#[migration("vector_nodes::generator_nodes::ArrowNode"@1 => @2)]
pub fn arrow_v1(node: &Node, inputs: &mut Inputs) {
	let number_at = |slot: usize| match node.inputs().get(slot).and_then(|slot| literal_of::<Tagged>(&slot.input)) {
		Some(Tagged::Number(number)) => Some(number),
		_ => None,
	};

	if let Some(Tagged::DVec2(pixels)) = node.inputs().get(ARROW_TO).and_then(|slot| literal_of::<Tagged>(&slot.input)) {
		inputs.set(ARROW_TO as u32, lengths(pixels));
	}
	for slot in [SHAFT_WIDTH, HEAD_WIDTH, HEAD_LENGTH] {
		if let Some(pixels) = number_at(slot) {
			inputs.set(slot as u32, length(pixels));
		}
	}
}

fn lengths(pixels: [f64; 2]) -> NodeInput {
	literal(Tagged::LengthVec2(TypedLengthVec2 {
		value: pixels,
		unit: LengthUnit::Pixel,
	}))
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
		SourceContract::new(5, |random, slot| match slot {
			0 => literal(Tagged::None),
			ARROW_TO => literal(Tagged::DVec2([random.below(100) as f64, random.below(100) as f64])),
			_ => literal(Tagged::Number(random.below(100) as f64)),
		})
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
			vec![literal(Tagged::None), literal(Tagged::DVec2([100., 0.])), number(10.), number(30.), number(20.)],
		);
		let migrated = fixture.migrate(&ArrowV1).unwrap();

		assert_eq!(fixture.inputs(&migrated, arrow), [literal(Tagged::None), lengths([100., 0.]), length(10.), length(30.), length(20.)]);
	}
}
