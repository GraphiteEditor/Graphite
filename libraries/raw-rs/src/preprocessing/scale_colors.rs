use crate::RawImage;

pub fn scale_colors(mut raw_image: RawImage) -> RawImage {
	let Some(mut white_balance_multiplier) = raw_image.white_balance_multiplier else {
		return raw_image;
	};

	if white_balance_multiplier[1] == 0. {
		white_balance_multiplier[1] = 1.;
	}

	// TODO: Move this at its correct location when highlights are implemented correctly.
	let highlight = 0;

	let normalize_white_balance = if highlight == 0 {
		white_balance_multiplier.iter().copied().fold(f64::INFINITY, f64::min)
	} else {
		white_balance_multiplier.iter().copied().fold(f64::NEG_INFINITY, f64::max)
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

	raw_image
}
