use super::structs::Identifier;
use super::*;
use glam::{DAffine2, DVec2};

/// Functionality that transforms Subpaths, such as split, reduce, offset, etc.
impl<PointId: Identifier> Subpath<PointId> {
	/// Returns [ManipulatorGroup]s with a reversed winding order.
	fn reverse_manipulator_groups(manipulator_groups: &[ManipulatorGroup<PointId>]) -> Vec<ManipulatorGroup<PointId>> {
		manipulator_groups
			.iter()
			.rev()
			.map(|group| ManipulatorGroup {
				anchor: group.anchor,
				in_handle: group.out_handle,
				out_handle: group.in_handle,
				id: PointId::new(),
			})
			.collect::<Vec<ManipulatorGroup<PointId>>>()
	}

	/// Returns a [Subpath] with a reversed winding order.
	/// Note that a reversed closed subpath will start on the same manipulator group and simply wind the other direction
	pub fn reverse(&self) -> Subpath<PointId> {
		let mut reversed = Subpath::reverse_manipulator_groups(self.manipulator_groups());
		if self.closed {
			reversed.rotate_right(1);
		};
		Subpath {
			manipulator_groups: reversed,
			closed: self.closed,
		}
	}

	/// Apply a transformation to all of the [ManipulatorGroup]s in the [Subpath].
	pub fn apply_transform(&mut self, affine_transform: DAffine2) {
		for manipulator_group in &mut self.manipulator_groups {
			manipulator_group.apply_transform(affine_transform);
		}
	}

	/// Returns a subpath that results from rotating this subpath around the origin by the given angle (in radians).
	pub fn rotate(&self, angle: f64) -> Subpath<PointId> {
		let mut rotated_subpath = self.clone();

		let affine_transform: DAffine2 = DAffine2::from_angle(angle);
		rotated_subpath.apply_transform(affine_transform);

		rotated_subpath
	}

	/// Returns a subpath that results from rotating this subpath around the provided point by the given angle (in radians).
	pub fn rotate_about_point(&self, angle: f64, pivot: DVec2) -> Subpath<PointId> {
		// Translate before and after the rotation to account for the pivot
		let translate: DAffine2 = DAffine2::from_translation(pivot);
		let rotate: DAffine2 = DAffine2::from_angle(angle);
		let translate_inverse = translate.inverse();

		let mut rotated_subpath = self.clone();
		rotated_subpath.apply_transform(translate * rotate * translate_inverse);
		rotated_subpath
	}
}
