use no_std_types::color::Color;
use raster_types::{CPU, Raster};
use vector_types::Gradient;

pub trait Adjust<P> {
	fn adjust(&mut self, map_fn: impl Fn(&P) -> P);
}

impl Adjust<Color> for Color {
	fn adjust(&mut self, map_fn: impl Fn(&Color) -> Color) {
		*self = map_fn(self);
	}
}

impl Adjust<Color> for Raster<CPU> {
	fn adjust(&mut self, map_fn: impl Fn(&Color) -> Color) {
		for color in self.data_mut().data.iter_mut() {
			*color = map_fn(color);
		}
	}
}

impl Adjust<Color> for Gradient {
	fn adjust(&mut self, map_fn: impl Fn(&Color) -> Color) {
		*self = self.map_colors(map_fn);
	}
}
