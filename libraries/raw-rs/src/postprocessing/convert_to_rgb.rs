use crate::{Pixel, PixelTransform, RawImage};

const CHANNELS_IN_RGB: usize = 3;

impl RawImage {
	pub fn convert_to_rgb_fn(&self) -> impl Fn(Pixel) -> Pixel {
		let Some(rgb_to_camera) = self.rgb_to_camera else { todo!() };

		move |mut pixel: Pixel| {
			let [red, blue, green] = std::array::from_fn(|i| i).map(|i| rgb_to_camera[i][0] * pixel.red as f64 + rgb_to_camera[i][1] * pixel.blue as f64 + rgb_to_camera[i][2] * pixel.green as f64);

			pixel.red = (red as u16).clamp(0, u16::MAX);
			pixel.green = (green as u16).clamp(0, u16::MAX);
			pixel.blue = (blue as u16).clamp(0, u16::MAX);
			pixel
		}
	}

	pub fn record_histogram_fn(&self) -> RecordHistogram {
		RecordHistogram::new()
	}
}

pub struct RecordHistogram {
	pub histogram: [[usize; 0x2000]; CHANNELS_IN_RGB],
}

impl RecordHistogram {
	fn new() -> RecordHistogram {
		RecordHistogram {
			histogram: [[0; 0x2000]; CHANNELS_IN_RGB],
		}
	}
}

impl PixelTransform for &mut RecordHistogram {
	fn apply(&mut self, pixel: Pixel) -> Pixel {
		self.histogram[0][pixel.red as usize >> CHANNELS_IN_RGB] += 1;
		self.histogram[1][pixel.blue as usize >> CHANNELS_IN_RGB] += 1;
		self.histogram[2][pixel.green as usize >> CHANNELS_IN_RGB] += 1;
		pixel
	}
}
