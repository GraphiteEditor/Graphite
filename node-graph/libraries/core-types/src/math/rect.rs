use crate::math::quad::Quad;
use glam::{DAffine2, DVec2};

#[derive(Debug, Clone, Default, Copy, PartialEq)]
/// An axis aligned rect defined by two vertices.
pub struct Rect(pub [DVec2; 2]);

impl Rect {
	/// Create a zero sized quad at the point
	#[must_use]
	pub fn from_point(point: DVec2) -> Self {
		Self([point; 2])
	}

	/// Convert a box defined by two corner points to a quad.
	#[must_use]
	pub fn from_box(bbox: [DVec2; 2]) -> Self {
		Self([bbox[0].min(bbox[1]), bbox[0].max(bbox[1])])
	}

	/// Create a quad from the center and offset (distance from center to middle of an edge)
	#[must_use]
	pub fn from_square(center: DVec2, offset: f64) -> Self {
		Self::from_box([center - offset, center + offset])
	}

	/// Create an AABB from an iter of points, returning None if empty.
	#[must_use]
	pub fn point_iter(points: impl Iterator<Item = DVec2>) -> Option<Self> {
		let mut bounds = None;
		for point in points {
			let bounds = bounds.get_or_insert(Self::from_point(point));
			bounds[0] = bounds[0].min(point);
			bounds[1] = bounds[1].max(point);
		}
		bounds
	}

	/// Get all the edges in the rect.
	#[must_use]
	pub fn edges(&self) -> [[DVec2; 2]; 4] {
		let corners = [self[0], DVec2::new(self[0].x, self[1].y), self[1], DVec2::new(self[1].y, self[0].x)];
		[[corners[0], corners[1]], [corners[1], corners[2]], [corners[2], corners[3]], [corners[3], corners[0]]]
	}

	/// Gets the center of a rect
	#[must_use]
	pub fn center(&self) -> DVec2 {
		self.0.iter().sum::<DVec2>() / 2.
	}

	/// Take the outside bounds of two axis aligned rectangles, which are defined by two corner points.
	#[must_use]
	pub fn combine_bounds(a: Self, b: Self) -> Self {
		Self::from_box([a[0].min(b[0]), a[1].max(b[1])])
	}

	/// Expand a rect by a certain amount on top/bottom and on left/right
	#[must_use]
	pub fn expand_by(&self, x: f64, y: f64) -> Self {
		let delta = DVec2::new(x, y);
		Self::from_box([self[0] - delta, self[1] + delta])
	}

	/// Checks if two rects intersect
	#[must_use]
	pub fn intersects(&self, other: Self) -> bool {
		let [mina, maxa] = [self[0].min(self[1]), self[0].max(self[1])];
		let [minb, maxb] = [other[0].min(other[1]), other[0].max(other[1])];
		mina.x <= maxb.x && minb.x <= maxa.x && mina.y <= maxb.y && minb.y <= maxa.y
	}

	/// Does this rect contain a point
	#[must_use]
	pub fn contains(&self, p: DVec2) -> bool {
		(self[0].x < p.x && p.x < self[1].x) && (self[0].y < p.y && p.y < self[1].y)
	}

	#[must_use]
	pub fn min(&self) -> DVec2 {
		self.0[0].min(self.0[1])
	}

	#[must_use]
	pub fn max(&self) -> DVec2 {
		self.0[0].max(self.0[1])
	}

	#[must_use]
	pub fn translate(&self, offset: DVec2) -> Self {
		Self([self.0[0] + offset, self.0[1] + offset])
	}
}

impl std::ops::Mul<Rect> for DAffine2 {
	type Output = Quad;

	fn mul(self, rhs: Rect) -> Self::Output {
		self * Quad::from_box(rhs.0)
	}
}

impl std::ops::Index<usize> for Rect {
	type Output = DVec2;
	fn index(&self, index: usize) -> &Self::Output {
		&self.0[index]
	}
}
impl std::ops::IndexMut<usize> for Rect {
	fn index_mut(&mut self, index: usize) -> &mut Self::Output {
		&mut self.0[index]
	}
}

impl From<Rect> for Quad {
	fn from(val: Rect) -> Self {
		Quad::from_box(val.0)
	}
}
