use dyn_any::{DynAny, StaticType};
use glam::{DAffine2, DVec2};

#[cfg_attr(not(target_arch = "spirv"), derive(Debug))]
#[derive(Clone, DynAny)]
pub struct AxisAlignedBbox {
	pub start: DVec2,
	pub end: DVec2,
}

impl AxisAlignedBbox {
	pub const ZERO: Self = Self { start: DVec2::ZERO, end: DVec2::ZERO };
	pub const ONE: Self = Self { start: DVec2::ZERO, end: DVec2::ONE };

	pub fn size(&self) -> DVec2 {
		self.end - self.start
	}

	pub fn to_transform(&self) -> DAffine2 {
		DAffine2::from_translation(self.start) * DAffine2::from_scale(self.size())
	}

	pub fn contains(&self, point: DVec2) -> bool {
		point.x >= self.start.x && point.x <= self.end.x && point.y >= self.start.y && point.y <= self.end.y
	}

	pub fn intersects(&self, other: &AxisAlignedBbox) -> bool {
		other.start.x <= self.end.x && other.end.x >= self.start.x && other.start.y <= self.end.y && other.end.y >= self.start.y
	}

	pub fn union(&self, other: &AxisAlignedBbox) -> AxisAlignedBbox {
		AxisAlignedBbox {
			start: DVec2::new(self.start.x.min(other.start.x), self.start.y.min(other.start.y)),
			end: DVec2::new(self.end.x.max(other.end.x), self.end.y.max(other.end.y)),
		}
	}
	pub fn union_non_empty(&self, other: &AxisAlignedBbox) -> Option<AxisAlignedBbox> {
		match (self.size() == DVec2::ZERO, other.size() == DVec2::ZERO) {
			(true, true) => None,
			(true, _) => Some(other.clone()),
			(_, true) => Some(self.clone()),
			_ => Some(AxisAlignedBbox {
				start: DVec2::new(self.start.x.min(other.start.x), self.start.y.min(other.start.y)),
				end: DVec2::new(self.end.x.max(other.end.x), self.end.y.max(other.end.y)),
			}),
		}
	}

	pub fn intersect(&self, other: &AxisAlignedBbox) -> AxisAlignedBbox {
		AxisAlignedBbox {
			start: DVec2::new(self.start.x.max(other.start.x), self.start.y.max(other.start.y)),
			end: DVec2::new(self.end.x.min(other.end.x), self.end.y.min(other.end.y)),
		}
	}
}

#[cfg_attr(not(target_arch = "spirv"), derive(Debug))]
#[derive(Clone)]
pub struct Bbox {
	pub top_left: DVec2,
	pub top_right: DVec2,
	pub bottom_left: DVec2,
	pub bottom_right: DVec2,
}

impl Bbox {
	pub fn unit() -> Self {
		Self {
			top_left: DVec2::new(0., 1.),
			top_right: DVec2::new(1., 1.),
			bottom_left: DVec2::new(0., 0.),
			bottom_right: DVec2::new(1., 0.),
		}
	}

	pub fn from_transform(transform: DAffine2) -> Self {
		Self {
			top_left: transform.transform_point2(DVec2::new(0., 1.)),
			top_right: transform.transform_point2(DVec2::new(1., 1.)),
			bottom_left: transform.transform_point2(DVec2::new(0., 0.)),
			bottom_right: transform.transform_point2(DVec2::new(1., 0.)),
		}
	}

	pub fn affine_transform(self, transform: DAffine2) -> Self {
		Self {
			top_left: transform.transform_point2(self.top_left),
			top_right: transform.transform_point2(self.top_right),
			bottom_left: transform.transform_point2(self.bottom_left),
			bottom_right: transform.transform_point2(self.bottom_right),
		}
	}

	pub fn to_axis_aligned_bbox(&self) -> AxisAlignedBbox {
		let start_x = self.top_left.x.min(self.top_right.x).min(self.bottom_left.x).min(self.bottom_right.x);
		let start_y = self.top_left.y.min(self.top_right.y).min(self.bottom_left.y).min(self.bottom_right.y);
		let end_x = self.top_left.x.max(self.top_right.x).max(self.bottom_left.x).max(self.bottom_right.x);
		let end_y = self.top_left.y.max(self.top_right.y).max(self.bottom_left.y).max(self.bottom_right.y);

		AxisAlignedBbox {
			start: DVec2::new(start_x, start_y),
			end: DVec2::new(end_x, end_y),
		}
	}
}
