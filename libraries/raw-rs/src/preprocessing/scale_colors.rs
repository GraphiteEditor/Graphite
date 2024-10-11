use crate::RawPixel;
use crate::{RawImage, SubtractBlack};

impl RawImage {
	pub fn scale_colors_fn(&self) -> impl Fn(RawPixel) -> u16 {
		let Some(mut white_balance_multiplier) = self.white_balance_multiplier else { todo!() };

		if white_balance_multiplier[1] == 0. {
			white_balance_multiplier[1] = 1.;
		}

		// TODO: Move this at its correct location when highlights are implemented correctly.
		let highlight = 0;

		let normalize_white_balance = if highlight == 0 {
			white_balance_multiplier.into_iter().fold(f64::INFINITY, f64::min)
		} else {
			white_balance_multiplier.into_iter().fold(f64::NEG_INFINITY, f64::max)
		};

		let black_level = match self.black {
			SubtractBlack::CfaGrid(x) => x,
			_ => unreachable!(),
		};

		let maximum = self.maximum - black_level.iter().max().unwrap();
		let final_multiplier = if normalize_white_balance > 0.00001 && maximum > 0 {
			let scale_to_16bit_multiplier = u16::MAX as f64 / maximum as f64;
			white_balance_multiplier.map(|x| x / normalize_white_balance * scale_to_16bit_multiplier)
		} else {
			[1., 1., 1., 1.]
		};

		move |pixel: RawPixel| {
			let cfa_index = 2 * (pixel.row % 2) + (pixel.column % 2);
			((pixel.value as f64) * final_multiplier[cfa_index]).min(u16::MAX as f64).max(0.) as u16
		}
	}
}
