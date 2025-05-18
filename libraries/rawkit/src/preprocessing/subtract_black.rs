use crate::RawPixel;
use crate::{RawImage, SubtractBlack};

impl RawImage {
	pub fn subtract_black_fn(&self) -> impl Fn(RawPixel) -> u16 + use<> {
		match self.black {
			SubtractBlack::CfaGrid(black_levels) => move |pixel: RawPixel| pixel.value.saturating_sub(black_levels[2 * (pixel.row % 2) + (pixel.column % 2)]),
			_ => todo!(),
		}
	}
}
