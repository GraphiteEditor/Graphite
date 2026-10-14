use super::pattern_square::{SquareNodes, Tagged, fill_square_with_pattern};
use migration_core::{Attributes, Downstream, Inputs, MigrationError, Node, decl, literal_of, migration};

const CLIP: u32 = 1;

/// The pattern fills the plane, so the Clip toggle that cut it off at a 100-unit square is gone, and a clipped pattern is read through that square
/// filled with it. A wired toggle counts as clipped.
#[migration("raster_nodes::std_nodes::NoisePatternNode"@1 => @2)]
pub fn noise_pattern_v1(node: &Node, inputs: &mut Inputs, downstream: &mut Downstream, attributes: &mut Attributes) -> Result<(), MigrationError> {
	let Some(clip) = node.inputs().get(CLIP as usize) else {
		return Err(MigrationError::invariant("Noise Pattern has a Clip input"));
	};
	let clipped = !matches!(literal_of::<Tagged>(&clip.input), Some(Tagged::Bool(false)));

	inputs.remove(CLIP);

	if clipped {
		let nodes = SquareNodes {
			rectangle: decl!("vector_nodes::generator_nodes::RectangleNode"@1),
			fill: decl!("graphene_core::vector::FillNode"@1),
			transform: decl!("transform_nodes::transform_nodes::TransformNode"@1),
		};
		fill_square_with_pattern(node, downstream, attributes, nodes, 100.);
	}

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use migration_core::{Declaration, NodeInput, literal};
	use migration_runner::testing::{Fixture, SourceContract, check_commutation, check_undo};

	const NOISE_PATTERN: Declaration = decl!("raster_nodes::std_nodes::NoisePatternNode"@1);
	const CONSUMER: Declaration = decl!("migration_testing::consumer"@1);

	fn contract() -> SourceContract {
		SourceContract {
			// The toggle a clipped pattern's readers move off of can't be reading that pattern, since its own migration removes it
			reads_own_output: |slot| slot != CLIP as usize,
			..SourceContract::new(16, |random, slot| match slot {
				1 => literal(Tagged::Bool(random.below(2) == 0)),
				_ => literal(Tagged::Number(random.below(10) as f64)),
			})
		}
	}

	fn noise_inputs(clip: bool) -> Vec<NodeInput> {
		let mut inputs: Vec<NodeInput> = (0..16).map(|slot| literal(Tagged::Number(slot as f64))).collect();
		inputs[CLIP as usize] = literal(Tagged::Bool(clip));
		inputs
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&NoisePatternV1, &contract());
		check_undo(&NoisePatternV1, &contract());
	}

	#[test]
	fn a_clipped_pattern_is_read_through_a_square_filled_with_it() {
		let mut fixture = Fixture::new(&NoisePatternV1);
		let noise = fixture.add(NOISE_PATTERN, noise_inputs(true));
		let consumer = fixture.add(CONSUMER, vec![NodeInput::Node { id: noise, index: 0 }]);
		let migrated = fixture.migrate(&NoisePatternV1).unwrap();

		// The settings after the removed toggle move down a slot
		let inputs = fixture.inputs(&migrated, noise);
		assert_eq!(inputs.len(), 15);
		assert_eq!(inputs[CLIP as usize], literal(Tagged::Number(2.)));

		let reads = |node, slot: usize| match fixture.inputs(&migrated, node)[slot] {
			NodeInput::Node { id, .. } => id,
			_ => panic!("slot {slot} of {node:?} should read a node"),
		};
		let placement = reads(consumer, 0);
		let fill = reads(placement, 0);
		let paint = reads(fill, 1);
		assert_eq!(reads(paint, 0), noise);
		assert_eq!(fixture.declaration_of(&migrated, reads(fill, 0)), Some(decl!("vector_nodes::generator_nodes::RectangleNode"@1)));
		assert_eq!(fixture.inputs(&migrated, placement)[1], literal(Tagged::DVec2([50., 50.])));
		assert_eq!(fixture.inputs(&migrated, paint)[1], literal(Tagged::DVec2([-50., -50.])));
	}

	#[test]
	fn an_unclipped_pattern_keeps_its_readers() {
		let mut fixture = Fixture::new(&NoisePatternV1);
		let noise = fixture.add(NOISE_PATTERN, noise_inputs(false));
		let consumer = fixture.add(CONSUMER, vec![NodeInput::Node { id: noise, index: 0 }]);
		let migrated = fixture.migrate(&NoisePatternV1).unwrap();

		assert_eq!(fixture.inputs(&migrated, noise).len(), 15);
		assert_eq!(fixture.inputs(&migrated, consumer), [NodeInput::Node { id: noise, index: 0 }]);
		assert_eq!(migrated.node_instances.len(), 2);
	}
}
