use glam::{DAffine2, DVec2};
use graphene::{
	color::Color,
	layers::{
		layer_info::LayerDataType,
		style::{self, Fill, Stroke},
	},
	LayerId, Operation,
};
use kurbo::{BezPath, PathEl};
use std::collections::HashSet;
use std::collections::VecDeque;

use crate::{
	consts::COLOR_ACCENT,
	document::DocumentMessageHandler,
	message_prelude::{generate_uuid, DocumentMessage, Message},
};

use super::{constants::ControlPointType, vector_anchor::VectorAnchor, vector_control_point::VectorControlPoint};

/// VectorShape represents a single kurbo shape and maintains a parallel data structure
/// For each kurbo path we keep a VectorShape which contains the handles and anchors for that path
#[derive(PartialEq, Clone, Debug, Default)]
pub struct VectorShape {
	/// The path to the shape layer
	pub layer_path: Vec<LayerId>,
	/// The outline of the shape via kurbo
	pub bez_path: kurbo::BezPath,
	/// The elements of the kurbo shape
	pub elements: Vec<kurbo::PathEl>,
	/// The anchors that are made up of the control points / handles
	pub anchors: Vec<VectorAnchor>,
	/// The overlays for the shape, anchors and manipulator handles
	pub shape_overlay: Option<Vec<LayerId>>,
	/// If the compound Bezier curve is closed
	pub closed: bool,
	/// The transformation matrix to apply
	pub transform: DAffine2,
	// Indices for the most recent select point anchors
	pub selected_anchor_indices: HashSet<usize>,
}
type IndexedEl = (usize, kurbo::PathEl);

impl VectorShape {
	pub fn new(layer_path: Vec<LayerId>, transform: DAffine2, bez_path: &BezPath, closed: bool, responses: &mut VecDeque<Message>) -> Self {
		let mut shape = VectorShape {
			layer_path,
			bez_path: bez_path.clone(),
			closed,
			transform,
			elements: bez_path.into_iter().collect(),
			..Default::default()
		};
		shape.shape_overlay = Some(shape.create_shape_outline_overlay(responses));
		shape.anchors = shape.create_anchors_from_kurbo(responses);

		// TODO: This is a hack to allow Text to work. The shape isn't a path until this message is sent (it appears)
		responses.push_back(
			Operation::SetShapePathInViewport {
				path: shape.layer_path.clone(),
				bez_path: shape.elements.clone().into_iter().collect(),
				transform: shape.transform.to_cols_array(),
			}
			.into(),
		);

		shape
	}

	/// Select an anchor
	pub fn select_anchor(&mut self, anchor_index: usize) -> &mut VectorAnchor {
		self.selected_anchor_indices.insert(anchor_index);
		&mut self.anchors[anchor_index]
	}

	/// Deselect an anchor
	pub fn deselect_anchor(&mut self, anchor_index: usize, responses: &mut VecDeque<Message>) {
		self.anchors[anchor_index].clear_selected_points(responses);
		self.selected_anchor_indices.remove(&anchor_index);
	}

	/// Select all the anchors in this shape
	pub fn select_all_anchors(&mut self, responses: &mut VecDeque<Message>) {
		for (index, anchor) in self.anchors.iter_mut().enumerate() {
			self.selected_anchor_indices.insert(index);
			anchor.select_point(0, true, responses);
		}
	}

	/// Clear all the selected anchors, and clear the selected points on the anchors
	pub fn clear_selected_anchors(&mut self, responses: &mut VecDeque<Message>) {
		for anchor_index in self.selected_anchor_indices.iter() {
			self.anchors[*anchor_index].clear_selected_points(responses);
		}
		self.selected_anchor_indices.clear();
	}

	/// Return all the selected anchors by reference
	pub fn selected_anchors(&self) -> impl Iterator<Item = &VectorAnchor> {
		self.anchors
			.iter()
			.enumerate()
			.filter_map(|(index, anchor)| if self.selected_anchor_indices.contains(&index) { Some(anchor) } else { None })
	}

