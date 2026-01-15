use std::sync::atomic::{AtomicU64, Ordering};

/// Unique identifier for guide lines
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct GuideId(u64);

static GUIDE_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

impl GuideId {
	/// Generate's a new unique guide ID
	pub fn new() -> Self {
		Self(GUIDE_ID_COUNTER.fetch_add(1, Ordering::Relaxed))
	}

	/// Creates a GuideId from a raw u64 value
	pub fn from_raw(id: u64) -> Self {
		Self(id)
	}

	/// Get the raw u64 value of this GuideId
	pub fn as_raw(&self) -> u64 {
		self.0
	}
}

impl Default for GuideId {
	fn default() -> Self {
		Self::new()
	}
}

/// Direction of a guide line
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum GuideDirection {
	Horizontal,
	Vertical,
}

/// A guide line in document space
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Guide {
	pub id: GuideId,
	pub direction: GuideDirection,
	/// Position in document space (Y for horizontal, X for vertical)
	pub position: f64,
}

impl Guide {
	/// Create a new guide line with a new unique ID
	pub fn new(direction: GuideDirection, position: f64) -> Self {
		Self {
			id: GuideId::new(),
			direction,
			position,
		}
	}

	/// Create a new guide line with a pre-existing ID
	pub fn with_id(id: GuideId, direction: GuideDirection, position: f64) -> Self {
		Self { id, direction, position }
	}

	/// Create a new horizontal guide at the given Y position
	pub fn horizontal(y: f64) -> Self {
		Self::new(GuideDirection::Horizontal, y)
	}

	/// Create a new vertical guide at the given X position
	pub fn vertical(x: f64) -> Self {
		Self::new(GuideDirection::Vertical, x)
	}
}
