use super::vector_shape::VectorShape;
use super::{constants::MINIMUM_MIRROR_THRESHOLD, vector_anchor::VectorAnchor, vector_control_point::VectorControlPoint};
use crate::message_prelude::Message;
use glam::DVec2;
use std::collections::{HashSet, VecDeque};

/// ShapeEditor is the container for all of the selected kurbo paths that are
/// represented as VectorShapes and provides functionality required
/// to query and create the VectorShapes / VectorAnchors / VectorControlPoints
#[derive(Clone, Debug, Default)]
pub struct ShapeEditor {
	// The shapes we can select anchors / handles from
	pub shapes_to_modify: Vec<VectorShape>,
	// Index of the shape that contained the most recent selected point
	pub selected_shape_indices: HashSet<usize>,
	// The initial drag position of the mouse on drag start
	pub drag_start_position: DVec2,
}

impl ShapeEditor {
	/// Select the first point within the selection threshold
	/// Returns true if we've found a point, false otherwise
	pub fn select_point(&mut self, mouse_position: DVec2, select_threshold: f64, add_to_selection: bool, responses: &mut VecDeque<Message>) -> bool {
		if self.shapes_to_modify.is_empty() {
			return false;
		}

		if let Some((shape_index, anchor_index, point_index)) = self.find_nearest_point_indicies(mouse_position, select_threshold) {
			log::trace!("Selecting: shape {} / anchor {} / point {}", shape_index, anchor_index, point_index);

			// Add this shape to the selection
			self.add_selected_shape(shape_index);

			// If the point we're selecting has already been selected
			// we can assume this point exists.. since we did just click on it hense the unwrap
			let is_point_selected = self.shapes_to_modify[shape_index].anchors[anchor_index].points[point_index].as_ref().unwrap().is_selected;

			// Deselected if we're not adding to the selection
			if !add_to_selection && !is_point_selected {
				self.deselect_all(responses);
			}

			let selected_shape = &mut self.shapes_to_modify[shape_index];
			selected_shape.elements = selected_shape.bez_path.clone().into_iter().collect();

			// Should we select or deselect the point?
			let should_select = if is_point_selected { !(add_to_selection && is_point_selected) } else { true };

			// Add which anchor and point was selected
			let selected_anchor = selected_shape.select_anchor(anchor_index);
			let selected_point = selected_anchor.select_point(point_index, should_select, responses);

			// Set the drag start position based on the selected point
			if let Some(point) = selected_point {
				self.drag_start_position = point.position;
			}

			// Due to the shape data structure not persisting across shape selection changes we need to rely on the kurbo path to know if we should mirror
			selected_anchor.set_mirroring((selected_anchor.angle_between_handles().abs() - std::f64::consts::PI).abs() < MINIMUM_MIRROR_THRESHOLD);
			return true;
		}
		false
	}

	/// Find a point that is within the selection threshold and return an index to the shape, anchor, and point
	pub fn find_nearest_point_indicies(&mut self, mouse_position: DVec2, select_threshold: f64) -> Option<(usize, usize, usize)> {
		if self.shapes_to_modify.is_empty() {
			return None;
		}

		let select_threshold_squared = select_threshold * select_threshold;
		// Find the closest control point among all elements of shapes_to_modify
		for shape_index in 0..self.shapes_to_modify.len() {
			if let Some((anchor_index, point_index, distance_squared)) = self.closest_point_indices(&self.shapes_to_modify[shape_index], mouse_position) {
				// Choose the first point under the threshold
				if distance_squared < select_threshold_squared {
					log::trace!("Selecting: shape {} / anchor {} / point {}", shape_index, anchor_index, point_index);
					return Some((shape_index, anchor_index, point_index));
				}
			}
		}
		None
	}

	/// A wrapper for find_nearest_point_indicies and returns a mutable VectorControlPoint
	pub fn find_nearest_point(&mut self, mouse_position: DVec2, select_threshold: f64) -> Option<&mut VectorControlPoint> {
		let (shape_index, anchor_index, point_index) = self.find_nearest_point_indicies(mouse_position, select_threshold)?;
		let selected_shape = &mut self.shapes_to_modify[shape_index];
		selected_shape.anchors[anchor_index].points[point_index].as_mut()
	}

