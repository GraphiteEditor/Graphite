use crate::{RawImage, RawPixel};

impl RawImage {
	pub fn scale_white_balance_fn(&self) -> impl Fn(RawPixel) -> u16 + use<> {
		let Some(mut white_balance) = self.white_balance else { todo!() };

		if white_balance[1] == 0. {
			white_balance[1] = 1.;
		}

		// TODO: Move this at its correct location when highlights are implemented correctly.
		let highlight = 0;

		let normalization_factor = if highlight == 0 {
			white_balance.into_iter().fold(f64::INFINITY, f64::min)
		} else {
			white_balance.into_iter().fold(f64::NEG_INFINITY, f64::max)
		};

		let normalized_white_balance = if normalization_factor > 0.00001 {
			white_balance.map(|x| x / normalization_factor)
		} else {
			[1., 1., 1., 1.]
		};

		move |pixel: RawPixel| {
			let cfa_index = 2 * (pixel.row % 2) + (pixel.column % 2);
			((pixel.value as f64) * normalized_white_balance[cfa_index]).min(u16::MAX as f64).max(0.) as u16
		}
	}
}
