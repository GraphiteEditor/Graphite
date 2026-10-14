use glam::DVec2;

/// An axis-aligned box in a work plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
	pub min: DVec2,
	pub max: DVec2,
}

impl Bounds {
	/// The box that contains nothing, which is the identity for [`Bounds::union`].
	pub const EMPTY: Self = Self {
		min: DVec2::INFINITY,
		max: DVec2::NEG_INFINITY,
	};

	/// The box that contains every point.
	pub const EVERYTHING: Self = Self {
		min: DVec2::NEG_INFINITY,
		max: DVec2::INFINITY,
	};

	pub fn from_point(point: DVec2) -> Self {
		Self { min: point, max: point }
	}

	pub fn from_points(points: impl IntoIterator<Item = DVec2>) -> Self {
		points.into_iter().fold(Self::EMPTY, |bounds, point| bounds.including(point))
	}

	pub fn from_rect(rect: kurbo::Rect) -> Self {
		Self {
			min: DVec2::new(rect.min_x(), rect.min_y()),
			max: DVec2::new(rect.max_x(), rect.max_y()),
		}
	}

	pub fn is_empty(&self) -> bool {
		self.min.x > self.max.x || self.min.y > self.max.y
	}

	pub fn center(&self) -> DVec2 {
		// Unlike adding the corners and halving, this stays finite for any finite corners
		DVec2::new(self.min.x.midpoint(self.max.x), self.min.y.midpoint(self.max.y))
	}

	pub fn size(&self) -> DVec2 {
		self.max - self.min
	}

	pub fn including(&self, point: DVec2) -> Self {
		Self {
			min: self.min.min(point),
			max: self.max.max(point),
		}
	}

	pub fn union(&self, other: &Self) -> Self {
		Self {
			min: self.min.min(other.min),
			max: self.max.max(other.max),
		}
	}

	pub fn inflated(&self, amount: f64) -> Self {
		Self {
			min: self.min - DVec2::splat(amount),
			max: self.max + DVec2::splat(amount),
		}
	}

	pub fn contains(&self, point: DVec2) -> bool {
		point.cmpge(self.min).all() && point.cmple(self.max).all()
	}

	pub fn intersects(&self, other: &Self) -> bool {
		self.min.cmple(other.max).all() && other.min.cmple(self.max).all()
	}

	/// The distance from a point to the nearest point of the box, which is zero inside it.
	pub fn distance_to(&self, point: DVec2) -> f64 {
		(self.min - point).max(point - self.max).max(DVec2::ZERO).length()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn empty_is_the_union_identity() {
		let bounds = Bounds::from_points([DVec2::new(1., 2.), DVec2::new(-3., 5.)]);

		assert!(Bounds::EMPTY.is_empty());
		assert_eq!(Bounds::EMPTY.union(&bounds), bounds);
		assert_eq!(bounds.min, DVec2::new(-3., 2.));
		assert_eq!(bounds.max, DVec2::new(1., 5.));
	}

	#[test]
	fn distance_is_zero_inside_and_measured_to_the_nearest_edge_or_corner_outside() {
		let bounds = Bounds::from_points([DVec2::ZERO, DVec2::new(10., 10.)]);

		assert_eq!(bounds.distance_to(DVec2::new(5., 5.)), 0.);
		assert_eq!(bounds.distance_to(DVec2::new(13., 5.)), 3.);
		assert_eq!(bounds.distance_to(DVec2::new(13., 14.)), 5.);
	}

	#[test]
	fn touching_boxes_intersect_and_separated_ones_do_not() {
		let bounds = Bounds::from_points([DVec2::ZERO, DVec2::new(10., 10.)]);

		assert!(bounds.intersects(&Bounds::from_points([DVec2::new(10., 0.), DVec2::new(20., 10.)])));
		assert!(!bounds.intersects(&Bounds::from_points([DVec2::new(10.5, 0.), DVec2::new(20., 10.)])));
		assert!(!bounds.intersects(&Bounds::EMPTY));
	}

	#[test]
	fn the_center_of_a_box_near_the_largest_finite_value_lies_within_it() {
		let point = DVec2::new(1e308, -f64::MAX);
		assert_eq!(Bounds::from_point(point).center(), point);

		let far = Bounds::from_points([point, DVec2::new(f64::MAX, -1e308)]);
		assert!(far.contains(far.center()));
	}
}
