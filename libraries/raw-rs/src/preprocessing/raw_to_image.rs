use crate::RawImage;

pub fn raw_to_image(raw_image: RawImage) -> RawImage {
	// let image = Vec::with_capacity(raw_image.width*raw_image.height*3);

	for row in 0..raw_image.height {
		for col in 0..raw_image.width {
			let value = raw_image.data[row * raw_image.width + col];
		}
	}

	raw_image
}
