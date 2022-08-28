use super::consts::ManipulatorType;
use super::manipulator_point::ManipulatorPoint;

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};

/// [ManipulatorGroup] is used to represent an anchor point + handles on the path that can be moved.
/// It contains 0-2 handles that are optionally available.
///
/// Overview:
/// ```text
///          ManipulatorGroup                <- Container for the anchor metadata and optional ManipulatorPoint
///                  |
///    [Option<ManipulatorPoint>; 3]         <- [0] is the anchor's draggable point (but not metadata), [1] is the
///      /           |           \              InHandle's draggable point, [2] is the OutHandle's draggable point
///     /            |            \
/// "Anchor"    "InHandle"    "OutHandle"    <- These are ManipulatorPoints and the only editable "primitive"
/// ```
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, Default)]
pub struct ManipulatorGroup {
	/// Editable points for the anchor and handles.
	pub points: [Option<ManipulatorPoint>; 3],

	#[serde(skip)]
	// TODO: Remove this from Graphene, editor state should be stored in the frontend if possible.
	/// The editor state of the anchor and handles.
	pub editor_state: ManipulatorGroupEditorState,
}

impl ManipulatorGroup {
	/// Create a new anchor with the given position.
	pub fn new_with_anchor(anchor_pos: DVec2) -> Self {
		Self {
			// An anchor and 2x None's which represent non-existent handles
			points: [Some(ManipulatorPoint::new(anchor_pos, ManipulatorType::Anchor)), None, None],
			editor_state: ManipulatorGroupEditorState::default(),
		}
	}

	/// Create a new anchor with the given anchor position and handles.
	pub fn new_with_handles(anchor_pos: DVec2, handle_in_pos: Option<DVec2>, handle_out_pos: Option<DVec2>) -> Self {
		Self {
			points: match (handle_in_pos, handle_out_pos) {
				(Some(pos1), Some(pos2)) => [
					Some(ManipulatorPoint::new(anchor_pos, ManipulatorType::Anchor)),
					Some(ManipulatorPoint::new(pos1, ManipulatorType::InHandle)),
					Some(ManipulatorPoint::new(pos2, ManipulatorType::OutHandle)),
				],
				(None, Some(pos2)) => [
					Some(ManipulatorPoint::new(anchor_pos, ManipulatorType::Anchor)),
					None,
					Some(ManipulatorPoint::new(pos2, ManipulatorType::OutHandle)),
				],
				(Some(pos1), None) => [
					Some(ManipulatorPoint::new(anchor_pos, ManipulatorType::Anchor)),
					Some(ManipulatorPoint::new(pos1, ManipulatorType::InHandle)),
					None,
				],
				(None, None) => [Some(ManipulatorPoint::new(anchor_pos, ManipulatorType::Anchor)), None, None],
			},
			editor_state: ManipulatorGroupEditorState::default(),
		}
	}

	// TODO Convert into bool in subpath
	/// Create a [ManipulatorGroup] that represents a close path command.
	pub fn closed() -> Self {
		Self {
			// An anchor (the first element) being `None` indicates a ClosePath (i.e. a path end command)
			points: [None, None, None],
			editor_state: ManipulatorGroupEditorState::default(),
		}
	}

	/// Answers whether this [ManipulatorGroup] represent a close shape command.
	pub fn is_close(&self) -> bool {
		self.points[ManipulatorType::Anchor].is_none() && self.points[ManipulatorType::InHandle].is_none()
	}

	/// Finds the closest [ManipulatorPoint] owned by this [ManipulatorGroup]. This may return the anchor or either handle.
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

	/// Move the selected points by the provided transform.
	pub fn move_selected_points(&mut self, delta: DVec2) {
		let mirror_angle = self.editor_state.mirror_angle_between_handles;
		// Invert distance since we want it to start disabled
		let mirror_distance = !self.editor_state.mirror_distance_between_handles;

		// Move the point absolutely or relatively depending on if the point is under the cursor (the last selected point)
		let move_point = |point: &mut ManipulatorPoint, delta: DVec2| {
			point.position += delta;
			assert!(point.position.is_finite(), "Point is not finite!")
		};

		// Find the correctly mirrored handle position based on mirroring settings
		let move_symmetrical_handle = |position: DVec2, opposing_handle: Option<&mut ManipulatorPoint>, center: DVec2| {
			// Early out for cases where we can't mirror
			if !mirror_angle || opposing_handle.is_none() {
				return;
			}
			let opposing_handle = opposing_handle.unwrap();

			// Keep rotational similarity, but distance variable
			let radius = if mirror_distance { center.distance(position) } else { center.distance(opposing_handle.position) };

			if let Some(offset) = (position - center).try_normalize() {
				opposing_handle.position = center - offset * radius;
				assert!(opposing_handle.position.is_finite(), "Opposing handle not finite!")
			}
		};

		// If no points are selected, why are we here at all?
		if !self.any_points_selected() {
			return;
		}

		// If the anchor is selected, ignore any handle mirroring/dragging and drag all points
		if self.is_anchor_selected() {
			for point in self.points_mut() {
				move_point(point, delta);
			}
			return;
		}

		// If the anchor isn't selected, but both handles are, drag only handles
		if self.both_handles_selected() {
			for point in self.selected_handles_mut() {
				move_point(point, delta);
			}
			return;
		}

		// If the anchor isn't selected, and only one handle is selected
		// Drag the single handle
		let reflect_center = self.points[ManipulatorType::Anchor].as_ref().unwrap().position;
		let selected_handle = self.selected_handles_mut().next().unwrap();
		move_point(selected_handle, delta);

		// Move the opposing handle symmetrically if our mirroring flags allow
		let selected_handle = &selected_handle.clone();
		let opposing_handle = self.opposing_handle_mut(selected_handle);
		move_symmetrical_handle(selected_handle.position, opposing_handle, reflect_center);
	}

