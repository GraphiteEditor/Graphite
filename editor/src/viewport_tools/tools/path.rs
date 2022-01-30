use crate::consts::{COLOR_ACCENT, SELECTION_THRESHOLD, VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE};
use crate::document::utility_types::{OverlayPooler, VectorManipulatorAnchor, VectorManipulatorPoint, VectorManipulatorSegment, VectorManipulatorShape};
use crate::document::DocumentMessageHandler;
use crate::frontend::utility_types::MouseCursorIcon;
use crate::input::keyboard::{Key, MouseMotion};
use crate::input::InputPreprocessorMessageHandler;
use crate::message_prelude::*;
use crate::misc::{HintData, HintGroup, HintInfo, KeysGroup};
use crate::viewport_tools::snapping::SnapHandler;
use crate::viewport_tools::tool::{DocumentToolData, Fsm, ToolActionHandlerData};

use graphene::color::Color;
use graphene::intersection::Quad;
use graphene::Operation;

use glam::{DAffine2, DVec2};
use graphene::layers::style::{self, Fill, Stroke};
use kurbo::{BezPath, PathEl, Vec2};
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct Path {
	fsm_state: PathToolFsmState,
	data: PathToolData,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Path)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum PathMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,
	#[remain::unsorted]
	DocumentIsDirty,
	#[remain::unsorted]
	SelectionChanged,

	// Tool-specific messages
	DragStart,
	DragStop,
	PointerMove,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Path {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		if action == ToolMessage::UpdateHints {
			self.fsm_state.update_hints(responses);
			return;
		}

		if action == ToolMessage::UpdateCursor {
			self.fsm_state.update_cursor(responses);
			return;
		}

		let new_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, data.2, responses);

		if self.fsm_state != new_state {
			self.fsm_state = new_state;
			self.fsm_state.update_hints(responses);
			self.fsm_state.update_cursor(responses);
		}
	}

	// Different actions depending on state may be wanted:
	fn actions(&self) -> ActionList {
		use PathToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(PathMessageDiscriminant; DragStart),
			Dragging => actions!(PathMessageDiscriminant; DragStop, PointerMove),
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PathToolFsmState {
	Ready,
	Dragging,
}

impl Default for PathToolFsmState {
	fn default() -> Self {
		PathToolFsmState::Ready
	}
}

#[derive(Default)]
struct PathToolData {
	overlay_pooler: OverlayPooler,
	manipulation_handler: ManipulationHandler,
	snap_handler: SnapHandler,

	overlay_pooler_initialized: bool,
}

enum OverlayPoolType {
	Shape = 0,
	Anchor = 1,
	Handle = 2,
	HandleLine = 3,
}

impl PathToolData {
	/// Refresh the pool and grow if needed
	pub fn setup_pools(&mut self, selected_shapes: &[VectorManipulatorShape], responses: &mut VecDeque<Message>) {
		let shapes_capacity = selected_shapes.len();
		let (anchors_capacity, handles_capacity, handle_lines_capacity) = calculate_total_overlays_per_type(selected_shapes);

		// Add shape pool and callback
		self.overlay_pooler.add_overlay_pool(OverlayPoolType::Shape as usize, shapes_capacity, responses, add_shape_outline);
		fn add_shape_outline(responses: &mut VecDeque<Message>) -> Vec<LayerId> {
			let layer_path = vec![generate_uuid()];
			let operation = Operation::AddOverlayShape {
				path: layer_path.clone(),
				bez_path: BezPath::default(),
				style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), Some(Fill::none())),
				closed: false,
			};
			responses.push_back(DocumentMessage::Overlays(operation.into()).into());
			layer_path
		}

		// Add anchor pool and callback
		self.overlay_pooler.add_overlay_pool(OverlayPoolType::Anchor as usize, anchors_capacity, responses, add_anchor_marker);
		fn add_anchor_marker(responses: &mut VecDeque<Message>) -> Vec<LayerId> {
			let layer_path = vec![generate_uuid()];
			let operation = Operation::AddOverlayRect {
				path: layer_path.clone(),
				transform: DAffine2::IDENTITY.to_cols_array(),
				style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 2.0)), Some(Fill::new(Color::WHITE))),
			};
			responses.push_back(DocumentMessage::Overlays(operation.into()).into());
			layer_path
		}

		// Add handle pool and callback
		self.overlay_pooler.add_overlay_pool(OverlayPoolType::Handle as usize, handles_capacity, responses, add_handle_marker);
		fn add_handle_marker(responses: &mut VecDeque<Message>) -> Vec<LayerId> {
			let layer_path = vec![generate_uuid()];
			let operation = Operation::AddOverlayEllipse {
				path: layer_path.clone(),
				transform: DAffine2::IDENTITY.to_cols_array(),
				style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 2.0)), Some(Fill::new(Color::WHITE))),
			};
			responses.push_back(DocumentMessage::Overlays(operation.into()).into());
			layer_path
		}

		// Add handle line pool and callback
		self.overlay_pooler
			.add_overlay_pool(OverlayPoolType::HandleLine as usize, handle_lines_capacity, responses, add_handle_line);
		fn add_handle_line(responses: &mut VecDeque<Message>) -> Vec<LayerId> {
			let layer_path = vec![generate_uuid()];
			let operation = Operation::AddOverlayLine {
				path: layer_path.clone(),
				transform: DAffine2::IDENTITY.to_cols_array(),
				style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), Some(Fill::none())),
			};
			responses.push_back(DocumentMessage::Overlays(operation.into()).into());
			layer_path
		}
	}
}

