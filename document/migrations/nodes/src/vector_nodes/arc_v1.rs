use migration_core::{Inputs, Node, NodeInput, literal, literal_of, migration};
use serde::{Deserialize, Serialize};

const RADIUS: usize = 1;
const START_ANGLE: usize = 2;
const SWEEP_ANGLE: usize = 3;

/// Frozen mirror of the `TaggedValue` variants this migration reads and writes.
#[derive(Serialize, Deserialize)]
enum Tagged {
	None,
	Number(f64),
	Length(TypedLength),
	Angle(TypedAngle),
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

#[derive(Serialize, Deserialize)]
struct TypedAngle {
	value: f64,
	unit: AngleUnit,
}

#[derive(Serialize, Deserialize)]
enum AngleUnit {
	Degree,
}

/// The radius is a length and the start and sweep angles are angles, which keep the numbers of pixels and degrees they held. Wired numbers still feed them, read as pixels and degrees.
#[migration("vector_nodes::generator_nodes::ArcNode"@1 => @2)]
pub fn arc_v1(node: &Node, inputs: &mut Inputs) {
	let number_at = |slot: usize| match node.inputs().get(slot).and_then(|slot| literal_of::<Tagged>(&slot.input)) {
		Some(Tagged::Number(number)) => Some(number),
		_ => None,
	};

	if let Some(pixels) = number_at(RADIUS) {
		inputs.set(RADIUS as u32, length(pixels));
	}
	for slot in [START_ANGLE, SWEEP_ANGLE] {
		if let Some(degrees) = number_at(slot) {
			inputs.set(slot as u32, angle(degrees));
		}
	}
}

fn length(pixels: f64) -> NodeInput {
	literal(Tagged::Length(TypedLength {
		value: pixels,
		unit: LengthUnit::Pixel,
	}))
}

fn angle(degrees: f64) -> NodeInput {
	literal(Tagged::Angle(TypedAngle {
		value: degrees,
		unit: AngleUnit::Degree,
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
		check_commutation(&ArcV1, &contract());
		check_undo(&ArcV1, &contract());
	}

	#[test]
	fn numbers_of_pixels_and_degrees_become_a_length_and_angles() {
		let mut fixture = Fixture::new(&ArcV1);
		let number = |number: f64| literal(Tagged::Number(number));
		let arc = fixture.add(
			decl!("vector_nodes::generator_nodes::ArcNode"@1),
			vec![literal(Tagged::None), number(50.), number(0.), number(270.), number(1.)],
		);
		let migrated = fixture.migrate(&ArcV1).unwrap();

		assert_eq!(fixture.inputs(&migrated, arc), [literal(Tagged::None), length(50.), angle(0.), angle(270.), number(1.)]);
	}
}
