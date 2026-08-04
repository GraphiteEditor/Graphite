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

macro_rules! impl_raw_pixel_transform {
	($($idx:tt $t:tt),+) => {
		impl<$($t,)+> RawPixelTransform for ($($t,)+)
		where
			$($t: RawPixelTransform,)+
		{
			fn apply(&mut self, mut pixel: RawPixel) -> u16 {
				$(pixel.value = self.$idx.apply(pixel);)*

				pixel.value
			}
		}
	};
}

impl_raw_pixel_transform!(0 A);
impl_raw_pixel_transform!(0 A, 1 B);
impl_raw_pixel_transform!(0 A, 1 B, 2 C);
impl_raw_pixel_transform!(0 A, 1 B, 2 C, 3 D);
impl_raw_pixel_transform!(0 A, 1 B, 2 C, 3 D, 4 E);
impl_raw_pixel_transform!(0 A, 1 B, 2 C, 3 D, 4 E, 5 F);
impl_raw_pixel_transform!(0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G);
impl_raw_pixel_transform!(0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G, 7 H);

pub trait PixelTransform {
	fn apply(&mut self, pixel: Pixel) -> [u16; CHANNELS_IN_RGB];
}

impl<T: Fn(Pixel) -> [u16; CHANNELS_IN_RGB]> PixelTransform for T {
	fn apply(&mut self, pixel: Pixel) -> [u16; CHANNELS_IN_RGB] {
		self(pixel)
	}
}

macro_rules! impl_pixel_transform {
	($($idx:tt $t:tt),+) => {
		impl<$($t,)+> PixelTransform for ($($t,)+)
		where
			$($t: PixelTransform,)+
		{
			fn apply(&mut self, mut pixel: Pixel) -> [u16; CHANNELS_IN_RGB] {
				$(pixel.values = self.$idx.apply(pixel);)*

				pixel.values
			}
		}
	};
}

impl_pixel_transform!(0 A);
impl_pixel_transform!(0 A, 1 B);
impl_pixel_transform!(0 A, 1 B, 2 C);
impl_pixel_transform!(0 A, 1 B, 2 C, 3 D);
impl_pixel_transform!(0 A, 1 B, 2 C, 3 D, 4 E);
impl_pixel_transform!(0 A, 1 B, 2 C, 3 D, 4 E, 5 F);
impl_pixel_transform!(0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G);
impl_pixel_transform!(0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G, 7 H);
