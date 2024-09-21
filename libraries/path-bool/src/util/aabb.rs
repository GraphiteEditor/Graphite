use glam::DVec2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Aabb {
	pub top: f64,
	pub right: f64,
	pub bottom: f64,
	pub left: f64,
}

pub(crate) fn bounding_boxes_overlap(a: &Aabb, b: &Aabb) -> bool {
	a.left <= b.right && b.left <= a.right && a.top <= b.bottom && b.top <= a.bottom
}

pub(crate) fn merge_bounding_boxes(a: Option<Aabb>, b: &Aabb) -> Aabb {
	match a {
		Some(a) => Aabb {
			top: a.top.min(b.top),
			right: a.right.max(b.right),
			bottom: a.bottom.max(b.bottom),
			left: a.left.min(b.left),
		},
		None => *b,
	}
}

pub(crate) fn extend_bounding_box(bounding_box: Option<Aabb>, point: DVec2) -> Aabb {
	match bounding_box {
		Some(bb) => Aabb {
			top: bb.top.min(point.y),
			right: bb.right.max(point.x),
			bottom: bb.bottom.max(point.y),
			left: bb.left.min(point.x),
		},
		None => Aabb {
			top: point.y,
			right: point.x,
			bottom: point.y,
			left: point.x,
		},
	}
}

pub(crate) fn bounding_box_max_extent(bounding_box: &Aabb) -> f64 {
	(bounding_box.right - bounding_box.left).max(bounding_box.bottom - bounding_box.top)
}

pub(crate) fn bounding_box_around_point(point: DVec2, padding: f64) -> Aabb {
	Aabb {
		top: point.y - padding,
		right: point.x + padding,
		bottom: point.y + padding,
		left: point.x - padding,
	}
}

pub(crate) fn expand_bounding_box(bounding_box: &Aabb, padding: f64) -> Aabb {
	Aabb {
		top: bounding_box.top - padding,
		right: bounding_box.right + padding,
		bottom: bounding_box.bottom + padding,
		left: bounding_box.left - padding,
	}
}
