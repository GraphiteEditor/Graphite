use std::collections::VecDeque;

use glam::{DAffine2, DVec2};
use graphene::{
	color::Color,
	layers::{
		layer_info::LayerDataType,
		simple_shape::Shape,
		style::{self, Fill, Stroke},
	},
	LayerId, Operation,
};
use kurbo::{BezPath, PathEl, PathSeg, Vec2};

use crate::{
	consts::{COLOR_ACCENT, VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE},
	document::DocumentMessageHandler,
	message_prelude::{generate_uuid, DocumentMessage, Message},
};

// Helps push values that end in approximately half, plus or minus some floating point imprecision, towards the same side of the round() function
const BIAS: f64 = 0.0001;

#[derive(Clone, Debug, Default)]
pub struct ManipulationHandler {
	// The selected shapes, the cloned path and the kurbo PathElements
	pub selected_shapes: Vec<VectorManipulatorShape>,
	pub selected_layer_path: Vec<LayerId>,
	pub selected_shape_elements: Vec<kurbo::PathEl>,
	// The shape that had a point selected from most recently
	pub selected_shape: usize,
	// This can represent any draggable point anchor or handle
	pub selected_point: VectorManipulatorPoint,
	// This is specifically the related anchor, even if we have a handle selected
	pub selected_anchor: VectorManipulatorAnchor,
	// Debounce for toggling mirroring with alt
	alt_mirror_toggle_debounce: bool,
}

impl ManipulationHandler {
	/// Select the first manipulator within the selection threshold
	pub fn select_manipulator(&mut self, mouse_position: DVec2, select_threshold: f64) -> bool {
		let select_threshold_squared = select_threshold * select_threshold;
		for shape_index in 0..self.selected_shapes.len() {
			let selected_shape = &self.selected_shapes[shape_index];
			// Find the closest control point for this shape
			let (anchor, point, distance) = self.closest_manipulator(selected_shape, mouse_position);
			// Choose the first manipulator under the threshold
			if distance < select_threshold_squared {
				self.selected_shape_elements = selected_shape.path.clone().into_iter().collect();
				self.selected_layer_path = selected_shape.layer_path.clone();
				self.selected_shape = shape_index;
				self.selected_point = point.clone();
				self.selected_anchor = anchor.clone();
				// Due to the shape data structure not persisting across selections we need to rely on the svg to tell know if we should mirror
				self.selected_anchor.handle_mirroring = (anchor.angle_between_handles() - std::f64::consts::PI).abs() < 0.1;
				self.alt_mirror_toggle_debounce = false;
				return true;
			}
		}
		false
	}

	/// Provide the currently selected shape
	pub fn selected_shape(&self) -> &VectorManipulatorShape {
		&self.selected_shapes[self.selected_shape]
	}

	/// A wrapper around move_point to handle mirror state / submit the changes
	pub fn move_selected_to(&mut self, target_position: DVec2, should_mirror: bool) -> Operation {
		let target_to_shape = self.selected_shape().transform.inverse().transform_point2(target_position);
		let target_position = Vec2::new(target_to_shape.x, target_to_shape.y);

		// Should we mirror the opposing handle or not?
		if !should_mirror && self.alt_mirror_toggle_debounce != should_mirror {
			self.selected_anchor.handle_mirroring = !self.selected_anchor.handle_mirroring;
		}
		self.alt_mirror_toggle_debounce = should_mirror;

		self.move_point(target_position);

		// We've made our changes to the shape, submit them
		Operation::SetShapePathInViewport {
			path: self.selected_layer_path.clone(),
			bez_path: self.selected_shape_elements.clone().into_iter().collect(),
			transform: self.selected_shape().transform.to_cols_array(),
		}
	}

