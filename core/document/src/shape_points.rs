use std::{fmt, ops::Add};

use kurbo::{PathEl, Point, Vec2};
use log::info;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapePoints {
	center: kurbo::Point,
	extent: kurbo::Vec2,
	sides: u8,
}

impl ShapePoints {
	/// A new shape from center, a point and the number of points.
	#[inline]
	pub fn new(center: impl Into<Point>, extent: impl Into<Vec2>, sides: u8) -> ShapePoints {
		ShapePoints {
			center: center.into(),
			extent: extent.into(),
			sides,
		}
	}

	// Gets the angle in radians between the longest line from the center and the apothem.
	#[inline]
	pub fn apothem_offset_angle(&self) -> f64 {
		std::f64::consts::PI / (self.sides as f64)
	}

	// Gets the apothem (the shortest distance from the center to the edge)
	#[inline]
	pub fn apothem(&self) -> f64 {
		self.apothem_offset_angle().cos() * (self.sides as f64)
	}

	// Gets the length of one side
	#[inline]
	pub fn side_length(&self) -> f64 {
		self.apothem_offset_angle().sin() * (self.sides as f64) * 2f64
	}
}

impl std::fmt::Display for ShapePoints {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		fn rotate(v: &Vec2, theta: f64) -> Vec2 {
			let cosine = theta.cos();
			let sine = theta.sin();
			Vec2::new(v.x * cosine - v.y * sine, v.x * sine + v.y * cosine)
		}
		for i in 0..self.sides {
			let radians = self.apothem_offset_angle() * ((i * 2 + (self.sides % 2)) as f64);
			let offset = rotate(&self.extent, radians);
			let point = self.center + offset;
			write!(f, "{},{} ", point.x, point.y)?;
		}

		Ok(())
	}
}

#[doc(hidden)]
pub struct ShapePathIter {
	shape: ShapePoints,
	ix: usize,
}

impl Iterator for ShapePathIter {
	type Item = PathEl;

	fn next(&mut self) -> Option<PathEl> {
		fn rotate(v: &Vec2, theta: f64) -> Vec2 {
			let cosine = theta.cos();
			let sine = theta.sin();
			Vec2::new(v.x * cosine - v.y * sine, v.x * sine + v.y * cosine)
		}
		self.ix += 1;
		match self.ix {
			1 => Some(PathEl::MoveTo(self.shape.center + self.shape.extent)),
			_ => {
				let radians = self.shape.apothem_offset_angle() * ((self.ix * 2 + (self.shape.sides % 2) as usize) as f64);
				let offset = rotate(&self.shape.extent, radians);
				let point = self.shape.center + offset;
				Some(PathEl::LineTo(point))
			}
		}
	}
}

impl Add<Vec2> for ShapePoints {
	type Output = ShapePoints;

	#[inline]
	fn add(self, movement: Vec2) -> ShapePoints {
		ShapePoints {
			center: self.center + movement,
			extent: self.extent,
			sides: self.sides,
		}
	}
}

impl kurbo::Shape for ShapePoints {
	type PathElementsIter = ShapePathIter;

	fn path_elements(&self, _tolerance: f64) -> Self::PathElementsIter {
		todo!()
	}

	#[inline]
	fn area(&self) -> f64 {
		self.apothem() * self.perimeter(2.1)
	}

	#[inline]
	fn perimeter(&self, _accuracy: f64) -> f64 {
		self.side_length() * (self.sides as f64)
	}

	fn winding(&self, _pt: Point) -> i32 {
		todo!()
	}

	fn bounding_box(&self) -> kurbo::Rect {
		todo!()
	}
}
