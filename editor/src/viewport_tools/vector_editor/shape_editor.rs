/*
Overview: (OUT OF DATE, WILL BE UPDATED)

						ShapeEditor
						/          \
				VectorShape ... VectorShape  <- ShapeEditor contains many VectorShapes
					/                 \
			   VectorAnchor ...  VectorAnchor <- VectorShape contains many VectorAnchors


					VectorAnchor <- Container for the anchor metadata and optional VectorControlPoints
						  /
			[Option<VectorControlPoint>; 3] <- [0] is the anchor's draggable point (but not metadata), [1] is the handle1's draggable point, [2] is the handle2's draggable point
			 /              |                      \
		"Anchor"        "Handle1"          "Handle2" <- These are VectorControlPoints and the only editable / draggable "primitive"
*/

use super::vector_shape::VectorShape;
use super::{constants::MINIMUM_MIRROR_THRESHOLD, vector_anchor::VectorAnchor, vector_control_point::VectorControlPoint};
use crate::document::DocumentMessageHandler;
use crate::message_prelude::Message;

use graphene::layers::layer_info::LayerDataType;

use glam::{DAffine2, DVec2};
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
}

impl ShapeEditor {
	/// Select the first point within the selection threshold
	/// Returns true if we've found a point, false otherwise
	// TODO Refactor to select_point_from(vectorshapes[..], ...)
	pub fn select_point(&mut self, mouse_position: DVec2, select_threshold: f64, add_to_selection: bool, responses: &mut VecDeque<Message>) -> bool {
		if self.shapes_to_modify.is_empty() {
			return false;
		}

		if let Some((shape_index, anchor_index, point_index)) = self.find_nearest_point_indicies(mouse_position, select_threshold) {
			log::trace!("Selecting: shape {} / anchor {} / point {}", shape_index, anchor_index, point_index);

			// Add this shape to the selection
			self.set_shape_selected(shape_index);

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
			selected_anchor.select_point(point_index, should_select);

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

	/// Clear all of the shapes we can modify
	pub fn clear_shapes_to_modify(&mut self) {
		self.shapes_to_modify.clear();
	}

	/// Add a shape to the hashset of shapes we consider for selection
	pub fn set_shape_selected(&mut self, shape_index: usize) {
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

	/// A mutable iterator of all the anchors, regardless of selection
	pub fn anchors_mut(&mut self) -> impl Iterator<Item = &mut VectorAnchor> {
		self.shapes_to_modify.iter_mut().flat_map(|shape| shape.anchors_mut())
	}

	/// Select the last anchor in this shape
	pub fn select_last_anchor(&mut self) -> Option<&mut VectorAnchor> {
		if let Some(last) = self.shapes_to_modify.last_mut() {
			return Some(last.select_last_anchor());
		}
		None
	}

	/// Select the Nth anchor of the shape, negative numbers index from the end
	pub fn select_nth_anchor(&mut self, shape_index: usize, anchor_index: i32) -> &mut VectorAnchor {
		let shape = &mut self.shapes_to_modify[shape_index];
		if anchor_index < 0 {
			let anchor_index = shape.anchors.len() - ((-anchor_index) as usize);
			shape.select_anchor(anchor_index)
		} else {
			let anchor_index = anchor_index as usize;
			shape.select_anchor(anchor_index)
		}
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
	pub fn move_selected_points(&mut self, target: DVec2, relative: bool, responses: &mut VecDeque<Message>) {
		for shape in self.selected_shapes_mut() {
			shape.move_selected(target, relative);
		}
	}

	/// Dissolve the selected points
	pub fn delete_selected_points(&mut self, responses: &mut VecDeque<Message>) {
		for shape in self.selected_shapes_mut() {
			shape.delete_selected();
		}
	}

	/// Toggle if the handles should mirror angle across the anchor positon
	pub fn toggle_selected_mirror_angle(&mut self) {
		for anchor in self.selected_anchors_mut() {
			anchor.handle_mirror_angle = !anchor.handle_mirror_angle;
		}
	}

	pub fn set_selected_mirror_options(&mut self, mirror_angle: bool, mirror_distance: bool) {
		for anchor in self.selected_anchors_mut() {
			anchor.handle_mirror_angle = mirror_angle;
			anchor.handle_mirror_distance = mirror_distance;
		}
	}

	/// Toggle if the handles should mirror distance across the anchor position
	pub fn toggle_selected_mirror_distance(&mut self) {
		for anchor in self.selected_anchors_mut() {
			anchor.handle_mirror_distance = !anchor.handle_mirror_distance;
		}
	}

	/// Deselect all anchors from the shapes the manipulation handler has created
	pub fn deselect_all(&mut self, responses: &mut VecDeque<Message>) {
		for shape in self.shapes_to_modify.iter_mut() {
			shape.clear_selected_anchors();
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

	/// Move the selected point based on mouse input, if this is a handle we can control if we are mirroring or not
	/// A wrapper around move_point to handle mirror state / submit the changes
	pub fn move_selected(&mut self, target: DVec2, relative: bool) {
		for selected_shape in self.selected_shapes() {
			selected_shape.move_selected(target, relative);
		}

		// We've made our changes to the shape, submit them
		// Send changes to the renderer
	}

	/// Delete the selected point
	/// A wrapper around move_point to handle mirror state / submit the changes
	pub fn delete_selected(&mut self) {
		for selected_shape in self.selected_shapes() {
			selected_shape.delete_selected();
		}

		// We've made our changes to the shape, submit them
		// Send changes to the renderer
	}
}
