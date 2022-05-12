use std::ops::Mul;

use super::{constants::ControlPointType, vector_control_point::VectorControlPoint};
use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};

/// VectorAnchor is used to represent an anchor point on the path that can be moved.
/// It contains 0-2 handles that are optionally displayed.
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct VectorAnchor {
	// Editable points for the anchor & handles
	pub points: [Option<VectorControlPoint>; 3],
	// Should we maintain the angle between the handles?

	// TODO Separate the editor func state from underlying data (use another struct)
	pub mirror_angle_active: bool,
	// Should we make the handles equidistance from the anchor?
	pub mirror_distance_active: bool,
}

impl Default for VectorAnchor {
	fn default() -> Self {
		Self {
			points: [None, None, None],
			mirror_angle_active: true,
			mirror_distance_active: true,
		}
	}
}

// TODO impl index for points

impl VectorAnchor {
	/// Create a new anchor with the given position
	pub fn new(anchor_pos: DVec2) -> Self {
		Self {
			// An anchor and 2x None's which represent non-existent handles
			points: [Some(VectorControlPoint::new(anchor_pos, ControlPointType::Anchor)), None, None],
			mirror_angle_active: false,
			mirror_distance_active: false,
		}
	}

	/// Create a new anchor with the given anchor position and handles
	pub fn new_with_handles(anchor_pos: DVec2, handle1_pos: DVec2, handle2_pos: DVec2) -> Self {
		Self {
			points: [
				Some(VectorControlPoint::new(anchor_pos, ControlPointType::Anchor)),
				Some(VectorControlPoint::new(handle1_pos, ControlPointType::Handle1)),
				Some(VectorControlPoint::new(handle2_pos, ControlPointType::Handle2)),
			],
			mirror_angle_active: false,
			mirror_distance_active: false,
		}
	}

	/// Finds the closest VectorControlPoint owned by this anchor. This can be the handles or the anchor itself
	pub fn closest_point(&self, target: glam::DVec2) -> usize {
		let mut closest_index: usize = 0;
		let mut closest_distance_squared: f64 = f64::MAX; // Not ideal
		for (index, point) in self.points.iter().enumerate() {
			if let Some(point) = point {
				let distance_squared = point.position.distance_squared(target);
				if distance_squared < closest_distance_squared {
					closest_distance_squared = distance_squared;
					closest_index = index;
				}
			}
		}
		closest_index
	}

	/// Move the selected points by the provided transform
	/// if relative is false the point is transformed and its original position is subtracted
	pub fn move_selected_points(&mut self, relative: bool, transform: &DAffine2) {
		for point in self.selected_points_mut() {
			if !relative {
				let copy = point.clone().position;
				point.transform(transform);
				point.move_by(&(-copy));
			}
			point.transform(transform);
		}
	}

	/// Returns true if any points in this anchor are selected
	pub fn any_points_selected(&self) -> bool {
		self.points.iter().flatten().any(|pnt| pnt.is_selected)
	}

	/// Returns true if the anchor point is selected
	pub fn is_anchor_selected(&self) -> bool {
		if let Some(anchor) = &self.points[0] {
			anchor.is_selected
		} else {
			false
		}
	}

	/// Set a point to selected by ID
	pub fn select_point(&mut self, point_id: usize, selected: bool) -> Option<&mut VectorControlPoint> {
		if let Some(point) = self.points[point_id].as_mut() {
			point.set_selected(selected);
		}
		self.points[point_id].as_mut()
	}

	/// Clear the selected points for this anchor
	pub fn clear_selected_points(&mut self) {
		for point in self.points.iter_mut().flatten() {
			point.set_selected(false);
		}
	}

	/// Provides the points in this anchor
	pub fn points(&self) -> impl Iterator<Item = &VectorControlPoint> {
		self.points.iter().flatten()
	}

	/// Provides the points in this anchor
	pub fn points_mut(&mut self) -> impl Iterator<Item = &mut VectorControlPoint> {
		self.points.iter_mut().flatten()
	}

	/// Provides the selected points in this anchor
	pub fn selected_points(&self) -> impl Iterator<Item = &VectorControlPoint> {
		self.points.iter().flatten().filter(|pnt| pnt.is_selected)
	}

	/// Provides mutable selected points in this anchor
	pub fn selected_points_mut(&mut self) -> impl Iterator<Item = &mut VectorControlPoint> {
		self.points.iter_mut().flatten().filter(|pnt| pnt.is_selected)
	}

	/// Angle between handles in radians
	pub fn angle_between_handles(&self) -> f64 {
		if let [Some(a1), Some(h1), Some(h2)] = &self.points {
			return (a1.position - h1.position).angle_between(a1.position - h2.position);
		}
		0.0
	}

	/// Returns the opposing handle to the handle provided
	/// Returns the anchor handle if the anchor is provided
	pub fn opposing_handle(&self, handle: &VectorControlPoint) -> &Option<VectorControlPoint> {
		&self.points[!handle.manipulator_type]
	}

	/// Set the mirroring state
	pub fn set_mirroring(&mut self, mirroring: bool) {
		self.mirror_angle_active = mirroring;
	}

	/// Helper function to more easily set position of VectorControlPoints
	pub fn set_point_position(&mut self, point_index: usize, position: DVec2) {
		if let Some(point) = &mut self.points[point_index] {
			point.position = position;
		}
	}

	pub fn transform(&mut self, transform: &DAffine2) {
		for point in self.points_mut() {
			point.position = transform.transform_point2(point.position);
		}
	}
}
