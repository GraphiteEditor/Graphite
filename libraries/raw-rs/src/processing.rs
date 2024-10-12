use crate::CHANNELS_IN_RGB;

#[derive(Clone, Copy)]
pub struct RawPixel {
	pub value: u16,
	pub row: usize,
	pub column: usize,
}

#[derive(Clone, Copy)]
pub struct Pixel {
	pub values: [u16; CHANNELS_IN_RGB],
	pub row: usize,
	pub column: usize,
}

pub trait RawPixelTransform {
	fn apply(&mut self, pixel: RawPixel) -> u16;
}

impl<T: Fn(RawPixel) -> u16> RawPixelTransform for T {
	fn apply(&mut self, pixel: RawPixel) -> u16 {
		self(pixel)
	}
}

impl<T1: RawPixelTransform, T2: RawPixelTransform> RawPixelTransform for (T1, T2) {
	fn apply(&mut self, mut pixel: RawPixel) -> u16 {
		pixel.value = self.0.apply(pixel);
		pixel.value = self.1.apply(pixel);

		pixel.value
	}
}

pub trait PixelTransform {
	fn apply(&mut self, pixel: Pixel) -> [u16; CHANNELS_IN_RGB];
}

impl<T: Fn(Pixel) -> [u16; CHANNELS_IN_RGB]> PixelTransform for T {
	fn apply(&mut self, pixel: Pixel) -> [u16; CHANNELS_IN_RGB] {
		self(pixel)
	}
}

impl<T1: PixelTransform, T2: PixelTransform> PixelTransform for (T1, T2) {
	fn apply(&mut self, mut pixel: Pixel) -> [u16; CHANNELS_IN_RGB] {
		pixel.values = self.0.apply(pixel);
		pixel.values = self.1.apply(pixel);

		pixel.values
	}
}
