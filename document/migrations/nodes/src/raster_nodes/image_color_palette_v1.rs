use crate::raster_kernel_pipeline;
use migration_core::{Inputs, migration};

/// It now also takes GPU rasters, and the GPU pipeline they run on in a new scope input.
#[migration("raster_nodes::image_color_palette::ImageColorPaletteNode"@1 => @2)]
pub fn image_color_palette_v1(inputs: &mut Inputs) {
	inputs.push(raster_kernel_pipeline());
}

#[cfg(test)]
mod tests {
	use super::*;
	use migration_core::literal;
	use migration_runner::testing::{SourceContract, check_commutation, check_undo};

	fn contract() -> SourceContract {
		SourceContract::new(2, |random, _slot| literal(random.below(10) as f64))
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&ImageColorPaletteV1, &contract());
		check_undo(&ImageColorPaletteV1, &contract());
	}
}
