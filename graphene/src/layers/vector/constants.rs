use std::ops::{Index, IndexMut};

use serde::{Deserialize, Serialize};

#[repr(usize)]
#[derive(PartialEq, Eq, Clone, Debug, Copy, Serialize, Deserialize)]
pub enum ManipulatorType {
	Anchor,
	InHandle,
	OutHandle,
}

impl ManipulatorType {
	pub fn from_index(index: usize) -> ManipulatorType {
		match index {
			0 => ManipulatorType::Anchor,
			1 => ManipulatorType::InHandle,
			2 => ManipulatorType::OutHandle,
			_ => ManipulatorType::Anchor,
		}
	}

	pub fn opposite_handle(self) -> ManipulatorType {
		match self {
			ManipulatorType::Anchor => ManipulatorType::Anchor,
			ManipulatorType::InHandle => ManipulatorType::OutHandle,
			ManipulatorType::OutHandle => ManipulatorType::InHandle,
		}
	}
}

// Allows us to use ManipulatorType for indexing
impl<T> Index<ManipulatorType> for [T; 3] {
	type Output = T;
	fn index(&self, mt: ManipulatorType) -> &T {
		&self[mt as usize]
	}
}
// Allows us to use ManipulatorType for indexing, mutably
impl<T> IndexMut<ManipulatorType> for [T; 3] {
	fn index_mut(&mut self, mt: ManipulatorType) -> &mut T {
		&mut self[mt as usize]
	}
}

// Remove when no longer needed
pub const SELECTION_THRESHOLD: f64 = 10.;