	/// Move the selected point to the specificed target position
	fn move_point(&mut self, target_position: Vec2) {
		let target_position_as_point = target_position.to_point();
		let (h1, h2) = &self.selected_anchor.handles;
		let h1_selected = !h1.is_none() && *h1.as_ref().unwrap() == self.selected_point;
		let h2_selected = !h2.is_none() && *h2.as_ref().unwrap() == self.selected_point;

		let place_mirrored_handle = |center: kurbo::Point, original: kurbo::Point, mirror: bool, selected: bool| -> kurbo::Point {
			if !selected || !mirror {
				return original;
			}

			// Keep rotational similarity, but distance variable
			let radius = center.distance(original);
			let phi = (center - target_position_as_point).atan2();

			kurbo::Point {
				x: radius * phi.cos() + center.x,
				y: radius * phi.sin() + center.y,
			}
		};

		// If neither handle is selected, we are dragging an anchor point
		if !(h1_selected || h2_selected) {
			// Move the anchor point and hande on the same path element
			let (selected, point) = match &self.selected_shape_elements[self.selected_anchor.point.element_id] {
				PathEl::MoveTo(p) => (PathEl::MoveTo(target_position_as_point), p),
				PathEl::LineTo(p) => (PathEl::LineTo(target_position_as_point), p),
				PathEl::QuadTo(a1, p) => (PathEl::QuadTo(*a1 - (*p - target_position_as_point), target_position_as_point), p),
				PathEl::CurveTo(a1, a2, p) => (PathEl::CurveTo(*a1, *a2 - (*p - target_position_as_point), target_position_as_point), p),
				PathEl::ClosePath => (PathEl::ClosePath, &target_position_as_point),
			};

			// Move the handle on the adjacent path element
			if let Some(handle) = h2 {
				let point_delta = (*point - target_position).to_vec2();
				let neighbor = match &self.selected_shape_elements[handle.element_id] {
					PathEl::MoveTo(p) => PathEl::MoveTo(*p),
					PathEl::LineTo(_) => PathEl::LineTo(target_position_as_point),
					PathEl::QuadTo(a1, p) => PathEl::QuadTo(*a1 - point_delta, *p),
					PathEl::CurveTo(a1, a2, p) => PathEl::CurveTo(*a1 - point_delta, *a2, *p),
					PathEl::ClosePath => PathEl::ClosePath,
				};
				self.selected_shape_elements[handle.element_id] = neighbor;

				// Handle the invisible point that can be caused by MoveTo
				if let Some(close_id) = self.selected_anchor.close_element_id {
					self.selected_shape_elements[close_id] = PathEl::MoveTo(target_position_as_point);
				}
			}
			self.selected_shape_elements[self.selected_point.element_id] = selected;
		}
		// We are dragging a handle
		else {
			let should_mirror = self.selected_anchor.handle_mirroring;

			// Move the selected handle
			let (selected, anchor) = match &self.selected_shape_elements[self.selected_point.element_id] {
				PathEl::MoveTo(p) => (PathEl::MoveTo(*p), *p),
				PathEl::LineTo(p) => (PathEl::LineTo(*p), *p),
				PathEl::QuadTo(_, p) => (PathEl::QuadTo(target_position_as_point, *p), *p),
				PathEl::CurveTo(a1, a2, p) => (
					PathEl::CurveTo(if h2_selected { target_position_as_point } else { *a1 }, if h1_selected { target_position_as_point } else { *a2 }, *p),
					*p,
				),
				PathEl::ClosePath => (PathEl::ClosePath, target_position_as_point),
			};

			// Move the opposing handle on the adjacent path element
			if let Some(handle) = self.selected_anchor.opposing_handle(&self.selected_point) {
				let neighbor = match &self.selected_shape_elements[handle.element_id] {
					PathEl::MoveTo(p) => PathEl::MoveTo(*p),
					PathEl::LineTo(p) => PathEl::LineTo(*p),
					PathEl::QuadTo(a1, p) => PathEl::QuadTo(*a1, *p),
					PathEl::CurveTo(a1, a2, p) => PathEl::CurveTo(
						place_mirrored_handle(anchor, *a1, h1_selected, should_mirror),
						place_mirrored_handle(*p, *a2, h2_selected, should_mirror),
						*p,
					),
					PathEl::ClosePath => PathEl::ClosePath,
				};
				self.selected_shape_elements[handle.element_id] = neighbor;
			}
			self.selected_shape_elements[self.selected_point.element_id] = selected;
		}
	}

