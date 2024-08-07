use crate::RawImage;

pub fn raw_to_image(mut raw_image: RawImage) -> RawImage {
	let mut image = Vec::with_capacity(raw_image.width * raw_image.height * 3);

	for row in 0..raw_image.height {
		for col in 0..raw_image.width {
			let mut pixel = [0u16; 3];
			let color_index = raw_image.cfa_pattern[2 * (row % 2) + (col % 2)];
			pixel[color_index as usize] = raw_image.data[row * raw_image.width + col];
			image.extend_from_slice(&pixel);
		}
	}

	raw_image.data = image;
	raw_image
}
