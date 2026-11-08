use migration_core::{Inputs, Node, NodeInput, literal, literal_of, migration};
use serde::{Deserialize, Serialize};

const POINT: usize = 1;

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

/// The point is a pair of lengths, which keeps the pixels it held. A wired Vec2 still feeds it, read as pixels.
#[migration("core_types::vector::PointInsideNode"@1 => @2)]
pub fn point_inside_v1(node: &Node, inputs: &mut Inputs) {
	let Some(slot) = node.inputs().get(POINT) else { return };
	let Some(Tagged::DVec2(pixels)) = literal_of::<Tagged>(&slot.input) else { return };

	inputs.set(POINT as u32, lengths(pixels));
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
		SourceContract::new(2, |random, slot| match slot {
			POINT => literal(Tagged::DVec2([random.below(100) as f64, random.below(100) as f64])),
			_ => literal(Tagged::None),
		})
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&PointInsideV1, &contract());
		check_undo(&PointInsideV1, &contract());
	}

	#[test]
	fn a_vec2_of_pixels_becomes_lengths_in_pixels() {
		let mut fixture = Fixture::new(&PointInsideV1);
		let node = fixture.add(decl!("core_types::vector::PointInsideNode"@1), vec![literal(Tagged::None), literal(Tagged::DVec2([30., 40.]))]);
		let migrated = fixture.migrate(&PointInsideV1).unwrap();

		assert_eq!(fixture.inputs(&migrated, node), [literal(Tagged::None), lengths([30., 40.])]);
	}
}
