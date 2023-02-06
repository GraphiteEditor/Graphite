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
