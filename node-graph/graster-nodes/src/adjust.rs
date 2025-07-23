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
		if let Some(v) = self {
			*v = map_fn(v)
		}
	}
}

#[cfg(feature = "std")]
mod adjust_std {
	use super::*;
	use graphene_core::gradient::GradientStops;
	use graphene_core::raster_types::{CPU, RasterDataTable};
	impl Adjust<Color> for GradientStops {
		fn adjust(&mut self, map_fn: impl Fn(&Color) -> Color) {
			for (_pos, c) in self.iter_mut() {
				*c = map_fn(c);
			}
		}
	}
	impl Adjust<Color> for RasterDataTable<CPU> {
		fn adjust(&mut self, map_fn: impl Fn(&Color) -> Color) {
			for instance in self.instance_mut_iter() {
				for c in instance.instance.data_mut().data.iter_mut() {
					*c = map_fn(c);
				}
			}
		}
	}
}
