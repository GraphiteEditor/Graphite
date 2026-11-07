use migration_core::{Inputs, Node, NodeInput, literal, literal_of, migration};
use serde::{Deserialize, Serialize};

const START_ANGLE: usize = 1;
const RADIUS: usize = 2;

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

/// The start angle is an angle and the radius is a length, which keep the numbers of degrees and pixels they held. Wired numbers still feed them, read as degrees and pixels.
#[migration("repeat_nodes::repeat_nodes::RepeatRadialNode"@1 => @2)]
pub fn repeat_radial_v1(node: &Node, inputs: &mut Inputs) {
	let number_at = |slot: usize| match node.inputs().get(slot).and_then(|slot| literal_of::<Tagged>(&slot.input)) {
		Some(Tagged::Number(number)) => Some(number),
		_ => None,
	};

	if let Some(pixels) = number_at(RADIUS) {
		inputs.set(RADIUS as u32, length(pixels));
	}
	if let Some(degrees) = number_at(START_ANGLE) {
		inputs.set(START_ANGLE as u32, angle(degrees));
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
		SourceContract::new(4, |random, slot| if slot == 0 { literal(Tagged::None) } else { literal(Tagged::Number(random.below(100) as f64)) })
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&RepeatRadialV1, &contract());
		check_undo(&RepeatRadialV1, &contract());
	}

	#[test]
	fn numbers_of_degrees_and_pixels_become_an_angle_and_a_length() {
		let mut fixture = Fixture::new(&RepeatRadialV1);
		let number = |number: f64| literal(Tagged::Number(number));
		let repeat_radial = fixture.add(
			decl!("repeat_nodes::repeat_nodes::RepeatRadialNode"@1),
			vec![literal(Tagged::None), number(45.), number(5.), number(5.)],
		);
		let migrated = fixture.migrate(&RepeatRadialV1).unwrap();

		assert_eq!(fixture.inputs(&migrated, repeat_radial), [literal(Tagged::None), angle(45.), length(5.), number(5.)]);
	}
}
