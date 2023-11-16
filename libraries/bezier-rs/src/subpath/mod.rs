mod core;
mod lookup;
mod manipulators;
mod solvers;
mod structs;
mod transform;
pub use structs::*;

use crate::Bezier;

use std::fmt::{Debug, Formatter, Result};
use std::ops::{Index, IndexMut};

/// Structure used to represent a path composed of [Bezier] curves.
#[derive(Clone, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Subpath<ManipulatorGroupId: crate::Identifier> {
	manipulator_groups: Vec<ManipulatorGroup<ManipulatorGroupId>>,
	pub closed: bool,
}

#[cfg(feature = "dyn-any")]
unsafe impl<ManipulatorGroupId: crate::Identifier> dyn_any::StaticType for Subpath<ManipulatorGroupId> {
	type Static = Subpath<ManipulatorGroupId>;
}

/// Iteration structure for iterating across each curve of a `Subpath`, using an intermediate `Bezier` representation.
pub struct SubpathIter<'a, ManipulatorGroupId: crate::Identifier> {
	index: usize,
	subpath: &'a Subpath<ManipulatorGroupId>,
}

impl<ManipulatorGroupId: crate::Identifier> Index<usize> for Subpath<ManipulatorGroupId> {
	type Output = ManipulatorGroup<ManipulatorGroupId>;

	fn index(&self, index: usize) -> &Self::Output {
		assert!(index < self.len(), "Index out of bounds in trait Index of SubPath.");
		&self.manipulator_groups[index]
	}
}

impl<ManipulatorGroupId: crate::Identifier> IndexMut<usize> for Subpath<ManipulatorGroupId> {
	fn index_mut(&mut self, index: usize) -> &mut Self::Output {
		assert!(index < self.len(), "Index out of bounds in trait IndexMut of SubPath.");
		&mut self.manipulator_groups[index]
	}
}

impl<ManipulatorGroupId: crate::Identifier> Iterator for SubpathIter<'_, ManipulatorGroupId> {
	type Item = Bezier;

	// Returns the Bezier representation of each `Subpath` segment, defined between a pair of adjacent manipulator points.
	fn next(&mut self) -> Option<Self::Item> {
		if self.subpath.is_empty() {
			return None;
		}
		let len = self.subpath.len() - 1
			+ match self.subpath.closed {
				true => 1,
				false => 0,
			};
		if self.index >= len {
			return None;
		}
		let start_index = self.index;
		let end_index = (self.index + 1) % self.subpath.len();
		self.index += 1;

		Some(self.subpath[start_index].to_bezier(&self.subpath[end_index]))
	}
}

impl<ManipulatorGroupId: crate::Identifier> Debug for Subpath<ManipulatorGroupId> {
	fn fmt(&self, f: &mut Formatter<'_>) -> Result {
		f.debug_struct("Subpath").field("closed", &self.closed).field("manipulator_groups", &self.manipulator_groups).finish()
	}
}