#[derive(Clone, Debug, Default)]
struct ManipulationHandler {
	// The selected shapes, the cloned path and the kurbo PathElements
	selected_shapes: Vec<VectorManipulatorShape>,
	selected_layer_path: Vec<LayerId>,
	selected_shape_elements: Vec<kurbo::PathEl>,
	// The shape that had a point selected from most recently
	selected_shape: usize,
	// This can represent any draggable point anchor or handle
	selected_point: VectorManipulatorPoint,
	// This is specifically the related anchor, even if we have a handle selected
	selected_anchor: VectorManipulatorAnchor,
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
	// Brute force comparison to determine which handle / anchor we want to select, O(n)
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

impl Fsm for PathToolFsmState {
	type ToolData = PathToolData;

	fn transition(
		self,
		event: ToolMessage,
		document: &DocumentMessageHandler,
		_tool_data: &DocumentToolData,
		data: &mut Self::ToolData,
		input: &InputPreprocessorMessageHandler,
		responses: &mut VecDeque<Message>,
	) -> Self {
		if let ToolMessage::Path(event) = event {
			use PathMessage::*;
			use PathToolFsmState::*;

			match (self, event) {
				(_, SelectionChanged) => {
					let selected_shapes = document.selected_visible_layers_vector_shapes();
					if !data.overlay_pooler_initialized {
						data.setup_pools(&selected_shapes, responses);
						data.overlay_pooler_initialized = true;
					}
					data.manipulation_handler.selected_shapes = selected_shapes;
					self
				}
				(_, DocumentIsDirty) => {
					let selected_shapes = document.selected_visible_layers_vector_shapes();
					if !data.overlay_pooler_initialized {
						data.setup_pools(&selected_shapes, responses);
						data.overlay_pooler_initialized = true;
					}
					data.manipulation_handler.selected_shapes = selected_shapes;

					// Update the VectorManipulator structures by reference. They need to match the kurbo data
					for shape in &mut data.manipulation_handler.selected_shapes {
						shape.update_shape(document);
					}

					// Recycle all overlays
					data.overlay_pooler.recycle_all_channels();

					// Helps push values that end in approximately half, plus or minus some floating point imprecision, towards the same side of the round() function
					const BIAS: f64 = 0.0001;

					// Draw the overlays for each shape
					for shape_to_draw in &data.manipulation_handler.selected_shapes {
						let (shape_layer_path, _) = &data.overlay_pooler.create_from_channel(OverlayPoolType::Shape as usize, responses);

						responses.push_back(
							DocumentMessage::Overlays(
								Operation::SetShapePathInViewport {
									path: shape_layer_path.clone(),
									bez_path: shape_to_draw.path.clone(),
									transform: shape_to_draw.transform.to_cols_array(),
								}
								.into(),
							)
							.into(),
						);
						responses.push_back(
							DocumentMessage::Overlays(
								Operation::SetLayerVisibility {
									path: shape_layer_path.clone(),
									visible: true,
								}
								.into(),
							)
							.into(),
						);

						let anchors = &shape_to_draw.anchors;

						// Draw the line connecting the anchor with handle for cubic and quadratic bezier segments
						for anchor in anchors {
							let (handle1, handle2) = anchor.handles;
							let mut draw_connector = |position: DVec2| {
								let (marker, _) = &data.overlay_pooler.create_from_channel(OverlayPoolType::HandleLine as usize, responses);
								let line_vector = anchor.point.position - position;
								let scale = DVec2::splat(line_vector.length());
								let angle = -line_vector.angle_between(DVec2::X);
								let translation = (position + BIAS).round() + DVec2::splat(0.5);
								let transform = DAffine2::from_scale_angle_translation(scale, angle, translation).to_cols_array();

								responses.push_back(DocumentMessage::Overlays(Operation::SetLayerTransformInViewport { path: marker.clone(), transform }.into()).into());
								responses.push_back(DocumentMessage::Overlays(Operation::SetLayerVisibility { path: marker.clone(), visible: true }.into()).into());
							};

							if let Some(handle) = handle1 {
								draw_connector(handle.position);
							}

							if let Some(handle) = handle2 {
								draw_connector(handle.position);
							}
						}

						// Draw the draggable square points on the end of every line segment or bezier curve segment
						for anchor in anchors {
							let scale = DVec2::splat(VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE);
							let angle = 0.;
							let translation = (anchor.point.position - (scale / 2.) + BIAS).round();
							let transform = DAffine2::from_scale_angle_translation(scale, angle, translation).to_cols_array();

							let (marker, _) = &data.overlay_pooler.create_from_channel(OverlayPoolType::Anchor as usize, responses);
							responses.push_back(DocumentMessage::Overlays(Operation::SetLayerTransformInViewport { path: marker.clone(), transform }.into()).into());
							responses.push_back(DocumentMessage::Overlays(Operation::SetLayerVisibility { path: marker.clone(), visible: true }.into()).into());
						}

						// Draw the draggable handle for cubic and quadratic bezier segments
						for anchor in anchors {
							let (handle1, handle2) = anchor.handles;

							let mut draw_handle = |position: DVec2| {
								let (marker, _) = &data.overlay_pooler.create_from_channel(OverlayPoolType::Handle as usize, responses);
								let scale = DVec2::splat(VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE);
								let angle = 0.;
								let translation = (position - (scale / 2.) + BIAS).round();
								let transform = DAffine2::from_scale_angle_translation(scale, angle, translation).to_cols_array();

								responses.push_back(DocumentMessage::Overlays(Operation::SetLayerTransformInViewport { path: marker.clone(), transform }.into()).into());
								responses.push_back(DocumentMessage::Overlays(Operation::SetLayerVisibility { path: marker.clone(), visible: true }.into()).into());
							};

							if let Some(handle) = handle1 {
								draw_handle(handle.position);
							}

							if let Some(handle) = handle2 {
								draw_handle(handle.position);
							}
						}
					}
					data.overlay_pooler.hide_all_extras(responses);

					self
				}
				(_, DragStart) => {
					// Select the first point within the threshold (in pixels)
					let select_threshold = SELECTION_THRESHOLD;
					if data.manipulation_handler.select_manipulator(input.mouse.position, select_threshold) {
						responses.push_back(DocumentMessage::StartTransaction.into());
						data.snap_handler.start_snap(document, document.visible_layers());
						let snap_points = data
							.manipulation_handler
							.selected_shapes
							.iter()
							.flat_map(|shape| shape.anchors.iter().map(|anchor| anchor.point.position))
							.collect();
						data.snap_handler.add_snap_points(document, snap_points);
						Dragging
					} else {
						// Select shapes directly under our mouse
						let intersection = document.graphene_document.intersects_quad_root(Quad::from_box([input.mouse.position, input.mouse.position]));
						if !intersection.is_empty() {
							responses.push_back(
								DocumentMessage::SetSelectedLayers {
									replacement_selected_layers: intersection,
								}
								.into(),
							);
						}
						Ready
					}
				}
				(Dragging, PointerMove) => {
					let should_not_mirror = input.keyboard.get(Key::KeyAlt as usize);

					// Move the selected points by the mouse position
					let snapped_position = data.snap_handler.snap_position(responses, input.viewport_bounds.size(), document, input.mouse.position);
					let move_operation = data.manipulation_handler.move_selected_to(snapped_position, !should_not_mirror);
					responses.push_back(move_operation.into());
					Dragging
				}
				(_, DragStop) => {
					data.snap_handler.cleanup(responses);
					Ready
				}
				(_, Abort) => {
					// Destory the overlay layer pools
					data.overlay_pooler_initialized = false;
					data.overlay_pooler.cleanup_all_channels(responses);
					Ready
				}
				(_, PointerMove) => self,
			}
		} else {
			self
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			PathToolFsmState::Ready => HintData(vec![
				HintGroup(vec![
					HintInfo {
						key_groups: vec![],
						mouse: Some(MouseMotion::Lmb),
						label: String::from("Select Point"),
						plus: false,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyShift])],
						mouse: None,
						label: String::from("Add/Remove Point (coming soon)"),
						plus: true,
					},
				]),
				HintGroup(vec![HintInfo {
					key_groups: vec![],
					mouse: Some(MouseMotion::LmbDrag),
					label: String::from("Drag Selected"),
					plus: false,
				}]),
				HintGroup(vec![
					HintInfo {
						key_groups: vec![
							KeysGroup(vec![Key::KeyArrowUp]),
							KeysGroup(vec![Key::KeyArrowRight]),
							KeysGroup(vec![Key::KeyArrowDown]),
							KeysGroup(vec![Key::KeyArrowLeft]),
						],
						mouse: None,
						label: String::from("Nudge Selected (coming soon)"),
						plus: false,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyShift])],
						mouse: None,
						label: String::from("Big Increment Nudge"),
						plus: true,
					},
				]),
				HintGroup(vec![
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyG])],
						mouse: None,
						label: String::from("Grab Selected (coming soon)"),
						plus: false,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyR])],
						mouse: None,
						label: String::from("Rotate Selected (coming soon)"),
						plus: false,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyS])],
						mouse: None,
						label: String::from("Scale Selected (coming soon)"),
						plus: false,
					},
				]),
			]),
			PathToolFsmState::Dragging => HintData(vec![HintGroup(vec![HintInfo {
				key_groups: vec![KeysGroup(vec![Key::KeyAlt])],
				mouse: None,
				label: String::from("Handle Mirroring Toggle"),
				plus: false,
			}])]),
		};

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default }.into());
	}
}

fn calculate_total_overlays_per_type(shapes: &[VectorManipulatorShape]) -> (usize, usize, usize) {
	shapes.iter().fold((0, 0, 0), |acc, shape| {
		let counts = calculate_shape_overlays_per_type(shape);
		(acc.0 + counts.0, acc.1 + counts.1, acc.2 + counts.2)
	})
}

fn calculate_shape_overlays_per_type(shape: &VectorManipulatorShape) -> (usize, usize, usize) {
	let (mut total_anchors, mut total_handles, mut total_anchor_handle_lines) = (0, 0, 0);

	for segment in &shape.segments {
		let (anchors, handles, anchor_handle_lines) = match segment {
			VectorManipulatorSegment::Line(_, _) => (1, 0, 0),
			VectorManipulatorSegment::Quad(_, _, _) => (1, 1, 1),
			VectorManipulatorSegment::Cubic(_, _, _, _) => (1, 2, 2),
		};
		total_anchors += anchors;
		total_handles += handles;
		total_anchor_handle_lines += anchor_handle_lines;
	}

	// A non-closed shape does not reuse the start and end point, so there is one extra
	if !shape.closed {
		total_anchors += 1;
	}

	(total_anchors, total_handles, total_anchor_handle_lines)
}
