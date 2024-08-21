use crate::Image;

pub fn convert_to_rgb(mut image: Image<u16>) -> Image<u16> {
	let Some(rgb_to_camera) = image.rgb_to_camera else { return image };

	let channels = image.channels as usize;
	let mut data = Vec::with_capacity(3 * image.width * image.height);
	let mut histogram = [[0; 0x2000]; 3];

	for i in 0..(image.height * image.width) {
		let start = i * channels;
		let end = start + channels;
		let input_pixel = &mut image.data[start..end];

		let mut output_pixel = [0.; 3];
		for (channel, &value) in input_pixel.iter().enumerate() {
			output_pixel[0] += rgb_to_camera[0][channel] * value as f64;
			output_pixel[1] += rgb_to_camera[1][channel] * value as f64;
			output_pixel[2] += rgb_to_camera[2][channel] * value as f64;
		}

		for i in 0..3 {
			let final_sum = output_pixel[i].min(u16::MAX as f64).max(0.);

			histogram[i][final_sum as usize >> 3] += 1;

			data.push(final_sum as u16);
		}
	}

	image.data = data;
	image.channels = 3;
	image.histogram = Some(histogram);

	image
}
