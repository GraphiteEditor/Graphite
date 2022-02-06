use glam::{DAffine2, DVec2};
use graphene::{
	color::Color,
	layers::{
		layer_info::LayerDataType,
		style::{self, Fill, PathStyle, Stroke},
	},
	LayerId, Operation,
};
use kurbo::{BezPath, PathEl, PathSeg, Point, Vec2};
use std::{collections::HashSet, ops::Index};
use std::{collections::VecDeque, ops::IndexMut};

use crate::{
	consts::{COLOR_ACCENT, VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE},
	document::DocumentMessageHandler,
	message_prelude::{generate_uuid, DocumentMessage, Message},
};

// Helps push values that end in approximately half, plus or minus some floating point imprecision, towards the same side of the round() function
const BIAS: f64 = 0.0001;

// The angle in radians to determine if the kurbo shape is mirroring
const MINIMUM_MIRROR_THRESHOLD: f64 = 0.1;

/* Light overview of structs in this file and hierarchy:

						ShapeEditor
						/		  \
				VectorShape ... VectorShape  <- ShapeEditor contains many
					/		         \
	VectorManipulatorAnchor ...  VectorManipulatorAnchor <- each VectorShape contains many


				VectorManipulatorAnchor <- Container for the anchor metadata and optional VectorManipulatorPoints
						/
			[Option<VectorManipulatorPoint>; 3] <- [0] is the anchor's draggable point (but not metadata), [1] is the handle1 point, [2] is the handle2 point
			/				|				\
		"Anchor"		"Handle1" 		 "Handle2" <- These are VectorManipulatorPoints and the only editable "primitive"
*/

#[repr(usize)]
#[derive(PartialEq, Clone, Debug)]
pub enum ManipulatorType {
	Anchor = 0,
	Handle1 = 1,
	Handle2 = 2,
}
// Allows us to use ManipulatorType for indexing
impl<T> Index<ManipulatorType> for [T; 3] {
	type Output = T;
	fn index(&self, mt: ManipulatorType) -> &T {
		&self[mt as usize]
	}
}
// Allows us to use ManipulatorType for indexing, mutably
impl<T> IndexMut<ManipulatorType> for [T; 3] {
	fn index_mut(&mut self, mt: ManipulatorType) -> &mut T {
		&mut self[mt as usize]
	}
}

/// ShapeEditor is the container for all of the selected kurbo paths that are
/// represented as VectorShapes and provides functionality required
/// to query and create the VectorShapes / VectorManipulators
// TODO Provide support for multiple selected points / drag select
#[derive(Clone, Debug, Default)]
pub struct ShapeEditor {
	// The shapes we can select anchors / handles from
	pub shapes_to_modify: Vec<VectorShape>,
	// Index of the shape that contained the most recent selected point
	pub selected_shape_indices: HashSet<usize>,
	// Have we selected a point in shapes_to_modify yet?
	pub has_had_point_selection: bool,
	// The initial drag position of the mouse on drag start
	pub drag_start_position: DVec2,
}

