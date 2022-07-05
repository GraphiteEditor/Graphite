// Overview:
//          ShapeEditor
//         /          \
//      selected_shape_layers <- Paths to selected layers that may contain VectorShapes
//        |               |
//  VectorShape ... VectorShape  <- Reference from layer paths, one Vectorshape per layer
//      /                 \
//  VectorAnchor ...  VectorAnchor <- VectorShape contains many VectorAnchors

use super::vector_anchor::VectorAnchor;
use super::vector_control_point::VectorControlPoint;
use super::vector_shape::VectorShape;
use crate::message_prelude::{DocumentMessage, Message};

use graphene::layers::vector::constants::ControlPointType;
use graphene::{LayerId, Operation};

use glam::DVec2;
use graphene::document::Document;
use std::collections::VecDeque;

/// ShapeEditor is the container for all of the layer paths that are
/// represented as VectorShapes and provides functionality required
/// to query and create the VectorShapes / VectorAnchors / VectorControlPoints
#[derive(Clone, Debug, Default)]
pub struct ShapeEditor {
	// The layers we can select and edit anchors / handles from
	selected_layers: Vec<Vec<LayerId>>,
}

// TODO Consider keeping a list of selected anchors to minimize traversals of the layers
impl ShapeEditor {
	/// Select the first point within the selection threshold
	/// Returns the points if found, none otherwise
	pub fn select_point(
		&self,
		document: &Document,
		mouse_position: DVec2,
		select_threshold: f64,
		add_to_selection: bool,
		responses: &mut VecDeque<Message>,
	) -> Option<Vec<(&[LayerId], u64, ControlPointType)>> {
		if self.selected_layers.is_empty() {
			return None;
		}

		if let Some((shape_layer_path, anchor_id, point_index)) = self.find_nearest_point_indicies(document, mouse_position, select_threshold) {
			log::trace!("Selecting: anchor {} / point {}", anchor_id, point_index);

			// If the point we're selecting has already been selected
			// we can assume this point exists.. since we did just click on it hense the unwrap
			let is_point_selected = self.shape(document, shape_layer_path).unwrap().anchors().by_id(anchor_id).unwrap().points[point_index]
				.as_ref()
				.unwrap()
				.editor_state
				.is_selected;

			let point_position = self.shape(document, shape_layer_path).unwrap().anchors().by_id(anchor_id).unwrap().points[point_index]
				.as_ref()
				.unwrap()
				.position;

			// The currently selected points (which are then modified to reflect the selection)
			let mut points = self
				.selected_layers()
				.iter()
				.filter_map(|path| document.layer(path).ok().map(|layer| (path, layer)))
				.filter_map(|(path, shape)| shape.as_vector_shape().map(|vector| (path, vector)))
				.flat_map(|(path, shape)| {
					shape
						.anchors()
						.enumerate()
						.filter(|(_id, anchor)| anchor.is_anchor_selected())
						.flat_map(|(id, anchor)| anchor.selected_points().map(move |point| (id, point.manipulator_type)))
						.map(|(anchor, control_point)| (path.as_slice(), *anchor, control_point))
				})
				.collect::<Vec<_>>();

			// let selected_shape = self.shape(document, shape_layer_path).unwrap();

			// Should we select or deselect the point?
			let should_select = if is_point_selected { !add_to_selection } else { true };

			// This is selecting the anchor only for now, next to generalize to points
			if should_select {
				let add = add_to_selection || is_point_selected;
				let point = (anchor_id, ControlPointType::from_index(point_index));
				// Clear all point in other selected shapes
				if !(add) {
					responses.push_back(DocumentMessage::DeselectAllVectorPoints.into());
					points = vec![(shape_layer_path, point.0, point.1)];
				} else {
					points.push((shape_layer_path, point.0, point.1));
				}
				responses.push_back(
					Operation::SelectVectorPoints {
						layer_path: shape_layer_path.to_vec(),
						point_ids: vec![point],
						add,
					}
					.into(),
				);
				// Snap the selected point to the cursor
				if let Ok(viewspace) = document.generate_transform_relative_to_viewport(shape_layer_path) {
					self.move_selected_points(mouse_position - viewspace.transform_point2(point_position), mouse_position, responses)
				}
			} else {
				responses.push_back(
					Operation::DeselectVectorPoints {
						layer_path: shape_layer_path.to_vec(),
						point_ids: vec![(anchor_id, ControlPointType::from_index(point_index))],
					}
					.into(),
				);
				points.retain(|x| *x != (shape_layer_path, anchor_id, ControlPointType::from_index(point_index)))
			}

			return Some(points);
		}

		// Deselect all points if no nearby point
		responses.push_back(DocumentMessage::DeselectAllVectorPoints.into());
		None
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
	pub fn set_selected_layers(&mut self, target_layers: Vec<Vec<LayerId>>) {
		self.selected_layers = target_layers;
	}

	pub fn selected_layers(&self) -> &Vec<Vec<LayerId>> {
		&self.selected_layers
	}

	pub fn selected_layers_ref(&self) -> Vec<&[LayerId]> {
		self.selected_layers.iter().map(|l| l.as_slice()).collect::<Vec<_>>()
	}

	/// Clear all of the shapes we can modify
	pub fn clear_selected_layers(&mut self) {
		self.selected_layers.clear();
	}

	pub fn has_selected_layers(&self) -> bool {
		!self.selected_layers.is_empty()
	}

	/// Provide the currently selected anchor by reference
	pub fn selected_anchors<'a>(&'a self, document: &'a Document) -> impl Iterator<Item = &'a VectorAnchor> {
		self.iter(document).flat_map(|shape| shape.selected_anchors())
	}

	/// A mutable iterator of all the anchors, regardless of selection
	pub fn anchors<'a>(&'a self, document: &'a Document) -> impl Iterator<Item = &'a VectorAnchor> {
		self.iter(document).flat_map(|shape| shape.anchors().iter())
	}

	/// Provide the currently selected points by reference
	pub fn selected_points<'a>(&'a self, document: &'a Document) -> impl Iterator<Item = &'a VectorControlPoint> {
		self.selected_anchors(document).flat_map(|anchors| anchors.selected_points())
	}

	/// Move the selected points by dragging the moue
	pub fn move_selected_points(&self, delta: DVec2, absolute_position: DVec2, responses: &mut VecDeque<Message>) {
		for layer_path in &self.selected_layers {
			responses.push_back(
				DocumentMessage::MoveSelectedVectorPoints {
					layer_path: layer_path.clone(),
					delta: (delta.x, delta.y),
					absolute_position: (absolute_position.x, absolute_position.y),
				}
				.into(),
			);
		}
	}

	/// Dissolve the selected points
	pub fn delete_selected_points(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(DocumentMessage::DeleteSelectedVectorPoints.into());
	}

	/// Toggle if the handles should mirror angle across the anchor positon
	pub fn toggle_handle_mirroring_on_selected(&self, toggle_angle: bool, toggle_distance: bool, responses: &mut VecDeque<Message>) {
		for layer_path in &self.selected_layers {
			responses.push_back(
				DocumentMessage::ToggleSelectedHandleMirroring {
					layer_path: layer_path.clone(),
					toggle_angle,
					toggle_distance,
				}
				.into(),
			);
		}
	}

	/// Deselect all anchors from the shapes the manipulation handler has created
	pub fn deselect_all_points(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(DocumentMessage::DeselectAllVectorPoints.into());
	}

	/// Iterate over the shapes
	pub fn iter<'a>(&'a self, document: &'a Document) -> impl Iterator<Item = &'a VectorShape> + 'a {
		self.selected_layers.iter().flat_map(|layer_id| document.layer(layer_id)).filter_map(|shape| shape.as_vector_shape())
	}

	/// Find a point that is within the selection threshold and return an index to the shape, anchor, and point
	fn find_nearest_point_indicies(&self, document: &Document, mouse_position: DVec2, select_threshold: f64) -> Option<(&[LayerId], u64, usize)> {
		if self.selected_layers.is_empty() {
			return None;
		}

		let select_threshold_squared = select_threshold * select_threshold;
		// Find the closest control point among all elements of shapes_to_modify
		for layer in self.selected_layers.iter() {
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
			let viewspace = document.generate_transform_relative_to_viewport(layer_path).ok()?;
			for (anchor_id, anchor) in shape.anchors().enumerate() {
				let point_index = anchor.closest_point(&viewspace, pos);
				if let Some(point) = &anchor.points[point_index] {
					if point.editor_state.can_be_selected {
						let distance_squared = viewspace.transform_point2(point.position).distance_squared(pos);
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

	fn shape<'a>(&'a self, document: &'a Document, layer_id: &[u64]) -> Option<&'a VectorShape> {
		document.layer(layer_id).ok()?.as_vector_shape()
	}
}
