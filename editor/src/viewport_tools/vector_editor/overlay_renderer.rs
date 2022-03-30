use std::collections::{HashMap, VecDeque};

use glam::DAffine2;
use kurbo::BezPath;

use crate::{
	consts::COLOR_ACCENT,
	message_prelude::{generate_uuid, DocumentMessage, Message},
};

use super::{constants::ControlPointType, vector_anchor::VectorAnchor, vector_control_point::VectorControlPoint, vector_shape::VectorShape};
use graphene::{
	color::Color,
	layers::style::{self, Fill, Stroke},
	LayerId, Operation,
};

struct OverlayRenderer {
	overlays: HashMap<VectorShape, Vec<Vec<LayerId>>>,
}

/// AnchorOverlay is the collection of overlays that make up an anchor
/// Notably the anchor point, the lines to the handles and the handles
type AnchorOverlays = Vec<[Option<Vec<LayerId>>; 5]>;

impl OverlayRenderer {
	pub fn new() -> Self {
		OverlayRenderer { overlays: HashMap::new() }
	}

	pub fn draw_overlays_for_shape(&mut self, shape: &VectorShape, responses: &mut VecDeque<Message>) {
		let outline = self.create_shape_outline_overlay(shape.to_bezpath(), responses);
		let anchors: AnchorOverlays = shape
			.anchors
			.iter()
			.map(|anchor| {
				[
					Some(self.create_anchor_overlay(anchor, responses)),
					self.create_handle_overlay(&anchor.points[ControlPointType::Handle1], responses),
					self.create_handle_overlay(&anchor.points[ControlPointType::Handle2], responses),
					self.create_handle_line_overlay(&anchor.points[ControlPointType::Handle1], responses),
					self.create_handle_line_overlay(&anchor.points[ControlPointType::Handle2], responses),
				]
			})
			.collect::<_>();
	}

	pub fn hide_overlays_for_shape(&mut self, shape: &VectorShape, responses: &mut VecDeque<Message>) {
		// Delete here
	}

