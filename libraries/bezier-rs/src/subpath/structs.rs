use super::Bezier;

use glam::DVec2;
use std::fmt::{Debug, Formatter, Result};

/// Structure used to represent a single anchor with up to two optional associated handles along a `Subpath`
#[derive(Copy, Clone, PartialEq)]
pub struct ManipulatorGroup {
	pub anchor: DVec2,
	pub in_handle: Option<DVec2>,
	pub out_handle: Option<DVec2>,
}

impl Debug for ManipulatorGroup {
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

impl ManipulatorGroup {
	pub fn to_bezier(&self, end_group: &ManipulatorGroup) -> Bezier {
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
