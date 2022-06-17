use std::{
	ops::{Index, IndexMut, Not},
	thread::JoinHandle,
};

use serde::{Deserialize, Serialize};

#[repr(usize)]
#[derive(PartialEq, Clone, Debug, Copy, Serialize, Deserialize)]
pub enum ControlPointType {
	Anchor = 0,
	InHandle = 1,
	OutHandle = 2,
}

impl ControlPointType {
	pub fn from_index(index: usize) -> ControlPointType {
		match index {
			0 => ControlPointType::Anchor,
			1 => ControlPointType::InHandle,
			2 => ControlPointType::OutHandle,
			_ => ControlPointType::Anchor,
		}
	}
}

impl Not for ControlPointType {
	type Output = Self;
	fn not(self) -> Self::Output {
		match self {
			ControlPointType::InHandle => ControlPointType::OutHandle,
			ControlPointType::OutHandle => ControlPointType::InHandle,
			_ => ControlPointType::Anchor,
		}
	}
}

// Allows us to use ManipulatorType for indexing
impl<T> Index<ControlPointType> for [T; 3] {
	type Output = T;
	fn index(&self, mt: ControlPointType) -> &T {
		&self[mt as usize]
	}
}
// Allows us to use ControlPointType for indexing, mutably
impl<T> IndexMut<ControlPointType> for [T; 3] {
	fn index_mut(&mut self, mt: ControlPointType) -> &mut T {
		&mut self[mt as usize]
	}
}

// Remove when no longer needed
pub const SELECTION_THRESHOLD: f64 = 10.;