	/// Create the kurbo shape that matches the selected viewport shape
	fn create_shape_outline_overlay(&self, bez_path: BezPath, responses: &mut VecDeque<Message>) -> Vec<LayerId> {
		let layer_path = vec![generate_uuid()];
		let operation = Operation::AddOverlayShape {
			path: layer_path.clone(),
			bez_path,
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), None),
			closed: false,
		};
		responses.push_back(DocumentMessage::Overlays(operation.into()).into());

		layer_path
	}

	/// Create a single anchor overlay and return its layer id
	fn create_anchor_overlay(&self, anchor: &VectorAnchor, responses: &mut VecDeque<Message>) -> Vec<LayerId> {
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
	fn create_handle_overlay(&self, handle: &Option<VectorControlPoint>, responses: &mut VecDeque<Message>) -> Option<Vec<LayerId>> {
		if handle.is_none() {
			return None;
		}

		let layer_path = vec![generate_uuid()];
		let operation = Operation::AddOverlayEllipse {
			path: layer_path.clone(),
			transform: DAffine2::IDENTITY.to_cols_array(),
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 2.0)), Some(Fill::new(Color::WHITE))),
		};
		responses.push_back(DocumentMessage::Overlays(operation.into()).into());
		Some(layer_path)
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

	/// Place an anchor overlay
	pub fn place_anchor_overlay(&self, responses: &mut VecDeque<Message>) {
		if let Some(anchor_point) = &self.points[ControlPointType::Anchor] {
			if let Some(anchor_overlay) = &anchor_point.overlay_path {
				let scale = DVec2::splat(VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE);
				let angle = 0.;
				let translation = (anchor_point.position - (scale / 2.) + ROUNDING_BIAS).round();
				let transform = DAffine2::from_scale_angle_translation(scale, angle, translation).to_cols_array();
				responses.push_back(
					DocumentMessage::Overlays(
						Operation::SetLayerTransformInViewport {
							path: anchor_overlay.clone(),
							transform,
						}
						.into(),
					)
					.into(),
				);
			}
		}
	}

	/// Updates the position of the handle's overlays based on the kurbo path
	pub fn place_handle_overlay(&self, responses: &mut VecDeque<Message>) {
		if let Some(anchor_point) = &self.points[ControlPointType::Anchor] {
			// Helper function to keep things DRY
			let mut place_handle_and_line = |handle: &VectorControlPoint, line: &Option<Vec<LayerId>>| {
				if let Some(line_overlay) = line {
					let line_vector = anchor_point.position - handle.position;
					let scale = DVec2::splat(line_vector.length());
					let angle = -line_vector.angle_between(DVec2::X);
					let translation = (handle.position + ROUNDING_BIAS).round() + DVec2::splat(0.5);
					let transform = DAffine2::from_scale_angle_translation(scale, angle, translation).to_cols_array();
					responses.push_back(
						DocumentMessage::Overlays(
							Operation::SetLayerTransformInViewport {
								path: line_overlay.clone(),
								transform,
							}
							.into(),
						)
						.into(),
					);
				}

				if let Some(line_overlay) = &handle.overlay_path {
					let scale = DVec2::splat(VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE);
					let angle = 0.;
					let translation = (handle.position - (scale / 2.) + ROUNDING_BIAS).round();
					let transform = DAffine2::from_scale_angle_translation(scale, angle, translation).to_cols_array();
					responses.push_back(
						DocumentMessage::Overlays(
							Operation::SetLayerTransformInViewport {
								path: line_overlay.clone(),
								transform,
							}
							.into(),
						)
						.into(),
					);
				}
			};

			let [_, h1, h2] = &self.points;
			let (line1, line2) = &self.handle_line_overlays;

			if let Some(handle) = &h1 {
				place_handle_and_line(handle, line1);
			}

			if let Some(handle) = &h2 {
				place_handle_and_line(handle, line2);
			}
		}
	}

	/// Removes the anchor overlay from the overlay document
	pub fn remove_anchor_overlay(&mut self, responses: &mut VecDeque<Message>) {
		if let Some(anchor_point) = &mut self.points[ControlPointType::Anchor] {
			if let Some(overlay_path) = &anchor_point.overlay_path {
				responses.push_front(DocumentMessage::Overlays(Operation::DeleteLayer { path: overlay_path.clone() }.into()).into());
			}
			anchor_point.overlay_path = None;
		}
	}

	/// Removes the handles overlay from the overlay document
	pub fn remove_handle_overlay(&mut self, responses: &mut VecDeque<Message>) {
		let [_, h1, h2] = &mut self.points;
		let (line1, line2) = &mut self.handle_line_overlays;

		// Helper function to keep things DRY
		let mut delete_message = |handle: &Option<Vec<LayerId>>| {
			if let Some(overlay_path) = handle {
				responses.push_front(DocumentMessage::Overlays(Operation::DeleteLayer { path: overlay_path.clone() }.into()).into());
			}
		};

		// Delete the handles themselves
		if let Some(handle) = h1 {
			delete_message(&handle.overlay_path);
			handle.overlay_path = None;
		}
		if let Some(handle) = h2 {
			delete_message(&handle.overlay_path);
			handle.overlay_path = None;
		}

		// Delete the handle line layers
		delete_message(line1);
		delete_message(line2);
		self.handle_line_overlays = (None, None);
	}

	/// Clear overlays for this anchor, do this prior to deletion
	pub fn remove_overlays(&mut self, responses: &mut VecDeque<Message>) {
		self.remove_anchor_overlay(responses);
		self.remove_handle_overlay(responses);
	}

	/// Sets the visibility of the anchors overlay
	pub fn set_anchor_visiblity(&self, visibility: bool, responses: &mut VecDeque<Message>) {
		if let Some(anchor_point) = &self.points[ControlPointType::Anchor] {
			if let Some(overlay_path) = &anchor_point.overlay_path {
				responses.push_back(self.visibility_message(overlay_path.clone(), visibility));
			}
		}
	}

	/// Sets the visibility of the handles overlay
	pub fn set_handle_visiblity(&self, visibility: bool, responses: &mut VecDeque<Message>) {
		let [_, h1, h2] = &self.points;
		let (line1, line2) = &self.handle_line_overlays;

		if let Some(handle) = h1 {
			if let Some(overlay_path) = &handle.overlay_path {
				responses.push_back(self.visibility_message(overlay_path.clone(), visibility));
			}
		}
		if let Some(handle) = h2 {
			if let Some(overlay_path) = &handle.overlay_path {
				responses.push_back(self.visibility_message(overlay_path.clone(), visibility));
			}
		}

		if let Some(overlay_path) = &line1 {
			responses.push_back(self.visibility_message(overlay_path.clone(), visibility));
		}
		if let Some(overlay_path) = &line2 {
			responses.push_back(self.visibility_message(overlay_path.clone(), visibility));
		}
	}

	/// Create a visibility message for an overlay
	fn visibility_message(&self, layer_path: Vec<LayerId>, visibility: bool) -> Message {
		DocumentMessage::Overlays(
			Operation::SetLayerVisibility {
				path: layer_path,
				visible: visibility,
			}
			.into(),
		)
		.into()
	}

	/// Sets the overlay style for this point
	pub fn set_overlay_style(&self, stroke_width: f32, stroke_color: Color, fill_color: Color, responses: &mut VecDeque<Message>) {
		if let Some(overlay_path) = &self.overlay_path {
			responses.push_back(
				DocumentMessage::Overlays(
					Operation::SetLayerStyle {
						path: overlay_path.clone(),
						style: PathStyle::new(Some(Stroke::new(stroke_color, stroke_width)), Some(Fill::new(fill_color))),
					}
					.into(),
				)
				.into(),
			);
		}
	}

	// 	/// Create an anchor on the boundary between two kurbo PathElements with optional handles
	// // TODO remove anything to do with overlays
	// fn create_anchor(&self, first: Option<IndexedEl>, second: Option<IndexedEl>, responses: &mut VecDeque<Message>) -> VectorAnchor {
	// 	let mut handle1 = None;
	// 	let mut anchor_position: glam::DVec2 = glam::DVec2::ZERO;
	// 	let mut handle2 = None;
	// 	let mut anchor_element_id: usize = 0;

	// 	let create_point = |id: usize, point: DVec2, overlay_path: Vec<LayerId>, manipulator_type: ControlPointType| -> VectorControlPoint {
	// 		VectorControlPoint {
	// 			// kurbo_element_id: id,
	// 			position: point,
	// 			// overlay_path: Some(overlay_path),
	// 			can_be_selected: true,
	// 			manipulator_type,
	// 			is_selected: false,
	// 		}
	// 	};

	// 	if let Some((first_element_id, first_element)) = first {
	// 		anchor_element_id = first_element_id;
	// 		match first_element {
	// 			kurbo::PathEl::MoveTo(anchor) | kurbo::PathEl::LineTo(anchor) => anchor_position = self.to_local_space(anchor),
	// 			kurbo::PathEl::QuadTo(handle, anchor) | kurbo::PathEl::CurveTo(_, handle, anchor) => {
	// 				anchor_position = self.to_local_space(anchor);
	// 				handle1 = Some(create_point(
	// 					first_element_id,
	// 					self.to_local_space(handle),
	// 					self.create_handle_overlay(responses),
	// 					ControlPointType::Handle1,
	// 				));
	// 			}
	// 			_ => (),
	// 		}
	// 	}

	// 	if let Some((second_element_id, second_element)) = second {
	// 		match second_element {
	// 			kurbo::PathEl::CurveTo(handle, _, _) | kurbo::PathEl::QuadTo(handle, _) => {
	// 				handle2 = Some(create_point(
	// 					second_element_id,
	// 					self.to_local_space(handle),
	// 					self.create_handle_overlay(responses),
	// 					ControlPointType::Handle2,
	// 				));
	// 			}
	// 			_ => (),
	// 		}
	// 	}

	// 	VectorAnchor {
	// 		// handle_line_overlays: (self.create_handle_line_overlay(&handle1, responses), self.create_handle_line_overlay(&handle2, responses)),
	// 		points: [
	// 			Some(create_point(anchor_element_id, anchor_position, self.create_anchor_overlay(responses), ControlPointType::Anchor)),
	// 			handle1,
	// 			handle2,
	// 		],
	// 		close_element_id: None,
	// 		handle_mirror_angle: true,
	// 		handle_mirror_distance: false,
	// 	}
	// }

	// /// Close the path by checking if the distance between the last element and the first MoveTo is less than the tolerance.
	// /// If so, create a new anchor at the first point. Otherwise, create a new anchor at the last point.
	// fn close_path(
	// 	&self,
	// 	points: &mut Vec<VectorAnchor>,
	// 	to_replace: usize,
	// 	first_path_element: Option<IndexedEl>,
	// 	last_path_element: Option<IndexedEl>,
	// 	recent_move_to: Option<IndexedEl>,
	// 	responses: &mut VecDeque<Message>,
	// ) {
	// 	if let (Some(first), Some(last), Some(move_to)) = (first_path_element, last_path_element, recent_move_to) {
	// 		let position_equal = match (move_to.1, last.1) {
	// 			(PathEl::MoveTo(p1), PathEl::LineTo(p2)) => p1.distance_squared(p2) < 0.01,
	// 			(PathEl::MoveTo(p1), PathEl::QuadTo(_, p2)) => p1.distance_squared(p2) < 0.01,
	// 			(PathEl::MoveTo(p1), PathEl::CurveTo(_, _, p2)) => p1.distance_squared(p2) < 0.01,
	// 			_ => false,
	// 		};

	// 		// Does this end in the same position it started?
	// 		if position_equal {
	// 			points[to_replace].remove_overlays(responses);
	// 			points[to_replace] = self.create_anchor(Some(last), Some(first), responses);
	// 			points[to_replace].close_element_id = Some(move_to.0);
	// 		} else {
	// 			points.push(self.create_anchor(Some(last), Some(first), responses));
	// 		}
	// 	}
	// }

	// Stuff we probably don't need under here -------------------------------------------------------------
	
	// /// Create the kurbo shape that matches the selected viewport shape
	// fn create_shape_outline_overlay(&self, responses: &mut VecDeque<Message>) -> Vec<LayerId> {
	// 	let layer_path = vec![generate_uuid()];
	// 	let operation = Operation::AddOverlayShape {
	// 		path: layer_path.clone(),
	// 		bez_path: self.bez_path.clone(),
	// 		style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), None),
	// 		closed: false,
	// 	};
	// 	responses.push_back(DocumentMessage::Overlays(operation.into()).into());

	// 	layer_path
	// }

	// /// Create a single anchor overlay and return its layer id
	// fn create_anchor_overlay(&self, responses: &mut VecDeque<Message>) -> Vec<LayerId> {
	// 	let layer_path = vec![generate_uuid()];
	// 	let operation = Operation::AddOverlayRect {
	// 		path: layer_path.clone(),
	// 		transform: DAffine2::IDENTITY.to_cols_array(),
	// 		style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 2.0)), Some(Fill::new(Color::WHITE))),
	// 	};
	// 	responses.push_back(DocumentMessage::Overlays(operation.into()).into());
	// 	layer_path
	// }

	// /// Create a single handle overlay and return its layer id
	// fn create_handle_overlay(&self, responses: &mut VecDeque<Message>) -> Vec<LayerId> {
	// 	let layer_path = vec![generate_uuid()];
	// 	let operation = Operation::AddOverlayEllipse {
	// 		path: layer_path.clone(),
	// 		transform: DAffine2::IDENTITY.to_cols_array(),
	// 		style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 2.0)), Some(Fill::new(Color::WHITE))),
	// 	};
	// 	responses.push_back(DocumentMessage::Overlays(operation.into()).into());
	// 	layer_path
	// }

	// /// Create the shape outline overlay and return its layer id
	// fn create_handle_line_overlay(&self, handle: &Option<VectorControlPoint>, responses: &mut VecDeque<Message>) -> Option<Vec<LayerId>> {
	// 	if handle.is_none() {
	// 		return None;
	// 	}

	// 	let layer_path = vec![generate_uuid()];
	// 	let operation = Operation::AddOverlayLine {
	// 		path: layer_path.clone(),
	// 		transform: DAffine2::IDENTITY.to_cols_array(),
	// 		style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), None),
	// 	};
	// 	responses.push_front(DocumentMessage::Overlays(operation.into()).into());

	// 	Some(layer_path)
	// }

	// /// Update the positions of the anchor points based on the kurbo path
	// fn place_shape_outline_overlay(&self, responses: &mut VecDeque<Message>) {
	// 	if let Some(overlay_path) = &self.shape_overlay {
	// 		responses.push_back(
	// 			DocumentMessage::Overlays(
	// 				Operation::SetShapePathInViewport {
	// 					path: overlay_path.clone(),
	// 					bez_path: self.bez_path.clone(),
	// 					transform: self.transform.to_cols_array(),
	// 				}
	// 				.into(),
	// 			)
	// 			.into(),
	// 		);
	// 	}
	// }

	// /// Update the positions of the anchor points based on the kurbo path
	// fn place_anchor_overlays(&self, responses: &mut VecDeque<Message>) {
	// 	for anchor in &self.anchors {
	// 		anchor.place_anchor_overlay(responses);
	// 	}
	// }

	// /// Update the positions of the handle points and lines based on the kurbo path
	// fn place_handle_overlays(&self, responses: &mut VecDeque<Message>) {
	// 	for anchor in &self.anchors {
	// 		anchor.place_handle_overlay(responses);
	// 	}
	// }

	// /// Remove all of the overlays from the shape
	// pub fn remove_overlays(&mut self, responses: &mut VecDeque<Message>) {
	// 	self.remove_shape_outline_overlay(responses);
	// 	self.remove_anchor_overlays(responses);
	// 	self.remove_handle_overlays(responses);
	// }

	// /// Remove the outline around the shape
	// pub fn remove_shape_outline_overlay(&mut self, responses: &mut VecDeque<Message>) {
	// 	if let Some(overlay_path) = &self.shape_overlay {
	// 		responses.push_back(DocumentMessage::Overlays(Operation::DeleteLayer { path: overlay_path.clone() }.into()).into());
	// 	}
	// 	self.shape_overlay = None;
	// }

	// /// Remove the all the anchor overlays
	// pub fn remove_anchor_overlays(&mut self, responses: &mut VecDeque<Message>) {
	// 	for anchor in &mut self.anchors {
	// 		anchor.remove_anchor_overlay(responses);
	// 	}
	// }

	// /// Remove the all the anchor overlays
	// pub fn remove_handle_overlays(&mut self, responses: &mut VecDeque<Message>) {
	// 	for anchor in &mut self.anchors {
	// 		anchor.remove_handle_overlay(responses);
	// 	}
	// }

	// /// Eventually we will want to hide the overlays instead of clearing them when selecting a new shape
	// pub fn set_overlay_visibility(&mut self, visibility: bool, responses: &mut VecDeque<Message>) {
	// 	self.set_shape_outline_visiblity(visibility, responses);
	// 	self.set_anchors_visiblity(visibility, responses);
	// 	self.set_handles_visiblity(visibility, responses);
	// }

	// /// Set the visibility of the shape outline
	// pub fn set_shape_outline_visiblity(&self, visibility: bool, responses: &mut VecDeque<Message>) {
	// 	if let Some(overlay_path) = &self.shape_overlay {
	// 		responses.push_back(
	// 			DocumentMessage::Overlays(
	// 				Operation::SetLayerVisibility {
	// 					path: overlay_path.clone(),
	// 					visible: visibility,
	// 				}
	// 				.into(),
	// 			)
	// 			.into(),
	// 		);
	// 	}
	// }

	// /// Set visibility on all of the anchors in this shape
	// pub fn set_anchors_visiblity(&self, visibility: bool, responses: &mut VecDeque<Message>) {
	// 	for anchor in &self.anchors {
	// 		anchor.set_anchor_visiblity(visibility, responses);
	// 	}
	// }

	// /// Set visibility on all of the handles in this shape
	// pub fn set_handles_visiblity(&self, visibility: bool, responses: &mut VecDeque<Message>) {
	// 	for anchor in &self.anchors {
	// 		anchor.set_handle_visiblity(visibility, responses);
	// 	}
	// }

	//const POINT_STROKE_WIDTH: f32 = 2.0;
	/*
	Need some logic for when a point is selected
	if selected {
			self.set_overlay_style(POINT_STROKE_WIDTH + 1.0, COLOR_ACCENT, COLOR_ACCENT, responses);
		} else {
			self.set_overlay_style(POINT_STROKE_WIDTH, COLOR_ACCENT, Color::WHITE, responses);
		}
	*/

	/// Create the anchors from the kurbo path, only done during of new anchors construction
	// TODO remove anything to do with overlays
	// fn create_anchors_from_kurbo(&self, responses: &mut VecDeque<Message>) -> Vec<VectorAnchor> {
	// 	// We need the indices paired with the kurbo path elements
	// 	let indexed_elements = self.bez_path.elements().iter().enumerate().map(|(index, element)| (index, *element)).collect::<Vec<IndexedEl>>();

	// 	// Create the manipulation points
	// 	let mut anchors: Vec<VectorAnchor> = vec![];
	// 	let (mut first_path_element, mut last_path_element): (Option<IndexedEl>, Option<IndexedEl>) = (None, None);
	// 	let mut last_move_to_element: Option<IndexedEl> = None;
	// 	let mut ended_with_close_path = false;
	// 	let mut first_move_to_id: usize = 0;

	// 	// TODO Consider using a LL(1) grammar to improve readability
	// 	// Create an anchor at each join between two kurbo segments
	// 	for elements in indexed_elements.windows(2) {
	// 		let (_, current_element) = elements[0];
	// 		let (_, next_element) = elements[1];
	// 		ended_with_close_path = false;

	// 		if matches!(current_element, kurbo::PathEl::ClosePath) {
	// 			continue;
	// 		}

	// 		// An anchor cannot stradle a line / curve segment and a ClosePath segment
	// 		if matches!(next_element, kurbo::PathEl::ClosePath) {
	// 			ended_with_close_path = true;
	// 			if self.closed {
	// 				self.close_path(&mut anchors, first_move_to_id, first_path_element, last_path_element, last_move_to_element, responses);
	// 			} else {
	// 				anchors.push(self.create_anchor(last_path_element, None, responses));
	// 			}
	// 			continue;
	// 		}

	// 		// Keep track of the first and last elements of this shape
	// 		if matches!(current_element, kurbo::PathEl::MoveTo(_)) {
	// 			last_move_to_element = Some(elements[0]);
	// 			first_path_element = Some(elements[1]);
	// 			first_move_to_id = anchors.len();
	// 		}
	// 		last_path_element = Some(elements[1]);

	// 		anchors.push(self.create_anchor(Some(elements[0]), Some(elements[1]), responses));
	// 	}

	// 	// If the path definition didn't include a ClosePath, we still need to behave as though it did
	// 	if !ended_with_close_path {
	// 		if self.closed {
	// 			self.close_path(&mut anchors, first_move_to_id, first_path_element, last_path_element, last_move_to_element, responses);
	// 		} else {
	// 			anchors.push(self.create_anchor(last_path_element, None, responses));
	// 		}
	// 	}

	// 	anchors
	// }

	/// Update the anchors to match the kurbo path
	// // TODO remove, no more kurbo read back
	// fn update_anchors_from_kurbo(&mut self, path: &BezPath) {
	// 	let space_transform = |point: kurbo::Point| self.transform.transform_point2(DVec2::from((point.x, point.y)));
	// 	for anchor_index in 0..self.anchors.len() {
	// 		let elements = path.elements();
	// 		let anchor = &mut self.anchors[anchor_index];
	// 		if let Some(anchor_point) = &mut anchor.points[ControlPointType::Anchor] {
	// 			match elements[anchor_point.kurbo_element_id] {
	// 				kurbo::PathEl::MoveTo(anchor_position) | kurbo::PathEl::LineTo(anchor_position) => anchor.set_point_position(ControlPointType::Anchor as usize, space_transform(anchor_position)),
	// 				kurbo::PathEl::QuadTo(handle_position, anchor_position) | kurbo::PathEl::CurveTo(_, handle_position, anchor_position) => {
	// 					anchor.set_point_position(ControlPointType::Anchor as usize, space_transform(anchor_position));
	// 					if anchor.points[ControlPointType::Handle1].is_some() {
	// 						anchor.set_point_position(ControlPointType::Handle1 as usize, space_transform(handle_position));
	// 					}
	// 				}
	// 				_ => (),
	// 			}
	// 			if let Some(handle) = &mut anchor.points[ControlPointType::Handle2] {
	// 				match elements[handle.kurbo_element_id] {
	// 					kurbo::PathEl::CurveTo(handle_position, _, _) | kurbo::PathEl::QuadTo(handle_position, _) => {
	// 						anchor.set_point_position(ControlPointType::Handle2 as usize, space_transform(handle_position));
	// 					}
	// 					_ => (),
	// 				}
	// 			}
	// 		}
	// 	}
	// }
}
