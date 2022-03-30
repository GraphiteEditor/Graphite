use crate::{
	consts::VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE,
	message_prelude::{DocumentMessage, Message},
};

use super::{
	constants::{ControlPointType, ROUNDING_BIAS},
	vector_control_point::VectorControlPoint,
};

use graphene::{LayerId, Operation};

use glam::{DAffine2, DVec2};
use kurbo::{PathEl, Point, Vec2};
use std::collections::VecDeque;

/// VectorAnchor is used to represent an anchor point on the path that can be moved.
/// It contains 0-2 handles that are optionally displayed.
#[derive(PartialEq, Clone, Debug, Default)]
pub struct VectorAnchor {
	// Editable points for the anchor & handles
	pub points: [Option<VectorControlPoint>; 3],
	// Does this anchor point have a path close element?
	pub close_element_id: Option<usize>,
	// Should we maintain the angle between the handles?
	pub handle_mirror_angle: bool,
	// Should we make the handles equidistance from the anchor?
	pub handle_mirror_distance: bool,
}

impl VectorAnchor {
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

	// TODO change relative to an enum (relative, absolute)
	/// Move the selected points by the provided delta
	pub fn move_selected_points(&mut self, translation: DVec2, relative: bool, transform: &DAffine2) {
		// TODO This needs to be rebuilt without usage of kurbo
	}

	/// Returns true if any points in this anchor are selected
	pub fn is_selected(&self) -> bool {
		self.points.iter().flatten().any(|pnt| pnt.is_selected)
	}

	/// Set a point to selected by ID
	pub fn select_point(&mut self, point_id: usize, selected: bool, responses: &mut VecDeque<Message>) -> Option<&mut VectorControlPoint> {
		if let Some(point) = self.points[point_id].as_mut() {
			point.set_selected(selected, responses);
		}
		self.points[point_id].as_mut()
	}

	/// Clear the selected points for this anchor
	pub fn clear_selected_points(&mut self, responses: &mut VecDeque<Message>) {
		for point in self.points.iter_mut().flatten() {
			point.set_selected(false, responses);
		}
	}

	/// Provides the selected points in this anchor
	pub fn points(&self) -> impl Iterator<Item = &VectorControlPoint> {
		self.points.iter().flatten()
	}

	/// Returns
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
	pub fn opposing_handle(&self, handle: &VectorControlPoint) -> &Option<VectorControlPoint> {
		if let Some(point) = &self.points[ControlPointType::Handle1] {
			if point == handle {
				return &self.points[ControlPointType::Handle2];
			}
		};

		if let Some(point) = &self.points[ControlPointType::Handle2] {
			if point == handle {
				return &self.points[ControlPointType::Handle1];
			}
		};
		&None
	}

	/// Set the mirroring state
	pub fn set_mirroring(&mut self, mirroring: bool) {
		self.handle_mirror_angle = mirroring;
	}

	/// Helper function to more easily set position of VectorControlPoints
	pub fn set_point_position(&mut self, point_index: usize, position: DVec2) {
		if let Some(point) = &mut self.points[point_index] {
			point.position = position;
		}
	}
}
