use crate::{Image, RawImage};

fn average(data: &[u16], indexes: impl Iterator<Item = i64>) -> u16 {
	let mut sum = 0;
	let mut count = 0;
	for index in indexes {
		if index >= 0 && (index as usize) < data.len() {
			sum += data[index as usize] as u32;
			count += 1;
		}
	}

	(sum / count) as u16
}

pub fn linear_demosaic(raw_image: RawImage) -> Image<u16> {
	match raw_image.cfa_pattern {
		[0, 1, 1, 2] => linear_demosaic_rggb(raw_image),
		_ => todo!(),
	}
}

fn linear_demosaic_rggb(mut raw_image: RawImage) -> Image<u16> {
	let width = raw_image.width as i64;
	let height = raw_image.height as i64;

	for row in 0..height {
		let row_by_width = row * width;

		for col in 0..width {
			let pixel_index = row_by_width + col;

			let vertical_indexes = [pixel_index + width, pixel_index - width];
			let horizontal_indexes = [pixel_index + 1, pixel_index - 1];
			let cross_indexes = [pixel_index + width, pixel_index - width, pixel_index + 1, pixel_index - 1];
			let diagonal_indexes = [pixel_index + width + 1, pixel_index - width + 1, pixel_index + width - 1, pixel_index - width - 1];

			match (row % 2 == 0, col % 2 == 0) {
				(true, true) => {
					let indexes = cross_indexes.iter().map(|x| 3 * x + 1);
					raw_image.data[3 * (pixel_index as usize) + 1] = average(&raw_image.data, indexes);

					let indexes = diagonal_indexes.iter().map(|x| 3 * x + 2);
					raw_image.data[3 * (pixel_index as usize) + 2] = average(&raw_image.data, indexes);
				}
				(true, false) => {
					let indexes = horizontal_indexes.iter().map(|x| 3 * x);
					raw_image.data[3 * (pixel_index as usize)] = average(&raw_image.data, indexes);

					let indexes = vertical_indexes.iter().map(|x| 3 * x + 2);
					raw_image.data[3 * (pixel_index as usize) + 2] = average(&raw_image.data, indexes);
				}
				(false, true) => {
					let indexes = vertical_indexes.iter().map(|x| 3 * x);
					raw_image.data[3 * (pixel_index as usize)] = average(&raw_image.data, indexes);

					let indexes = horizontal_indexes.iter().map(|x| 3 * x + 2);
					raw_image.data[3 * (pixel_index as usize) + 2] = average(&raw_image.data, indexes);
				}
				(false, false) => {
					let indexes = cross_indexes.iter().map(|x| 3 * x + 1);
					raw_image.data[3 * (pixel_index as usize) + 1] = average(&raw_image.data, indexes);

					let indexes = diagonal_indexes.iter().map(|x| 3 * x);
					raw_image.data[3 * (pixel_index as usize)] = average(&raw_image.data, indexes);
				}
			}
		}
	}

	Image {
		channels: 3,
		data: raw_image.data,
		width: raw_image.width,
		height: raw_image.height,
		rgb_to_camera: raw_image.rgb_to_camera,
	}
}
