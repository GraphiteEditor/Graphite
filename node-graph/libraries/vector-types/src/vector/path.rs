use core_types::list::List;
use graphene_hash::CacheHash;

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
}
