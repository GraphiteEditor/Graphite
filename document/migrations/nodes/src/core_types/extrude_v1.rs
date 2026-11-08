use migration_core::{Inputs, Node, NodeInput, literal, literal_of, migration};
use serde::{Deserialize, Serialize};

const DIRECTION: usize = 1;

/// Frozen mirror of the `TaggedValue` variants this migration reads and writes.
#[derive(Serialize, Deserialize)]
enum Tagged {
	None,
	DVec2([f64; 2]),
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

/// The direction is a pair of lengths, which keeps the pixels it held. A wired Vec2 still feeds it, read as pixels.
#[migration("core_types::vector::ExtrudeNode"@1 => @2)]
pub fn extrude_v1(node: &Node, inputs: &mut Inputs) {
	let Some(slot) = node.inputs().get(DIRECTION) else { return };
	let Some(Tagged::DVec2(pixels)) = literal_of::<Tagged>(&slot.input) else { return };

	inputs.set(DIRECTION as u32, lengths(pixels));
}

fn lengths(pixels: [f64; 2]) -> NodeInput {
	literal(Tagged::LengthVec2(TypedLengthVec2 {
		value: pixels,
		unit: LengthUnit::Pixel,
	}))
}

#[cfg(test)]
mod tests {
	use super::*;
	use migration_core::decl;
	use migration_runner::testing::{Fixture, SourceContract, check_commutation, check_undo};

	fn contract() -> SourceContract {
		SourceContract::new(3, |random, slot| match slot {
			DIRECTION => literal(Tagged::DVec2([random.below(100) as f64, random.below(100) as f64])),
			_ => literal(Tagged::None),
		})
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&ExtrudeV1, &contract());
		check_undo(&ExtrudeV1, &contract());
	}

	#[test]
	fn a_vec2_of_pixels_becomes_lengths_in_pixels() {
		let mut fixture = Fixture::new(&ExtrudeV1);
		let node = fixture.add(
			decl!("core_types::vector::ExtrudeNode"@1),
			vec![literal(Tagged::None), literal(Tagged::DVec2([30., 40.])), literal(Tagged::None)],
		);
		let migrated = fixture.migrate(&ExtrudeV1).unwrap();

		assert_eq!(fixture.inputs(&migrated, node), [literal(Tagged::None), lengths([30., 40.]), literal(Tagged::None)]);
	}
}
