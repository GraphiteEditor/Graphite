use crate::Image;

const CHANNELS_IN_RGB: usize = 3;

pub fn convert_to_rgb(mut image: Image<u16>) -> Image<u16> {
	let Some(rgb_to_camera) = image.rgb_to_camera else { return image };

	let mut histogram = [[0; 0x2000]; CHANNELS_IN_RGB];

	for input_pixel in image.data.chunks_mut(3) {
		let mut output_pixel = [0.; CHANNELS_IN_RGB];
		for (channel, &value) in input_pixel.iter().enumerate() {
			output_pixel[0] += rgb_to_camera[0][channel] * value as f64;
			output_pixel[1] += rgb_to_camera[1][channel] * value as f64;
			output_pixel[2] += rgb_to_camera[2][channel] * value as f64;
		}

		for (index, (output_pixel_channel, histogram_channel)) in output_pixel.iter().zip(histogram.iter_mut()).enumerate() {
			let final_sum = (*output_pixel_channel as u16).clamp(0, u16::MAX);

			histogram_channel[final_sum as usize >> CHANNELS_IN_RGB] += 1;

			input_pixel[index] = final_sum;
		}
	}

	// `image.channels` might be 4 instead of 3 if an obscure Bayer filter is used, such as RGBE or CYGM, instead of the typical RGGB.
	// See: <https://github.com/GraphiteEditor/Graphite/pull/1923#discussion_r1725070342>.
	// Therefore it is converted to 3 here.
	image.channels = CHANNELS_IN_RGB as u8;
	image.histogram = Some(histogram);

	image
}
