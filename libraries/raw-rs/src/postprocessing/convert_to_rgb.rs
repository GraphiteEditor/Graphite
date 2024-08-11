use crate::Image;

pub fn convert_to_rgb(mut image: Image<u16>) -> Image<u16> {
	println!("data: {:?}", &image.data[..10]);

	if let Some(rgb_to_camera) = image.rgb_to_camera {
		let channels = image.channels as usize;
		let mut data = Vec::with_capacity(3 * image.width * image.height);

		for i in 0..(image.height * image.width) {
			let input_pixel = &mut image.data[channels * i..channels * (i + 1)];
			let mut output_pixel = [0.; 3];
			for c in 0..channels {
				for i in 0..3 {
					output_pixel[i] += rgb_to_camera[i][c] * input_pixel[c] as f64;
				}
			}

			for i in 0..3 {
				output_pixel[i] = output_pixel[i].min(u16::MAX as f64).max(0.);
			}

			data.extend(output_pixel.iter().map(|&x| x as u16))
		}

		image.data = data;
		image.channels = 3;
	}

	image
}
