mod core;
mod lookup;
mod manipulators;
mod solvers;
mod structs;
mod transform;
pub use structs::*;

use crate::Bezier;

use std::ops::{Index, IndexMut};

/// Structure used to represent a path composed of [Bezier] curves.
#[derive(Clone, PartialEq)]
pub struct Subpath<ManipulatorGroupId: crate::ManipulatorGroupId> {
	manipulator_groups: Vec<ManipulatorGroup<ManipulatorGroupId>>,
	closed: bool,
}

/// Iteration structure for iterating across each curve of a `Subpath`, using an intermediate `Bezier` representation.
pub struct SubpathIter<'a, ManipulatorGroupId: crate::ManipulatorGroupId> {
	index: usize,
	sub_path: &'a Subpath<ManipulatorGroupId>,
}

impl<ManipulatorGroupId: crate::ManipulatorGroupId> Index<usize> for Subpath<ManipulatorGroupId> {
	type Output = ManipulatorGroup<ManipulatorGroupId>;

	fn index(&self, index: usize) -> &Self::Output {
		assert!(index < self.len(), "Index out of bounds in trait Index of SubPath.");
		&self.manipulator_groups[index]
	}
}

impl<ManipulatorGroupId: crate::ManipulatorGroupId> IndexMut<usize> for Subpath<ManipulatorGroupId> {
	fn index_mut(&mut self, index: usize) -> &mut Self::Output {
		assert!(index < self.len(), "Index out of bounds in trait IndexMut of SubPath.");
		&mut self.manipulator_groups[index]
	}
}

impl<ManipulatorGroupId: crate::ManipulatorGroupId> Iterator for SubpathIter<'_, ManipulatorGroupId> {
	type Item = Bezier;

	// Returns the Bezier representation of each `Subpath` segment, defined between a pair of adjacent manipulator points.
	fn next(&mut self) -> Option<Self::Item> {
		let len = self.sub_path.len() - 1
			+ match self.sub_path.closed {
				true => 1,
				false => 0,
			};
		if self.index >= len {
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
			Some(Bezier::from_cubic_dvec2(start, handle1, handle2, end))
		} else if let Some(handle) = out_handle.or(in_handle) {
			Some(Bezier::from_quadratic_dvec2(start, handle, end))
		} else {
			Some(Bezier::from_linear_dvec2(start, end))
		}
	}
}
