use crate::{Pixel, RawImage, CHANNELS_IN_RGB};

impl RawImage {
	pub fn convert_to_rgb_fn(&self) -> impl Fn(Pixel) -> [u16; CHANNELS_IN_RGB] {
		let Some(rgb_to_camera) = self.rgb_to_camera else { todo!() };

		move |pixel: Pixel| {
			std::array::from_fn(|i| i)
				.map(|i| rgb_to_camera[i].iter().zip(pixel.values.iter()).map(|(&coeff, &value)| coeff * value as f64).sum())
				.map(|x: f64| (x as u16).clamp(0, u16::MAX))
		}
	}
}