	/// Return all the selected anchors, mutable
	pub fn selected_anchors_mut(&mut self) -> impl Iterator<Item = &mut VectorAnchor> {
		self.anchors
			.iter_mut()
			.enumerate()
			.filter_map(|(index, anchor)| if self.selected_anchor_indices.contains(&index) { Some(anchor) } else { None })
	}

	/// Move the selected point based on mouse input, if this is a handle we can control if we are mirroring or not
	/// A wrapper around move_point to handle mirror state / submit the changes
	pub fn move_selected(&mut self, position_delta: DVec2, should_mirror_handles: bool, responses: &mut VecDeque<Message>) {
		let transform = &self.transform.clone();
		let mut edited_bez_path = self.elements.clone();

		for selected_anchor in self.selected_anchors_mut() {
			// Should we mirror the opposing handle or not?
			if !should_mirror_handles && selected_anchor.mirroring_debounce != should_mirror_handles {
				selected_anchor.handles_are_mirroring = !selected_anchor.handles_are_mirroring;
			}
			selected_anchor.mirroring_debounce = should_mirror_handles;

			selected_anchor.move_selected_points(position_delta, &mut edited_bez_path, transform);
		}

		// We've made our changes to the shape, submit them
		responses.push_back(
			Operation::SetShapePathInViewport {
				path: self.layer_path.clone(),
				bez_path: edited_bez_path.into_iter().collect(),
				transform: self.transform.to_cols_array(),
			}
			.into(),
		);
	}

	/// Update the anchors and segments to match the kurbo shape
	/// Should be called whenever the kurbo shape changes
	pub fn update_shape(&mut self, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		let viewport_transform = document.graphene_document.generate_transform_relative_to_viewport(&self.layer_path).unwrap();
		let layer = document.graphene_document.layer(&self.layer_path).unwrap();
		if let LayerDataType::Shape(shape) = &layer.data {
			let path = shape.path.clone();
			self.transform = viewport_transform;

			// Update point positions
			self.update_anchors_from_kurbo(&path);

			self.bez_path = path;

			// Update the overlays to represent the changes to the kurbo path
			self.place_shape_outline_overlay(responses);
			self.place_anchor_overlays(responses);
			self.place_handle_overlays(responses);
		}
	}

	/// Place points in local space
	fn to_local_space(&self, point: kurbo::Point) -> DVec2 {
		self.transform.transform_point2(DVec2::from((point.x, point.y)))
	}

	/// Create an anchor on the boundary between two kurbo PathElements with optional handles
	fn create_anchor(&self, first: Option<IndexedEl>, second: Option<IndexedEl>, responses: &mut VecDeque<Message>) -> VectorAnchor {
		let mut handle1 = None;
		let mut anchor_position: glam::DVec2 = glam::DVec2::ZERO;
		let mut handle2 = None;
		let mut anchor_element_id: usize = 0;

		let create_point = |id: usize, point: DVec2, overlay_path: Vec<LayerId>, manipulator_type: ControlPointType| -> VectorControlPoint {
			VectorControlPoint {
				kurbo_element_id: id,
				position: point,
				overlay_path: Some(overlay_path),
				can_be_selected: true,
				manipulator_type,
				is_selected: false,
			}
		};

		if let Some((first_element_id, first_element)) = first {
			anchor_element_id = first_element_id;
			match first_element {
				kurbo::PathEl::MoveTo(anchor) | kurbo::PathEl::LineTo(anchor) => anchor_position = self.to_local_space(anchor),
				kurbo::PathEl::QuadTo(handle, anchor) | kurbo::PathEl::CurveTo(_, handle, anchor) => {
					anchor_position = self.to_local_space(anchor);
					handle1 = Some(create_point(
						first_element_id,
						self.to_local_space(handle),
						self.create_handle_overlay(responses),
						ControlPointType::Handle1,
					));
				}
				_ => (),
			}
		}

		if let Some((second_element_id, second_element)) = second {
			match second_element {
				kurbo::PathEl::CurveTo(handle, _, _) | kurbo::PathEl::QuadTo(handle, _) => {
					handle2 = Some(create_point(
						second_element_id,
						self.to_local_space(handle),
						self.create_handle_overlay(responses),
						ControlPointType::Handle2,
					));
				}
				_ => (),
			}
		}

		VectorAnchor {
			handle_line_overlays: (self.create_handle_line_overlay(&handle1, responses), self.create_handle_line_overlay(&handle2, responses)),
			points: [
				Some(create_point(anchor_element_id, anchor_position, self.create_anchor_overlay(responses), ControlPointType::Anchor)),
				handle1,
				handle2,
			],
			close_element_id: None,
			handles_are_mirroring: true,
			mirroring_debounce: false,
		}
	}

