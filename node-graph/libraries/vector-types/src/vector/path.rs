use super::misc::dvec2_to_point;
use core_types::list::List;
use glam::DVec2;
use graphene_hash::CacheHash;
use kurbo::{CubicBez, Line, PathSeg, QuadBez};

/// The curve family a path's point references weave in, fixing the stride of its reference pattern.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, CacheHash)]
pub enum CurveType {
	/// Straight segments between consecutive anchors, with every reference an anchor.
	#[default]
	Linear,
	/// Quadratic Bézier segments, with one handle reference between each pair of anchors.
	Quadratic,
	/// Cubic Bézier segments, with two handle references between each pair of anchors.
	Cubic,
}

impl CurveType {
	/// Returns the number of references stepping from one anchor to the next: the segment's handle count plus one.
	pub fn stride(&self) -> usize {
		match self {
			Self::Linear => 1,
			Self::Quadratic => 2,
			Self::Cubic => 3,
		}
	}
}

impl std::fmt::Display for CurveType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Linear => write!(f, "Linear"),
			Self::Quadratic => write!(f, "Quadratic"),
			Self::Cubic => write!(f, "Cubic"),
		}
	}
}

/// One connected stroke of geometry: an ordered list of references (indices into the owning `Vector`'s point pool)
/// woven in its curve type's stride order. Welding is two paths, or two ends of one path, referencing the same anchor.
/// Per-reference data such as `ATTR_SMOOTH` rides this list's attributes.
#[derive(Clone, Debug, Default, PartialEq, CacheHash)]
pub struct Path(pub List<u32>);

impl Path {
	/// Whether this path is closed, indicated by its first and last references naming the same pool point.
	pub fn is_closed(&self) -> bool {
		let len = self.0.len();
		len > 1 && self.0.element(0) == self.0.element(len - 1)
	}

	/// Whether the reference at the given index is an anchor (rather than a handle) under the given curve type.
	pub fn is_anchor_reference(&self, index: usize, curve_type: CurveType) -> bool {
		index.is_multiple_of(curve_type.stride())
	}

	/// The number of segments this path draws, one fewer than its anchor count when open and equal to it when closed.
	pub fn segment_count(&self, curve_type: CurveType) -> usize {
		self.0.len().saturating_sub(1) / curve_type.stride()
	}

	/// The pool position named by the reference at `index`, or `None` if either lookup falls out of bounds.
	pub fn position(&self, pool: &List<DVec2>, index: usize) -> Option<DVec2> {
		pool.element(*self.0.element(index)? as usize).copied()
	}

	/// The segment at `index`, resolved against the point pool.
	pub fn segment(&self, pool: &List<DVec2>, curve_type: CurveType, index: usize) -> Option<PathSeg> {
		let start = index * curve_type.stride();
		let point = |offset: usize| self.position(pool, start + offset).map(dvec2_to_point);

		Some(match curve_type {
			CurveType::Linear => PathSeg::Line(Line::new(point(0)?, point(1)?)),
			CurveType::Quadratic => PathSeg::Quad(QuadBez::new(point(0)?, point(1)?, point(2)?)),
			CurveType::Cubic => PathSeg::Cubic(CubicBez::new(point(0)?, point(1)?, point(2)?, point(3)?)),
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::list::Item;

	fn pool(positions: &[(f64, f64)]) -> List<DVec2> {
		positions.iter().map(|&(x, y)| Item::new_from_element(DVec2::new(x, y))).collect()
	}

	fn path(references: &[u32]) -> Path {
		Path(references.iter().map(|&reference| Item::new_from_element(reference)).collect())
	}

	#[test]
	fn open_linear_path() {
		let pool = pool(&[(0., 0.), (10., 0.), (10., 10.)]);
		let path = path(&[0, 1, 2]);

		assert!(!path.is_closed());
		assert_eq!(path.segment_count(CurveType::Linear), 2);
		assert_eq!(path.segment(&pool, CurveType::Linear, 1), Some(PathSeg::Line(Line::new((10., 0.), (10., 10.)))));
		assert_eq!(path.segment(&pool, CurveType::Linear, 2), None);
	}

	// A closed path repeats its first reference last, so anchors and segments count alike
	#[test]
	fn closed_cubic_path() {
		let pool = pool(&[(0., 0.), (5., 0.), (10., 5.), (10., 10.), (10., 15.), (5., 20.)]);
		let path = path(&[0, 1, 2, 3, 4, 5, 0]);

		assert!(path.is_closed());
		assert_eq!(path.segment_count(CurveType::Cubic), 2);
		assert_eq!(
			path.segment(&pool, CurveType::Cubic, 1),
			Some(PathSeg::Cubic(CubicBez::new((10., 10.), (10., 15.), (5., 20.), (0., 0.))))
		);
	}

	// Anchor occurrences fall on stride multiples, and the references between them are handles
	#[test]
	fn anchor_references_by_stride() {
		let path = path(&[0, 1, 2, 3]);

		assert!(path.is_anchor_reference(0, CurveType::Cubic));
		assert!(!path.is_anchor_reference(1, CurveType::Cubic));
		assert!(!path.is_anchor_reference(2, CurveType::Cubic));
		assert!(path.is_anchor_reference(3, CurveType::Cubic));
		assert!(path.is_anchor_reference(2, CurveType::Quadratic));
		assert!(path.is_anchor_reference(2, CurveType::Linear));
	}

	// A reference naming a point the pool does not have drops its segment instead of panicking
	#[test]
	fn dangling_reference_skips_its_segment() {
		let pool = pool(&[(0., 0.), (10., 0.)]);
		let path = path(&[0, 1, 99]);

		assert_eq!(path.segment_count(CurveType::Linear), 2);
		assert!(path.segment(&pool, CurveType::Linear, 0).is_some());
		assert_eq!(path.segment(&pool, CurveType::Linear, 1), None);
	}
}