	// TODO Use quadtree or some equivalent spatial acceleration structure to improve this to O(log(n))
	/// Brute force comparison to determine which handle / anchor we want to select, O(n)
	fn closest_manipulator<'a>(&self, shape: &'a VectorManipulatorShape, pos: glam::DVec2) -> (&'a VectorManipulatorAnchor, &'a VectorManipulatorPoint, f64) {
		let mut closest_anchor: &'a VectorManipulatorAnchor = &shape.anchors[0];
		let mut closest_point: &'a VectorManipulatorPoint = &shape.anchors[0].point;
		let mut closest_distance: f64 = f64::MAX; // Not ideal
		for anchor in shape.anchors.iter() {
			let point = anchor.closest_handle_or_anchor(pos);
			let distance_squared = point.position.distance_squared(pos);
			if distance_squared < closest_distance {
				closest_distance = distance_squared;
				closest_anchor = anchor;
				closest_point = point;
			}
		}
		(closest_anchor, closest_point, closest_distance)
	}
}

#[derive(PartialEq, Clone, Debug, Default)]
pub struct VectorManipulatorShape {
	/// The path to the layer
	pub layer_path: Vec<LayerId>,
	/// The outline of the shape
	pub path: kurbo::BezPath,
	/// The segments containing the control points / manipulator handles
	pub segments: Vec<VectorManipulatorSegment>,
	/// The control points / manipulator handles
	pub anchors: Vec<VectorManipulatorAnchor>,
	/// The overlays for the shape, anchors and manipulator handles
	pub shape_overlay: Option<Vec<LayerId>>,
	/// The compound Bezier curve is closed
	pub closed: bool,
	/// The transformation matrix to apply
	pub transform: DAffine2,
}

impl VectorManipulatorShape {
	// TODO: Figure out a more elegant way to construct this
	pub fn new(layer_path: Vec<LayerId>, transform: DAffine2, shape: &Shape, responses: &mut VecDeque<Message>) -> Self {
		let mut manipulator_shape = VectorManipulatorShape {
			layer_path,
			path: shape.path.clone(),
			closed: shape.closed,
			transform,
			segments: vec![],
			anchors: vec![],
			shape_overlay: None,
		};
		manipulator_shape.segments = manipulator_shape.create_segments_from_kurbo();
		manipulator_shape.anchors = manipulator_shape.create_anchors_from_kurbo(responses);
		manipulator_shape.shape_overlay = Some(manipulator_shape.add_shape_outline_overlay(responses));
		manipulator_shape
	}

	/// Place points in local space
	fn to_local_space(&self, point: kurbo::Point) -> DVec2 {
		self.transform.transform_point2(DVec2::from((point.x, point.y)))
	}