	/// Close the path by checking if the distance between the last element and the first MoveTo is less than the tolerance.
	/// If so, create a new anchor at the first point. Otherwise, create a new anchor at the last point.
	fn close_path(
		&self,
		points: &mut Vec<VectorAnchor>,
		to_replace: usize,
		first_path_element: Option<IndexedEl>,
		last_path_element: Option<IndexedEl>,
		recent_move_to: Option<IndexedEl>,
		responses: &mut VecDeque<Message>,
	) {
		if let (Some(first), Some(last), Some(move_to)) = (first_path_element, last_path_element, recent_move_to) {
			let position_equal = match (move_to.1, last.1) {
				(PathEl::MoveTo(p1), PathEl::LineTo(p2)) => p1.distance_squared(p2) < 0.01,
				(PathEl::MoveTo(p1), PathEl::QuadTo(_, p2)) => p1.distance_squared(p2) < 0.01,
				(PathEl::MoveTo(p1), PathEl::CurveTo(_, _, p2)) => p1.distance_squared(p2) < 0.01,
				_ => false,
			};

			// Does this end in the same position it started?
			if position_equal {
				points[to_replace].remove_overlays(responses);
				points[to_replace] = self.create_anchor(Some(last), Some(first), responses);
				points[to_replace].close_element_id = Some(move_to.0);
			} else {
				points.push(self.create_anchor(Some(last), Some(first), responses));
			}
		}
	}

	/// Create the anchors from the kurbo path, only done during of new anchors construction
	fn create_anchors_from_kurbo(&self, responses: &mut VecDeque<Message>) -> Vec<VectorAnchor> {
		// We need the indices paired with the kurbo path elements
		let indexed_elements = self.bez_path.elements().iter().enumerate().map(|(index, element)| (index, *element)).collect::<Vec<IndexedEl>>();

		// Create the manipulation points
		let mut anchors: Vec<VectorAnchor> = vec![];
		let (mut first_path_element, mut last_path_element): (Option<IndexedEl>, Option<IndexedEl>) = (None, None);
		let mut last_move_to_element: Option<IndexedEl> = None;
		let mut ended_with_close_path = false;
		let mut first_move_to_id: usize = 0;

		// TODO Consider using a LL(1) grammar to improve readability
		// Create an anchor at each join between two kurbo segments
		for elements in indexed_elements.windows(2) {
			let (_, current_element) = elements[0];
			let (_, next_element) = elements[1];
			ended_with_close_path = false;

			if matches!(current_element, kurbo::PathEl::ClosePath) {
				continue;
			}

			// An anchor cannot stradle a line / curve segment and a ClosePath segment
			if matches!(next_element, kurbo::PathEl::ClosePath) {
				ended_with_close_path = true;
				if self.closed {
					self.close_path(&mut anchors, first_move_to_id, first_path_element, last_path_element, last_move_to_element, responses);
				} else {
					anchors.push(self.create_anchor(last_path_element, None, responses));
				}
				continue;
			}

			// Keep track of the first and last elements of this shape
			if matches!(current_element, kurbo::PathEl::MoveTo(_)) {
				last_move_to_element = Some(elements[0]);
				first_path_element = Some(elements[1]);
				first_move_to_id = anchors.len();
			}
			last_path_element = Some(elements[1]);

			anchors.push(self.create_anchor(Some(elements[0]), Some(elements[1]), responses));
		}

		// If the path definition didn't include a ClosePath, we still need to behave as though it did
		if !ended_with_close_path {
			if self.closed {
				self.close_path(&mut anchors, first_move_to_id, first_path_element, last_path_element, last_move_to_element, responses);
			} else {
				anchors.push(self.create_anchor(last_path_element, None, responses));
			}
		}

		anchors
	}

