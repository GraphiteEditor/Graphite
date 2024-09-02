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

fn linear_demosaic_rggb(raw_image: RawImage) -> Image<u16> {
	let mut image = vec![0; raw_image.width * raw_image.height * 3];
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

			let pixel_index = pixel_index as usize;
			match (row % 2 == 0, col % 2 == 0) {
				(true, true) => {
					image[3 * pixel_index] = raw_image.data[pixel_index];
					image[3 * pixel_index + 1] = average(&raw_image.data, cross_indexes.into_iter());
					image[3 * pixel_index + 2] = average(&raw_image.data, diagonal_indexes.into_iter());
				}
				(true, false) => {
					image[3 * pixel_index] = average(&raw_image.data, horizontal_indexes.into_iter());
					image[3 * pixel_index + 1] = raw_image.data[pixel_index];
					image[3 * pixel_index + 2] = average(&raw_image.data, vertical_indexes.into_iter());
				}
				(false, true) => {
					image[3 * pixel_index] = average(&raw_image.data, vertical_indexes.into_iter());
					image[3 * pixel_index + 1] = raw_image.data[pixel_index];
					image[3 * pixel_index + 2] = average(&raw_image.data, horizontal_indexes.into_iter());
				}
				(false, false) => {
					image[3 * pixel_index] = average(&raw_image.data, diagonal_indexes.into_iter());
					image[3 * pixel_index + 1] = average(&raw_image.data, cross_indexes.into_iter());
					image[3 * pixel_index + 2] = raw_image.data[pixel_index];
				}
			}
		}
	}

	Image {
		channels: 3,
		data: image,
		width: raw_image.width,
		height: raw_image.height,
		rgb_to_camera: raw_image.rgb_to_camera,
		histogram: None,
	}
}