impl ShapeEditor {
	/// Select the first point within the selection threshold
	pub fn select_point(&mut self, mouse_position: DVec2, select_threshold: f64, add_to_selection: bool, responses: &mut VecDeque<Message>) -> bool {
		if self.shapes_to_modify.is_empty() {
			return false;
		}

		let select_threshold_squared = select_threshold * select_threshold;
		// Find the closest control point among all elements of shapes_to_modify
		for shape_index in 0..self.shapes_to_modify.len() {
			let (anchor_index, point_index, distance_squared) = self.closest_manipulator_indices(&self.shapes_to_modify[shape_index], mouse_position);
			// Choose the first point under the threshold
			if distance_squared < select_threshold_squared {
				// Add this shape to the selection
				self.add_selected_shape(shape_index);

				// If the point we're selecting has already been selected
				// we can assume this point exists.. since we did just click on it hense the unwrap
				let is_point_selected = self.shapes_to_modify[shape_index].anchors[anchor_index].points[point_index].as_ref().unwrap().is_selected();

				// Deselected if we're not adding to the selection
				if !add_to_selection && !is_point_selected {
					self.deselect_all(responses);
				}

				let selected_shape = &mut self.shapes_to_modify[shape_index];

				// Refresh the elements of the path
				selected_shape.elements = selected_shape.bez_path.clone().into_iter().collect();

				// Add which anchor and point was selected
				let selected_point = selected_shape.select_anchor(anchor_index).set_selected(point_index, true, responses);
				// Set the mouse position for dragging
				if let Some(point) = selected_point {
					self.drag_start_position = point.position;
				}

				// Due to the shape data structure not persisting across shape selection changes we need to rely on the kurbo path to know if we should mirror
				let selected_anchor = &mut selected_shape.anchors[anchor_index];
				selected_anchor.set_mirroring((selected_anchor.angle_between_handles().abs() - std::f64::consts::PI).abs() < MINIMUM_MIRROR_THRESHOLD);
				self.has_had_point_selection = true;
				return true;
			}
		}
		false
	}

