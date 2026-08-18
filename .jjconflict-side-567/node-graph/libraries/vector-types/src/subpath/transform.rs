use super::structs::Identifier;
use super::*;
use glam::DAffine2;

impl<PointId: Identifier> Subpath<PointId> {
	/// Apply a transformation to all of the [ManipulatorGroup]s in the [Subpath].
	pub fn apply_transform(&mut self, affine_transform: DAffine2) {
		for manipulator_group in &mut self.manipulator_groups {
			manipulator_group.apply_transform(affine_transform);
		}
	}
}