	/// Create the anchors from the kurbo path, only done on construction
	fn create_anchors_from_kurbo(&self, responses: &mut VecDeque<Message>) -> Vec<VectorManipulatorAnchor> {
		type IndexedEl = (usize, kurbo::PathEl);

		// Create an anchor on the boundary between two kurbo PathElements with optional handles
		let mut create_anchor_manipulator = |first: IndexedEl, second: IndexedEl| -> VectorManipulatorAnchor {
			let mut handle1 = None;
			let mut anchor_position: glam::DVec2 = glam::DVec2::ZERO;
			let mut handle2 = None;
			let (first_id, first_element) = first;
			let (second_id, second_element) = second;

			match first_element {
				kurbo::PathEl::MoveTo(anchor) | kurbo::PathEl::LineTo(anchor) => anchor_position = self.to_local_space(anchor),
				kurbo::PathEl::QuadTo(handle, anchor) | kurbo::PathEl::CurveTo(_, handle, anchor) => {
					anchor_position = self.to_local_space(anchor);
					handle1 = Some(VectorManipulatorPoint {
						element_id: first_id,
						position: self.to_local_space(handle),
						point_overlay: Some(self.add_handle_overlay(responses)),
					});
				}
				_ => (),
			}

			match second_element {
				kurbo::PathEl::CurveTo(handle, _, _) | kurbo::PathEl::QuadTo(handle, _) => {
					handle2 = Some(VectorManipulatorPoint {
						element_id: second_id,
						position: self.to_local_space(handle),
						point_overlay: Some(self.add_handle_overlay(responses)),
					});
				}
				_ => (),
			}

			VectorManipulatorAnchor {
				point: VectorManipulatorPoint {
					element_id: first_id,
					position: anchor_position,
					point_overlay: Some(self.add_anchor_overlay(responses)),
				},
				close_element_id: None,
				handle_line_overlays: (self.add_handle_line_overlay(&handle1, responses), self.add_handle_line_overlay(&handle2, responses)),
				handles: (handle1, handle2),
				handle_mirroring: true,
			}
		};

		// We need the indices paired with the kurbo path elements
		let indexed_elements = self.path.elements().iter().enumerate().map(|(index, element)| (index, *element)).collect::<Vec<IndexedEl>>();

		// Create the manipulation points
		let mut points: Vec<VectorManipulatorAnchor> = vec![];
		let (mut first, mut last): (Option<IndexedEl>, Option<IndexedEl>) = (None, None);
		let mut close_element_id: Option<usize> = None;

		// Create an anchor at each join between two kurbo segments
		for elements in indexed_elements.windows(2) {
			let (current_index, current_element) = elements[0];
			let (_, next_element) = elements[1];

			// An anchor cannot stradle a line / curve segment and a ClosePath segment
			if matches!(next_element, kurbo::PathEl::ClosePath) {
				break;
			}

			// TODO: Currently a unique case for [MoveTo, CurveTo, ...], refactor more generally if possible
			if matches!(current_element, kurbo::PathEl::MoveTo(_)) && (matches!(next_element, kurbo::PathEl::CurveTo(_, _, _)) || matches!(next_element, kurbo::PathEl::QuadTo(_, _))) {
				close_element_id = Some(current_index);
				continue;
			}

			// Keep track of the first and last elements of this shape
			if first.is_none() {
				first = Some(elements[0]);
			}
			last = Some(elements[1]);

			points.push(create_anchor_manipulator(elements[0], elements[1]));
		}

		// Close the shape
		if let (Some(first), Some(last)) = (first, last) {
			let mut element = create_anchor_manipulator(last, first);
			element.close_element_id = close_element_id;
			points.push(element);
		}

		points
	}

	/// Create the segments from the kurbo shape
	fn create_segments_from_kurbo(&self) -> Vec<VectorManipulatorSegment> {
		self.path
			.segments()
			.map(|segment| -> VectorManipulatorSegment {
				match segment {
					PathSeg::Line(line) => VectorManipulatorSegment::Line(self.to_local_space(line.p0), self.to_local_space(line.p1)),
					PathSeg::Quad(quad) => VectorManipulatorSegment::Quad(self.to_local_space(quad.p0), self.to_local_space(quad.p1), self.to_local_space(quad.p2)),
					PathSeg::Cubic(cubic) => VectorManipulatorSegment::Cubic(
						self.to_local_space(cubic.p0),
						self.to_local_space(cubic.p1),
						self.to_local_space(cubic.p2),
						self.to_local_space(cubic.p3),
					),
				}
			})
			.collect::<Vec<VectorManipulatorSegment>>()
	}

	/// Update the anchors to natch the kurbo path
	fn update_anchors(&mut self, path: &BezPath) {
		let space_transform = |point: kurbo::Point| self.transform.transform_point2(DVec2::from((point.x, point.y)));
		for anchor_index in 0..self.anchors.len() {
			let elements = path.elements();
			let anchor = &mut self.anchors[anchor_index];
			match elements[anchor.point.element_id] {
				kurbo::PathEl::MoveTo(anchor_position) | kurbo::PathEl::LineTo(anchor_position) => anchor.point.position = space_transform(anchor_position),
				kurbo::PathEl::QuadTo(handle_position, anchor_position) | kurbo::PathEl::CurveTo(_, handle_position, anchor_position) => {
					anchor.point.position = space_transform(anchor_position);
					if let Some(handle) = &mut anchor.handles.0 {
						handle.position = space_transform(handle_position);
						anchor.handles.0 = Some(handle.clone());
					}
				}
				_ => (),
			}
			if let Some(handle) = &mut anchor.handles.1 {
				match elements[handle.element_id] {
					kurbo::PathEl::CurveTo(handle_position, _, _) | kurbo::PathEl::QuadTo(handle_position, _) => {
						handle.position = space_transform(handle_position);
						anchor.handles.1 = Some(handle.clone());
					}
					_ => (),
				}
			}
		}
	}

