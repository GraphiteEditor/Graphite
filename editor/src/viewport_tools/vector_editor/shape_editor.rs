/*
Overview:

						ShapeEditor
						/          \
				   selected_shape_layers <- Paths to selected layers that may contain VectorShapes
					 |               |
				VectorShape ... VectorShape  <- Reference from layer paths, one Vectorshape per layer
					/                 \
			   VectorAnchor ...  VectorAnchor <- VectorShape contains many VectorAnchors


					VectorAnchor <- Container for the anchor metadata and optional VectorControlPoints
						  /
			[Option<VectorControlPoint>; 3] <- [0] is the anchor's draggable point (but not metadata), [1] is the handle1's draggable point, [2] is the handle2's draggable point
			 /              |                      \
		"Anchor"        "Handle1"          "Handle2" <- These are VectorControlPoints and the only editable / draggable "primitive"
*/

use std::collections::VecDeque;

use crate::message_prelude::{DocumentMessage, Message};

use super::vector_shape::VectorShape;
use super::{constants::MINIMUM_MIRROR_THRESHOLD, vector_anchor::VectorAnchor, vector_control_point::VectorControlPoint};

use glam::DVec2;
use graphene::document::Document;
use graphene::LayerId;

/// ShapeEditor is the container for all of the layer paths that are
/// represented as VectorShapes and provides functionality required
/// to query and create the VectorShapes / VectorAnchors / VectorControlPoints
#[derive(Clone, Debug, Default)]
pub struct ShapeEditor {
	// The layers we can select and edit anchors / handles from
	target_layers: Vec<Vec<LayerId>>,
}

// TODO Consider keeping a list of selected anchors to minimize traversals of the layers
impl ShapeEditor {
	/// Select the first point within the selection threshold
	/// Returns true if we've found a point, false otherwise
	// TODO Refactor to select_point_from(vectorshapes[..], ...)
	pub fn select_point(&self, document: &Document, mouse_position: DVec2, select_threshold: f64, add_to_selection: bool, responses: &mut VecDeque<Message>) -> bool {
		if self.target_layers.is_empty() {
			return false;
		}

		if let Some((shape_layer_path, anchor_id, point_index)) = self.find_nearest_point_indicies(document, mouse_position, select_threshold) {
			log::trace!("Selecting: anchor {} / point {}", anchor_id, point_index);

			// If the point we're selecting has already been selected
			// we can assume this point exists.. since we did just click on it hense the unwrap
			let is_point_selected = self.shape(document, shape_layer_path).unwrap().anchors().by_id(anchor_id).unwrap().points[point_index]
				.as_ref()
				.unwrap()
				.is_selected;

			let selected_shape = self.shape(document, shape_layer_path).unwrap();
			// TODO kurbo bez_path are no long present in the vector shapes, resolve fallout
			// selected_shape.elements = selected_shape.bez_path.clone().into_iter().collect();

			// Should we select or deselect the point?
			let should_select = if is_point_selected { !(add_to_selection && is_point_selected) } else { true };

			// This is selecting the anchor only for now, next to generalize to points
			responses.push_back(
				DocumentMessage::SelectVectorPoints {
					layer_path: shape_layer_path.to_vec(),
					anchor_ids: vec![anchor_id],
					add: !add_to_selection && !is_point_selected,
				}
				.into(),
			);

			// Add which anchor and point was selected
			// let selected_anchor = selected_shape.select_anchor(anchor_id).unwrap();
			// selected_anchor.select_point(point_index, should_select);
			// TODO Send message to select instead

			// Due to the shape data structure not persisting across shape selection changes we need to rely on the kurbo path to know if we should mirror
			// selected_anchor.set_mirroring((selected_anchor.angle_between_handles().abs() - std::f64::consts::PI).abs() < MINIMUM_MIRROR_THRESHOLD);
			// TODO Send message to select instead
			return true;
		}
		false
	}

