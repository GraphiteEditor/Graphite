mod core;
mod lookup;
mod manipulators;
mod solvers;
mod structs;
mod transform;

use crate::Bezier;
pub use core::*;
use std::fmt::{Debug, Formatter, Result};
use std::ops::{Index, IndexMut};
pub use structs::*;

/// Structure used to represent a path composed of [Bezier] curves.
#[derive(Clone, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Subpath<PointId: crate::Identifier> {
	manipulator_groups: Vec<ManipulatorGroup<PointId>>,
	pub closed: bool,
}

#[cfg(feature = "dyn-any")]
unsafe impl<PointId: crate::Identifier> dyn_any::StaticType for Subpath<PointId> {
	type Static = Subpath<PointId>;
}

/// Iteration structure for iterating across each curve of a `Subpath`, using an intermediate `Bezier` representation.
pub struct SubpathIter<'a, PointId: crate::Identifier> {
	index: usize,
	subpath: &'a Subpath<PointId>,
	is_always_closed: bool,
}

impl<PointId: crate::Identifier> Index<usize> for Subpath<PointId> {
	type Output = ManipulatorGroup<PointId>;

	fn index(&self, index: usize) -> &Self::Output {
		assert!(index < self.len(), "Index out of bounds in trait Index of SubPath.");
		&self.manipulator_groups[index]
	}
}

impl<PointId: crate::Identifier> IndexMut<usize> for Subpath<PointId> {
	fn index_mut(&mut self, index: usize) -> &mut Self::Output {
		assert!(index < self.len(), "Index out of bounds in trait IndexMut of SubPath.");
		&mut self.manipulator_groups[index]
	}
}

impl<PointId: crate::Identifier> Iterator for SubpathIter<'_, PointId> {
	type Item = Bezier;

	// Returns the Bezier representation of each `Subpath` segment, defined between a pair of adjacent manipulator points.
	fn next(&mut self) -> Option<Self::Item> {
		if self.subpath.is_empty() {
			return None;
		}
		let closed = if self.is_always_closed { true } else { self.subpath.closed };
		let len = self.subpath.len() - 1 + if closed { 1 } else { 0 };
		if self.index >= len {
			return None;
		}
		let start_index = self.index;
		let end_index = (self.index + 1) % self.subpath.len();
		self.index += 1;

		Some(self.subpath[start_index].to_bezier(&self.subpath[end_index]))
	}
}

impl<PointId: crate::Identifier> Debug for Subpath<PointId> {
	fn fmt(&self, f: &mut Formatter<'_>) -> Result {
		f.debug_struct("Subpath").field("closed", &self.closed).field("manipulator_groups", &self.manipulator_groups).finish()
	}
}
