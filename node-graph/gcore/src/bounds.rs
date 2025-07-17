use crate::Color;
use glam::{DAffine2, DVec2, IVec2, UVec2};

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

none_impl!(Vec<Color>);

impl BoundingBox for u32 {
	fn bounding_box(&self, _transform: DAffine2, _include_stroke: bool) -> Option<[DVec2; 2]> {
		text_bbox(i32_width(*self as i32))
	}
}

impl BoundingBox for f64 {
	fn bounding_box(&self, _transform: DAffine2, _include_stroke: bool) -> Option<[DVec2; 2]> {
		text_bbox(f64_width(*self))
	}
}

impl BoundingBox for DVec2 {
	fn bounding_box(&self, _transform: DAffine2, _include_stroke: bool) -> Option<[DVec2; 2]> {
		let width_x = f64_width(self.x);
		let width_y = f64_width(self.y);
		let total_width = width_x + width_y + 50.;
		text_bbox(total_width)
	}
}

impl BoundingBox for IVec2 {
	fn bounding_box(&self, _transform: DAffine2, _include_stroke: bool) -> Option<[DVec2; 2]> {
		let width_x = i32_width(self.x);
		let width_y = i32_width(self.y);
		let total_width = width_x + width_y + 50.;
		text_bbox(total_width)
	}
}

impl BoundingBox for UVec2 {
	fn bounding_box(&self, _transform: DAffine2, _include_stroke: bool) -> Option<[DVec2; 2]> {
		let width_x = i32_width(self.x as i32);
		let width_y = i32_width(self.y as i32);
		let total_width = width_x + width_y + 50.;
		text_bbox(total_width)
	}
}

impl BoundingBox for bool {
	fn bounding_box(&self, _transform: DAffine2, _include_stroke: bool) -> Option<[DVec2; 2]> {
		text_bbox(60.)
	}
}

impl BoundingBox for String {
	fn bounding_box(&self, _transform: DAffine2, _include_stroke: bool) -> Option<[DVec2; 2]> {
		let width = self.len() * 16;
		text_bbox(width as f64)
	}
}

impl BoundingBox for Option<Color> {
	fn bounding_box(&self, _transform: DAffine2, _include_stroke: bool) -> Option<[DVec2; 2]> {
		Some([(0., -5.).into(), (150., 110.).into()])
	}
}

fn f64_width(f64: f64) -> f64 {
	let left_of_decimal_width = i32_width(f64 as i32);
	left_of_decimal_width + 5. + 2. * 16.
}

fn i32_width(i32: i32) -> f64 {
	let number_of_digits = (i32.abs()).checked_ilog10().unwrap_or(0) + 1;
	let mut width = number_of_digits * 16;
	if i32 < 0 {
		width += 20;
	}
	width.into()
}

fn text_bbox(width: f64) -> Option<[DVec2; 2]> {
	Some([(-width / 2., 0.).into(), (width / 2., 30.).into()])
}