	/// Delete any [ManipulatorPoint] that are selected, this includes handles or the anchor.
	pub fn delete_selected(&mut self) {
		for point_option in self.points.iter_mut() {
			if let Some(point) = point_option {
				if point.editor_state.is_selected {
					*point_option = None;
				}
			}
		}
	}

	/// Returns true if any points in this [ManipulatorGroup] are selected.
	pub fn any_points_selected(&self) -> bool {
		self.points.iter().flatten().any(|point| point.editor_state.is_selected)
	}

	/// Returns true if the anchor point is selected.
	pub fn is_anchor_selected(&self) -> bool {
		if let Some(anchor) = &self.points[0] {
			anchor.editor_state.is_selected
		} else {
			false
		}
	}

	/// Determines if the two handle points are selected.
	pub fn both_handles_selected(&self) -> bool {
		self.points.iter().skip(1).flatten().filter(|pnt| pnt.editor_state.is_selected).count() == 2
	}

	/// Set a point, given its [ManipulatorType] enum integer ID, to a chosen selected state.
	pub fn select_point(&mut self, point_id: usize, selected: bool) -> Option<&mut ManipulatorPoint> {
		if let Some(point) = self.points[point_id].as_mut() {
			point.set_selected(selected);
		}
		self.points[point_id].as_mut()
	}

	/// Clear the selected points for this [ManipulatorGroup].
	pub fn clear_selected_points(&mut self) {
		for point in self.points.iter_mut().flatten() {
			point.set_selected(false);
		}
	}

	/// Provides the points in this [ManipulatorGroup].
	pub fn points(&self) -> impl Iterator<Item = &ManipulatorPoint> {
		self.points.iter().flatten()
	}

	/// Provides the points in this [ManipulatorGroup] as mutable.
	pub fn points_mut(&mut self) -> impl Iterator<Item = &mut ManipulatorPoint> {
		self.points.iter_mut().flatten()
	}

	/// Provides the selected points in this [ManipulatorGroup].
	pub fn selected_points(&self) -> impl Iterator<Item = &ManipulatorPoint> {
		self.points.iter().flatten().filter(|pnt| pnt.editor_state.is_selected)
	}

	/// Provides mutable selected points in this [ManipulatorGroup].
	pub fn selected_points_mut(&mut self) -> impl Iterator<Item = &mut ManipulatorPoint> {
		self.points.iter_mut().flatten().filter(|pnt| pnt.editor_state.is_selected)
	}

	/// Provides the selected handles attached to this [ManipulatorGroup].
	pub fn selected_handles(&self) -> impl Iterator<Item = &ManipulatorPoint> {
		self.points.iter().skip(1).flatten().filter(|pnt| pnt.editor_state.is_selected)
	}

	/// Provides the mutable selected handles attached to this [ManipulatorGroup].
	pub fn selected_handles_mut(&mut self) -> impl Iterator<Item = &mut ManipulatorPoint> {
		self.points.iter_mut().skip(1).flatten().filter(|pnt| pnt.editor_state.is_selected)
	}

	/// Angle between handles, in radians.
	pub fn angle_between_handles(&self) -> f64 {
		if let [Some(a1), Some(h1), Some(h2)] = &self.points {
			(a1.position - h1.position).angle_between(a1.position - h2.position)
		} else {
			0.
		}
	}

	/// Returns the opposing handle to the handle provided.
	/// Returns [None] if the provided handle is of type [ManipulatorType::Anchor].
	/// Returns [None] if the opposing handle doesn't exist.
	pub fn opposing_handle(&self, handle: &ManipulatorPoint) -> Option<&ManipulatorPoint> {
		if handle.manipulator_type == ManipulatorType::Anchor {
			return None;
		}
		self.points[handle.manipulator_type.opposite_handle()].as_ref()
	}

	/// Returns the opposing handle to the handle provided, mutable.
	/// Returns [None] if the provided handle is of type [ManipulatorType::Anchor].
	/// Returns [None] if the opposing handle doesn't exist.
	pub fn opposing_handle_mut(&mut self, handle: &ManipulatorPoint) -> Option<&mut ManipulatorPoint> {
		if handle.manipulator_type == ManipulatorType::Anchor {
			return None;
		}
		self.points[handle.manipulator_type.opposite_handle()].as_mut()
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

	/// Helper function to more easily set position of [ManipulatorPoints]
	pub fn set_point_position(&mut self, point_index: usize, position: DVec2) {
		assert!(position.is_finite(), "Tried to set_point_position to non finite");
		if let Some(point) = &mut self.points[point_index] {
			point.position = position;
		} else {
			self.points[point_index] = Some(ManipulatorPoint::new(position, ManipulatorType::from_index(point_index)))
		}
	}

	/// Apply an affine transformation the points
	pub fn transform(&mut self, transform: &DAffine2) {
		for point in self.points_mut() {
			point.transform(transform);
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManipulatorGroupEditorState {
	// Whether the angle between the handles should be maintained
	pub mirror_angle_between_handles: bool,
	// Whether the distance between the handles should be equidistant to the anchor
	pub mirror_distance_between_handles: bool,
}

impl Default for ManipulatorGroupEditorState {
	fn default() -> Self {
		Self {
			mirror_angle_between_handles: true,
			mirror_distance_between_handles: true,
		}
	}
}
