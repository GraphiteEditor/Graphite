use glam::{DAffine2, DVec2};

#[derive(Debug, Clone, Default, Copy)]
/// A quad defined by four vertices.
pub struct Quad([DVec2; 4]);

impl Quad {
	/// Convert a box defined by two corner points to a quad.
	pub fn from_box(bbox: [DVec2; 2]) -> Self {
		let size = bbox[1] - bbox[0];
		Self([bbox[0], bbox[0] + size * DVec2::X, bbox[1], bbox[0] + size * DVec2::Y])
	}

	/// Get all the edges in the quad.
	pub fn lines_glam(&self) -> impl Iterator<Item = bezier_rs::Bezier> + '_ {
		[[self.0[0], self.0[1]], [self.0[1], self.0[2]], [self.0[2], self.0[3]], [self.0[3], self.0[0]]]
			.into_iter()
			.map(|[start, end]| bezier_rs::Bezier::from_linear_dvec2(start, end))
	}

	/// Generates a [crate::vector::Subpath] of the quad
	pub fn subpath(&self) -> crate::vector::Subpath {
		crate::vector::Subpath::from_points(self.0.into_iter(), true)
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
}

impl core::ops::Mul<Quad> for DAffine2 {
	type Output = Quad;

	fn mul(self, rhs: Quad) -> Self::Output {
		Quad(rhs.0.map(|point| self.transform_point2(point)))
	}
}
