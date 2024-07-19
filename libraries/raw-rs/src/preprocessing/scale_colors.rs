use crate::RawImage;

const XYZ_TO_RGB: [[f64; 3]; 3] = [[0.412453, 0.357580, 0.180423], [0.212671, 0.715160, 0.072169], [0.019334, 0.119193, 0.950227]];

pub fn scale_colors(mut raw_image: RawImage) -> RawImage {
	if let Some(camera_to_xyz) = raw_image.camera_to_xyz {
		let mut camera_to_rgb = [[0.; 3]; 3];
		for i in 0..3 {
			for j in 0..3 {
				for k in 0..3 {
					camera_to_rgb[i][j] += camera_to_xyz[i * 3 + k] * XYZ_TO_RGB[k][j];
				}
			}
		}

		let mut white_balance_multiplier = camera_to_rgb.map(|x| 1. / x.iter().sum::<f64>());

		if white_balance_multiplier[1] == 0. {
			white_balance_multiplier[1] = 1.;
		}

		// TODO: Move this at its correct location when highlights are implemented correctly.
		let highlight = 0;

		let normalize_white_balance = if highlight == 0 {
			white_balance_multiplier.iter().fold(f64::INFINITY, |a, &b| a.min(b))
		} else {
			white_balance_multiplier.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b))
		};

		let final_multiplier = if normalize_white_balance > 0.00001 && raw_image.maximum > 0 {
			let scale_to_16bit_multiplier = u16::MAX as f64 / raw_image.maximum as f64;
			white_balance_multiplier.map(|x| x / normalize_white_balance * scale_to_16bit_multiplier)
		} else {
			[1., 1., 1.]
		};

		for i in 0..(raw_image.height * raw_image.width) {
			for (c, multiplier) in final_multiplier.iter().enumerate() {
				raw_image.data[3 * i + c] = ((raw_image.data[3 * i + c] as f64) * multiplier).min(u16::MAX as f64).max(0.) as u16;
			}
		}
	}

	raw_image
}