	/// Set the shapes we consider for selection, we will choose draggable handles / anchors from these shapes.
	pub fn set_shapes_to_modify(&mut self, selected_shapes: Vec<VectorShape>) {
		self.shapes_to_modify = selected_shapes;
	}

	/// Add a shape to the hashset of shapes we consider for selection
	pub fn add_selected_shape(&mut self, shape_index: usize) {
		self.selected_shape_indices.insert(shape_index);
	}

	/// Provide the shapes that the currently selected points are a part of
	pub fn selected_shapes(&self) -> impl Iterator<Item = &VectorShape> {
		self.shapes_to_modify
			.iter()
			.enumerate()
			.filter_map(|(index, shape)| if self.selected_shape_indices.contains(&index) { Some(shape) } else { None })
	}

	/// Provide the mutable shapes that the currently selected points are a part of
	pub fn selected_shapes_mut(&mut self) -> impl Iterator<Item = &mut VectorShape> {
		self.shapes_to_modify
			.iter_mut()
			.enumerate()
			.filter_map(|(index, shape)| if self.selected_shape_indices.contains(&index) { Some(shape) } else { None })
	}

	/// Provide the currently selected anchor by reference
	pub fn selected_anchors(&self) -> impl Iterator<Item = &VectorAnchor> {
		self.selected_shapes().flat_map(|shape| shape.selected_anchors())
	}

	/// Provide the currently selected anchors by mutable reference
	pub fn selected_anchors_mut(&mut self) -> impl Iterator<Item = &mut VectorAnchor> {
		self.selected_shapes_mut().flat_map(|shape| shape.selected_anchors_mut())
	}

	/// Provide the currently selected points by reference
	pub fn selected_points(&self) -> impl Iterator<Item = &VectorControlPoint> {
		self.selected_shapes().flat_map(|shape| shape.selected_anchors()).flat_map(|anchors| anchors.selected_points())
	}

	/// Provide the currently selected points by mutable reference
	pub fn selected_points_mut(&mut self) -> impl Iterator<Item = &mut VectorControlPoint> {
		self.selected_shapes_mut()
			.flat_map(|shape| shape.selected_anchors_mut())
			.flat_map(|anchors| anchors.selected_points_mut())
	}

	/// Move the selected points by dragging the moue
	pub fn move_selected_points(&mut self, mouse_position: DVec2, should_mirror: bool, responses: &mut VecDeque<Message>) {
		let drag_start_position = self.drag_start_position;
		for shape in self.selected_shapes_mut() {
			shape.move_selected(mouse_position - drag_start_position, should_mirror, responses);
		}
	}

	/// Remove all of the overlays from the shapes the manipulation handler has created
	pub fn deselect_all(&mut self, responses: &mut VecDeque<Message>) {
		for shape in self.shapes_to_modify.iter_mut() {
			shape.clear_selected_anchors(responses);
			// Apply the final elements to the shape
			// Fixes the snapback problem
			shape.elements = shape.bez_path.clone().into_iter().collect();
		}
	}

	/// Remove all of the overlays for the VectorManipulators / shape
	pub fn remove_overlays(&mut self, responses: &mut VecDeque<Message>) {
		for shape in self.shapes_to_modify.iter_mut() {
			shape.remove_overlays(responses)
		}
	}

	// TODO Use quadtree or some equivalent spatial acceleration structure to improve this to O(log(n))
	/// Find the closest point, anchor and distance so we can select path elements
	/// Brute force comparison to determine which handle / anchor we want to select, O(n)
	fn closest_point_indices(&self, shape: &VectorShape, pos: glam::DVec2) -> Option<(usize, usize, f64)> {
		let mut closest_distance_squared: f64 = f64::MAX; // Not ideal
		let mut result: Option<(usize, usize, f64)> = None;
		for (anchor_index, anchor) in shape.anchors.iter().enumerate() {
			let point_index = anchor.closest_point(pos);
			if let Some(point) = &anchor.points[point_index] {
				if point.can_be_selected {
					let distance_squared = point.position.distance_squared(pos);
					if distance_squared < closest_distance_squared {
						closest_distance_squared = distance_squared;
						result = Some((anchor_index, point_index, distance_squared));
					}
				}
			}
		}
		result
	}
}
