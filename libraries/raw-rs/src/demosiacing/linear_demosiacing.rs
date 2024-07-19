use crate::{Image, RawImage};

fn average(data: &[u16], indexes: &[i64]) -> u16 {
	let mut sum: u32 = 0;
	let mut count = 0;
	for &index in indexes {
		if index >= 0 && index < data.len() as i64 {
			sum += data[index as usize] as u32;
			count += 1;
		}
	}

	(sum / count) as u16
}

pub fn linear_demosiac(raw_image: RawImage) -> Image<u16> {
	match raw_image.cfa_pattern {
		[0, 1, 1, 2] => linear_demosiac_rggb(raw_image),
		_ => todo!(),
	}
}

fn linear_demosiac_rggb(mut raw_image: RawImage) -> Image<u16> {
	let width = raw_image.width as i64;
	let height = raw_image.height as i64;

	for row in 0..height {
		for col in 0..width {
			let pixel_index = row * width + col;
			let vertical_indexes = [pixel_index + width, pixel_index - width];
			let horizontal_indexes = [pixel_index + 1, pixel_index - 1];
			let cross_indexes = [pixel_index + width, pixel_index - width, pixel_index + 1, pixel_index - 1];
			let diagonal_indexes = [pixel_index + width + 1, pixel_index - width + 1, pixel_index + width - 1, pixel_index - width - 1];
			if row % 2 == 0 {
				if col % 2 == 0 {
					let indexes: Vec<_> = cross_indexes.iter().map(|x| 3 * x + 1).collect();
					raw_image.data[3 * (pixel_index as usize) + 1] = average(&raw_image.data, &indexes);

					let indexes: Vec<_> = diagonal_indexes.iter().map(|x| 3 * x + 2).collect();
					raw_image.data[3 * (pixel_index as usize) + 2] = average(&raw_image.data, &indexes);
				} else {
					let indexes: Vec<_> = horizontal_indexes.iter().map(|x| 3 * x).collect();
					raw_image.data[3 * (pixel_index as usize)] = average(&raw_image.data, &indexes);

					let indexes: Vec<_> = vertical_indexes.iter().map(|x| 3 * x + 2).collect();
					raw_image.data[3 * (pixel_index as usize) + 2] = average(&raw_image.data, &indexes);
				}
			} else {
				if col % 2 == 0 {
					let indexes: Vec<_> = vertical_indexes.iter().map(|x| 3 * x).collect();
					raw_image.data[3 * (pixel_index as usize)] = average(&raw_image.data, &indexes);

					let indexes: Vec<_> = horizontal_indexes.iter().map(|x| 3 * x + 2).collect();
					raw_image.data[3 * (pixel_index as usize) + 2] = average(&raw_image.data, &indexes);
				} else {
					let indexes: Vec<_> = cross_indexes.iter().map(|x| 3 * x + 1).collect();
					raw_image.data[3 * (pixel_index as usize) + 1] = average(&raw_image.data, &indexes);

					let indexes: Vec<_> = diagonal_indexes.iter().map(|x| 3 * x).collect();
					raw_image.data[3 * (pixel_index as usize)] = average(&raw_image.data, &indexes);
				}
			}
		}
	}

	Image {
		channels: 3,
		data: raw_image.data,
		width: raw_image.width,
		height: raw_image.height,
	}
}