	/// Update the segments to match the kurbo shape
	fn update_segments(&mut self, path: &BezPath) {
		path.segments().enumerate().for_each(|(index, segment)| {
			self.segments[index] = match segment {
				PathSeg::Line(line) => VectorManipulatorSegment::Line(self.to_local_space(line.p0), self.to_local_space(line.p1)),
				PathSeg::Quad(quad) => VectorManipulatorSegment::Quad(self.to_local_space(quad.p0), self.to_local_space(quad.p1), self.to_local_space(quad.p2)),
				PathSeg::Cubic(cubic) => VectorManipulatorSegment::Cubic(
					self.to_local_space(cubic.p0),
					self.to_local_space(cubic.p1),
					self.to_local_space(cubic.p2),
					self.to_local_space(cubic.p3),
				),
			};
		});
	}

	/// Update the anchors and segments to match the kurbo shape
	pub fn update_shape(&mut self, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		let viewport_transform = document.graphene_document.generate_transform_relative_to_viewport(&self.layer_path).unwrap();
		let layer = document.graphene_document.layer(&self.layer_path).unwrap();
		if let LayerDataType::Shape(shape) = &layer.data {
			let path = shape.path.clone();
			self.transform = viewport_transform;

			// Update point positions
			self.update_anchors(&path);

			// Update the segment positions
			self.update_segments(&path);

			self.path = path;

			// Update the overlays to represent the changes to the kurbo path
			self.place_shape_outline_overlay(responses);
			self.place_anchor_overlays(responses);
			self.place_handle_overlays(responses);
		}
	}

