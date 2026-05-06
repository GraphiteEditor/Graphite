use crate::application::generate_uuid;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct GuideLineId(u64);

impl GuideLineId {
	pub fn new() -> Self {
		Self(generate_uuid())
	}

	pub fn from_raw(id: u64) -> Self {
		Self(id)
	}

	pub fn as_raw(&self) -> u64 {
		self.0
	}
}

impl Default for GuideLineId {
	fn default() -> Self {
		Self::new()
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum GuideLineDirection {
	Horizontal,
	Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GuideLine {
	pub id: GuideLineId,
	pub direction: GuideLineDirection,
	/// Position in document space (Y coordinate for horizontal guides, X coordinate for vertical guides)
	pub position: f64,
}

impl GuideLine {
	pub fn new(direction: GuideLineDirection, position: f64) -> Self {
		Self {
			id: GuideLineId::new(),
			direction,
			position,
		}
	}

	pub fn with_id(id: GuideLineId, direction: GuideLineDirection, position: f64) -> Self {
		Self { id, direction, position }
	}

	pub fn horizontal(y: f64) -> Self {
		Self::new(GuideLineDirection::Horizontal, y)
	}

	pub fn vertical(x: f64) -> Self {
		Self::new(GuideLineDirection::Vertical, x)
	}
}
