mod consts;
mod core;
mod lookup;
mod manipulators;
mod solvers;
mod structs;
mod transform;

pub use core::*;
use kurbo::PathSeg;
use std::fmt::{Debug, Formatter, Result};
pub use structs::*;

/// Structure used to represent a path composed of [Bezier] curves.
#[derive(Clone, PartialEq, graphene_hash::CacheHash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Subpath<PointId: Identifier> {
	manipulator_groups: Vec<ManipulatorGroup<PointId>>,
	pub closed: bool,
}

/// Iteration structure for iterating across each curve of a `Subpath`, using an intermediate `Bezier` representation.
pub struct SubpathIter<'a, PointId: Identifier> {
	index: usize,
	subpath: &'a Subpath<PointId>,
	is_always_closed: bool,
}

impl<PointId: Identifier> Iterator for SubpathIter<'_, PointId> {
	type Item = PathSeg;

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

		Some(self.subpath.manipulator_groups[start_index].to_bezier(&self.subpath.manipulator_groups[end_index]))
	}
}

impl<PointId: Identifier> Debug for Subpath<PointId> {
	fn fmt(&self, f: &mut Formatter<'_>) -> Result {
		f.debug_struct("Subpath").field("closed", &self.closed).field("manipulator_groups", &self.manipulator_groups).finish()
	}
}
