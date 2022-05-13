use std::ops::{Index, IndexMut};

// Helps push values that end in approximately half, plus or minus some floating point imprecision, towards the same side of the round() function
pub const ROUNDING_BIAS: f64 = 0.002;
// The angle threshold in radians that we should mirror handles if we are below
pub const MINIMUM_MIRROR_THRESHOLD: f64 = 0.1;

#[repr(usize)]
#[derive(PartialEq, Clone, Debug, Copy)]
pub enum ControlPointType {
	Anchor = 0,
	Handle1 = 1,
	Handle2 = 2,
}

// Allows us to use ManipulatorType for indexing
impl<T> Index<ControlPointType> for [T; 3] {
	type Output = T;
	fn index(&self, mt: ControlPointType) -> &T {
		&self[mt as usize]
	}
}
// Allows us to use ManipulatorType for indexing, mutably
impl<T> IndexMut<ControlPointType> for [T; 3] {
	fn index_mut(&mut self, mt: ControlPointType) -> &mut T {
		&mut self[mt as usize]
	}
}
