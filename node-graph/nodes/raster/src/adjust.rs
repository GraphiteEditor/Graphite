use no_std_types::color::Color;

pub trait Adjust<P> {
	fn adjust(&mut self, map_fn: impl Fn(&P) -> P);
}
impl Adjust<Color> for Color {
	fn adjust(&mut self, map_fn: impl Fn(&Color) -> Color) {
		*self = map_fn(self);
	}
}

#[cfg(feature = "std")]
mod adjust_std {
	use super::*;
	use core_types::table::Table;
	use raster_types::{CPU, Raster};
	use vector_types::GradientStops;

	impl Adjust<Color> for Table<Raster<CPU>> {
		fn adjust(&mut self, map_fn: impl Fn(&Color) -> Color) {
			for mut row in self.iter_mut() {
				for color in row.element_mut().data_mut().data.iter_mut() {
					*color = map_fn(color);
				}
			}
		}
	}
	impl Adjust<Color> for Table<Color> {
		fn adjust(&mut self, map_fn: impl Fn(&Color) -> Color) {
			for mut row in self.iter_mut() {
				*row.element_mut() = map_fn(row.element());
			}
		}
	}
	impl Adjust<Color> for Table<GradientStops> {
		fn adjust(&mut self, map_fn: impl Fn(&Color) -> Color) {
			for mut row in self.iter_mut() {
				row.element_mut().adjust(&map_fn);
			}
		}
	}
	impl Adjust<Color> for GradientStops {
		fn adjust(&mut self, map_fn: impl Fn(&Color) -> Color) {
			for color in self.color.iter_mut() {
				*color = map_fn(color);
			}
		}
	}
}
