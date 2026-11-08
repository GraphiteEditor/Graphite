use migration_core::{Inputs, Node, NodeInput, literal, literal_of, migration};
use serde::{Deserialize, Serialize};

const SPACING: usize = 6;
const ANGLES: usize = 7;

/// Frozen mirror of the `TaggedValue` variants this migration reads and writes.
#[derive(Serialize, Deserialize)]
enum Tagged {
	None,
	Number(f64),
	DVec2([f64; 2]),
	LengthVec2(TypedLengthVec2),
	AngleVec2(TypedAngleVec2),
}

#[derive(Serialize, Deserialize)]
struct TypedLengthVec2 {
	value: [f64; 2],
	unit: LengthUnit,
}

#[derive(Serialize, Deserialize)]
struct TypedAngleVec2 {
	value: [f64; 2],
	unit: AngleUnit,
}

#[derive(Serialize, Deserialize)]
enum LengthUnit {
	Pixel,
}

#[derive(Serialize, Deserialize)]
enum AngleUnit {
	Degree,
}

/// The spacing is a pair of lengths, filled from one number in both axes, and the isometric angles are a pair of angles, which keep the pixels and degrees they held. Wired values still feed them, read in those units.
#[migration("vector_nodes::generator_nodes::GridNode"@1 => @2)]
pub fn grid_v1(node: &Node, inputs: &mut Inputs) {
	let literal_at = |slot: usize| node.inputs().get(slot).and_then(|slot| literal_of::<Tagged>(&slot.input));

	match literal_at(SPACING) {
		Some(Tagged::Number(pixels)) => inputs.set(SPACING as u32, lengths([pixels, pixels])),
		Some(Tagged::DVec2(pixels)) => inputs.set(SPACING as u32, lengths(pixels)),
		_ => {}
	}
	if let Some(Tagged::DVec2(degrees)) = literal_at(ANGLES) {
		inputs.set(ANGLES as u32, angles(degrees));
	}
}

fn lengths(pixels: [f64; 2]) -> NodeInput {
	literal(Tagged::LengthVec2(TypedLengthVec2 {
		value: pixels,
		unit: LengthUnit::Pixel,
	}))
}

fn angles(degrees: [f64; 2]) -> NodeInput {
	literal(Tagged::AngleVec2(TypedAngleVec2 {
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
		SourceContract::new(8, |random, slot| match slot {
			SPACING if random.below(2) == 0 => literal(Tagged::Number(random.below(100) as f64)),
			SPACING | ANGLES => literal(Tagged::DVec2([random.below(100) as f64, random.below(100) as f64])),
			_ => literal(Tagged::None),
		})
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&GridV1, &contract());
		check_undo(&GridV1, &contract());
	}

	#[test]
	fn pixels_and_degrees_become_typed_lengths_and_angles() {
		let mut fixture = Fixture::new(&GridV1);
		let none = || literal(Tagged::None);
		let square = fixture.add(
			decl!("vector_nodes::generator_nodes::GridNode"@1),
			vec![none(), none(), none(), none(), none(), none(), literal(Tagged::Number(10.)), literal(Tagged::DVec2([30., 30.]))],
		);
		let rectangular = fixture.add(
			decl!("vector_nodes::generator_nodes::GridNode"@1),
			vec![none(), none(), none(), none(), none(), none(), literal(Tagged::DVec2([10., 20.])), literal(Tagged::DVec2([30., 60.]))],
		);
		let migrated = fixture.migrate(&GridV1).unwrap();

		assert_eq!(
			fixture.inputs(&migrated, square),
			[none(), none(), none(), none(), none(), none(), lengths([10., 10.]), angles([30., 30.])]
		);
		assert_eq!(
			fixture.inputs(&migrated, rectangular),
			[none(), none(), none(), none(), none(), none(), lengths([10., 20.]), angles([30., 60.])]
		);
	}
}
