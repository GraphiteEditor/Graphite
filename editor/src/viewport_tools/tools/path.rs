use crate::consts::{COLOR_ACCENT, SELECTION_THRESHOLD, VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE};
use crate::document::utility_types::{VectorManipulatorAnchor, VectorManipulatorPoint, VectorManipulatorSegment, VectorManipulatorShape};
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
use graphene::layers::style::{self, Fill, Stroke};
use graphene::Operation;

use glam::{DAffine2, DVec2};
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

#[derive(Clone, Debug, Default)]
struct PathToolData {
	anchor_marker_pool: Vec<Vec<LayerId>>,
	handle_marker_pool: Vec<Vec<LayerId>>,
	anchor_handle_line_pool: Vec<Vec<LayerId>>,
	shape_outline_pool: Vec<Vec<LayerId>>,

	manipulation_handler: ManipulationHandler,
	snap_handler: SnapHandler,
}

impl PathToolData {}

#[derive(Clone, Debug, Default)]
struct ManipulationHandler {
	selected_shapes: Vec<VectorManipulatorShape>,
	selected_layer_path: Vec<LayerId>,
	// overlay_path: Vec<LayerId>, // Re-add when overlays are enabled again
	selected_shape: usize,
	selected_shape_elements: Vec<kurbo::PathEl>,

	selected_point: VectorManipulatorPoint,
	selected_anchor: VectorManipulatorAnchor,
}

