use crate::{RawImage, SubtractBlack};

pub fn subtract_black(raw_image: RawImage) -> RawImage {
	let mut raw_image = match raw_image.black {
		SubtractBlack::None => raw_image,
		SubtractBlack::Value(_) => todo!(),
		SubtractBlack::CfaGrid(_) => subtract_black_cfa_grid(raw_image),
	};

	raw_image.black = SubtractBlack::None;
	raw_image
}

pub fn subtract_black_cfa_grid(mut raw_image: RawImage) -> RawImage {
	let height = raw_image.height;
	let width = raw_image.width;
	let black_level = match raw_image.black {
		SubtractBlack::CfaGrid(x) => x,
		_ => unreachable!(),
	};

	for row in 0..raw_image.height {
		for col in 0..raw_image.width {
			raw_image.data[row * width + height] -= black_level[2 * (row % 2) + (col % 2)]
		}
	}

	raw_image.maximum -= black_level.iter().max().unwrap();

	raw_image
}