	/// Update the anchors to match the kurbo path
	fn update_anchors_from_kurbo(&mut self, path: &BezPath) {
		let space_transform = |point: kurbo::Point| self.transform.transform_point2(DVec2::from((point.x, point.y)));
		for anchor_index in 0..self.anchors.len() {
			let elements = path.elements();
			let anchor = &mut self.anchors[anchor_index];
			if let Some(anchor_point) = &mut anchor.points[ControlPointType::Anchor] {
				match elements[anchor_point.kurbo_element_id] {
					kurbo::PathEl::MoveTo(anchor_position) | kurbo::PathEl::LineTo(anchor_position) => anchor.set_point_position(ControlPointType::Anchor as usize, space_transform(anchor_position)),
					kurbo::PathEl::QuadTo(handle_position, anchor_position) | kurbo::PathEl::CurveTo(_, handle_position, anchor_position) => {
						anchor.set_point_position(ControlPointType::Anchor as usize, space_transform(anchor_position));
						if anchor.points[ControlPointType::Handle1].is_some() {
							anchor.set_point_position(ControlPointType::Handle1 as usize, space_transform(handle_position));
						}
					}
					_ => (),
				}
				if let Some(handle) = &mut anchor.points[ControlPointType::Handle2] {
					match elements[handle.kurbo_element_id] {
						kurbo::PathEl::CurveTo(handle_position, _, _) | kurbo::PathEl::QuadTo(handle_position, _) => {
							anchor.set_point_position(ControlPointType::Handle2 as usize, space_transform(handle_position));
						}
						_ => (),
					}
				}
			}
		}
	}