impl ManipulationHandler {
	// Select the first manipulator within the threshold
	pub fn select_manipulator(&mut self, mouse_position: DVec2, select_threshold: f64, should_mirror: bool) -> bool {
		// TODO convert select_threshold to viewspace, so it remains consistent with zoom level
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
				self.selected_anchor.handle_mirroring = should_mirror;
				return true;
			}
		}
		false
	}

	pub fn selected_shape(&self) -> &VectorManipulatorShape {
		&self.selected_shapes[self.selected_shape]
	}

	pub fn move_selected_to(&mut self, mouse_position: DVec2, should_mirror: bool) -> Operation {
		let mouse_to_shape = self.selected_shape().transform.inverse().transform_point2(mouse_position);
		let mouse_position = Vec2::new(mouse_to_shape.x, mouse_to_shape.y);
		self.selected_anchor.handle_mirroring = should_mirror;
		self.move_point(mouse_position);

		Operation::SetShapePathInViewport {
			path: self.selected_layer_path.clone(),
			bez_path: self.selected_shape_elements.clone().into_iter().collect(),
			transform: self.selected_shape().transform.to_cols_array(),
		}
	}

	fn move_point(&mut self, mouse_position: Vec2) {
		let mouse_position_as_point = mouse_position.to_point();
		let (h1, h2) = &self.selected_anchor.handles;
		let h1_selected = !h1.is_none() && *h1.as_ref().unwrap() == self.selected_point;
		let h2_selected = !h2.is_none() && *h2.as_ref().unwrap() == self.selected_point;

		let place_mirrored_handle = |center: kurbo::Point, original: kurbo::Point, offset_angle: f64, mirror: bool, selected: bool, element_shared_with_anchor: bool| -> kurbo::Point {
			if !selected || !mirror {
				return original;
			}

			// Keep rotational similarity, but distance variable
			let radius = center.distance(original);
			let phi = (center - mouse_position_as_point).atan2();
			let flip = if element_shared_with_anchor { 1.0 } else { -1.0 };
			let angle = phi + ((flip * offset_angle) - std::f64::consts::PI);

			kurbo::Point {
				x: radius * angle.cos(),
				y: radius * angle.sin(),
			} + center.to_vec2()
		};

		// If neither handle is selected, we are dragging an anchor point
		if !(h1_selected || h2_selected) {
			// Move the anchor point and hande on the same path element
			let (selected, point) = match &self.selected_shape_elements[self.selected_anchor.point.element_id] {
				PathEl::MoveTo(p) => (PathEl::MoveTo(mouse_position_as_point), p),
				PathEl::LineTo(p) => (PathEl::LineTo(mouse_position_as_point), p),
				PathEl::QuadTo(a1, p) => (PathEl::QuadTo(*a1 - (*p - mouse_position_as_point), mouse_position_as_point), p),
				PathEl::CurveTo(a1, a2, p) => (PathEl::CurveTo(*a1, *a2 - (*p - mouse_position_as_point), mouse_position_as_point), p),
				PathEl::ClosePath => (PathEl::ClosePath, &mouse_position_as_point),
			};

			// Move the handle on the adjacent path element
			if let Some(handle) = h2 {
				let point_delta = (*point - mouse_position).to_vec2();
				let neighbor = match &self.selected_shape_elements[handle.element_id] {
					PathEl::MoveTo(p) => PathEl::MoveTo(*p),
					PathEl::LineTo(_) => PathEl::LineTo(mouse_position_as_point),
					PathEl::QuadTo(a1, p) => PathEl::QuadTo(*a1 - point_delta, *p),
					PathEl::CurveTo(a1, a2, p) => PathEl::CurveTo(*a1 - point_delta, *a2, *p),
					PathEl::ClosePath => PathEl::ClosePath,
				};
				self.selected_shape_elements[handle.element_id] = neighbor;

				// Handle the invisible point
				if let Some(close_id) = self.selected_anchor.close_element_id {
					self.selected_shape_elements[close_id] = PathEl::MoveTo(mouse_position_as_point);
				}
			}
			self.selected_shape_elements[self.selected_point.element_id] = selected;
		}
		// We are dragging a handle
		else {
			// Move the selected handle
			let (selected, anchor) = match &self.selected_shape_elements[self.selected_point.element_id] {
				PathEl::MoveTo(p) => (PathEl::MoveTo(*p), *p),
				PathEl::LineTo(p) => (PathEl::LineTo(*p), *p),
				PathEl::QuadTo(_, p) => (PathEl::QuadTo(mouse_position_as_point, *p), *p),
				PathEl::CurveTo(a1, a2, p) => (
					PathEl::CurveTo(if h2_selected { mouse_position_as_point } else { *a1 }, if h1_selected { mouse_position_as_point } else { *a2 }, *p),
					*p,
				),
				PathEl::ClosePath => (PathEl::ClosePath, mouse_position_as_point),
			};

			let is_mirroring = self.selected_anchor.handle_mirroring;
			let angle_offset = self.selected_anchor.angle_between_handles();

			// Move the opposing handle on the adjacent path element
			if let Some(handle) = self.selected_anchor.opposing_handle(&self.selected_point) {
				let neighbor = match &self.selected_shape_elements[handle.element_id] {
					PathEl::MoveTo(p) => PathEl::MoveTo(*p),
					PathEl::LineTo(p) => PathEl::LineTo(*p),
					PathEl::QuadTo(a1, p) => PathEl::QuadTo(*a1, *p),
					PathEl::CurveTo(a1, a2, p) => PathEl::CurveTo(
						place_mirrored_handle(anchor, *a1, angle_offset, h1_selected, is_mirroring, true),
						place_mirrored_handle(*p, *a2, angle_offset, h2_selected, is_mirroring, false),
						*p,
					),
					PathEl::ClosePath => PathEl::ClosePath,
				};
				self.selected_shape_elements[handle.element_id] = neighbor;
			}
			self.selected_shape_elements[self.selected_point.element_id] = selected;
		}
	}

	// Todo Move the overlay changes to when selected, not drag start
	// responses.push_back(
	// 	DocumentMessage::Overlay(
	// 		Operation::SetLayerFill {
	// 			path: data.selection.overlay_path.clone(),
	// 			color: COLOR_ACCENT,
	// 		}
	// 		.into(),
	// 	)
	// 	.into(),
	// );

	// TODO Use quadtree or some equivalent spatial locality data structure to improve this to O(log(n))
	// Brute force comparison to determine which handle / anchor we want to select, O(n)
	fn closest_manipulator<'a>(&self, shape: &'a VectorManipulatorShape, pos: glam::DVec2) -> (&'a VectorManipulatorAnchor, &'a VectorManipulatorPoint, f64) {
		let mut closest_anchor: &'a VectorManipulatorAnchor = &shape.points[0];
		let mut closest_point: &'a VectorManipulatorPoint = &shape.points[0].point;
		let mut closest_distance: f64 = f64::MAX; // Not ideal
		for anchor in shape.points.iter() {
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
					data.manipulation_handler.selected_shapes = document.selected_visible_layers_vector_shapes();
					self
				}
				(_, DocumentIsDirty) => {
					let (mut anchor_i, mut handle_i, mut line_i, mut shape_i) = (0, 0, 0, 0);

					// Update the shapes by reference
					document.update_selected_vector_shapes(&mut data.manipulation_handler.selected_shapes);

					// Grow the overlay pools by the shortfall, if any
					let (total_anchors, total_handles, total_anchor_handle_lines) = calculate_total_overlays_per_type(&data.manipulation_handler.selected_shapes);
					let total_shapes = data.manipulation_handler.selected_shapes.len();
					grow_overlay_pool_entries(&mut data.shape_outline_pool, total_shapes, add_shape_outline, responses);
					grow_overlay_pool_entries(&mut data.anchor_handle_line_pool, total_anchor_handle_lines, add_anchor_handle_line, responses);
					grow_overlay_pool_entries(&mut data.anchor_marker_pool, total_anchors, add_anchor_marker, responses);
					grow_overlay_pool_entries(&mut data.handle_marker_pool, total_handles, add_handle_marker, responses);

					// Helps push values that end in approximately half, plus or minus some floating point imprecision, towards the same side of the round() function
					const BIAS: f64 = 0.0001;

					// Draw the overlays for each shape
					for shape_to_draw in &data.manipulation_handler.selected_shapes {
						let shape_layer_path = &data.shape_outline_pool[shape_i];

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
						shape_i += 1;

						let segment = shape_manipulator_points(shape_to_draw);

						// Draw the line connecting the anchor with handle for cubic and quadratic bezier segments
						for anchor_handle_line in segment.anchor_handle_lines {
							let marker = &data.anchor_handle_line_pool[line_i];

							let line_vector = anchor_handle_line.0 - anchor_handle_line.1;

							let scale = DVec2::splat(line_vector.length());
							let angle = -line_vector.angle_between(DVec2::X);
							let translation = (anchor_handle_line.1 + BIAS).round() + DVec2::splat(0.5);
							let transform = DAffine2::from_scale_angle_translation(scale, angle, translation).to_cols_array();

							responses.push_back(DocumentMessage::Overlays(Operation::SetLayerTransformInViewport { path: marker.clone(), transform }.into()).into());
							responses.push_back(DocumentMessage::Overlays(Operation::SetLayerVisibility { path: marker.clone(), visible: true }.into()).into());

							line_i += 1;
						}

						// Draw the draggable square points on the end of every line segment or bezier curve segment
						for anchor in segment.anchors {
							let scale = DVec2::splat(VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE);
							let angle = 0.;
							let translation = (anchor - (scale / 2.) + BIAS).round();
							let transform = DAffine2::from_scale_angle_translation(scale, angle, translation).to_cols_array();

							let marker = &data.anchor_marker_pool[anchor_i];
							responses.push_back(DocumentMessage::Overlays(Operation::SetLayerTransformInViewport { path: marker.clone(), transform }.into()).into());
							responses.push_back(DocumentMessage::Overlays(Operation::SetLayerVisibility { path: marker.clone(), visible: true }.into()).into());

							anchor_i += 1;
						}

						// Draw the draggable handle for cubic and quadratic bezier segments
						for handle in segment.handles {
							let marker = &data.handle_marker_pool[handle_i];

							let scale = DVec2::splat(VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE);
							let angle = 0.;
							let translation = (handle - (scale / 2.) + BIAS).round();
							let transform = DAffine2::from_scale_angle_translation(scale, angle, translation).to_cols_array();

							responses.push_back(DocumentMessage::Overlays(Operation::SetLayerTransformInViewport { path: marker.clone(), transform }.into()).into());
							responses.push_back(DocumentMessage::Overlays(Operation::SetLayerVisibility { path: marker.clone(), visible: true }.into()).into());

							handle_i += 1;
						}
					}

					// Hide the remaining pooled overlays
					for i in anchor_i..data.anchor_marker_pool.len() {
						let marker = data.anchor_marker_pool[i].clone();
						responses.push_back(DocumentMessage::Overlays(Operation::SetLayerVisibility { path: marker, visible: false }.into()).into());
					}
					for i in handle_i..data.handle_marker_pool.len() {
						let marker = data.handle_marker_pool[i].clone();
						responses.push_back(DocumentMessage::Overlays(Operation::SetLayerVisibility { path: marker, visible: false }.into()).into());
					}
					for i in line_i..data.anchor_handle_line_pool.len() {
						let line = data.anchor_handle_line_pool[i].clone();
						responses.push_back(DocumentMessage::Overlays(Operation::SetLayerVisibility { path: line, visible: false }.into()).into());
					}
					for i in shape_i..data.shape_outline_pool.len() {
						let shape_i = data.shape_outline_pool[i].clone();
						responses.push_back(DocumentMessage::Overlays(Operation::SetLayerVisibility { path: shape_i, visible: false }.into()).into());
					}

					self
				}
				(_, DragStart) => {
					let should_not_mirror = input.keyboard.get(Key::KeyAlt as usize);
					// Select the first point within the threshold (in pixels)
					let select_threshold = SELECTION_THRESHOLD;
					if data.manipulation_handler.select_manipulator(input.mouse.position, select_threshold, !should_not_mirror) {
						responses.push_back(DocumentMessage::StartTransaction.into());
						data.snap_handler.start_snap(document, document.visible_layers());
						let snap_points = data
							.manipulation_handler
							.selected_shapes
							.iter()
							.flat_map(|shape| shape.points.iter().map(|anchor| anchor.point.position))
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
					// Todo Move the overlay changes to when deselected, not drag stop
					// let style = PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 2.0)), Some(Fill::new(Color::WHITE)));
					// responses.push_back(
					// 	DocumentMessage::Overlay(
					// 		Operation::SetLayerStyle {
					// 			path: data.selector.overlay_path.clone(),
					// 			style,
					// 		}
					// 		.into(),
					// 	)
					// 	.into(),
					// );
					data.snap_handler.cleanup(responses);
					Ready
				}
				(_, Abort) => {
					// Destory the overlay layer pools
					while let Some(layer) = data.anchor_marker_pool.pop() {
						responses.push_back(DocumentMessage::Overlays(Operation::DeleteLayer { path: layer }.into()).into());
					}
					while let Some(layer) = data.handle_marker_pool.pop() {
						responses.push_back(DocumentMessage::Overlays(Operation::DeleteLayer { path: layer }.into()).into());
					}
					while let Some(layer) = data.anchor_handle_line_pool.pop() {
						responses.push_back(DocumentMessage::Overlays(Operation::DeleteLayer { path: layer }.into()).into());
					}
					while let Some(layer) = data.shape_outline_pool.pop() {
						responses.push_back(DocumentMessage::Overlays(Operation::DeleteLayer { path: layer }.into()).into());
					}

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
				label: String::from("Handle Mirroring Disabled"),
				plus: false,
			}])]),
		};

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default }.into());
	}
}