	/// Set the shapes we consider for selection, we will choose draggable handles / anchors from these shapes.
	pub fn set_shapes_to_modify(&mut self, selected_shapes: Vec<VectorShape>) {
		self.has_had_point_selection = false;
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
	pub fn selected_anchors(&self) -> impl Iterator<Item = &VectorManipulatorAnchor> {
		self.selected_shapes().flat_map(|shape| shape.selected_anchors())
	}

	/// Provide the currently selected anchors by mutable reference
	pub fn selected_anchors_mut(&mut self) -> impl Iterator<Item = &mut VectorManipulatorAnchor> {
		self.selected_shapes_mut().flat_map(|shape| shape.selected_anchors_mut())
	}

	/// Provide the currently selected points by reference
	pub fn selected_points(&self) -> impl Iterator<Item = &VectorManipulatorPoint> {
		self.selected_shapes().flat_map(|shape| shape.selected_anchors()).flat_map(|anchors| anchors.selected_points())
	}

	/// Provide the currently selected points by mutable reference
	pub fn selected_points_mut(&mut self) -> impl Iterator<Item = &mut VectorManipulatorPoint> {
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
			shape.clear_selected_anchors(responses)
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
	fn closest_manipulator_indices(&self, shape: &VectorShape, pos: glam::DVec2) -> (usize, usize, f64) {
		let mut closest_anchor_index: usize = 0;
		let mut closest_point_index: usize = 0;
		let mut closest_distance_squared: f64 = f64::MAX; // Not ideal
		for (anchor_index, anchor) in shape.anchors.iter().enumerate() {
			let point_index = anchor.closest_handle_or_anchor(pos);
			if let Some(point) = &anchor.points[point_index] {
				if point.can_be_selected {
					let distance_squared = point.position.distance_squared(pos);
					if distance_squared < closest_distance_squared {
						closest_distance_squared = distance_squared;
						closest_anchor_index = anchor_index;
						closest_point_index = point_index;
					}
				}
			}
		}
		(closest_anchor_index, closest_point_index, closest_distance_squared)
	}
}

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
	/// The segments containing the control points / manipulator handles
	pub segments: Vec<VectorManipulatorSegment>,
	/// The anchors that are made up of the control points / handles
	pub anchors: Vec<VectorManipulatorAnchor>,
	/// The overlays for the shape, anchors and manipulator handles
	pub shape_overlay: Option<Vec<LayerId>>,
	/// If the compound Bezier curve is closed
	pub closed: bool,
	/// The transformation matrix to apply
	pub transform: DAffine2,
	// Index of the most recently select point's anchor
	pub selected_anchor_indices: HashSet<usize>,
}
type IndexedEl = (usize, kurbo::PathEl);

impl VectorShape {
	// TODO: Figure out a more elegant way to construct this
	pub fn new(layer_path: Vec<LayerId>, transform: DAffine2, bez_path: &BezPath, closed: bool, responses: &mut VecDeque<Message>) -> Self {
		let mut shape = VectorShape {
			layer_path,
			bez_path: bez_path.clone(),
			closed,
			transform,
			elements: vec![],
			segments: vec![],
			anchors: vec![],
			shape_overlay: None,
			selected_anchor_indices: HashSet::<usize>::new(),
		};
		shape.elements = bez_path.into_iter().collect();
		shape.segments = shape.create_segments_from_kurbo();
		shape.shape_overlay = Some(shape.create_shape_outline_overlay(responses));
		shape.anchors = shape.create_anchors_from_kurbo(responses);
		// shape.select_all_anchors(responses);

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
	pub fn select_anchor(&mut self, anchor_index: usize) -> &mut VectorManipulatorAnchor {
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
			anchor.set_selected(0, true, responses);
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
	pub fn selected_anchors(&self) -> impl Iterator<Item = &VectorManipulatorAnchor> {
		self.anchors
			.iter()
			.enumerate()
			.filter_map(|(index, anchor)| if self.selected_anchor_indices.contains(&index) { Some(anchor) } else { None })
	}

	/// Return all the selected anchors, mutable
	pub fn selected_anchors_mut(&mut self) -> impl Iterator<Item = &mut VectorManipulatorAnchor> {
		self.anchors
			.iter_mut()
			.enumerate()
			.filter_map(|(index, anchor)| if self.selected_anchor_indices.contains(&index) { Some(anchor) } else { None })
	}

	/// Move the selected point based on mouse input, if this is a handle we can control if we are mirroring or not
	/// A wrapper around move_point to handle mirror state / submit the changes
	pub fn move_selected(&mut self, position_delta: DVec2, should_mirror: bool, responses: &mut VecDeque<Message>) {
		let transform = &self.transform.clone();
		let mut edited_bez_path = self.elements.clone();

		for selected_anchor in self.selected_anchors_mut() {
			// Should we mirror the opposing handle or not?
			if !should_mirror && selected_anchor.mirroring_debounce != should_mirror {
				selected_anchor.handles_are_mirroring = !selected_anchor.handles_are_mirroring;
			}
			selected_anchor.mirroring_debounce = should_mirror;

			selected_anchor.move_anchor_points(position_delta, &mut edited_bez_path, transform);
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

	/// Place points in local space
	fn to_local_space(&self, point: kurbo::Point) -> DVec2 {
		self.transform.transform_point2(DVec2::from((point.x, point.y)))
	}

	/// Create an anchor on the boundary between two kurbo PathElements with optional handles
	fn create_anchor_manipulator(&self, first: IndexedEl, second: IndexedEl, responses: &mut VecDeque<Message>) -> VectorManipulatorAnchor {
		let mut handle1 = None;
		let mut anchor_position: glam::DVec2 = glam::DVec2::ZERO;
		let mut handle2 = None;
		let (first_id, first_element) = first;
		let (second_id, second_element) = second;

		let create_point = |id: usize, point: DVec2, overlay_path: Vec<LayerId>, manipulator_type: ManipulatorType| -> VectorManipulatorPoint {
			VectorManipulatorPoint {
				element_id: id,
				position: point,
				overlay_path: Some(overlay_path),
				can_be_selected: true,
				is_selected: false,
				manipulator_type,
			}
		};

		match first_element {
			kurbo::PathEl::MoveTo(anchor) | kurbo::PathEl::LineTo(anchor) => anchor_position = self.to_local_space(anchor),
			kurbo::PathEl::QuadTo(handle, anchor) | kurbo::PathEl::CurveTo(_, handle, anchor) => {
				anchor_position = self.to_local_space(anchor);
				handle1 = Some(create_point(first_id, self.to_local_space(handle), self.create_handle_overlay(responses), ManipulatorType::Handle1));
			}
			_ => (),
		}

		match second_element {
			kurbo::PathEl::CurveTo(handle, _, _) | kurbo::PathEl::QuadTo(handle, _) => {
				handle2 = Some(create_point(second_id, self.to_local_space(handle), self.create_handle_overlay(responses), ManipulatorType::Handle2));
			}
			_ => (),
		}

		VectorManipulatorAnchor {
			handle_line_overlays: (self.create_handle_line_overlay(&handle1, responses), self.create_handle_line_overlay(&handle2, responses)),
			points: [
				Some(create_point(first_id, anchor_position, self.create_anchor_overlay(responses), ManipulatorType::Anchor)),
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
		points: &mut Vec<VectorManipulatorAnchor>,
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

			if position_equal {
				points[to_replace].remove_overlays(responses);
				points[to_replace] = self.create_anchor_manipulator(last, first, responses);
				points[to_replace].close_element_id = Some(move_to.0);
			} else {
				points.push(self.create_anchor_manipulator(last, first, responses));
			}
		}
	}

	/// Create the anchors from the kurbo path, only done during of new anchors construction
	fn create_anchors_from_kurbo(&self, responses: &mut VecDeque<Message>) -> Vec<VectorManipulatorAnchor> {
		// We need the indices paired with the kurbo path elements
		let indexed_elements = self.bez_path.elements().iter().enumerate().map(|(index, element)| (index, *element)).collect::<Vec<IndexedEl>>();

		// Create the manipulation points
		let mut points: Vec<VectorManipulatorAnchor> = vec![];
		let (mut first_path_element, mut last_path_element): (Option<IndexedEl>, Option<IndexedEl>) = (None, None);
		let mut last_move_to_element: Option<IndexedEl> = None;
		let mut ended_with_close_path = false;
		let mut replace_id: usize = 0;

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
				// Does this end in the same position it started?
				self.close_path(&mut points, replace_id, first_path_element, last_path_element, last_move_to_element, responses);

				continue;
			}

			// Keep track of the first and last elements of this shape
			if matches!(current_element, kurbo::PathEl::MoveTo(_)) {
				last_move_to_element = Some(elements[0]);
				first_path_element = Some(elements[1]);
				replace_id = points.len();
			}
			last_path_element = Some(elements[1]);

			points.push(self.create_anchor_manipulator(elements[0], elements[1], responses));
		}

		// If the path definition didn't include a ClosePath, we still need to behave as though it did
		if !ended_with_close_path {
			// Close the shape
			self.close_path(&mut points, replace_id, first_path_element, last_path_element, last_move_to_element, responses);
		}
		points
	}

	/// Create the segments from the kurbo shape
	fn create_segments_from_kurbo(&self) -> Vec<VectorManipulatorSegment> {
		self.bez_path
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

	/// Update the anchors to match the kurbo path
	fn update_anchors_from_kurbo(&mut self, path: &BezPath) {
		let space_transform = |point: kurbo::Point| self.transform.transform_point2(DVec2::from((point.x, point.y)));
		for anchor_index in 0..self.anchors.len() {
			let elements = path.elements();
			let anchor = &mut self.anchors[anchor_index];
			if let Some(anchor_point) = &mut anchor.points[ManipulatorType::Anchor] {
				match elements[anchor_point.element_id] {
					kurbo::PathEl::MoveTo(anchor_position) | kurbo::PathEl::LineTo(anchor_position) => anchor.set_point_position(ManipulatorType::Anchor as usize, space_transform(anchor_position)),
					kurbo::PathEl::QuadTo(handle_position, anchor_position) | kurbo::PathEl::CurveTo(_, handle_position, anchor_position) => {
						anchor.set_point_position(ManipulatorType::Anchor as usize, space_transform(anchor_position));
						if anchor.points[ManipulatorType::Handle1].is_some() {
							anchor.set_point_position(ManipulatorType::Handle1 as usize, space_transform(handle_position));
						}
					}
					_ => (),
				}
				if let Some(handle) = &mut anchor.points[ManipulatorType::Handle2] {
					match elements[handle.element_id] {
						kurbo::PathEl::CurveTo(handle_position, _, _) | kurbo::PathEl::QuadTo(handle_position, _) => {
							anchor.set_point_position(ManipulatorType::Handle2 as usize, space_transform(handle_position));
						}
						_ => (),
					}
				}
			}
		}
	}

	/// Update the segments to match the kurbo shape
	fn update_segments_from_kurbo(&mut self, path: &BezPath) {
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
	/// Should be called whenever the kurbo shape changes
	pub fn update_shape(&mut self, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		let viewport_transform = document.graphene_document.generate_transform_relative_to_viewport(&self.layer_path).unwrap();
		let layer = document.graphene_document.layer(&self.layer_path).unwrap();
		if let LayerDataType::Shape(shape) = &layer.data {
			let path = shape.path.clone();
			self.transform = viewport_transform;

			// Update point positions
			self.update_anchors_from_kurbo(&path);

			// Update the segment positions
			self.update_segments_from_kurbo(&path);

			self.bez_path = path;

			// Update the overlays to represent the changes to the kurbo path
			self.place_shape_outline_overlay(responses);
			self.place_anchor_overlays(responses);
			self.place_handle_overlays(responses);
		}
	}

	/// Create the kurbo shape that matches the selected viewport shape
	fn create_shape_outline_overlay(&self, responses: &mut VecDeque<Message>) -> Vec<LayerId> {
		let layer_path = vec![generate_uuid()];
		let operation = Operation::AddOverlayShape {
			path: layer_path.clone(),
			bez_path: self.bez_path.clone(),
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), Some(Fill::none())),
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
	fn create_handle_line_overlay(&self, handle: &Option<VectorManipulatorPoint>, responses: &mut VecDeque<Message>) -> Option<Vec<LayerId>> {
		if handle.is_none() {
			return None;
		}

		let layer_path = vec![generate_uuid()];
		let operation = Operation::AddOverlayLine {
			path: layer_path.clone(),
			transform: DAffine2::IDENTITY.to_cols_array(),
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), Some(Fill::none())),
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
	#[warn(dead_code)]
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

/// Used to alias PathSeg for our own purposes
#[derive(PartialEq, Clone, Debug)]
pub enum VectorManipulatorSegment {
	Line(DVec2, DVec2),
	Quad(DVec2, DVec2, DVec2),
	Cubic(DVec2, DVec2, DVec2, DVec2),
}

/// VectorManipulatorAnchor is used to represent an anchor point on the path that can be moved.
/// It contains 0-2 handles that are optionally displayed.
#[derive(PartialEq, Clone, Debug, Default)]
pub struct VectorManipulatorAnchor {
	// Editable points for the anchor & handles
	pub points: [Option<VectorManipulatorPoint>; 3],
	// Does this anchor point have a path close element?
	pub close_element_id: Option<usize>,
	// Should we mirror the handles?
	pub handles_are_mirroring: bool,
	// A debounce to handle alt toggling
	pub mirroring_debounce: bool,

	// The overlays for this handle line rendering
	pub handle_line_overlays: (Option<Vec<LayerId>>, Option<Vec<LayerId>>),
}

impl VectorManipulatorAnchor {
	/// Finds the closest VectorManipulatorPoint owned by this anchor. This can be the handles or the anchor itself
	pub fn closest_handle_or_anchor(&self, target: glam::DVec2) -> usize {
		self.points
			.iter()
			.flatten()
			.enumerate()
			.reduce(|(idx1, pnt1), (idx2, pnt2)| {
				if pnt1.position.distance_squared(target) < pnt2.position.distance_squared(target) {
					(idx1, pnt1)
				} else {
					(idx2, pnt2)
				}
			})
			.unwrap()
			.0
	}

	// TODO Cleanup the internals of this function
	/// Move the selected points to the specificed target position
	fn move_anchor_points(&mut self, position_delta: DVec2, path_to_update: &mut Vec<kurbo::PathEl>, transform: &DAffine2) {
		let place_mirrored_handle = |center: kurbo::Point, original: kurbo::Point, target: kurbo::Point, mirror: bool, selected: bool| -> kurbo::Point {
			if !selected || !mirror {
				return original;
			}

			// Keep rotational similarity, but distance variable
			let radius = center.distance(original);
			let phi = (center - target).atan2();

			kurbo::Point {
				x: radius * phi.cos() + center.x,
				y: radius * phi.sin() + center.y,
			}
		};

		for selected_point in self.selected_points() {
			let delta = transform.inverse().transform_vector2(position_delta);
			let delta = Vec2::new(delta.x, delta.y);
			let h1_selected = ManipulatorType::Handle1 == selected_point.manipulator_type;
			let h2_selected = ManipulatorType::Handle2 == selected_point.manipulator_type;

			// This section is particularly ugly and could use revision. Kurbo makes it somewhat difficult based on its approach.
			// If neither handle is selected, we are dragging an anchor point
			if !(h1_selected || h2_selected) {
				let handle1_exists_and_selected = self.points[ManipulatorType::Handle1].is_some() && self.points[ManipulatorType::Handle1].as_ref().unwrap().is_selected();
				// Move the anchor point and handle on the same path element
				let selected = match &path_to_update[selected_point.element_id] {
					PathEl::MoveTo(p) => PathEl::MoveTo(*p + delta),
					PathEl::LineTo(p) => PathEl::LineTo(*p + delta),
					PathEl::QuadTo(a1, p) => PathEl::QuadTo(*a1, *p + delta),
					PathEl::CurveTo(a1, a2, p) => PathEl::CurveTo(*a1, if handle1_exists_and_selected { *a2 } else { *a2 + delta }, *p + delta),
					PathEl::ClosePath => PathEl::ClosePath,
				};

				// Move the handle on the adjacent path element
				if let Some(handle) = &self.points[ManipulatorType::Handle2] {
					if !handle.is_selected() {
						let neighbor = match &path_to_update[handle.element_id] {
							PathEl::MoveTo(p) => PathEl::MoveTo(*p),
							PathEl::LineTo(p) => PathEl::LineTo(*p),
							PathEl::QuadTo(a1, p) => PathEl::QuadTo(*a1, *p),
							PathEl::CurveTo(a1, a2, p) => PathEl::CurveTo(*a1 + delta, *a2, *p),
							PathEl::ClosePath => PathEl::ClosePath,
						};
						path_to_update[handle.element_id] = neighbor;
					}
				}

				if let Some(close_id) = self.close_element_id {
					// Move the invisible point that can be caused by MoveTo / closing the path
					path_to_update[close_id] = match &path_to_update[close_id] {
						PathEl::MoveTo(p) => PathEl::MoveTo(*p + delta),
						PathEl::LineTo(p) => PathEl::LineTo(*p + delta),
						PathEl::QuadTo(a1, p) => PathEl::QuadTo(*a1, *p + delta),
						PathEl::CurveTo(a1, a2, p) => PathEl::CurveTo(*a1, *a2 + delta, *p + delta),
						PathEl::ClosePath => PathEl::ClosePath,
					};
				}

				path_to_update[selected_point.element_id] = selected;
			}
			// We are dragging a handle
			else {
				// Only move the handles if we don't have both handles selected
				let should_mirror = self.handles_are_mirroring;
				// Move the selected handle
				let (selected, anchor, selected_handle) = match &path_to_update[selected_point.element_id] {
					PathEl::MoveTo(p) => (PathEl::MoveTo(*p), *p, *p),
					PathEl::LineTo(p) => (PathEl::LineTo(*p), *p, *p),
					PathEl::QuadTo(a1, p) => (PathEl::QuadTo(*a1 + delta, *p), *p, *a1 + delta),
					PathEl::CurveTo(a1, a2, p) => {
						let a1_point = if h2_selected { *a1 + delta } else { *a1 };
						let a2_point = if h1_selected { *a2 + delta } else { *a2 };
						(PathEl::CurveTo(a1_point, a2_point, *p), *p, if h1_selected { a2_point } else { a1_point })
					}
					PathEl::ClosePath => (PathEl::ClosePath, Point::ZERO, Point::ZERO),
				};

				let opposing_handle = self.opposing_handle(selected_point);
				let only_one_handle_selected = !(selected_point.is_selected() && opposing_handle.is_some() && opposing_handle.as_ref().unwrap().is_selected());
				if only_one_handle_selected {
					// Move the opposing handle on the adjacent path element
					if let Some(handle) = opposing_handle {
						let handle_point = transform.inverse().transform_point2(handle.position);
						let handle_point = Point { x: handle_point.x, y: handle_point.y };
						let neighbor = match &path_to_update[handle.element_id] {
							PathEl::MoveTo(p) => PathEl::MoveTo(*p),
							PathEl::LineTo(p) => PathEl::LineTo(*p),
							PathEl::QuadTo(a1, p) => PathEl::QuadTo(*a1, *p),
							PathEl::CurveTo(a1, a2, p) => PathEl::CurveTo(
								place_mirrored_handle(anchor, if h1_selected { handle_point } else { *a1 }, selected_handle, h1_selected, should_mirror),
								place_mirrored_handle(*p, if h2_selected { handle_point } else { *a2 }, selected_handle, h2_selected, should_mirror),
								*p,
							),
							PathEl::ClosePath => PathEl::ClosePath,
						};
						path_to_update[handle.element_id] = neighbor;
					}
				}
				path_to_update[selected_point.element_id] = selected;
			}
		}
	}

	/// Returns true is any points in this anchor are selected
	pub fn is_selected(&self) -> bool {
		self.points.iter().flatten().any(|pnt| pnt.is_selected())
	}

	/// Set a point to selected by ID
	pub fn set_selected(&mut self, point_id: usize, selected: bool, responses: &mut VecDeque<Message>) -> Option<&mut VectorManipulatorPoint> {
		if let Some(point) = self.points[point_id].as_mut() {
			point.set_selected(selected, responses);
		}
		self.points[point_id].as_mut()
	}

	/// Clear the selected points for this anchor
	pub fn clear_selected_points(&mut self, responses: &mut VecDeque<Message>) {
		for point in self.points.iter_mut().flatten() {
			point.set_selected(false, responses);
		}
	}

	/// Provides the selected points in this anchor
	pub fn selected_points(&self) -> impl Iterator<Item = &VectorManipulatorPoint> {
		self.points.iter().flatten().filter(|pnt| pnt.is_selected())
	}

	/// Provides mutable selected points in this anchor
	pub fn selected_points_mut(&mut self) -> impl Iterator<Item = &mut VectorManipulatorPoint> {
		self.points.iter_mut().flatten().filter(|pnt| pnt.is_selected())
	}

	/// Angle between handles in radians
	pub fn angle_between_handles(&self) -> f64 {
		if let [Some(a1), Some(h1), Some(h2)] = &self.points {
			return (a1.position - h1.position).angle_between(a1.position - h2.position);
		}
		0.0
	}

	/// Returns the opposing handle to the handle provided
	pub fn opposing_handle(&self, handle: &VectorManipulatorPoint) -> &Option<VectorManipulatorPoint> {
		if let Some(point) = &self.points[ManipulatorType::Handle1] {
			if *point == *handle {
				return &self.points[ManipulatorType::Handle2];
			}
		};

		if let Some(point) = &self.points[ManipulatorType::Handle2] {
			if *point == *handle {
				return &self.points[ManipulatorType::Handle1];
			}
		};
		&None
	}

	/// Set the mirroring state
	pub fn set_mirroring(&mut self, mirroring: bool) {
		self.handles_are_mirroring = mirroring;
	}

	/// Return the anchor position or a sane default?
	pub fn anchor_point_position(&self) -> DVec2 {
		if let Some(anchor) = &self.points[ManipulatorType::Anchor] {
			return anchor.position;
		}
		DVec2::ZERO
	}

	/// Helper function to more easily set position of VectorManipulatorPoints
	pub fn set_point_position(&mut self, point_index: usize, position: DVec2) {
		if let Some(point) = &mut self.points[point_index] {
			point.position = position;
		}
	}

	/// Updates the position of the anchor based on the kurbo path
	pub fn place_anchor_overlay(&self, responses: &mut VecDeque<Message>) {
		if let Some(anchor_point) = &self.points[ManipulatorType::Anchor] {
			if let Some(anchor_overlay) = &anchor_point.overlay_path {
				let scale = DVec2::splat(VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE);
				let angle = 0.;
				let translation = (anchor_point.position - (scale / 2.) + BIAS).round();
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
		if let Some(anchor_point) = &self.points[ManipulatorType::Anchor] {
			// Helper function to keep things DRY
			let mut place_handle_and_line = |handle: &VectorManipulatorPoint, line: &Option<Vec<LayerId>>| {
				if let Some(line_overlay) = line {
					let line_vector = anchor_point.position - handle.position;
					let scale = DVec2::splat(line_vector.length());
					let angle = -line_vector.angle_between(DVec2::X);
					let translation = (handle.position + BIAS).round() + DVec2::splat(0.5);
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
					let translation = (handle.position - (scale / 2.) + BIAS).round();
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
		if let Some(anchor_point) = &mut self.points[ManipulatorType::Anchor] {
			if let Some(overlay_path) = &anchor_point.overlay_path {
				responses.push_back(DocumentMessage::Overlays(Operation::DeleteLayer { path: overlay_path.clone() }.into()).into());
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
				responses.push_back(DocumentMessage::Overlays(Operation::DeleteLayer { path: overlay_path.clone() }.into()).into());
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
		if let Some(anchor_point) = &self.points[ManipulatorType::Anchor] {
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
}

/// VectorManipulatorPoint represents any grabbable point, anchor or handle
#[derive(PartialEq, Clone, Debug)]
pub struct VectorManipulatorPoint {
	// The associated position in the BezPath
	pub element_id: usize,
	// The sibling element if this is a handle
	pub position: glam::DVec2,
	// The path to the overlay for this point rendering
	pub overlay_path: Option<Vec<LayerId>>,
	// The type of manipulator this point is
	pub manipulator_type: ManipulatorType,
	// Can be selected
	can_be_selected: bool,
	// Is this point currently selected?
	is_selected: bool,
}

impl Default for VectorManipulatorPoint {
	fn default() -> Self {
		Self {
			element_id: 0,
			position: DVec2::ZERO,
			overlay_path: None,
			manipulator_type: ManipulatorType::Anchor,
			can_be_selected: true,
			is_selected: false,
		}
	}
}

const POINT_STROKE_WIDTH: f32 = 2.0;

impl VectorManipulatorPoint {
	pub fn is_selected(&self) -> bool {
		self.is_selected
	}

	/// Sets if this point is selected and updates the overlay to represent that
	pub fn set_selected(&mut self, selected: bool, responses: &mut VecDeque<Message>) {
		if selected {
			self.set_overlay_style(POINT_STROKE_WIDTH + 1.0, COLOR_ACCENT, COLOR_ACCENT, responses);
		} else {
			self.set_overlay_style(POINT_STROKE_WIDTH, COLOR_ACCENT, Color::WHITE, responses);
		}
		self.is_selected = selected;
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
}
