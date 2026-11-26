use migration_core::{Inputs, literal, migration};
use serde::{Deserialize, Serialize};

/// Frozen mirror of the `TaggedValue` variants this migration writes.
#[derive(Serialize, Deserialize)]
enum Tagged {
	Number(f64),
	Bool(bool),
	MaskMode(MaskMode),
}

#[derive(Serialize, Deserialize)]
enum MaskMode {
	Luminance,
}

/// 'Mask' became an effect masking any graphic by any other, keeping its content and mask and gaining its mode, density, feather, and invert at their defaults.
#[migration("raster_nodes::std_nodes::MaskNode"@1 => "blending_nodes::masks::MaskNode"@2)]
pub fn mask_by_any_graphic(inputs: &mut Inputs) {
	inputs.push(literal(Tagged::MaskMode(MaskMode::Luminance)));
	inputs.push(literal(Tagged::Number(100.)));
	inputs.push(literal(Tagged::Number(0.)));
	inputs.push(literal(Tagged::Bool(false)));
}

#[cfg(test)]
mod tests {
	use super::*;
	use migration_core::decl;
	use migration_runner::testing::{Fixture, SourceContract, check_commutation, check_undo};

	fn contract() -> SourceContract {
		SourceContract::new(2, |random, _slot| literal(random.below(10) as f64))
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&MaskByAnyGraphic, &contract());
		check_undo(&MaskByAnyGraphic, &contract());
	}

	#[test]
	fn keeps_the_content_and_mask_and_adds_the_defaults() {
		let (content, stencil) = (literal(1.), literal(2.));

		let mut fixture = Fixture::new(&MaskByAnyGraphic);
		let mask = fixture.add(decl!("raster_nodes::std_nodes::MaskNode"@1), vec![content.clone(), stencil.clone()]);
		let migrated = fixture.migrate(&MaskByAnyGraphic).unwrap();

		assert_eq!(fixture.declaration_of(&migrated, mask), Some(decl!("blending_nodes::masks::MaskNode"@2)));
		let defaults = [
			literal(Tagged::MaskMode(MaskMode::Luminance)),
			literal(Tagged::Number(100.)),
			literal(Tagged::Number(0.)),
			literal(Tagged::Bool(false)),
		];
		assert_eq!(fixture.inputs(&migrated, mask), [[content, stencil].as_slice(), &defaults].concat());
	}
}
