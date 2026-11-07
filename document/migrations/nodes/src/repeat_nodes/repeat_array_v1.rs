use migration_core::{Inputs, Node, literal, literal_of, migration};
use serde::{Deserialize, Serialize};

const ANGLE: usize = 2;

/// Frozen mirror of the `TaggedValue` variants this migration reads and writes.
#[derive(Serialize, Deserialize)]
enum Tagged {
	None,
	Number(f64),
	Angle(TypedAngle),
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

/// The angle is an angle, which keeps the number of degrees it held. A wired number still feeds it, read as degrees.
#[migration("repeat_nodes::repeat_nodes::RepeatArrayNode"@1 => @2)]
pub fn repeat_array_v1(node: &Node, inputs: &mut Inputs) {
	let Some(slot) = node.inputs().get(ANGLE) else { return };
	let Some(Tagged::Number(degrees)) = literal_of::<Tagged>(&slot.input) else { return };

	inputs.set(
		ANGLE as u32,
		literal(Tagged::Angle(TypedAngle {
			value: degrees,
			unit: AngleUnit::Degree,
		})),
	);
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
		check_commutation(&RepeatArrayV1, &contract());
		check_undo(&RepeatArrayV1, &contract());
	}

	#[test]
	fn a_number_of_degrees_becomes_an_angle_in_degrees() {
		let mut fixture = Fixture::new(&RepeatArrayV1);
		let number = |number: f64| literal(Tagged::Number(number));
		let repeat_array = fixture.add(decl!("repeat_nodes::repeat_nodes::RepeatArrayNode"@1), vec![literal(Tagged::None), number(0.), number(90.), number(5.)]);
		let migrated = fixture.migrate(&RepeatArrayV1).unwrap();

		let angle = literal(Tagged::Angle(TypedAngle { value: 90., unit: AngleUnit::Degree }));
		assert_eq!(fixture.inputs(&migrated, repeat_array), [literal(Tagged::None), number(0.), angle, number(5.)]);
	}
}
