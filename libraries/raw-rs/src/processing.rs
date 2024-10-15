use crate::CHANNELS_IN_RGB;
use fortuples::fortuples;

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

fortuples! {
	#[tuples::min_size(1)]
	#[tuples::max_size(8)]
	impl RawPixelTransform for #Tuple
	where
		#(#Member: RawPixelTransform),*
	{
		fn apply(&mut self, mut pixel: RawPixel) -> u16 {
			#(pixel.value = #self.apply(pixel);)*

			pixel.value
		}
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

fortuples! {
	#[tuples::min_size(1)]
	#[tuples::max_size(8)]
	impl PixelTransform for #Tuple
	where
		#(#Member: PixelTransform),*
	{
		fn apply(&mut self, mut pixel: Pixel) -> [u16; CHANNELS_IN_RGB] {
			#(pixel.values = #self.apply(pixel);)*

			pixel.values
		}
	}
}
