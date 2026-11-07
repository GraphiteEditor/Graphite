use migration_core::{Inputs, Node, NodeInput, literal, literal_of, migration};
use serde::{Deserialize, Serialize};

const RADIUS: usize = 1;
const MIN_ANGLE_THRESHOLD: usize = 4;

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

/// The radius is a length and the minimum angle threshold is an angle, which keep the numbers of pixels and degrees they held. Wired numbers still feed them, read as pixels and degrees.
#[migration("core_types::vector::RoundCornersNode"@1 => @2)]
pub fn round_corners_v1(node: &Node, inputs: &mut Inputs) {
	let number_at = |slot: usize| match node.inputs().get(slot).and_then(|slot| literal_of::<Tagged>(&slot.input)) {
		Some(Tagged::Number(number)) => Some(number),
		_ => None,
	};

	if let Some(pixels) = number_at(RADIUS) {
		inputs.set(RADIUS as u32, length(pixels));
	}
	if let Some(degrees) = number_at(MIN_ANGLE_THRESHOLD) {
		inputs.set(MIN_ANGLE_THRESHOLD as u32, angle(degrees));
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
		check_commutation(&RoundCornersV1, &contract());
		check_undo(&RoundCornersV1, &contract());
	}

	#[test]
	fn numbers_of_pixels_and_degrees_become_a_length_and_an_angle() {
		let mut fixture = Fixture::new(&RoundCornersV1);
		let number = |number: f64| literal(Tagged::Number(number));
		let round_corners = fixture.add(
			decl!("core_types::vector::RoundCornersNode"@1),
			vec![literal(Tagged::None), number(10.), number(0.5), number(100.), number(5.)],
		);
		let migrated = fixture.migrate(&RoundCornersV1).unwrap();

		assert_eq!(fixture.inputs(&migrated, round_corners), [literal(Tagged::None), length(10.), number(0.5), number(100.), angle(5.)]);
	}
}
