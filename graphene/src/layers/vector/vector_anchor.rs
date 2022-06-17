use std::ops::Deref;

use super::{
	constants::{ControlPointType, SELECTION_THRESHOLD},
	vector_control_point::VectorControlPoint,
};
use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};

/// Brief overview of VectorAnchor
///                    VectorAnchor <- Container for the anchor metadata and optional VectorControlPoints
///                          /
///            [Option<VectorControlPoint>; 3] <- [0] is the anchor's draggable point (but not metadata), [1] is the InHandle's draggable point, [2] is the OutHandle's draggable point
///          /              |                      \
///      "Anchor"        "InHandle"             "OutHandle" <- These are VectorControlPoints and the only editable "primitive"

/// VectorAnchor is used to represent an anchor point + handles on the path that can be moved.
/// It contains 0-2 handles that are optionally available.
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, Default)]
pub struct VectorAnchor {
	// Editable points for the anchor & handles
	pub points: [Option<VectorControlPoint>; 3],

	#[serde(skip)]
	// The editor state of the anchor and handles
	pub editor_state: VectorAnchorState,
}

impl VectorAnchor {
	/// Create a new anchor with the given position
	pub fn new(anchor_pos: DVec2) -> Self {
		Self {
			// An anchor and 2x None's which represent non-existent handles
			points: [Some(VectorControlPoint::new(anchor_pos, ControlPointType::Anchor)), None, None],
			editor_state: VectorAnchorState::default(),
		}
	}

	/// Create a new anchor with the given anchor position and handles
	pub fn new_with_handles(anchor_pos: DVec2, handle_in_pos: Option<DVec2>, handle_out_pos: Option<DVec2>) -> Self {
		Self {
			points: match (handle_in_pos, handle_out_pos) {
				(Some(pos1), Some(pos2)) => [
					Some(VectorControlPoint::new(anchor_pos, ControlPointType::Anchor)),
					Some(VectorControlPoint::new(pos1, ControlPointType::InHandle)),
					Some(VectorControlPoint::new(pos2, ControlPointType::OutHandle)),
				],
				(None, Some(pos2)) => [
					Some(VectorControlPoint::new(anchor_pos, ControlPointType::Anchor)),
					None,
					Some(VectorControlPoint::new(pos2, ControlPointType::OutHandle)),
				],
				(Some(pos1), None) => [
					Some(VectorControlPoint::new(anchor_pos, ControlPointType::Anchor)),
					Some(VectorControlPoint::new(pos1, ControlPointType::InHandle)),
					None,
				],
				(None, None) => [Some(VectorControlPoint::new(anchor_pos, ControlPointType::Anchor)), None, None],
			},
			editor_state: VectorAnchorState::default(),
		}
	}

	/// Create a VectorAnchor that represents a close path signal
	pub fn closed() -> Self {
		Self {
			// An anchor being None indicates a ClosePath (aka a path end)
			points: [None, None, None],
			editor_state: VectorAnchorState::default(),
		}
	}

	/// Finds the closest VectorControlPoint owned by this anchor. This can be the handles or the anchor itself
	pub fn closest_point(&self, transform_space: &DAffine2, target: glam::DVec2) -> usize {
		let mut closest_index: usize = 0;
		let mut closest_distance_squared: f64 = f64::MAX; // Not ideal
		for (index, point) in self.points.iter().enumerate() {
			if let Some(point) = point {
				let distance_squared = transform_space.transform_point2(point.position).distance_squared(target);
				if distance_squared < closest_distance_squared {
					closest_distance_squared = distance_squared;
					closest_index = index;
				}
			}
		}
		closest_index
	}

	/// Move the selected points by the provided transform
	pub fn move_selected_points(&mut self, delta: DVec2, absolute_position: DVec2, viewspace: &DAffine2) {
		// TODO Use an ID as opposed to distance, stopgap for now
		let is_drag_target = |point: &mut VectorControlPoint| -> bool { viewspace.transform_point2(absolute_position).distance(viewspace.transform_point2(point.position)) < SELECTION_THRESHOLD };

		let move_absolute = |point: &mut VectorControlPoint, position: DVec2| {
			point.position = position;
		};

		let move_relative = |point: &mut VectorControlPoint, delta: DVec2| {
			point.position += delta;
		};

		// If the anchor is selected ignore any handle mirroring / dragging
		if self.is_anchor_selected() {
			for point in self.points_mut() {
				if is_drag_target(point) {
					move_absolute(point, absolute_position)
				} else {
					move_relative(point, delta)
				}
			}
			return;
		}

		for point in self.selected_handles_mut() {
			if is_drag_target(point) {
				move_absolute(point, absolute_position)
			} else {
				move_relative(point, delta)
			}
		}

		// Apply any secondary motion to unselected points
		if let Some(position) = self.reflected_handle_position(
			self.points[ControlPointType::InHandle].as_ref(),
			self.editor_state.mirror_angle_between_handles,
			self.editor_state.mirror_distance_between_handles,
		) {
			move_absolute(self.points[ControlPointType::OutHandle].as_mut().unwrap(), position)
		}
		if let Some(position) = self.reflected_handle_position(
			self.points[ControlPointType::OutHandle].as_ref(),
			self.editor_state.mirror_angle_between_handles,
			self.editor_state.mirror_distance_between_handles,
		) {
			move_absolute(self.points[ControlPointType::InHandle].as_mut().unwrap(), position)
		}
	}

