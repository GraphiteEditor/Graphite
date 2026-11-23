use glam::IVec2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
	pub min: IVec2,
	pub max: IVec2,
}

impl Rect {
	#[must_use]
	pub fn new(min: IVec2, max: IVec2) -> Self {
		Self { min, max }
	}

	#[must_use]
	pub fn contains(&self, point: IVec2) -> bool {
		point.cmpge(self.min).all() && point.cmplt(self.max).all()
	}

	#[must_use]
	pub fn intersects(&self, other: &Rect) -> bool {
		self.min.cmplt(other.max).all() && other.min.cmplt(self.max).all()
	}
}
