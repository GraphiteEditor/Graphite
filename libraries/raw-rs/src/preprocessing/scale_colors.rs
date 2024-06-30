use crate::RawImage;

const XYZ_TO_RGB: [[f64; 3]; 3] = [[0.412453, 0.357580, 0.180423], [0.212671, 0.715160, 0.072169], [0.019334, 0.119193, 0.950227]];

pub fn scale_colors(mut raw_image: RawImage) -> RawImage {
	if let Some(cam_to_xyz) = raw_image.cam_to_xyz {
		let mut cam_rgb = [[0.0f64; 3]; 3];
		for i in 0..3 {
			for j in 0..3 {
				for k in 0..3 {
					cam_rgb[i][j] += cam_to_xyz[i * 3 + k] * XYZ_TO_RGB[k][j];
				}
			}
		}

		let mut pre_mul = cam_rgb.map(|x| 1. / x.iter().sum::<f64>());

		if pre_mul[1] == 0. {
			pre_mul[1] = 1.;
		}

		// TODO: Move this at its correct location when highlights are implemented correctly.
		let highlight = 0;

		let dmin = pre_mul.iter().fold(f64::INFINITY, |a, &b| a.min(b));
		let dmax = if highlight == 0 { dmin } else { pre_mul.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)) };

		let scale_mul = if dmax > 0.00001 && raw_image.maximum > 0 {
			pre_mul.map(|x| x / dmax * (u16::MAX as f64) / raw_image.maximum as f64)
		} else {
			[1.0, 1.0, 1.0]
		};

		for i in 0..(raw_image.height * raw_image.width) {
			for c in 0..3 {
				raw_image.data[i + c] = ((raw_image.data[i + c] as f64) * scale_mul[c]).min(u16::MAX as f64) as u16;
			}
		}
	}

	raw_image
}
