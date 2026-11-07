use migration_core::{Inputs, Node, NodeInput, literal, literal_of, migration};
use serde::{Deserialize, Serialize};

const SIZE: usize = 3;
const LETTER_SPACING: usize = 5;
const LETTER_TILT: usize = 6;

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

/// The size and letter spacing are lengths and the letter tilt is an angle, which keep the numbers of pixels and degrees they held. Wired numbers still feed them, read as pixels and degrees.
#[migration("graphene_std::text::TextNode"@1 => @2)]
pub fn text_v1(node: &Node, inputs: &mut Inputs) {
	let number_at = |slot: usize| match node.inputs().get(slot).and_then(|slot| literal_of::<Tagged>(&slot.input)) {
		Some(Tagged::Number(number)) => Some(number),
		_ => None,
	};

	for slot in [SIZE, LETTER_SPACING] {
		if let Some(pixels) = number_at(slot) {
			inputs.set(slot as u32, length(pixels));
		}
	}
	if let Some(degrees) = number_at(LETTER_TILT) {
		inputs.set(LETTER_TILT as u32, angle(degrees));
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
		SourceContract::new(12, |random, slot| if slot == 0 { literal(Tagged::None) } else { literal(Tagged::Number(random.below(100) as f64)) })
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&TextV1, &contract());
		check_undo(&TextV1, &contract());
	}

	#[test]
	fn numbers_of_pixels_and_degrees_become_lengths_and_an_angle() {
		let mut fixture = Fixture::new(&TextV1);
		let number = |number: f64| literal(Tagged::Number(number));
		let text = fixture.add(
			decl!("graphene_std::text::TextNode"@1),
			vec![
				literal(Tagged::None),
				number(0.),
				number(0.),
				number(24.),
				number(1.2),
				number(2.),
				number(10.),
				number(0.),
				number(100.),
				number(0.),
				number(100.),
				number(0.),
			],
		);
		let migrated = fixture.migrate(&TextV1).unwrap();

		assert_eq!(
			fixture.inputs(&migrated, text),
			[
				literal(Tagged::None),
				number(0.),
				number(0.),
				length(24.),
				number(1.2),
				length(2.),
				angle(10.),
				number(0.),
				number(100.),
				number(0.),
				number(100.),
				number(0.)
			]
		);
	}
}
