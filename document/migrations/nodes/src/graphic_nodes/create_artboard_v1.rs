use migration_core::{Inputs, Node, NodeInput, literal, literal_of, migration};
use serde::{Deserialize, Serialize};

const LOCATION: usize = 1;
const DIMENSIONS: usize = 2;

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

/// The location and dimensions are pairs of lengths, which keep the pixels they held. Wired Vec2s still feed them, read as pixels.
#[migration("graphic_nodes::artboard::CreateArtboardNode"@1 => @2)]
pub fn create_artboard_v1(node: &Node, inputs: &mut Inputs) {
	for slot in [LOCATION, DIMENSIONS] {
		if let Some(Tagged::DVec2(pixels)) = node.inputs().get(slot).and_then(|slot| literal_of::<Tagged>(&slot.input)) {
			inputs.set(slot as u32, lengths(pixels));
		}
	}
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
		SourceContract::new(5, |random, slot| match slot {
			LOCATION | DIMENSIONS => literal(Tagged::DVec2([random.below(100) as f64, random.below(100) as f64])),
			_ => literal(Tagged::None),
		})
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&CreateArtboardV1, &contract());
		check_undo(&CreateArtboardV1, &contract());
	}

	#[test]
	fn vec2s_of_pixels_become_lengths_in_pixels() {
		let mut fixture = Fixture::new(&CreateArtboardV1);
		let none = || literal(Tagged::None);
		let artboard = fixture.add(
			decl!("graphic_nodes::artboard::CreateArtboardNode"@1),
			vec![none(), literal(Tagged::DVec2([10., 20.])), literal(Tagged::DVec2([1920., 1080.])), none(), none()],
		);
		let migrated = fixture.migrate(&CreateArtboardV1).unwrap();

		assert_eq!(fixture.inputs(&migrated, artboard), [none(), lengths([10., 20.]), lengths([1920., 1080.]), none(), none()]);
	}
}
