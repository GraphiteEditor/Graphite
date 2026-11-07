use migration_core::{Inputs, Node, literal, literal_of, migration};
use serde::{Deserialize, Serialize};

const RADIUS: usize = 2;

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
#[migration("vector_nodes::generator_nodes::RegularPolygonNode"@1 => @2)]
pub fn regular_polygon_v1(node: &Node, inputs: &mut Inputs) {
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
		SourceContract::new(3, |random, slot| if slot == 0 { literal(Tagged::None) } else { literal(Tagged::Number(random.below(100) as f64)) })
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&RegularPolygonV1, &contract());
		check_undo(&RegularPolygonV1, &contract());
	}

	#[test]
	fn a_number_of_pixels_becomes_a_length_in_pixels() {
		let mut fixture = Fixture::new(&RegularPolygonV1);
		let number = |number: f64| literal(Tagged::Number(number));
		let polygon = fixture.add(decl!("vector_nodes::generator_nodes::RegularPolygonNode"@1), vec![literal(Tagged::None), number(6.), number(50.)]);
		let migrated = fixture.migrate(&RegularPolygonV1).unwrap();

		let radius = literal(Tagged::Length(TypedLength { value: 50., unit: LengthUnit::Pixel }));
		assert_eq!(fixture.inputs(&migrated, polygon), [literal(Tagged::None), number(6.), radius]);
	}
}
