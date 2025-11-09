use glam::{DAffine2, DVec2};

#[derive(Debug, Clone, Default, Copy)]
/// A quad defined by four vertices. Clockwise from the top left:
///
/// `top_left`, `top_right`, `bottom_right`, `bottom_left`.
pub struct Quad(pub [DVec2; 4]);

impl Quad {
	/// Get the top left corner of the quad.
	pub fn top_left(&self) -> DVec2 {
		self.0[0]
	}

	/// Get the top right corner of the quad.
	pub fn top_right(&self) -> DVec2 {
		self.0[1]
	}

	/// Get the bottom right corner of the quad.
	pub fn bottom_right(&self) -> DVec2 {
		self.0[2]
	}

	/// Get the bottom left corner of the quad.
	pub fn bottom_left(&self) -> DVec2 {
		self.0[3]
	}

	/// Create a zero-sized quad at the point.
	pub fn from_point(point: DVec2) -> Self {
		Self([point; 4])
	}

	/// Convert a box defined by two corner points to a quad. The points must be given as `minimum (top left)` then `maximum (bottom right)`.
	pub fn from_box(bbox: [DVec2; 2]) -> Self {
		let size = bbox[1] - bbox[0];
		Self([bbox[0], bbox[0] + size * DVec2::X, bbox[1], bbox[0] + size * DVec2::Y])
	}

	/// Create a quad from the center and offset (distance from center to middle of an edge)
	pub fn from_square(center: DVec2, offset: f64) -> Self {
		Self::from_box([center - offset, center + offset])
	}

	/// Get all the edges in the quad.
	pub fn all_edges(&self) -> [[DVec2; 2]; 4] {
		[[self.0[0], self.0[1]], [self.0[1], self.0[2]], [self.0[2], self.0[3]], [self.0[3], self.0[0]]]
	}

	/// Get two edges as bases.
	pub fn edges(&self) -> [[DVec2; 2]; 2] {
		[[self.0[0], self.0[1]], [self.0[1], self.0[2]]]
	}

	/// Returns true only if the width and height are both greater than or equal to the given width.
	pub fn all_sides_at_least_width(&self, width: f64) -> bool {
		self.edges().into_iter().all(|[a, b]| (a - b).length_squared() >= width.powi(2))
	}

	/// Generates the axis aligned bounding box of the quad
	pub fn bounding_box(&self) -> [DVec2; 2] {
		[
			self.0.into_iter().reduce(|a, b| a.min(b)).unwrap_or_default(),
			self.0.into_iter().reduce(|a, b| a.max(b)).unwrap_or_default(),
		]
	}

	/// Gets the center of a quad
	pub fn center(&self) -> DVec2 {
		self.0.iter().sum::<DVec2>() / 4.
	}

	/// Take the outside bounds of two axis aligned rectangles, which are defined by two corner points.
	pub fn combine_bounds(a: [DVec2; 2], b: [DVec2; 2]) -> [DVec2; 2] {
		[a[0].min(b[0]), a[1].max(b[1])]
	}

	/// "Clip" bounds of `a` to the limits of `b`.
	pub fn clip(a: [DVec2; 2], b: [DVec2; 2]) -> [DVec2; 2] {
		[
			a[0].max(b[0]), // Constrain min corner
			a[1].min(b[1]), // Constrain max corner
		]
	}

	/// Expand a quad by a certain amount on all sides.
	///
	/// Not currently very optimized
	pub fn inflate(&self, offset: f64) -> Quad {
		let offset = |index_before, index, index_after| {
			let [point_before, point, point_after]: [DVec2; 3] = [self.0[index_before], self.0[index], self.0[index_after]];
			let [line_in, line_out] = [point - point_before, point_after - point];
			let angle = line_in.angle_to(-line_out);
			let offset_length = offset / (std::f64::consts::FRAC_PI_2 - angle / 2.).cos();
			point + (line_in.perp().normalize_or_zero() + line_out.perp().normalize_or_zero()).normalize_or_zero() * offset_length
		};
		Self([offset(3, 0, 1), offset(0, 1, 2), offset(1, 2, 3), offset(2, 3, 0)])
	}

	/// Does this quad contain a point
	///
	/// Code from https://wrfranklin.org/Research/Short_Notes/pnpoly.html
	pub fn contains(&self, p: DVec2) -> bool {
		let mut inside = false;
		for (i, j) in (0..4).zip([3, 0, 1, 2]) {
			if (self.0[i].y > p.y) != (self.0[j].y > p.y) && p.x < ((self.0[j].x - self.0[i].x) * (p.y - self.0[i].y) / (self.0[j].y - self.0[i].y) + self.0[i].x) {
				inside = !inside;
			}
		}
		inside
	}

