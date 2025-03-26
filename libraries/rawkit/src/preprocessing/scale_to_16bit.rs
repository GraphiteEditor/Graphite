use crate::{RawImage, RawPixel, SubtractBlack};

impl RawImage {
	pub fn scale_to_16bit_fn(&self) -> impl Fn(RawPixel) -> u16 + use<> {
		let black_level = match self.black {
			SubtractBlack::CfaGrid(x) => x,
			_ => unreachable!(),
		};

		let maximum = self.maximum - black_level.iter().max().unwrap();
		let scale_to_16bit_multiplier = if maximum > 0 { u16::MAX as f64 / maximum as f64 } else { 1. };

		move |pixel: RawPixel| ((pixel.value as f64) * scale_to_16bit_multiplier).min(u16::MAX as f64).max(0.) as u16
	}
}
