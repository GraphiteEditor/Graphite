use glam::{BVec2, DVec2};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Aabb {
	min: DVec2,
	max: DVec2,
}

impl Default for Aabb {
	fn default() -> Self {
		Self {
			min: DVec2::INFINITY,
			max: DVec2::NEG_INFINITY,
		}
	}
}

impl Aabb {
	#[inline]
	pub(crate) fn min(&self) -> DVec2 {
		self.min
	}
	#[inline]
	pub(crate) fn max(&self) -> DVec2 {
		self.max
	}

	pub(crate) const fn new(left: f64, top: f64, right: f64, bottom: f64) -> Self {
		Aabb {
			min: DVec2::new(left, top),
			max: DVec2::new(right, bottom),
		}
	}
	#[inline]
	pub(crate) fn top(&self) -> f64 {
		self.min.y
	}
	#[inline]
	pub(crate) fn left(&self) -> f64 {
		self.min.x
	}
	#[inline]
	pub(crate) fn right(&self) -> f64 {
		self.max.x
	}
	#[inline]
	pub(crate) fn bottom(&self) -> f64 {
		self.max.y
	}
}

#[inline]
pub(crate) fn bounding_boxes_overlap(a: &Aabb, b: &Aabb) -> bool {
	(a.min.cmple(b.max) & b.min.cmple(a.max)) == BVec2::TRUE
}

#[inline]
pub(crate) fn merge_bounding_boxes(a: &Aabb, b: &Aabb) -> Aabb {
	Aabb {
		min: a.min.min(b.min),
		max: a.max.max(b.max),
	}
}

#[inline]
pub(crate) fn extend_bounding_box(bounding_box: Option<Aabb>, point: DVec2) -> Aabb {
	match bounding_box {
		Some(bb) => Aabb {
			min: bb.min.min(point),
			max: bb.max.max(point),
		},
		None => Aabb { min: point, max: point },
	}
}

pub(crate) fn bounding_box_max_extent(bounding_box: &Aabb) -> f64 {
	(bounding_box.max - bounding_box.min).max_element()
}

pub(crate) fn bounding_box_around_point(point: DVec2, padding: f64) -> Aabb {
	Aabb {
		min: point - DVec2::splat(padding),
		max: point + DVec2::splat(padding),
	}
}

pub(crate) fn expand_bounding_box(bounding_box: &Aabb, padding: f64) -> Aabb {
	Aabb {
		min: bounding_box.min - DVec2::splat(padding),
		max: bounding_box.max + DVec2::splat(padding),
	}
}
