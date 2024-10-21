use crate::{Image, Pixel, Transform};

impl Image<u16> {
	pub fn transform_iter(&self) -> (usize, usize, impl Iterator<Item = Pixel> + use<'_>) {
		let (final_width, final_height) = if self.transform.will_swap_coordinates() {
			(self.height, self.width)
		} else {
			(self.width, self.height)
		};

		let index_0_0 = inverse_transform_index(self.transform, 0, 0, self.width, self.height);
		let index_0_1 = inverse_transform_index(self.transform, 0, 1, self.width, self.height);
		let index_1_0 = inverse_transform_index(self.transform, 1, 0, self.width, self.height);

		let column_step = (index_0_1.0 - index_0_0.0, index_0_1.1 - index_0_0.1);
		let row_step = (index_1_0.0 - index_0_0.0, index_1_0.1 - index_0_0.1);
		let mut index = index_0_0;

		let channels = self.channels as usize;

		(
			final_width,
			final_height,
			(0..final_height).flat_map(move |row| {
				let temp = (0..final_width).map(move |column| {
					let initial_index = (self.width as i64 * index.0 + index.1) as usize;
					let pixel = &self.data[channels * initial_index..channels * (initial_index + 1)];
					index = (index.0 + column_step.0, index.1 + column_step.1);

					Pixel {
						values: pixel.try_into().unwrap(),
						row,
						column,
					}
				});

				index = (index.0 + row_step.0, index.1 + row_step.1);

				temp
			}),
		)
	}
}

pub fn inverse_transform_index(transform: Transform, mut row: usize, mut column: usize, width: usize, height: usize) -> (i64, i64) {
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

	(row as i64, column as i64)
}
