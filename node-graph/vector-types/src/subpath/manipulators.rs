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

	/// Access a [ManipulatorGroup] from a PointId.
	pub fn manipulator_from_id(&self, id: PointId) -> Option<&ManipulatorGroup<PointId>> {
		self.manipulator_groups.iter().find(|manipulator_group| manipulator_group.id == id)
	}

	/// Access a mutable [ManipulatorGroup] from a PointId.
	pub fn manipulator_mut_from_id(&mut self, id: PointId) -> Option<&mut ManipulatorGroup<PointId>> {
		self.manipulator_groups.iter_mut().find(|manipulator_group| manipulator_group.id == id)
	}

	/// Access the index of a [ManipulatorGroup] from a PointId.
	pub fn manipulator_index_from_id(&self, id: PointId) -> Option<usize> {
		self.manipulator_groups.iter().position(|manipulator_group| manipulator_group.id == id)
	}

	/// Insert a manipulator group at an index.
	pub fn insert_manipulator_group(&mut self, index: usize, group: ManipulatorGroup<PointId>) {
		assert!(group.is_finite(), "Inserting non finite manipulator group");
		self.manipulator_groups.insert(index, group)
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

	/// Remove a manipulator group at an index.
	pub fn remove_manipulator_group(&mut self, index: usize) -> ManipulatorGroup<PointId> {
		self.manipulator_groups.remove(index)
	}
}
