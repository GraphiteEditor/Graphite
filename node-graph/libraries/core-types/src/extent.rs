//! The typed surface handed to an `extent(fn)` helper: the node's inputs in
//! declaration order, then the queried level. Values read without unsafe or
//! internal fields, upstream extents query per level, and the one blessed
//! context modification is per-copy derived promotion. Anything beyond this
//! vocabulary uses `extent_raw(fn)`, which keeps the full node/ctx/level form.

use crate::gpoll::{Extent, GPoll};

/// A wired value input; `get` evaluates the edge and yields the typed element.
pub struct ValueIn<'a, T> {
	read: &'a dyn Fn() -> GPoll<T>,
}

impl<'a, T> ValueIn<'a, T> {
	pub fn new(read: &'a dyn Fn() -> GPoll<T>) -> Self {
		Self { read }
	}

	pub fn get(&self) -> GPoll<T> {
		(self.read)()
	}
}

/// An upstream edge's extents. For derived (per-copy) content the query runs
/// at the given copy's promoted context; `at` queries copy 0, the uniform
/// default. For ordinary edges the copy is ignored.
pub struct ExtentIn<'a> {
	query: &'a dyn Fn(u64, u8) -> GPoll<Extent>,
}

impl<'a> ExtentIn<'a> {
	pub fn new(query: &'a dyn Fn(u64, u8) -> GPoll<Extent>) -> Self {
		Self { query }
	}

	pub fn at(&self, level: LevelIn) -> GPoll<Extent> {
		(self.query)(0, level.level)
	}

	pub fn at_copy(&self, copy: u64, level: LevelIn) -> GPoll<Extent> {
		(self.query)(copy, level.level)
	}
}

/// The queried absolute level (innermost `0`), paired with the node's depth.
#[derive(Clone, Copy, Debug)]
pub struct LevelIn {
	pub level: u8,
	pub depth: u8,
}

impl LevelIn {
	pub fn new(level: u8, depth: u8) -> Self {
		Self { level, depth }
	}

	/// Whether the query targets the node's own pushed (outermost) level.
	pub fn pushed(&self) -> bool {
		self.level + 1 == self.depth
	}
}
