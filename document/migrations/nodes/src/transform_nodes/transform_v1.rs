use migration_core::{Inputs, Node, NodeInput, literal, literal_of, migration};
use serde::{Deserialize, Serialize};

const TRANSLATION: usize = 1;
const ROTATION: usize = 2;
const SKEW: usize = 4;

/// Frozen mirror of the `TaggedValue` variants this migration reads and writes.
#[derive(Serialize, Deserialize)]
enum Tagged {
	None,
	Number(f64),
	DVec2([f64; 2]),
	Angle(TypedAngle),
	LengthVec2(TypedLengthVec2),
	AngleVec2(TypedAngleVec2),
}

#[derive(Serialize, Deserialize)]
struct TypedAngle {
	value: f64,
	unit: AngleUnit,
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

/// The translation is a pair of lengths and the rotation and skew are angles, which keep the pixels and degrees they held. Wired values still feed them, read in those units.
#[migration("transform_nodes::transform_nodes::TransformNode"@1 => @2)]
pub fn transform_v1(node: &Node, inputs: &mut Inputs) {
	let literal_at = |slot: usize| node.inputs().get(slot).and_then(|slot| literal_of::<Tagged>(&slot.input));

	if let Some(Tagged::DVec2(pixels)) = literal_at(TRANSLATION) {
		inputs.set(TRANSLATION as u32, translation(pixels));
	}
	if let Some(Tagged::Number(degrees)) = literal_at(ROTATION) {
		inputs.set(ROTATION as u32, rotation(degrees));
	}
	if let Some(Tagged::DVec2(degrees)) = literal_at(SKEW) {
		inputs.set(SKEW as u32, skew(degrees));
	}
}

/// The whole-list counterpart of the Transform node, with parameters that migrate the same way.
#[migration("transform_nodes::transform_nodes::TransformListNode"@1 => @2)]
pub fn transform_list_v1(node: &Node, inputs: &mut Inputs) {
	transform_v1(node, inputs);
}

fn translation(pixels: [f64; 2]) -> NodeInput {
	literal(Tagged::LengthVec2(TypedLengthVec2 {
		value: pixels,
		unit: LengthUnit::Pixel,
	}))
}

fn rotation(degrees: f64) -> NodeInput {
	literal(Tagged::Angle(TypedAngle {
		value: degrees,
		unit: AngleUnit::Degree,
	}))
}

fn skew(degrees: [f64; 2]) -> NodeInput {
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
		SourceContract::new(5, |random, slot| match slot {
			0 => literal(Tagged::None),
			ROTATION => literal(Tagged::Number(random.below(360) as f64)),
			_ => literal(Tagged::DVec2([random.below(100) as f64, random.below(100) as f64])),
		})
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&TransformV1, &contract());
		check_undo(&TransformV1, &contract());
	}

	#[test]
	fn pixels_and_degrees_become_typed_lengths_and_angles() {
		let mut fixture = Fixture::new(&TransformV1);
		let vec2 = |x: f64, y: f64| literal(Tagged::DVec2([x, y]));
		let transform = fixture.add(
			decl!("transform_nodes::transform_nodes::TransformNode"@1),
			vec![literal(Tagged::None), vec2(10., 20.), literal(Tagged::Number(45.)), vec2(2., 3.), vec2(15., 0.)],
		);
		let migrated = fixture.migrate(&TransformV1).unwrap();

		assert_eq!(
			fixture.inputs(&migrated, transform),
			[literal(Tagged::None), translation([10., 20.]), rotation(45.), vec2(2., 3.), skew([15., 0.])]
		);
	}
}