	fn add_shape_outline_overlay(&self, responses: &mut VecDeque<Message>) -> Vec<LayerId> {
		let layer_path = vec![generate_uuid()];
		let operation = Operation::AddOverlayShape {
			path: layer_path.clone(),
			bez_path: self.path.clone(),
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), Some(Fill::none())),
			closed: false,
		};
		responses.push_back(DocumentMessage::Overlays(operation.into()).into());

		layer_path
	}

	/// Create a single anchor overlay
	fn add_anchor_overlay(&self, responses: &mut VecDeque<Message>) -> Vec<LayerId> {
		let layer_path = vec![generate_uuid()];
		let operation = Operation::AddOverlayRect {
			path: layer_path.clone(),
			transform: DAffine2::IDENTITY.to_cols_array(),
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 2.0)), Some(Fill::new(Color::WHITE))),
		};
		responses.push_back(DocumentMessage::Overlays(operation.into()).into());
		layer_path
	}

	fn add_handle_overlay(&self, responses: &mut VecDeque<Message>) -> Vec<LayerId> {
		let layer_path = vec![generate_uuid()];
		let operation = Operation::AddOverlayEllipse {
			path: layer_path.clone(),
			transform: DAffine2::IDENTITY.to_cols_array(),
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 2.0)), Some(Fill::new(Color::WHITE))),
		};
		responses.push_back(DocumentMessage::Overlays(operation.into()).into());
		layer_path
	}

	fn add_handle_line_overlay(&self, handle: &Option<VectorManipulatorPoint>, responses: &mut VecDeque<Message>) -> Option<Vec<LayerId>> {
		if handle.is_none() {
			return None;
		}

		let layer_path = vec![generate_uuid()];
		let operation = Operation::AddOverlayLine {
			path: layer_path.clone(),
			transform: DAffine2::IDENTITY.to_cols_array(),
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), Some(Fill::none())),
		};
		responses.push_back(DocumentMessage::Overlays(operation.into()).into());

		Some(layer_path)
	}

	/// Update the positions of the anchor points based on the kurbo path
	fn place_shape_outline_overlay(&self, responses: &mut VecDeque<Message>) {
		if let Some(overlay) = &self.shape_overlay {
			responses.push_back(
				DocumentMessage::Overlays(
					Operation::SetShapePathInViewport {
						path: overlay.clone(),
						bez_path: self.path.clone(),
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
			if let Some(overlay) = &anchor.point.point_overlay {
				let scale = DVec2::splat(VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE);
				let angle = 0.;
				let translation = (anchor.point.position - (scale / 2.) + BIAS).round();
				let transform = DAffine2::from_scale_angle_translation(scale, angle, translation).to_cols_array();
				responses.push_back(DocumentMessage::Overlays(Operation::SetLayerTransformInViewport { path: overlay.clone(), transform }.into()).into());
			}
		}
	}

	/// Update the positions of the handle points and lines based on the kurbo path
	fn place_handle_overlays(&self, responses: &mut VecDeque<Message>) {
		for anchor in &self.anchors {
			// Helper function to keep things DRY
			let mut place_handle_and_line = |handle: &VectorManipulatorPoint, line: &Option<Vec<LayerId>>| {
				if let Some(overlay) = line {
					let line_vector = anchor.point.position - handle.position;
					let scale = DVec2::splat(line_vector.length());
					let angle = -line_vector.angle_between(DVec2::X);
					let translation = (handle.position + BIAS).round() + DVec2::splat(0.5);
					let transform = DAffine2::from_scale_angle_translation(scale, angle, translation).to_cols_array();
					responses.push_back(DocumentMessage::Overlays(Operation::SetLayerTransformInViewport { path: overlay.clone(), transform }.into()).into());
				}

				if let Some(overlay) = &handle.point_overlay {
					let scale = DVec2::splat(VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE);
					let angle = 0.;
					let translation = (handle.position - (scale / 2.) + BIAS).round();
					let transform = DAffine2::from_scale_angle_translation(scale, angle, translation).to_cols_array();
					responses.push_back(DocumentMessage::Overlays(Operation::SetLayerTransformInViewport { path: overlay.clone(), transform }.into()).into());
				}
			};

			let (h1, h2) = &anchor.handles;
			let (line1, line2) = &anchor.handle_line_overlays;

			if let Some(handle) = &h1 {
				place_handle_and_line(handle, line1);
			}

			if let Some(handle) = &h2 {
				place_handle_and_line(handle, line2);
			}
		}
	}

	/// Remove all of the overlays from the shape
	pub fn remove_all_overlays(&mut self, responses: &mut VecDeque<Message>) {
		self.remove_shape_outline_overlay(responses);
		self.remove_anchor_overlays(responses);
		self.remove_handle_overlays(responses);
	}

	/// Remove the outline around the shape
	fn remove_shape_outline_overlay(&mut self, responses: &mut VecDeque<Message>) {
		if let Some(overlay) = &self.shape_overlay {
			responses.push_back(DocumentMessage::Overlays(Operation::DeleteLayer { path: overlay.clone() }.into()).into());
		}
		self.shape_overlay = None;
	}

	/// Remove the all the anchor overlays
	fn remove_anchor_overlays(&mut self, responses: &mut VecDeque<Message>) {
		for anchor in &mut self.anchors {
			if let Some(overlay) = &anchor.point.point_overlay {
				responses.push_back(DocumentMessage::Overlays(Operation::DeleteLayer { path: overlay.clone() }.into()).into());
			}
			anchor.point.point_overlay = None;
		}
	}

	/// Remove the all the anchor overlays
	fn remove_handle_overlays(&mut self, responses: &mut VecDeque<Message>) {
		for anchor in &mut self.anchors {
			let (h1, h2) = &mut anchor.handles;
			let (line1, line2) = &mut anchor.handle_line_overlays;

			// Helper function to keep things DRY
			let mut delete_message = |handle: &Option<Vec<LayerId>>| {
				if let Some(overlay) = handle {
					responses.push_back(DocumentMessage::Overlays(Operation::DeleteLayer { path: overlay.clone() }.into()).into());
				}
			};

			// Delete the handles themselves
			if let Some(handle) = h1 {
				delete_message(&handle.point_overlay);
				handle.point_overlay = None;
			}
			if let Some(handle) = h2 {
				delete_message(&handle.point_overlay);
				handle.point_overlay = None;
			}

			// Delete the handle line layers
			delete_message(line1);
			delete_message(line2);
			anchor.handle_line_overlays = (None, None);
		}
	}

	/// Eventually we will want to hide the overlays instead of clearing them constantly
	#[warn(dead_code)]
	pub fn set_all_overlay_visibility(&mut self, visibility: bool, responses: &mut VecDeque<Message>) {
		self.set_shape_outline_visiblity(visibility, responses);
		self.set_anchor_visiblity(visibility, responses);
		self.set_handle_visiblity(visibility, responses);
	}

	fn set_shape_outline_visiblity(&self, visibility: bool, responses: &mut VecDeque<Message>) {
		if let Some(overlay) = &self.shape_overlay {
			responses.push_back(self.visibility_message(overlay.clone(), visibility));
		}
	}

	fn set_anchor_visiblity(&self, visibility: bool, responses: &mut VecDeque<Message>) {
		for anchor in &self.anchors {
			if let Some(overlay) = &anchor.point.point_overlay {
				responses.push_back(self.visibility_message(overlay.clone(), visibility));
			}
		}
	}

	pub fn set_handle_visiblity(&self, visibility: bool, responses: &mut VecDeque<Message>) {
		for anchor in &self.anchors {
			let (h1, h2) = &anchor.handles;
			let (line1, line2) = &anchor.handle_line_overlays;

			if let Some(handle) = h1 {
				if let Some(overlay) = &handle.point_overlay {
					responses.push_back(self.visibility_message(overlay.clone(), visibility));
				}
			}
			if let Some(handle) = h2 {
				if let Some(overlay) = &handle.point_overlay {
					responses.push_back(self.visibility_message(overlay.clone(), visibility));
				}
			}

			if let Some(overlay) = &line1 {
				responses.push_back(self.visibility_message(overlay.clone(), visibility));
			}
			if let Some(overlay) = &line2 {
				responses.push_back(self.visibility_message(overlay.clone(), visibility));
			}
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
}

#[derive(PartialEq, Clone, Debug)]
pub enum VectorManipulatorSegment {
	Line(DVec2, DVec2),
	Quad(DVec2, DVec2, DVec2),
	Cubic(DVec2, DVec2, DVec2, DVec2),
}

#[derive(PartialEq, Clone, Debug, Default)]
pub struct VectorManipulatorAnchor {
	// The associated position in the BezPath
	pub point: VectorManipulatorPoint,
	// Does this anchor point have a path close element we also needs to move?
	pub close_element_id: Option<usize>,
	// Should we mirror the handles
	pub handle_mirroring: bool,
	// Anchor handles
	pub handles: (Option<VectorManipulatorPoint>, Option<VectorManipulatorPoint>),
	// The overlays for this handle line rendering
	pub handle_line_overlays: (Option<Vec<LayerId>>, Option<Vec<LayerId>>),
}

impl VectorManipulatorAnchor {
	pub fn closest_handle_or_anchor(&self, target: glam::DVec2) -> &VectorManipulatorPoint {
		let mut closest_point: &VectorManipulatorPoint = &self.point;
		let mut distance = self.point.position.distance_squared(target);
		let (handle1, handle2) = &self.handles;
		if let Some(handle1) = handle1 {
			let handle1_dist = handle1.position.distance_squared(target);
			if distance > handle1_dist {
				distance = handle1_dist;
				closest_point = handle1;
			}
		}

		if let Some(handle2) = handle2 {
			let handle2_dist = handle2.position.distance_squared(target);
			if distance > handle2_dist {
				closest_point = handle2;
			}
		}

		closest_point
	}

	/// Angle between handles in radians
	pub fn angle_between_handles(&self) -> f64 {
		if let (Some(h1), Some(h2)) = &self.handles {
			return (self.point.position - h1.position).angle_between(self.point.position - h2.position);
		}
		0.0
	}

	pub fn opposing_handle(&self, handle: &VectorManipulatorPoint) -> &Option<VectorManipulatorPoint> {
		if Some(handle) == self.handles.0.as_ref() {
			&self.handles.1
		} else {
			&self.handles.0
		}
	}
}

#[derive(PartialEq, Clone, Debug, Default)]
pub struct VectorManipulatorPoint {
	// The associated position in the BezPath
	pub element_id: usize,
	// The sibling element if this is a handle
	pub position: glam::DVec2,
	// the overlay for this point rendering
	pub point_overlay: Option<Vec<LayerId>>,
}

impl VectorManipulatorPoint {
	pub(crate) fn clone(&self) -> VectorManipulatorPoint {
		VectorManipulatorPoint {
			element_id: self.element_id,
			position: self.position,
			point_overlay: self.point_overlay.clone(),
		}
	}
}
