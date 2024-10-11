use crate::{Image, Transform};

pub fn transform(mut image: Image<u16>) -> Image<u16> {
	if image.transform.is_identity() {
		return image;
	}

	let channels = image.channels as usize;
	let mut data = vec![0; channels * image.width * image.height];

	let (final_width, final_height) = if image.transform.will_swap_coordinates() {
		(image.height, image.width)
	} else {
		(image.width, image.height)
	};

	let mut initial_index = inverse_transform_index(image.transform, 0, 0, image.width, image.height);
	let column_step = inverse_transform_index(image.transform, 0, 1, image.width, image.height) as i64 - initial_index as i64;
	let row_step = inverse_transform_index(image.transform, 1, 0, image.width, image.height) as i64 - inverse_transform_index(image.transform, 0, final_width, image.width, image.height) as i64;

	for row in 0..final_height {
		for col in 0..final_width {
			let transformed_index = final_width * row + col;

			let copy_from_range = channels * initial_index..channels * (initial_index + 1);
			let copy_to_range = channels * transformed_index..channels * (transformed_index + 1);
			data[copy_to_range].copy_from_slice(&image.data[copy_from_range]);

			initial_index = (initial_index as i64 + column_step) as usize;
		}
		initial_index = (initial_index as i64 + row_step) as usize;
	}

	image.data = data;
	image.width = final_width;
	image.height = final_height;

	image
}

pub fn inverse_transform_index(transform: Transform, mut row: usize, mut column: usize, width: usize, height: usize) -> usize {
	let value = match transform {
		Transform::Horizontal => 0,
		Transform::MirrorHorizontal => 1,
		Transform::Rotate180 => 3,
		Transform::MirrorVertical => 2,
		Transform::MirrorHorizontalRotate270 => 4,
		Transform::Rotate90 => 6,
		Transform::MirrorHorizontalRotate90 => 7,
		Transform::Rotate270 => 5,
	};

	if value & 4 != 0 {
		std::mem::swap(&mut row, &mut column)
	}

	if value & 2 != 0 {
		row = height - 1 - row;
	}

	if value & 1 != 0 {
		column = width - 1 - column;
	}

	width * row + column
}