	/// A wrapper for find_nearest_point_indicies and returns a VectorControlPoint
	pub fn find_nearest_point<'a>(&'a self, document: &'a Document, mouse_position: DVec2, select_threshold: f64) -> Option<&'a VectorControlPoint> {
		let (shape_layer_path, anchor_id, point_index) = self.find_nearest_point_indicies(document, mouse_position, select_threshold)?;
		let selected_shape = self.shape(document, shape_layer_path).unwrap();
		if let Some(anchor) = selected_shape.anchors().by_id(anchor_id) {
			return anchor.points[point_index].as_ref();
		}
		None
	}

	/// Set the shapes we consider for selection, we will choose draggable handles / anchors from these shapes.
	pub fn set_target_layers(&mut self, target_layers: Vec<Vec<LayerId>>) {
		self.target_layers = target_layers;
	}

	pub fn target_layers(&self) -> &Vec<Vec<LayerId>> {
		&self.target_layers
	}

	pub fn target_layers_ref(&self) -> Vec<&[LayerId]> {
		self.target_layers.iter().map(|l| l.as_slice()).collect::<Vec<_>>()
	}

	/// Clear all of the shapes we can modify
	pub fn clear_target_layers(&mut self) {
		self.target_layers.clear();
	}

	pub fn has_target_layers(&self) -> bool {
		!self.target_layers.is_empty()
	}

	/// Provide the currently selected anchor by reference
	pub fn selected_anchors<'a>(&'a self, document: &'a Document) -> impl Iterator<Item = &'a VectorAnchor> {
		self.iter(document).flat_map(|shape| shape.selected_anchors())
	}

	/// A mutable iterator of all the anchors, regardless of selection
	pub fn anchors<'a>(&'a self, document: &'a Document) -> impl Iterator<Item = &'a VectorAnchor> {
		self.iter(document).flat_map(|shape| shape.iter())
	}

