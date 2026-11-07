use migration_core::{Inputs, Node, NodeInput, literal, literal_of, migration};
use serde::{Deserialize, Serialize};

const MIN: usize = 1;
const MAX: usize = 2;

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

/// The ends of the range are angles, which keep the numbers of degrees they held. Wired numbers still feed them, read as degrees.
#[migration("transform_nodes::transform_nodes::VaryRotationsNode"@2 => @3)]
pub fn vary_rotations_v2(node: &Node, inputs: &mut Inputs) {
	let number_at = |slot: usize| match node.inputs().get(slot).and_then(|slot| literal_of::<Tagged>(&slot.input)) {
		Some(Tagged::Number(number)) => Some(number),
		_ => None,
	};

	for slot in [MIN, MAX] {
		if let Some(degrees) = number_at(slot) {
			inputs.set(slot as u32, angle(degrees));
		}
	}
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
		SourceContract::new(8, |random, slot| if slot == 0 { literal(Tagged::None) } else { literal(Tagged::Number(random.below(100) as f64)) })
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&VaryRotationsV2, &contract());
		check_undo(&VaryRotationsV2, &contract());
	}

	#[test]
	fn numbers_of_degrees_become_angles_in_degrees() {
		let mut fixture = Fixture::new(&VaryRotationsV2);
		let number = |number: f64| literal(Tagged::Number(number));
		let vary_rotations = fixture.add(
			decl!("transform_nodes::transform_nodes::VaryRotationsNode"@2),
			vec![literal(Tagged::None), number(-180.), number(180.), number(0.5), number(0.), number(1.), number(0.), number(0.)],
		);
		let migrated = fixture.migrate(&VaryRotationsV2).unwrap();

		assert_eq!(
			fixture.inputs(&migrated, vary_rotations),
			[literal(Tagged::None), angle(-180.), angle(180.), number(0.5), number(0.), number(1.), number(0.), number(0.)]
		);
	}
}
