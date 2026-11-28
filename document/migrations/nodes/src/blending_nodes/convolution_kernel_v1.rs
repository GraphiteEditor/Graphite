use migration_core::{Inputs, literal, migration};
use serde::{Deserialize, Serialize};

/// Frozen mirror of the `TaggedValue` variant this migration writes.
#[derive(Serialize, Deserialize)]
enum Tagged {
	Bool(bool),
}

/// 'Convolution Kernel' gained Apply to Alpha after its offset, left off to keep the content's coverage as it is.
#[migration("blending_nodes::filters::ConvolutionKernelNode"@1 => @2)]
pub fn convolution_kernel_v1(inputs: &mut Inputs) {
	inputs.push(literal(Tagged::Bool(false)));
}

#[cfg(test)]
mod tests {
	use super::*;
	use migration_core::decl;
	use migration_runner::testing::{Fixture, SourceContract, check_commutation, check_undo};

	fn contract() -> SourceContract {
		SourceContract::new(5, |random, _slot| literal(random.below(10) as f64))
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&ConvolutionKernelV1, &contract());
		check_undo(&ConvolutionKernelV1, &contract());
	}

	#[test]
	fn keeps_the_inputs_and_appends_apply_to_alpha_turned_off() {
		let inputs: Vec<_> = (0..5).map(|index| literal(index as f64)).collect();

		let mut fixture = Fixture::new(&ConvolutionKernelV1);
		let kernel = fixture.add(decl!("blending_nodes::filters::ConvolutionKernelNode"@1), inputs.clone());
		let migrated = fixture.migrate(&ConvolutionKernelV1).unwrap();

		assert_eq!(fixture.declaration_of(&migrated, kernel), Some(decl!("blending_nodes::filters::ConvolutionKernelNode"@2)));
		assert_eq!(fixture.inputs(&migrated, kernel), [inputs, vec![literal(Tagged::Bool(false))]].concat());
	}
}
