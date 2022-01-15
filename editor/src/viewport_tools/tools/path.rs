use crate::consts::{COLOR_ACCENT, VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE};
use crate::document::utility_types::{VectorManipulatorSegment, VectorManipulatorShape};
use crate::document::DocumentMessageHandler;
use crate::frontend::utility_types::FrontendMouseCursor;
use crate::input::keyboard::{Key, MouseMotion};
use crate::input::InputPreprocessorMessageHandler;
use crate::message_prelude::*;
use crate::misc::{HintData, HintGroup, HintInfo, KeysGroup};
use crate::viewport_tools::tool::{DocumentToolData, Fsm, ToolActionHandlerData};

use graphene::color::Color;
use graphene::layers::style::{self, Fill, PathStyle, Stroke};
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
	Abort,
	DocumentIsDirty,

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
	selected_shapes: Vec<VectorManipulatorShape>,
	selection: PathToolSelection,
}

impl PathToolData {}

#[derive(Clone, Debug, Default)]
struct PathToolSelection {
	closest_layer_path: Vec<LayerId>,
	closest_shape_id: usize,
	overlay_path: Vec<LayerId>,
	bez_path_elements: Vec<kurbo::PathEl>,
	bez_segment_id: usize,
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
				(_, DocumentIsDirty) => {
					let (mut anchor_i, mut handle_i, mut line_i, mut shape_i) = (0, 0, 0, 0);

					let shapes_to_draw = document.selected_visible_layers_vector_points();
					// Grow the overlay pools by the shortfall, if any
					let (total_anchors, total_handles, total_anchor_handle_lines) = calculate_total_overlays_per_type(&shapes_to_draw);
					let total_shapes = shapes_to_draw.len();
					grow_overlay_pool_entries(&mut data.shape_outline_pool, total_shapes, add_shape_outline, responses);
					grow_overlay_pool_entries(&mut data.anchor_handle_line_pool, total_anchor_handle_lines, add_anchor_handle_line, responses);
					grow_overlay_pool_entries(&mut data.anchor_marker_pool, total_anchors, add_anchor_marker, responses);
					grow_overlay_pool_entries(&mut data.handle_marker_pool, total_handles, add_handle_marker, responses);

					// Helps push values that end in approximately half, plus or minus some floating point imprecision, towards the same side of the round() function
					const BIAS: f64 = 0.0001;

					// Draw the overlays for each shape
					for shape_to_draw in &shapes_to_draw {
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
					// todo: DRY refactor (this arm is very similar to the (_, RedrawOverlay) arm)

					let mouse_pos = input.mouse.position;
					let mut points = Vec::new();

					let (mut anchor_i, mut handle_i, _line_i, _shape_i) = (0, 0, 0, 0);
					let shapes_to_draw = document.selected_visible_layers_vector_points();
					let (total_anchors, total_handles, _total_anchor_handle_lines) = calculate_total_overlays_per_type(&shapes_to_draw);
					grow_overlay_pool_entries(&mut data.anchor_marker_pool, total_anchors, add_anchor_marker, responses);
					grow_overlay_pool_entries(&mut data.handle_marker_pool, total_handles, add_handle_marker, responses);

					#[derive(Debug)]
					enum PointType {
						Anchor { anchor_i: usize, layer_path: Vec<LayerId>, shape_offset: usize },
						Handle { handle_i: usize, layer_path: Vec<LayerId>, shape_offset: usize },
					}
					#[derive(Debug)]
					struct Point {
						point_type: PointType,
						mouse_proximity: f64,
					}

					impl Point {
						fn new(_position: DVec2, point_type: PointType, mouse_proximity: f64) -> Self {
							Self { point_type, mouse_proximity }
						}
					}

					// TODO simplify the following block
					let select_threshold = 6.;
					let select_threshold_squared = select_threshold * select_threshold;

					for (shape_offset, shape_to_draw) in shapes_to_draw.iter().enumerate() {
						let segment = shape_manipulator_points(shape_to_draw);

						for anchor in segment.anchors {
							let d2 = mouse_pos.distance_squared(anchor);
							if d2 < select_threshold_squared {
								points.push(Point::new(
									anchor,
									PointType::Anchor {
										anchor_i,
										layer_path: shape_to_draw.layer_path.clone(),
										shape_offset,
									},
									d2,
								));
							}
							anchor_i += 1;
						}

						for (_, handle) in segment.handles.into_iter().enumerate() {
							let d2 = mouse_pos.distance_squared(handle);
							if d2 < select_threshold_squared {
								points.push(Point::new(
									handle,
									PointType::Handle {
										handle_i,
										layer_path: shape_to_draw.layer_path.clone(),
										shape_offset,
									},
									d2,
								));
							}
							handle_i += 1;
						}
					}

					points.sort_by(|a, b| a.mouse_proximity.partial_cmp(&b.mouse_proximity).unwrap_or(std::cmp::Ordering::Equal));
					let closest_point_within_click_threshold = points.first();

					if let Some(point) = closest_point_within_click_threshold {
						let path = match point.point_type {
							PointType::Anchor {
								anchor_i,
								ref layer_path,
								shape_offset,
							} => {
								data.selected_shapes = shapes_to_draw;
								let shape = &data.selected_shapes[shape_offset];
								let path = shape.path.clone();
								let bez: Vec<PathEl> = (&path).into_iter().collect();
								let transformed = shape.transform.inverse().transform_point2(input.mouse.position);
								data.selection.bez_segment_id = closest_anchor(&bez, Vec2::new(transformed.x, transformed.y));
								data.selection.bez_path_elements = bez;
								data.selection.closest_layer_path = layer_path.clone();
								data.selection.closest_shape_id = shape_offset;
								data.anchor_marker_pool[anchor_i].clone()
							}
							PointType::Handle {
								handle_i,
								ref layer_path,
								shape_offset,
							} => {
								// TODO make this work for the handles, right now just selects the anchors
								data.selected_shapes = shapes_to_draw;
								let shape = &data.selected_shapes[shape_offset];
								let path = shape.path.clone();
								let bez: Vec<PathEl> = (&path).into_iter().collect();
								let transformed = shape.transform.inverse().transform_point2(input.mouse.position);
								data.selection.bez_segment_id = closest_anchor(&bez, Vec2::new(transformed.x, transformed.y));
								data.selection.bez_path_elements = bez;
								data.selection.closest_layer_path = layer_path.clone();
								data.selection.closest_shape_id = shape_offset;
								data.handle_marker_pool[handle_i].clone()
							}
						};

						data.selection.overlay_path = path;
						responses.push_back(
							DocumentMessage::Overlays(
								Operation::SetLayerFill {
									path: data.selection.overlay_path.clone(),
									color: COLOR_ACCENT,
								}
								.into(),
							)
							.into(),
						);
						Dragging
					} else {
						Ready
					}
				}
				(Dragging, PointerMove) => {
					let shape = &data.selected_shapes[data.selection.closest_shape_id];
					let transformed = shape.transform.inverse().transform_point2(input.mouse.position);
					let delta: Vec2 = Vec2::new(transformed.x, transformed.y);
					let replacement = match &data.selection.bez_path_elements[data.selection.bez_segment_id] {
						PathEl::MoveTo(_) => PathEl::MoveTo(delta.to_point()),
						PathEl::LineTo(_) => PathEl::LineTo(delta.to_point()),
						PathEl::QuadTo(a1, _) => PathEl::QuadTo(*a1, delta.to_point()),
						PathEl::CurveTo(a1, a2, _) => PathEl::CurveTo(*a1, *a2, delta.to_point()),
						PathEl::ClosePath => unreachable!(),
					};
					data.selection.bez_path_elements[data.selection.bez_segment_id] = replacement;

					responses.push_back(
						Operation::SetShapePathInViewport {
							path: data.selection.closest_layer_path.clone(),
							bez_path: data.selection.bez_path_elements.clone().into_iter().collect(),
							transform: shape.transform.to_cols_array(),
						}
						.into(),
					);

					Dragging
				}
				(_, PointerMove) => self,
				(_, DragStop) => {
					let style = PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 2.0)), Some(Fill::new(Color::WHITE)));
					responses.push_back(
						DocumentMessage::Overlays(
							Operation::SetLayerStyle {
								path: data.selection.overlay_path.clone(),
								style,
							}
							.into(),
						)
						.into(),
					);
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
						label: String::from("Select Point (coming soon)"),
						plus: false,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyShift])],
						mouse: None,
						label: String::from("Add/Remove Point"),
						plus: true,
					},
				]),
				HintGroup(vec![HintInfo {
					key_groups: vec![],
					mouse: Some(MouseMotion::LmbDrag),
					label: String::from("Drag Selected (coming soon)"),
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
			PathToolFsmState::Dragging => HintData(vec![]),
		};

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(FrontendMessage::DisplayMouseCursor { cursor: FrontendMouseCursor::Default }.into());
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

// Brute force comparison to determine which path element we want to select
fn closest_anchor(bez: &[kurbo::PathEl], pos: kurbo::Vec2) -> usize {
	let mut closest: usize = 0;
	let mut closest_distance: f64 = f64::MAX;
	for (i, el) in bez.iter().enumerate() {
		let p = match el {
			kurbo::PathEl::MoveTo(p) => Some(p.to_vec2()),
			kurbo::PathEl::LineTo(p) => Some(p.to_vec2()),
			kurbo::PathEl::QuadTo(_, p) => Some(p.to_vec2()),
			kurbo::PathEl::CurveTo(_, _, p) => Some(p.to_vec2()),
			kurbo::PathEl::ClosePath => None,
		};
		if p.is_none() {
			continue;
		}
		let distance_squared = (p.unwrap() - pos).hypot2();
		if distance_squared < closest_distance {
			closest_distance = distance_squared;
			closest = i;
		}
	}
	closest
}
