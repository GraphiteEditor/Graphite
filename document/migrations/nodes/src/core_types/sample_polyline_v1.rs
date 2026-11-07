use migration_core::{Inputs, Node, NodeInput, literal, literal_of, migration};
use serde::{Deserialize, Serialize};

const SEPARATION: usize = 2;
const START_OFFSET: usize = 4;
const STOP_OFFSET: usize = 5;

/// Frozen mirror of the `TaggedValue` variants this migration reads and writes.
#[derive(Serialize, Deserialize)]
enum Tagged {
	None,
	Number(f64),
	Length(TypedLength),
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

/// The separation and start and stop offsets are lengths, which keep the numbers of pixels they held. Wired numbers still feed them, read as pixels.
#[migration("core_types::vector::SamplePolylineNode"@1 => @2)]
pub fn sample_polyline_v1(node: &Node, inputs: &mut Inputs) {
	let number_at = |slot: usize| match node.inputs().get(slot).and_then(|slot| literal_of::<Tagged>(&slot.input)) {
		Some(Tagged::Number(number)) => Some(number),
		_ => None,
	};

	for slot in [SEPARATION, START_OFFSET, STOP_OFFSET] {
		if let Some(pixels) = number_at(slot) {
			inputs.set(slot as u32, length(pixels));
		}
	}
}

fn length(pixels: f64) -> NodeInput {
	literal(Tagged::Length(TypedLength {
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
		SourceContract::new(7, |random, slot| if slot == 0 { literal(Tagged::None) } else { literal(Tagged::Number(random.below(100) as f64)) })
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&SamplePolylineV1, &contract());
		check_undo(&SamplePolylineV1, &contract());
	}

	#[test]
	fn numbers_of_pixels_become_lengths_in_pixels() {
		let mut fixture = Fixture::new(&SamplePolylineV1);
		let number = |number: f64| literal(Tagged::Number(number));
		let sample_polyline = fixture.add(
			decl!("core_types::vector::SamplePolylineNode"@1),
			vec![literal(Tagged::None), number(0.), number(100.), number(100.), number(10.), number(20.), number(0.)],
		);
		let migrated = fixture.migrate(&SamplePolylineV1).unwrap();

		assert_eq!(
			fixture.inputs(&migrated, sample_polyline),
			[literal(Tagged::None), number(0.), length(100.), number(100.), length(10.), length(20.), number(0.)]
		);
	}
}