	/// Create the kurbo shape that matches the selected viewport shape
	fn create_shape_outline_overlay(&self, responses: &mut VecDeque<Message>) -> Vec<LayerId> {
		let layer_path = vec![generate_uuid()];
		let operation = Operation::AddOverlayShape {
			path: layer_path.clone(),
			bez_path: self.bez_path.clone(),
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), None),
			closed: false,
		};
		responses.push_back(DocumentMessage::Overlays(operation.into()).into());

		layer_path
	}

	/// Create a single anchor overlay and return its layer id
	fn create_anchor_overlay(&self, responses: &mut VecDeque<Message>) -> Vec<LayerId> {
		let layer_path = vec![generate_uuid()];
		let operation = Operation::AddOverlayRect {
			path: layer_path.clone(),
			transform: DAffine2::IDENTITY.to_cols_array(),
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 2.0)), Some(Fill::new(Color::WHITE))),
		};
		responses.push_back(DocumentMessage::Overlays(operation.into()).into());
		layer_path
	}

	/// Create a single handle overlay and return its layer id
	fn create_handle_overlay(&self, responses: &mut VecDeque<Message>) -> Vec<LayerId> {
		let layer_path = vec![generate_uuid()];
		let operation = Operation::AddOverlayEllipse {
			path: layer_path.clone(),
			transform: DAffine2::IDENTITY.to_cols_array(),
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 2.0)), Some(Fill::new(Color::WHITE))),
		};
		responses.push_back(DocumentMessage::Overlays(operation.into()).into());
		layer_path
	}

	/// Create the shape outline overlay and return its layer id
	fn create_handle_line_overlay(&self, handle: &Option<VectorControlPoint>, responses: &mut VecDeque<Message>) -> Option<Vec<LayerId>> {
		if handle.is_none() {
			return None;
		}

		let layer_path = vec![generate_uuid()];
		let operation = Operation::AddOverlayLine {
			path: layer_path.clone(),
			transform: DAffine2::IDENTITY.to_cols_array(),
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), None),
		};
		responses.push_front(DocumentMessage::Overlays(operation.into()).into());

		Some(layer_path)
	}

	/// Update the positions of the anchor points based on the kurbo path
	fn place_shape_outline_overlay(&self, responses: &mut VecDeque<Message>) {
		if let Some(overlay_path) = &self.shape_overlay {
			responses.push_back(
				DocumentMessage::Overlays(
					Operation::SetShapePathInViewport {
						path: overlay_path.clone(),
						bez_path: self.bez_path.clone(),
						transform: self.transform.to_cols_array(),
					}
					.into(),
				)
				.into(),
			);
		}
	}

	/// Update the positions of the anchor points based on the kurbo path
	fn place_anchor_overlays(&self, responses: &mut VecDeque<Message>) {
		for anchor in &self.anchors {
			anchor.place_anchor_overlay(responses);
		}
	}

	/// Update the positions of the handle points and lines based on the kurbo path
	fn place_handle_overlays(&self, responses: &mut VecDeque<Message>) {
		for anchor in &self.anchors {
			anchor.place_handle_overlay(responses);
		}
	}

	/// Remove all of the overlays from the shape
	pub fn remove_overlays(&mut self, responses: &mut VecDeque<Message>) {
		self.remove_shape_outline_overlay(responses);
		self.remove_anchor_overlays(responses);
		self.remove_handle_overlays(responses);
	}

	/// Remove the outline around the shape
	pub fn remove_shape_outline_overlay(&mut self, responses: &mut VecDeque<Message>) {
		if let Some(overlay_path) = &self.shape_overlay {
			responses.push_back(DocumentMessage::Overlays(Operation::DeleteLayer { path: overlay_path.clone() }.into()).into());
		}
		self.shape_overlay = None;
	}

	/// Remove the all the anchor overlays
	pub fn remove_anchor_overlays(&mut self, responses: &mut VecDeque<Message>) {
		for anchor in &mut self.anchors {
			anchor.remove_anchor_overlay(responses);
		}
	}

	/// Remove the all the anchor overlays
	pub fn remove_handle_overlays(&mut self, responses: &mut VecDeque<Message>) {
		for anchor in &mut self.anchors {
			anchor.remove_handle_overlay(responses);
		}
	}

	/// Eventually we will want to hide the overlays instead of clearing them when selecting a new shape
	pub fn set_overlay_visibility(&mut self, visibility: bool, responses: &mut VecDeque<Message>) {
		self.set_shape_outline_visiblity(visibility, responses);
		self.set_anchors_visiblity(visibility, responses);
		self.set_handles_visiblity(visibility, responses);
	}

	/// Set the visibility of the shape outline
	pub fn set_shape_outline_visiblity(&self, visibility: bool, responses: &mut VecDeque<Message>) {
		if let Some(overlay_path) = &self.shape_overlay {
			responses.push_back(
				DocumentMessage::Overlays(
					Operation::SetLayerVisibility {
						path: overlay_path.clone(),
						visible: visibility,
					}
					.into(),
				)
				.into(),
			);
		}
	}

	/// Set visibility on all of the anchors in this shape
	pub fn set_anchors_visiblity(&self, visibility: bool, responses: &mut VecDeque<Message>) {
		for anchor in &self.anchors {
			anchor.set_anchor_visiblity(visibility, responses);
		}
	}

	/// Set visibility on all of the handles in this shape
	pub fn set_handles_visiblity(&self, visibility: bool, responses: &mut VecDeque<Message>) {
		for anchor in &self.anchors {
			anchor.set_handle_visiblity(visibility, responses);
		}
	}
}
