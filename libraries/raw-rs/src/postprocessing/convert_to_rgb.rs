use crate::Image;

const CHANNELS_IN_RGB: usize = 3;

pub fn convert_to_rgb(mut image: Image<u16>) -> Image<u16> {
	let Some(rgb_to_camera) = image.rgb_to_camera else { return image };

	// Rarely this might be 4 instead of 3 if an obscure Bayer filter is used, such as RGBE or CYGM, instead of the typical RGGB.
	// See: <https://github.com/GraphiteEditor/Graphite/pull/1923#discussion_r1725070342>.
	let channels = image.channels as usize;
	let mut data = Vec::with_capacity(CHANNELS_IN_RGB * image.width * image.height);
	let mut histogram = [[0; 0x2000]; CHANNELS_IN_RGB];

	for i in 0..(image.height * image.width) {
		let start = i * channels;
		let end = start + channels;
		let input_pixel = &mut image.data[start..end];

		let mut output_pixel = [0.; CHANNELS_IN_RGB];
		for (channel, &value) in input_pixel.iter().enumerate() {
			output_pixel[0] += rgb_to_camera[0][channel] * value as f64;
			output_pixel[1] += rgb_to_camera[1][channel] * value as f64;
			output_pixel[2] += rgb_to_camera[2][channel] * value as f64;
		}

		for (output_pixel_channel, histogram_channel) in output_pixel.iter().zip(histogram.iter_mut()) {
			let final_sum = (*output_pixel_channel as u16).clamp(0, u16::MAX);

			histogram_channel[final_sum as usize >> CHANNELS_IN_RGB] += 1;

			data.push(final_sum);
		}
	}

	image.data = data;
	image.histogram = Some(histogram);
	image.channels = CHANNELS_IN_RGB as u8;

	image
}
