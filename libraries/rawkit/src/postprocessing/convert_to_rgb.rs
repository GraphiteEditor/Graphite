use crate::{CHANNELS_IN_RGB, Pixel, RawImage};

impl RawImage {
	pub fn convert_to_rgb_fn(&self) -> impl Fn(Pixel) -> [u16; CHANNELS_IN_RGB] + use<> {
		let Some(camera_to_rgb) = self.camera_to_rgb else { todo!() };

		move |pixel: Pixel| {
			std::array::from_fn(|i| i)
				.map(|i| camera_to_rgb[i].iter().zip(pixel.values.iter()).map(|(&coeff, &value)| coeff * value as f64).sum())
				.map(|x: f64| (x as u16).clamp(0, u16::MAX))
		}
	}
}
