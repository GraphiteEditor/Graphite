use crate::application::generate_uuid;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct GuideId(u64);

impl GuideId {
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

impl Default for GuideId {
	fn default() -> Self {
		Self::new()
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum GuideDirection {
	Horizontal,
	Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Guide {
	pub id: GuideId,
	pub direction: GuideDirection,
	/// Position in document space (Y coordinate for horizontal guides, X coordinate for vertical guides)
	pub position: f64,
}

impl Guide {
	pub fn new(direction: GuideDirection, position: f64) -> Self {
		Self {
			id: GuideId::new(),
			direction,
			position,
		}
	}

	pub fn with_id(id: GuideId, direction: GuideDirection, position: f64) -> Self {
		Self { id, direction, position }
	}

	pub fn horizontal(y: f64) -> Self {
		Self::new(GuideDirection::Horizontal, y)
	}

	pub fn vertical(x: f64) -> Self {
		Self::new(GuideDirection::Vertical, x)
	}
}
