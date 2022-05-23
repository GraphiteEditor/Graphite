use super::{constants::ControlPointType, vector_control_point::VectorControlPoint};
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
	pub fn new_with_handles(anchor_pos: DVec2, handle_in_pos: DVec2, handle_out_pos: DVec2) -> Self {
		Self {
			points: [
				Some(VectorControlPoint::new(anchor_pos, ControlPointType::Anchor)),
				Some(VectorControlPoint::new(handle_in_pos, ControlPointType::InHandle)),
				Some(VectorControlPoint::new(handle_out_pos, ControlPointType::OutHandle)),
			],
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
		self.editor_state.mirror_angle_between_handles = mirroring;
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

