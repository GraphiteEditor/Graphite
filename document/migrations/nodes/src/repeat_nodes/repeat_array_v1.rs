use migration_core::{Inputs, Node, NodeInput, literal, literal_of, migration};
use serde::{Deserialize, Serialize};

const DIRECTION: usize = 1;
const ANGLE: usize = 2;

/// Frozen mirror of the `TaggedValue` variants this migration reads and writes.
#[derive(Serialize, Deserialize)]
enum Tagged {
	None,
	Number(f64),
	DVec2([f64; 2]),
	Angle(TypedAngle),
	LengthVec2(TypedLengthVec2),
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

#[derive(Serialize, Deserialize)]
struct TypedAngle {
	value: f64,
	unit: AngleUnit,
}

#[derive(Serialize, Deserialize)]
enum AngleUnit {
	Degree,
}

/// The direction is a pair of lengths and the angle is an angle, which keep the pixels and degrees they held. Wired values still feed them, read in those units.
#[migration("repeat_nodes::repeat_nodes::RepeatArrayNode"@1 => @2)]
pub fn repeat_array_v1(node: &Node, inputs: &mut Inputs) {
	let literal_at = |slot: usize| node.inputs().get(slot).and_then(|slot| literal_of::<Tagged>(&slot.input));

	if let Some(Tagged::DVec2(pixels)) = literal_at(DIRECTION) {
		inputs.set(DIRECTION as u32, direction(pixels));
	}
	if let Some(Tagged::Number(degrees)) = literal_at(ANGLE) {
		inputs.set(ANGLE as u32, angle(degrees));
	}
}

fn direction(pixels: [f64; 2]) -> NodeInput {
	literal(Tagged::LengthVec2(TypedLengthVec2 {
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
		SourceContract::new(4, |random, slot| match slot {
			0 => literal(Tagged::None),
			DIRECTION => literal(Tagged::DVec2([random.below(100) as f64, random.below(100) as f64])),
			_ => literal(Tagged::Number(random.below(100) as f64)),
		})
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&RepeatArrayV1, &contract());
		check_undo(&RepeatArrayV1, &contract());
	}

	#[test]
	fn a_number_of_degrees_becomes_an_angle_in_degrees() {
		let mut fixture = Fixture::new(&RepeatArrayV1);
		let number = |number: f64| literal(Tagged::Number(number));
		let repeat_array = fixture.add(
			decl!("repeat_nodes::repeat_nodes::RepeatArrayNode"@1),
			vec![literal(Tagged::None), literal(Tagged::DVec2([100., 50.])), number(90.), number(5.)],
		);
		let migrated = fixture.migrate(&RepeatArrayV1).unwrap();

		assert_eq!(fixture.inputs(&migrated, repeat_array), [literal(Tagged::None), direction([100., 50.]), angle(90.), number(5.)]);
	}
}
