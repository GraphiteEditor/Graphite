use super::Bezier;

use glam::{DAffine2, DVec2};
use std::{
	fmt::{Debug, Formatter, Result},
	hash::Hash,
};

/// An id type used for each [ManipulatorGroup].
pub trait Identifier: Sized + Clone + PartialEq + Hash + 'static {
	fn new() -> Self;
}

/// An empty id type for use in tests
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ManipulatorGroup<ManipulatorGroupId: crate::Identifier> {
	pub anchor: DVec2,
	pub in_handle: Option<DVec2>,
	pub out_handle: Option<DVec2>,
	pub id: ManipulatorGroupId,
}

// TODO: Remove once we no longer need to hash floats in Graphite
impl<ManipulatorGroupId: crate::Identifier> Hash for ManipulatorGroup<ManipulatorGroupId> {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.anchor.to_array().iter().for_each(|x| x.to_bits().hash(state));
		self.in_handle.is_some().hash(state);
		if let Some(in_handle) = self.in_handle {
			in_handle.to_array().iter().for_each(|x| x.to_bits().hash(state));
		}
		self.out_handle.is_some().hash(state);
		if let Some(out_handle) = self.out_handle {
			out_handle.to_array().iter().for_each(|x| x.to_bits().hash(state));
		}
		self.id.hash(state);
	}
}

#[cfg(feature = "dyn-any")]
unsafe impl<ManipulatorGroupId: crate::Identifier> dyn_any::StaticType for ManipulatorGroup<ManipulatorGroupId> {
	type Static = ManipulatorGroup<ManipulatorGroupId>;
}

impl<ManipulatorGroupId: crate::Identifier> Debug for ManipulatorGroup<ManipulatorGroupId> {
	fn fmt(&self, f: &mut Formatter<'_>) -> Result {
		f.debug_struct("ManipulatorGroup")
			.field("anchor", &self.anchor)
			.field("in_handle", &self.in_handle)
			.field("out_handle", &self.out_handle)
			.finish()
	}
}

impl<ManipulatorGroupId: crate::Identifier> ManipulatorGroup<ManipulatorGroupId> {
	/// Construct a new manipulator group from an anchor, in handle and out handle
	pub fn new(anchor: DVec2, in_handle: Option<DVec2>, out_handle: Option<DVec2>) -> Self {
		let id = ManipulatorGroupId::new();
		Self { anchor, in_handle, out_handle, id }
	}

	/// Construct a new manipulator point with just an anchor position
	pub fn new_anchor(anchor: DVec2) -> Self {
		Self::new(anchor, Some(anchor), Some(anchor))
	}

	/// Construct a new manipulator group from an anchor, in handle, out handle and an id
	pub fn new_with_id(anchor: DVec2, in_handle: Option<DVec2>, out_handle: Option<DVec2>, id: ManipulatorGroupId) -> Self {
		Self { anchor, in_handle, out_handle, id }
	}

	/// Construct a new manipulator point with just an anchor position and an id
	pub fn new_anchor_with_id(anchor: DVec2, id: ManipulatorGroupId) -> Self {
		Self::new_with_id(anchor, Some(anchor), Some(anchor), id)
	}

	/// Create a bezier curve that starts at the current manipulator group and finishes in the `end_group` manipulator group.
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

	/// Apply a transformation to all of the [ManipulatorGroup] points
	pub fn apply_transform(&mut self, affine_transform: DAffine2) {
		self.anchor = affine_transform.transform_point2(self.anchor);
		self.in_handle = self.in_handle.map(|in_handle| affine_transform.transform_point2(in_handle));
		self.out_handle = self.out_handle.map(|out_handle| affine_transform.transform_point2(out_handle));
	}

	/// Are all handles at finite positions
	pub fn is_finite(&self) -> bool {
		self.anchor.is_finite() && self.in_handle.map_or(true, |handle| handle.is_finite()) && self.out_handle.map_or(true, |handle| handle.is_finite())
	}

	/// Reverse directions of handles
	pub fn flip(mut self) -> Self {
		std::mem::swap(&mut self.in_handle, &mut self.out_handle);
		self
	}

	pub fn has_in_handle(&self) -> bool {
		self.in_handle.map(|handle| Self::has_handle(self.anchor, handle)).unwrap_or(false)
	}

	pub fn has_out_handle(&self) -> bool {
		self.out_handle.map(|handle| Self::has_handle(self.anchor, handle)).unwrap_or(false)
	}

	fn has_handle(anchor: DVec2, handle: DVec2) -> bool {
		!((handle.x - anchor.x).abs() < f64::EPSILON && (handle.y - anchor.y).abs() < f64::EPSILON)
	}
}

#[derive(Copy, Clone)]
pub enum AppendType {
	IgnoreStart,
	SmoothJoin(f64),
}
