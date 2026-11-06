use migration_core::{Inputs, Node, migration};

/// 'Mix Colors' takes the under color first, like a 'Merge' takes its stack, where 'Mix' took the over color first.
#[migration("raster_nodes::blending_nodes::MixNode"@1 => "raster_nodes::blending_nodes::MixColorsNode"@2)]
pub fn mix_colors_from_mix_node(node: &Node, inputs: &mut Inputs) {
	let [over, under, ..] = node.inputs() else { return };

	inputs.set(0, under.input.clone());
	inputs.set(1, over.input.clone());
}

#[cfg(test)]
mod tests {
	use super::*;
	use migration_core::{decl, literal};
	use migration_runner::testing::{Fixture, SourceContract, check_commutation, check_undo};

	fn contract() -> SourceContract {
		SourceContract::new(4, |random, _slot| literal(random.below(10) as f64))
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&MixColorsFromMixNode, &contract());
		check_undo(&MixColorsFromMixNode, &contract());
	}

	#[test]
	fn swaps_the_colors_and_keeps_the_rest() {
		let (over, under, blend_mode, opacity) = (literal(1.), literal(2.), literal(3.), literal(4.));

		let mut fixture = Fixture::new(&MixColorsFromMixNode);
		let mix = fixture.add(decl!("raster_nodes::blending_nodes::MixNode"@1), vec![over.clone(), under.clone(), blend_mode.clone(), opacity.clone()]);
		let migrated = fixture.migrate(&MixColorsFromMixNode).unwrap();

		assert_eq!(fixture.declaration_of(&migrated, mix), Some(decl!("raster_nodes::blending_nodes::MixColorsNode"@2)));
		assert_eq!(fixture.inputs(&migrated, mix), [under, over, blend_mode, opacity]);
	}
}
