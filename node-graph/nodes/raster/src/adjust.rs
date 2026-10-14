use core_types::color::Color;
use graphic_types::Adjustment;
use raster_kernels::adjustments::{adjust_color, adjust_colors};
use raster_types::{CPU, Raster};
use vector_types::Gradient;

pub trait Adjust {
	fn adjust(&mut self, adjustment: &Adjustment);
}

impl Adjust for Color {
	fn adjust(&mut self, adjustment: &Adjustment) {
		*self = adjust_color(adjustment, *self);
	}
}

impl Adjust for Raster<CPU> {
	fn adjust(&mut self, adjustment: &Adjustment) {
		adjust_colors(adjustment, &mut self.data_mut().data);
	}
}

impl Adjust for Gradient {
	fn adjust(&mut self, adjustment: &Adjustment) {
		*self = self.map_colors(|color| adjust_color(adjustment, *color));
	}
}