	/// Delete any VectorControlPoint that are selected, this includes handles or the anchor
	pub fn delete_selected(&mut self) {
		for point_option in self.points.iter_mut() {
			if let Some(point) = point_option {
				if point.editor_state.is_selected {
					*point_option = None;
				}
			}
		}
	}

	/// Returns true if any points in this anchor are selected
	pub fn any_points_selected(&self) -> bool {
		self.points.iter().flatten().any(|pnt| pnt.editor_state.is_selected)
	}

	/// Returns true if the anchor point is selected
	pub fn is_anchor_selected(&self) -> bool {
		if let Some(anchor) = &self.points[0] {
			anchor.editor_state.is_selected
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
		self.points.iter().flatten().filter(|pnt| pnt.editor_state.is_selected)
	}

	/// Provides mutable selected points in this anchor
	pub fn selected_points_mut(&mut self) -> impl Iterator<Item = &mut VectorControlPoint> {
		self.points.iter_mut().flatten().filter(|pnt| pnt.editor_state.is_selected)
	}

	/// Provides the selected handles attached to this anchor
	pub fn selected_handles(&self) -> impl Iterator<Item = &VectorControlPoint> {
		self.points.iter().skip(1).flatten().filter(|pnt| pnt.editor_state.is_selected)
	}

	/// Provides the mutable selected handles attached to this anchor
	pub fn selected_handles_mut(&mut self) -> impl Iterator<Item = &mut VectorControlPoint> {
		self.points.iter_mut().skip(1).flatten().filter(|pnt| pnt.editor_state.is_selected)
	}

	/// Angle between handles in radians
	pub fn angle_between_handles(&self) -> f64 {
		if let [Some(a1), Some(h1), Some(h2)] = &self.points {
			return (a1.position - h1.position).angle_between(a1.position - h2.position);
		}
		0.0
	}

	/// Find the correctly mirrored handle position based on mirroring settings
	fn reflected_handle_position(&self, handle: Option<&VectorControlPoint>, mirror_angle: bool, mirror_distance: bool) -> Option<DVec2> {
		if let Some(handle) = handle {
			let opposing_handle = &self.opposing_handle(handle).as_ref();
			// Early out for cases where we can't mirror
			if !mirror_angle || !handle.is_selected() || opposing_handle.is_none() || opposing_handle.unwrap().is_selected() {
				return None;
			}

			let opposing_handle = opposing_handle.unwrap();
			let center = self.points[ControlPointType::Anchor].as_ref().unwrap();

			// Keep rotational similarity, but distance variable
			let radius = if mirror_distance {
				center.position.distance(handle.position)
			} else {
				center.position.distance(opposing_handle.position)
			};
			let phi = center.position - opposing_handle.position;
			let phi = phi.y.atan2(phi.x);
			return Some(DVec2::new(radius * phi.cos() + center.position.x, radius * phi.sin() + center.position.y));
		}
		None
	}

	/// Returns the opposing handle to the handle provided
	/// Returns the anchor handle if the anchor is provided
	pub fn opposing_handle(&self, handle: &VectorControlPoint) -> &Option<VectorControlPoint> {
		&self.points[!handle.manipulator_type]
	}

	/// Set the mirroring state
	pub fn toggle_mirroring(&mut self, toggle_distance: bool, toggle_angle: bool) {
		if toggle_distance {
			self.editor_state.mirror_distance_between_handles = !self.editor_state.mirror_distance_between_handles;
		}
		if toggle_angle {
			self.editor_state.mirror_angle_between_handles = !self.editor_state.mirror_angle_between_handles;
		}
	}

	/// Helper function to more easily set position of VectorControlPoints
	pub fn set_point_position(&mut self, point_index: usize, position: DVec2) {
		if let Some(point) = &mut self.points[point_index] {
			point.position = position;
		}
	}

	/// Apply an affine transformation the points
	pub fn transform(&mut self, transform: &DAffine2) {
		for point in self.points_mut() {
			point.transform(transform);
		}
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct VectorAnchorState {
	// If we should maintain the angle between the handles
	pub mirror_angle_between_handles: bool,
	// If we should make the handles equidistance from the anchor?
	pub mirror_distance_between_handles: bool,
}

impl Default for VectorAnchorState {
	fn default() -> Self {
		Self {
			mirror_angle_between_handles: true,
			mirror_distance_between_handles: true,
		}
	}
}
