// use super::consts::MAX_ABSOLUTE_DIFFERENCE;
// use super::utils::{SubpathTValue};
use super::*;

impl<PointId: super::structs::Identifier> Subpath<PointId> {
	/// Get whether the subpath is closed.
	pub fn closed(&self) -> bool {
		self.closed
	}

	/// Set whether the subpath is closed.
	pub fn set_closed(&mut self, new_closed: bool) {
		self.closed = new_closed;
	}

	/// Push a manipulator group to the end.
	pub fn push_manipulator_group(&mut self, group: ManipulatorGroup<PointId>) {
		assert!(group.is_finite(), "Pushing non finite manipulator group");
		self.manipulator_groups.push(group)
	}

	/// Get a mutable reference to the last manipulator
	pub fn last_manipulator_group_mut(&mut self) -> Option<&mut ManipulatorGroup<PointId>> {
		self.manipulator_groups.last_mut()
	}
}