	/// Select the last anchor in this shape
	pub fn select_last_anchor<'a>(&'a self, document: &'a Document, layer_id: &[LayerId], responses: VecDeque<Message>) {
		// if let Some(last) = self.shape(document, layer_id) {
		// 	return last.select_last_anchor();
		// }
	}

	/// Select the Nth anchor of the shape, negative numbers index from the end
	pub fn select_nth_anchor<'a>(&'a self, document: &'a Document, layer_id: &'a [LayerId], anchor_index: i32, responses: VecDeque<Message>) {
		if let Some(shape) = self.shape(document, layer_id) {
			if anchor_index < 0 {
				let anchor_index = shape.anchors().len() - ((-anchor_index) as usize);
			// Send message instead
			//return shape.select_anchor_by_index(anchor_index);
			} else {
				let anchor_index = anchor_index as usize;
				// Send message instead
				//return shape.select_anchor_by_index(anchor_index);
			}
		}
	}

	/// Provide the currently selected points by reference
	pub fn selected_points<'a>(&'a self, document: &'a Document) -> impl Iterator<Item = &'a VectorControlPoint> {
		self.selected_anchors(document).flat_map(|anchors| anchors.selected_points())
	}

	/// Move the selected points by dragging the moue
	pub fn move_selected_points<'a>(&'a self, document: &'a Document, target: DVec2, relative: bool, responses: &VecDeque<Message>) {
		for shape in self.iter(document) {
			// shape.move_selected(target, relative);
		}
		// We've made our changes to the shape, submit them
		// TODO Send changes to the renderer
	}

	/// Dissolve the selected points
	pub fn delete_selected_points(&self, document: &Document, responses: &VecDeque<Message>) {
		for shape in self.iter(document) {
			// shape.delete_selected();
		}
	}

	/// Toggle if the handles should mirror angle across the anchor positon
	pub fn toggle_selected_mirror_angle(&self, document: &Document, responses: &VecDeque<Message>) {
		// for anchor in self.selected_anchors(document) {
		// 	anchor.mirror_angle_active = !anchor.mirror_angle_active;
		// }
		// TODO Send a message instead
	}

	pub fn set_selected_mirror_options(&self, document: &Document, mirror_angle: bool, mirror_distance: bool, responses: &VecDeque<Message>) {
		// for anchor in self.selected_anchors(document) {
		// 	anchor.mirror_angle_active = mirror_angle;
		// 	anchor.mirror_distance_active = mirror_distance;
		// }
		// TODO Send a message instead
	}

	/// Toggle if the handles should mirror distance across the anchor position
	pub fn toggle_selected_mirror_distance(&self, document: &Document, responses: &VecDeque<Message>) {
		// for anchor in self.selected_anchors(document) {
		// 	anchor.mirror_distance_active = !anchor.mirror_distance_active;
		// }
		// TODO Send a message instead
	}

	/// Deselect all anchors from the shapes the manipulation handler has created
	pub fn deselect_all_points(&self, document: &Document, responses: &VecDeque<Message>) {
		for shape in self.iter(document) {
			// shape.clear_selected_anchors();
			// TODO Send a message instead
		}
	}

	/// Iterate over the shapes
	pub fn iter<'a>(&'a self, document: &'a Document) -> impl Iterator<Item = &'a VectorShape> + 'a {
		self.target_layers.iter().map(|layer_id| document.layer(layer_id)).flatten().filter_map(|shape| shape.as_vector_shape())
	}

	/// Find a point that is within the selection threshold and return an index to the shape, anchor, and point
	fn find_nearest_point_indicies(&self, document: &Document, mouse_position: DVec2, select_threshold: f64) -> Option<(&[LayerId], u64, usize)> {
		if self.target_layers.is_empty() {
			return None;
		}

		let select_threshold_squared = select_threshold * select_threshold;
		// Find the closest control point among all elements of shapes_to_modify
		for layer in self.target_layers.iter() {
			if let Some((anchor_id, point_index, distance_squared)) = self.closest_point_in_layer(document, layer, mouse_position) {
				// Choose the first point under the threshold
				if distance_squared < select_threshold_squared {
					log::trace!("Selecting: anchor {} / point {}", anchor_id, point_index);
					return Some((layer, anchor_id, point_index));
				}
			}
		}
		None
	}

	// TODO Use quadtree or some equivalent spatial acceleration structure to improve this to O(log(n))
	/// Find the closest point, anchor and distance so we can select path elements
	/// Brute force comparison to determine which handle / anchor we want to select, O(n)
	fn closest_point_in_layer(&self, document: &Document, layer_path: &[LayerId], pos: glam::DVec2) -> Option<(u64, usize, f64)> {
		let mut closest_distance_squared: f64 = f64::MAX; // Not ideal
		let mut result: Option<(u64, usize, f64)> = None;

		if let Some(shape) = document.layer(layer_path).ok()?.as_vector_shape() {
			for (anchor_id, anchor) in shape.anchors().enumerate() {
				let point_index = anchor.closest_point(pos);
				if let Some(point) = &anchor.points[point_index] {
					if point.can_be_selected {
						let distance_squared = point.position.distance_squared(pos);
						if distance_squared < closest_distance_squared {
							closest_distance_squared = distance_squared;
							result = Some((*anchor_id, point_index, distance_squared));
						}
					}
				}
			}
		}
		result
	}

	// Accessor proxies for VectorShapes contained within the document + layers
	fn shapes<'a>(&'a self, document: &'a Document) -> Vec<&'a VectorShape> {
		self.target_layers.iter().flat_map(|layer_id| document.vector_shape_ref(layer_id)).collect()
	}

	fn shape<'a>(&'a self, document: &'a Document, layer_id: &[u64]) -> Option<&'a VectorShape> {
		document.layer(layer_id).ok()?.as_vector_shape()
	}
}
