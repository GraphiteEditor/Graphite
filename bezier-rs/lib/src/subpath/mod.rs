mod core;
mod lookup;
mod structs;
pub use structs::*;

use crate::{Bezier, BezierHandles};

use std::ops::{Index, IndexMut};

/// Structure used to represent a path composed of Bezier curves.
pub struct SubPath {
	manipulator_groups: Vec<ManipulatorGroup>,
	closed: bool,
}

impl Index<usize> for SubPath {
	type Output = ManipulatorGroup;

	fn index(&self, index: usize) -> &Self::Output {
		assert!(index < self.len());
		&self.manipulator_groups[index]
	}
}

impl IndexMut<usize> for SubPath {
	fn index_mut(&mut self, index: usize) -> &mut Self::Output {
		assert!(index < self.len());
		&mut self.manipulator_groups[index]
	}
}

impl Iterator for SubPathIter<'_> {
	type Item = Bezier;

	fn next(&mut self) -> Option<Self::Item> {
		if self.index >= self.sub_path.len() - 1 + (self.sub_path.closed as usize) {
			return None;
		}
		let start_index = self.index;
		let end_index = (self.index + 1) % self.sub_path.len();
		self.index += 1;

		let start = self.sub_path[start_index].anchor;
		let end = self.sub_path[end_index].anchor;
		let handle1 = self.sub_path[start_index].out_handle;
		let handle2 = self.sub_path[end_index].in_handle;

		if handle1.is_none() && handle2.is_none() {
			return Some(Bezier::from_linear_dvec2(start, end));
		}
		if handle1.is_none() || handle2.is_none() {
			return Some(Bezier::from_quadratic_dvec2(start, handle1.or(handle2).unwrap(), end));
		}
		Some(Bezier::from_cubic_dvec2(start, handle1.unwrap(), handle2.unwrap(), end))
	}
}
