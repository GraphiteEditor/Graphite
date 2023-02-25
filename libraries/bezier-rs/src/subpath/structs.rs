use super::Bezier;

use glam::DVec2;
use std::fmt::{Debug, Formatter, Result};

/// An id type used for each [ManipulatorGroup].
pub trait Identifier: Sized + Clone + PartialEq {
	fn new() -> Self;
}

/// An empty id type for use in tests
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg(test)]
pub(crate) struct EmptyId;

#[cfg(test)]
impl Identifier for EmptyId {
	fn new() -> Self {
		Self
	}
}

/// Structure used to represent a single anchor with up to two optional associated handles along a `Subpath`
#[derive(Copy, Clone, PartialEq)]
pub struct ManipulatorGroup<ManipulatorGroupId: crate::Identifier> {
	pub anchor: DVec2,
	pub in_handle: Option<DVec2>,
	pub out_handle: Option<DVec2>,
	pub id: ManipulatorGroupId,
}

impl<ManipulatorGroupId: crate::Identifier> Debug for ManipulatorGroup<ManipulatorGroupId> {
	fn fmt(&self, f: &mut Formatter<'_>) -> Result {
		if self.in_handle.is_some() && self.out_handle.is_some() {
			write!(f, "anchor: {}, in: {}, out: {}", self.anchor, self.in_handle.unwrap(), self.out_handle.unwrap())
		} else if self.in_handle.is_some() {
			write!(f, "anchor: {}, in: {}, out: n/a", self.anchor, self.in_handle.unwrap())
		} else if self.out_handle.is_some() {
			write!(f, "anchor: {}, in: n/a, out: {}", self.anchor, self.out_handle.unwrap())
		} else {
			write!(f, "anchor: {}, in: n/a, out: n/a", self.anchor)
		}
	}
}

impl<ManipulatorGroupId: crate::Identifier> ManipulatorGroup<ManipulatorGroupId> {
	pub fn to_bezier(&self, end_group: &ManipulatorGroup<ManipulatorGroupId>) -> Bezier {
		let start = self.anchor;
		let end = end_group.anchor;
		let out_handle = self.out_handle;
		let in_handle = end_group.in_handle;

		match (out_handle, in_handle) {
			(Some(handle1), Some(handle2)) => Bezier::from_cubic_dvec2(start, handle1, handle2, end),
			(Some(handle), None) | (None, Some(handle)) => Bezier::from_quadratic_dvec2(start, handle, end),
			(None, None) => Bezier::from_linear_dvec2(start, end),
		}
	}
}
