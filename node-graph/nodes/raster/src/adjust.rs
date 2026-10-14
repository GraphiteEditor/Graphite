use graphic_types::Adjustment;
use no_std_types::color::Color;
use raster_types::{CPU, Raster};
use vector_types::Gradient;

pub trait Adjust {
	fn adjust(&mut self, adjustment: &Adjustment);
}

impl Adjust for Color {
	fn adjust(&mut self, adjustment: &Adjustment) {
		*self = adjustment.apply_to_color(*self);
	}
}

impl Adjust for Raster<CPU> {
	fn adjust(&mut self, adjustment: &Adjustment) {
		adjustment.apply(&mut self.data_mut().data);
	}
}

impl Adjust for Gradient {
	fn adjust(&mut self, adjustment: &Adjustment) {
		*self = self.map_colors(|color| adjustment.apply_to_color(*color));
	}
}
