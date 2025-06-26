use crate::Color;
use glam::{DAffine2, DVec2};

pub trait BoundingBox {
	fn bounding_box(&self, transform: DAffine2, include_stroke: bool) -> Option<[DVec2; 2]>;
}

macro_rules! none_impl {
	($t:path) => {
		impl BoundingBox for $t {
			fn bounding_box(&self, _transform: DAffine2, _include_stroke: bool) -> Option<[DVec2; 2]> {
				None
			}
		}
	};
}

none_impl!(String);
none_impl!(bool);
none_impl!(f32);
none_impl!(f64);
none_impl!(DVec2);
none_impl!(Option<Color>);
none_impl!(Vec<Color>);
