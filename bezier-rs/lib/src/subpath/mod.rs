mod core;
mod lookup;
mod structs;
pub use structs::*;

use crate::Bezier;

use std::ops::{Index, IndexMut};

/// Structure used to represent a path composed of [Bezier] curves.
pub struct Subpath {
	manipulator_groups: Vec<ManipulatorGroup>,
	closed: bool,
}

impl Index<usize> for Subpath {
	type Output = ManipulatorGroup;

	fn index(&self, index: usize) -> &Self::Output {
		assert!(index < self.len());
		&self.manipulator_groups[index]
	}
}

impl IndexMut<usize> for Subpath {
	fn index_mut(&mut self, index: usize) -> &mut Self::Output {
		assert!(index < self.len());
		&mut self.manipulator_groups[index]
	}
}

impl Iterator for SubpathIter<'_> {
	type Item = Bezier;

	// Returns the Bezier representation of each `Subpath` segment, defined between a pair of adjacent manipulator points.
	fn next(&mut self) -> Option<Self::Item> {
		if self.index >= self.sub_path.len() - 1 + (self.sub_path.closed as usize) {
			return None;
		}
		let start_index = self.index;
		let end_index = (self.index + 1) % self.sub_path.len();
		self.index += 1;

		let start = self.sub_path[start_index].anchor;
		let end = self.sub_path[end_index].anchor;
		let out_handle = self.sub_path[start_index].out_handle;
		let in_handle = self.sub_path[end_index].in_handle;

		if let (Some(handle1), Some(handle2)) = (out_handle, in_handle) {
			return Some(Bezier::from_cubic_dvec2(start, handle1, handle2, end));
		} else if let Some(handle) = out_handle.or(in_handle) {
			return Some(Bezier::from_quadratic_dvec2(start, handle, end));
		}
		Some(Bezier::from_linear_dvec2(start, end))
	}
}