	/// https://www.cs.rpi.edu/~cutler/classes/computationalgeometry/F23/lectures/02_line_segment_intersections.pdf
	fn line_intersection_t(a: DVec2, b: DVec2, c: DVec2, d: DVec2) -> (f64, f64) {
		let t = ((a.x - c.x) * (c.y - d.y) - (a.y - c.y) * (c.x - d.x)) / ((a.x - b.x) * (c.y - d.y) - (a.y - b.y) * (c.x - d.x));
		let u = ((a.x - c.x) * (a.y - b.y) - (a.y - c.y) * (a.x - b.x)) / ((a.x - b.x) * (c.y - d.y) - (a.y - b.y) * (c.x - d.x));

		(t, u)
	}

	fn intersect_lines(a: DVec2, b: DVec2, c: DVec2, d: DVec2) -> Option<DVec2> {
		let (t, u) = Self::line_intersection_t(a, b, c, d);
		((0. ..=1.).contains(&t) && (0. ..=1.).contains(&u)).then(|| a + t * (b - a))
	}

	pub fn intersect_rays(a: DVec2, a_direction: DVec2, b: DVec2, b_direction: DVec2) -> Option<DVec2> {
		let (t, u) = Self::line_intersection_t(a, a + a_direction, b, b + b_direction);
		(t.is_finite() && u.is_finite()).then(|| a + t * a_direction)
	}

	pub fn intersects(&self, other: Quad) -> bool {
		let intersects = self
			.all_edges()
			.into_iter()
			.any(|[a, b]| other.all_edges().into_iter().any(|[c, d]| Self::intersect_lines(a, b, c, d).is_some()));
		self.contains(other.center()) || other.contains(self.center()) || intersects
	}
}

impl std::ops::Mul<Quad> for DAffine2 {
	type Output = Quad;

	fn mul(self, rhs: Quad) -> Self::Output {
		Quad(rhs.0.map(|point| self.transform_point2(point)))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn offset_quad() {
		fn eq(a: Quad, b: Quad) -> bool {
			a.0.iter().zip(b.0).all(|(a, b)| a.abs_diff_eq(b, 0.0001))
		}

		assert!(eq(Quad::from_box([DVec2::ZERO, DVec2::ONE]).inflate(0.5), Quad::from_box([DVec2::splat(-0.5), DVec2::splat(1.5)])));
		assert!(eq(Quad::from_box([DVec2::ONE, DVec2::ZERO]).inflate(0.5), Quad::from_box([DVec2::splat(1.5), DVec2::splat(-0.5)])));
		assert!(eq(
			(DAffine2::from_scale(DVec2::new(-1., 1.)) * Quad::from_box([DVec2::ZERO, DVec2::ONE])).inflate(0.5),
			DAffine2::from_scale(DVec2::new(-1., 1.)) * Quad::from_box([DVec2::splat(-0.5), DVec2::splat(1.5)])
		));
	}
	#[test]
	fn quad_contains() {
		assert!(Quad::from_box([DVec2::ZERO, DVec2::ONE]).contains(DVec2::splat(0.5)));
		assert!(Quad::from_box([DVec2::ONE, DVec2::ZERO]).contains(DVec2::splat(0.5)));
		assert!(Quad::from_box([DVec2::splat(300.), DVec2::splat(500.)]).contains(DVec2::splat(350.)));
		assert!((DAffine2::from_scale(DVec2::new(-1., 1.)) * Quad::from_box([DVec2::ZERO, DVec2::ONE])).contains(DVec2::new(-0.5, 0.5)));

		assert!(!Quad::from_box([DVec2::ZERO, DVec2::ONE]).contains(DVec2::new(1., 1.1)));
		assert!(!Quad::from_box([DVec2::ONE, DVec2::ZERO]).contains(DVec2::new(0.5, -0.01)));
		assert!(!(DAffine2::from_scale(DVec2::new(-1., 1.)) * Quad::from_box([DVec2::ZERO, DVec2::ONE])).contains(DVec2::splat(0.5)));
	}

	#[test]
	fn intersect_lines() {
		assert_eq!(
			Quad::intersect_lines(DVec2::new(-5., 5.), DVec2::new(5., 5.), DVec2::new(2., 7.), DVec2::new(2., 3.)),
			Some(DVec2::new(2., 5.))
		);
		assert_eq!(Quad::intersect_lines(DVec2::new(4., 6.), DVec2::new(4., 5.), DVec2::new(2., 7.), DVec2::new(2., 3.)), None);
		assert_eq!(Quad::intersect_lines(DVec2::new(-5., 5.), DVec2::new(5., 5.), DVec2::new(2., 7.), DVec2::new(2., 9.)), None);
	}
	#[test]
	fn intersect_quad() {
		assert!(Quad::from_box([DVec2::ZERO, DVec2::splat(5.)]).intersects(Quad::from_box([DVec2::splat(4.), DVec2::splat(7.)])));
		assert!(Quad::from_box([DVec2::ZERO, DVec2::splat(5.)]).intersects(Quad::from_box([DVec2::splat(4.), DVec2::splat(4.2)])));
		assert!(!Quad::from_box([DVec2::ZERO, DVec2::splat(3.)]).intersects(Quad::from_box([DVec2::splat(4.), DVec2::splat(4.2)])));
	}
}
