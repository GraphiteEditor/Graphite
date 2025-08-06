use graphene_core_shaders::color::Color;

pub trait Adjust<P> {
	fn adjust(&mut self, map_fn: impl Fn(&P) -> P);
}
impl Adjust<Color> for Color {
	fn adjust(&mut self, map_fn: impl Fn(&Color) -> Color) {
		*self = map_fn(self);
	}
}
impl Adjust<Color> for Option<Color> {
	fn adjust(&mut self, map_fn: impl Fn(&Color) -> Color) {
		if let Some(color) = self {
			*color = map_fn(color)
		}
	}
}

#[cfg(feature = "std")]
mod adjust_std {
	use super::*;
	use graphene_core::gradient::GradientStops;
	use graphene_core::raster_types::{CPU, Raster};
	use graphene_core::table::Table;

	impl Adjust<Color> for GradientStops {
		fn adjust(&mut self, map_fn: impl Fn(&Color) -> Color) {
			for (_, color) in self.iter_mut() {
				*color = map_fn(color);
			}
		}
	}
	impl Adjust<Color> for Table<Raster<CPU>> {
		fn adjust(&mut self, map_fn: impl Fn(&Color) -> Color) {
			for row in self.iter_mut() {
				for color in row.element.data_mut().data.iter_mut() {
					*color = map_fn(color);
				}
			}
		}
	}
}
