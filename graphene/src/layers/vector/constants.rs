use std::ops::{Index, IndexMut};

use serde::{Deserialize, Serialize};

#[repr(usize)]
#[derive(PartialEq, Clone, Debug, Copy, Serialize, Deserialize)]
pub enum ControlPointType {
	Anchor = 0,
	Handle1 = 1,
	Handle2 = 2,
}

// Allows us to use ControlPointType for indexing
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