struct VectorManipulatorTypes {
	anchors: Vec<glam::DVec2>,
	handles: Vec<glam::DVec2>,
	anchor_handle_lines: Vec<(glam::DVec2, glam::DVec2)>,
}

fn shape_manipulator_points(shape: &VectorManipulatorShape) -> VectorManipulatorTypes {
	// TODO: Performance can be improved by using three iterators (calling `.iter()` for each of the three) instead of a vector, achievable with some file restructuring
	let initial_counts = calculate_shape_overlays_per_type(shape);
	let mut result = VectorManipulatorTypes {
		anchors: Vec::with_capacity(initial_counts.0),
		handles: Vec::with_capacity(initial_counts.1),
		anchor_handle_lines: Vec::with_capacity(initial_counts.2),
	};

	for (i, segment) in shape.segments.iter().enumerate() {
		// An open shape needs an extra point, which is part of the first segment (when `i` is 0)
		let include_start_and_end = !shape.closed && i == 0;

		match segment {
			VectorManipulatorSegment::Line(a1, a2) => {
				result.anchors.extend(if include_start_and_end { vec![*a1, *a2] } else { vec![*a2] });
			}
			VectorManipulatorSegment::Quad(a1, h1, a2) => {
				result.anchors.extend(if include_start_and_end { vec![*a1, *a2] } else { vec![*a2] });
				result.handles.extend(vec![*h1]);
				result.anchor_handle_lines.extend(vec![(*h1, *a1)]);
			}
			VectorManipulatorSegment::Cubic(a1, h1, h2, a2) => {
				result.anchors.extend(if include_start_and_end { vec![*a1, *a2] } else { vec![*a2] });
				result.handles.extend(vec![*h1, *h2]);
				result.anchor_handle_lines.extend(vec![(*h1, *a1), (*h2, *a2)]);
			}
		};
	}

	result
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

fn grow_overlay_pool_entries<F>(pool: &mut Vec<Vec<LayerId>>, total: usize, add_overlay_function: F, responses: &mut VecDeque<Message>)
where
	F: Fn(&mut VecDeque<Message>) -> Vec<LayerId>,
{
	if pool.len() < total {
		let additional = total - pool.len();

		pool.reserve(additional);

		for _ in 0..additional {
			let marker = add_overlay_function(responses);
			pool.push(marker);
		}
	}
}

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

fn add_anchor_handle_line(responses: &mut VecDeque<Message>) -> Vec<LayerId> {
	let layer_path = vec![generate_uuid()];
	let operation = Operation::AddOverlayLine {
		path: layer_path.clone(),
		transform: DAffine2::IDENTITY.to_cols_array(),
		style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), Some(Fill::none())),
	};
	responses.push_back(DocumentMessage::Overlays(operation.into()).into());

	layer_path
}

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
