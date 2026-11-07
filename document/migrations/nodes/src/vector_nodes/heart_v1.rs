use migration_core::{Inputs, Node, literal, literal_of, migration};
use serde::{Deserialize, Serialize};

const TIP_ANGLE: usize = 6;

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

/// The tip angle is an angle, which keeps the number of degrees it held. A wired number still feeds it, read as degrees.
#[migration("vector_nodes::generator_nodes::HeartNode"@1 => @2)]
pub fn heart_v1(node: &Node, inputs: &mut Inputs) {
	let Some(slot) = node.inputs().get(TIP_ANGLE) else { return };
	let Some(Tagged::Number(degrees)) = literal_of::<Tagged>(&slot.input) else { return };

	inputs.set(
		TIP_ANGLE as u32,
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
		SourceContract::new(7, |random, slot| if slot == 0 { literal(Tagged::None) } else { literal(Tagged::Number(random.below(100) as f64)) })
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&HeartV1, &contract());
		check_undo(&HeartV1, &contract());
	}

	#[test]
	fn a_number_of_degrees_becomes_an_angle_in_degrees() {
		let mut fixture = Fixture::new(&HeartV1);
		let number = |number: f64| literal(Tagged::Number(number));
		let heart = fixture.add(
			decl!("vector_nodes::generator_nodes::HeartNode"@1),
			vec![literal(Tagged::None), number(70.), number(0.), number(0.), number(70.), number(50.), number(75.)],
		);
		let migrated = fixture.migrate(&HeartV1).unwrap();

		let tip_angle = literal(Tagged::Angle(TypedAngle { value: 75., unit: AngleUnit::Degree }));
		assert_eq!(
			fixture.inputs(&migrated, heart),
			[literal(Tagged::None), number(70.), number(0.), number(0.), number(70.), number(50.), tip_angle]
		);
	}
}
