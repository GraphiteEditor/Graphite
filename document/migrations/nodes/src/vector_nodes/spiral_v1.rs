use migration_core::{Inputs, Node, NodeInput, literal, literal_of, migration};
use serde::{Deserialize, Serialize};

const START_ANGLE: usize = 3;
const INNER_RADIUS: usize = 4;
const OUTER_RADIUS: usize = 5;
const ANGULAR_RESOLUTION: usize = 6;

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

/// The radii are lengths and the start angle and angular resolution are angles, which keep the numbers of pixels and degrees they held. Wired numbers still feed them, read as pixels and degrees.
#[migration("vector_nodes::generator_nodes::SpiralNode"@1 => @2)]
pub fn spiral_v1(node: &Node, inputs: &mut Inputs) {
	let number_at = |slot: usize| match node.inputs().get(slot).and_then(|slot| literal_of::<Tagged>(&slot.input)) {
		Some(Tagged::Number(number)) => Some(number),
		_ => None,
	};

	for slot in [INNER_RADIUS, OUTER_RADIUS] {
		if let Some(pixels) = number_at(slot) {
			inputs.set(slot as u32, length(pixels));
		}
	}
	for slot in [START_ANGLE, ANGULAR_RESOLUTION] {
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
		SourceContract::new(7, |random, slot| if slot == 0 { literal(Tagged::None) } else { literal(Tagged::Number(random.below(100) as f64)) })
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&SpiralV1, &contract());
		check_undo(&SpiralV1, &contract());
	}

	#[test]
	fn numbers_of_pixels_and_degrees_become_lengths_and_angles() {
		let mut fixture = Fixture::new(&SpiralV1);
		let number = |number: f64| literal(Tagged::Number(number));
		let spiral = fixture.add(
			decl!("vector_nodes::generator_nodes::SpiralNode"@1),
			vec![literal(Tagged::None), number(0.), number(5.), number(0.), number(0.), number(25.), number(90.)],
		);
		let migrated = fixture.migrate(&SpiralV1).unwrap();

		assert_eq!(
			fixture.inputs(&migrated, spiral),
			[literal(Tagged::None), number(0.), number(5.), angle(0.), length(0.), length(25.), angle(90.)]
		);
	}
}
