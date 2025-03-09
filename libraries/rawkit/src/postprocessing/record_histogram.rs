use crate::{CHANNELS_IN_RGB, Histogram, Pixel, PixelTransform, RawImage};

impl RawImage {
	pub fn record_histogram_fn(&self) -> RecordHistogram {
		RecordHistogram::new()
	}
}

pub struct RecordHistogram {
	pub histogram: Histogram,
}

impl RecordHistogram {
	fn new() -> RecordHistogram {
		RecordHistogram {
			histogram: [[0; 0x2000]; CHANNELS_IN_RGB],
		}
	}
}

impl PixelTransform for &mut RecordHistogram {
	fn apply(&mut self, pixel: Pixel) -> [u16; CHANNELS_IN_RGB] {
		self.histogram
			.iter_mut()
			.zip(pixel.values.iter())
			.for_each(|(histogram, &value)| histogram[value as usize >> CHANNELS_IN_RGB] += 1);
		pixel.values
	}
}
