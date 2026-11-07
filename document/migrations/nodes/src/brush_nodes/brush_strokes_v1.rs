use migration_core::{Inputs, Node, literal, literal_of, migration};
use serde::{Deserialize, Serialize};

const DIAMETER: usize = 2;

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

/// The diameter is a length, which keeps the number of pixels it held. A wired number still feeds it, read as pixels.
#[migration("brush_nodes::BrushStrokesNode"@1 => @2)]
pub fn brush_strokes_v1(node: &Node, inputs: &mut Inputs) {
	let Some(slot) = node.inputs().get(DIAMETER) else { return };
	let Some(Tagged::Number(pixels)) = literal_of::<Tagged>(&slot.input) else { return };

	inputs.set(
		DIAMETER as u32,
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
		SourceContract::new(5, |random, slot| if slot == 0 { literal(Tagged::None) } else { literal(Tagged::Number(random.below(100) as f64)) })
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&BrushStrokesV1, &contract());
		check_undo(&BrushStrokesV1, &contract());
	}

	#[test]
	fn a_number_of_pixels_becomes_a_length_in_pixels() {
		let mut fixture = Fixture::new(&BrushStrokesV1);
		let number = |number: f64| literal(Tagged::Number(number));
		let brush_strokes = fixture.add(decl!("brush_nodes::BrushStrokesNode"@1), vec![literal(Tagged::None), number(0.), number(40.), number(0.), number(100.)]);
		let migrated = fixture.migrate(&BrushStrokesV1).unwrap();

		let diameter = literal(Tagged::Length(TypedLength { value: 40., unit: LengthUnit::Pixel }));
		assert_eq!(fixture.inputs(&migrated, brush_strokes), [literal(Tagged::None), number(0.), diameter, number(0.), number(100.)]);
	}
}
