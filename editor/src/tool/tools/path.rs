use crate::consts::COLOR_ACCENT;
use crate::consts::VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE;
use crate::document::DocumentMessageHandler;
use crate::document::VectorManipulatorSegment;
use crate::document::VectorManipulatorShape;
use crate::input::keyboard::{Key, MouseMotion};
use crate::input::InputPreprocessor;
use crate::message_prelude::*;
use crate::misc::{HintData, HintGroup, HintInfo, KeysGroup};
use crate::tool::ToolActionHandlerData;
use crate::tool::{DocumentToolData, Fsm};
use glam::{DAffine2, DVec2};
use graphene::color::Color;
use graphene::layers::style;
use graphene::layers::style::Fill;
use graphene::layers::style::Stroke;
use graphene::Operation;
use kurbo::BezPath;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct Path {
	fsm_state: PathToolFsmState,
	data: PathToolData,
}

#[impl_message(Message, ToolMessage, Path)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum PathMessage {
	MouseDown,
	RedrawOverlay,
	Abort,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Path {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		if action == ToolMessage::UpdateHints {
			self.fsm_state.update_hints(responses);
			return;
		}

		let new_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, data.2, responses);

		if self.fsm_state != new_state {
			self.fsm_state = new_state;
			self.fsm_state.update_hints(responses);
		}
	}

	// different actions depending on state may be wanted:
	// fn actions(&self) -> ActionList {
	// 	use PathToolFsmState::*;
	// 	match self.fsm_state {
	// 		Ready => actions!(PathMessageDiscriminant; MouseDown),
	// 	}
	// }
	advertise_actions!(PathMessageDiscriminant; MouseDown);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PathToolFsmState {
	Ready,
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
}

impl PathToolData {}

impl Fsm for PathToolFsmState {
	type ToolData = PathToolData;

	fn transition(
		self,
		event: ToolMessage,
		document: &DocumentMessageHandler,
		_tool_data: &DocumentToolData,
		data: &mut Self::ToolData,
		input: &InputPreprocessor,
		responses: &mut VecDeque<Message>,
	) -> Self {
		if let ToolMessage::Path(event) = event {
			use PathMessage::*;
			use PathToolFsmState::*;
			match (self, event) {
				(_, RedrawOverlay) => {
					let (mut anchor_i, mut handle_i, mut line_i, mut shape_i) = (0, 0, 0, 0);

					let shapes_to_draw = document.selected_layers_vector_points();
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
							Operation::SetShapePathInViewport {
								path: shape_layer_path.clone(),
								bez_path: shape_to_draw.path.clone(),
								transform: shape_to_draw.transform.to_cols_array(),
							}
							.into(),
						);
						responses.push_back(
							Operation::SetLayerVisibility {
								path: shape_layer_path.clone(),
								visible: true,
							}
							.into(),
						);
						shape_i += 1;

						for segment in &shape_to_draw.segments {
							// TODO: We draw each anchor point twice because segment has it on both ends, fix this
							let (anchors, handles, anchor_handle_lines) = match segment {
								VectorManipulatorSegment::Line(a1, a2) => (vec![*a1, *a2], vec![], vec![]),
								VectorManipulatorSegment::Quad(a1, h1, a2) => (vec![*a1, *a2], vec![*h1], vec![(*h1, *a1)]),
								VectorManipulatorSegment::Cubic(a1, h1, h2, a2) => (vec![*a1, *a2], vec![*h1, *h2], vec![(*h1, *a1), (*h2, *a2)]),
							};

							// Draw the line connecting the anchor with handle for cubic and quadratic bezier segments
							for anchor_handle_line in anchor_handle_lines {
								let marker = data.anchor_handle_line_pool[line_i].clone();

								let line_vector = anchor_handle_line.0 - anchor_handle_line.1;

								let scale = DVec2::splat(line_vector.length());
								let angle = -line_vector.angle_between(DVec2::X);
								let translation = (anchor_handle_line.1 + BIAS).round() + DVec2::splat(0.5);
								let transform = DAffine2::from_scale_angle_translation(scale, angle, translation).to_cols_array();

								responses.push_back(Operation::SetLayerTransformInViewport { path: marker.clone(), transform }.into());
								responses.push_back(Operation::SetLayerVisibility { path: marker, visible: true }.into());

								line_i += 1;
							}

							// Draw the draggable square points on the end of every line segment or bezier curve segment
							for anchor in anchors {
								let scale = DVec2::splat(VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE);
								let angle = 0.;
								let translation = (anchor - (scale / 2.) + BIAS).round();
								let transform = DAffine2::from_scale_angle_translation(scale, angle, translation).to_cols_array();

								let marker = &data.anchor_marker_pool[anchor_i];
								responses.push_back(Operation::SetLayerTransformInViewport { path: marker.clone(), transform }.into());
								responses.push_back(Operation::SetLayerVisibility { path: marker.clone(), visible: true }.into());
								anchor_i += 1;
							}

							// Draw the draggable handle for cubic and quadratic bezier segments
							for handle in handles {
								let marker = &data.handle_marker_pool[handle_i];

								let scale = DVec2::splat(VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE);
								let angle = 0.;
								let translation = (handle - (scale / 2.) + BIAS).round();
								let transform = DAffine2::from_scale_angle_translation(scale, angle, translation).to_cols_array();

								responses.push_back(Operation::SetLayerTransformInViewport { path: marker.clone(), transform }.into());
								responses.push_back(Operation::SetLayerVisibility { path: marker.clone(), visible: true }.into());

								handle_i += 1;
							}
						}
					}

					// Hide the remaining pooled overlays
					for i in anchor_i..data.anchor_marker_pool.len() {
						let marker = data.anchor_marker_pool[i].clone();
						responses.push_back(Operation::SetLayerVisibility { path: marker, visible: false }.into());
					}
					for i in handle_i..data.handle_marker_pool.len() {
						let marker = data.handle_marker_pool[i].clone();
						responses.push_back(Operation::SetLayerVisibility { path: marker, visible: false }.into());
					}
					for i in line_i..data.anchor_handle_line_pool.len() {
						let line = data.anchor_handle_line_pool[i].clone();
						responses.push_back(Operation::SetLayerVisibility { path: line, visible: false }.into());
					}
					for i in shape_i..data.shape_outline_pool.len() {
						let shape_i = data.shape_outline_pool[i].clone();
						responses.push_back(Operation::SetLayerVisibility { path: shape_i, visible: false }.into());
					}

					self
				}
				(_, MouseDown) => {
					// todo: DRY refactor (this arm is very similar to the (_, RedrawOverlay) arm)
					// WIP: selecting control point working
					// next: correctly modifying path
					// to test: E, make ellipse, A, click control point, see it change color
					// todo: use cargo clippy?

					let mouse_pos = input.mouse.position;
					let mut points = Vec::new();

					let (mut anchor_i, mut handle_i, _line_i, mut shape_i) = (0, 0, 0, 0);
					let shapes_to_draw = document.selected_layers_vector_points();
					let (total_anchors, total_handles, _total_anchor_handle_lines) = calculate_total_overlays_per_type(&shapes_to_draw);
					grow_overlay_pool_entries(&mut data.anchor_marker_pool, total_anchors, add_anchor_marker, responses);
					grow_overlay_pool_entries(&mut data.handle_marker_pool, total_handles, add_handle_marker, responses);

					#[derive(Debug)]
					enum PointRef {
						Anchor(usize),
						Handle(usize),
					}
					#[derive(Debug)]
					struct Point(DVec2, PointRef, f64);

					// todo: use const?
					// use crate::consts::SELECTION_TOLERANCE;
					// let tolerance = DVec2::splat(SELECTION_TOLERANCE);
					// let quad = Quad::from_box([mouse_pos - tolerance, mouse_pos + tolerance]);
					let select_threshold = 6.;
					let select_threshold_squared = select_threshold * select_threshold;

					for shape_to_draw in &shapes_to_draw {
						let shape_layer_path = &data.shape_outline_pool[shape_i];
						shape_i += 1;

						let bez = {
							use kurbo::PathEl;
							let mut bez: Vec<_> = shape_to_draw.path.clone().into_iter().collect();
							for a in &mut bez {
								match a {
									PathEl::LineTo(p) => {
										p.x += 5.;
									}
									_ => {}
								}
							}
							let bez: BezPath = bez.into_iter().collect();
							bez
						};

						// todo: change path correctly
						responses.push_back(
							Operation::SetShapePathInViewport {
								path: shape_layer_path.clone(),
								bez_path: bez,
								transform: shape_to_draw.transform.to_cols_array(),
							}
							.into(),
						);

						for segment in &shape_to_draw.segments {
							// TODO: We draw each anchor point twice because segment has it on both ends, fix this
							// TODO: DRY, see above
							let (anchors, handles, _anchor_handle_lines) = match segment {
								VectorManipulatorSegment::Line(a1, a2) => (vec![*a1, *a2], vec![], vec![]),
								VectorManipulatorSegment::Quad(a1, h1, a2) => (vec![*a1, *a2], vec![*h1], vec![(*h1, *a1)]),
								VectorManipulatorSegment::Cubic(a1, h1, h2, a2) => (vec![*a1, *a2], vec![*h1, *h2], vec![(*h1, *a1), (*h2, *a2)]),
							};

							for anchor in anchors {
								let d2 = mouse_pos.distance_squared(anchor.into());
								if d2 < select_threshold_squared {
									points.push(Point(anchor.clone(), PointRef::Anchor(anchor_i), d2));
								}
								anchor_i += 1;
							}
							for handle in handles {
								let d2 = mouse_pos.distance_squared(handle.into());
								if d2 < select_threshold_squared {
									points.push(Point(handle.clone(), PointRef::Handle(handle_i), d2));
								}
								handle_i += 1;
							}
						}
					}

					points.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
					let closest_point_within_click_threshold = points.first();
					// log::debug!("PATH TOOL: {:?} {:?}", mouse_pos, points);

					if let Some(point) = closest_point_within_click_threshold {
						let path = match point.1 {
							PointRef::Anchor(i) => data.anchor_marker_pool[i].clone(),
							PointRef::Handle(i) => data.handle_marker_pool[i].clone(),
						};
						// todo: use Operation::SetShapePathInViewport instead
						// 	currently using SetLayerFill just to show some effect
						responses.push_back(
							Operation::SetLayerFill {
								path,
								color: Color::from_rgb8(if mouse_pos.x > 500. { 0 } else { 255 }, if mouse_pos.x > 500. { 255 } else { 0 }, 0),
							}
							.into(),
						);
					}

					self
				}
				(_, Abort) => {
					// Destory the overlay layer pools
					while let Some(layer) = data.anchor_marker_pool.pop() {
						responses.push_back(Operation::DeleteLayer { path: layer }.into());
					}
					while let Some(layer) = data.handle_marker_pool.pop() {
						responses.push_back(Operation::DeleteLayer { path: layer }.into());
					}
					while let Some(layer) = data.anchor_handle_line_pool.pop() {
						responses.push_back(Operation::DeleteLayer { path: layer }.into());
					}
					while let Some(layer) = data.shape_outline_pool.pop() {
						responses.push_back(Operation::DeleteLayer { path: layer }.into());
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
		};

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}
}

fn calculate_total_overlays_per_type(shapes_to_draw: &Vec<VectorManipulatorShape>) -> (usize, usize, usize) {
	let (mut total_anchors, mut total_handles, mut total_anchor_handle_lines) = (0, 0, 0);

	for shape_to_draw in shapes_to_draw {
		for segment in &shape_to_draw.segments {
			let (anchors, handles, anchor_handle_lines) = match segment {
				VectorManipulatorSegment::Line(_, _) => (2, 0, 0),
				VectorManipulatorSegment::Quad(_, _, _) => (2, 1, 1),
				VectorManipulatorSegment::Cubic(_, _, _, _) => (2, 2, 2),
			};
			total_anchors += anchors;
			total_handles += handles;
			total_anchor_handle_lines += anchor_handle_lines;
		}
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
	responses.push_back(
		Operation::AddOverlayRect {
			path: layer_path.clone(),
			transform: DAffine2::IDENTITY.to_cols_array(),
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 2.0)), Some(Fill::new(Color::WHITE))),
		}
		.into(),
	);

	layer_path
}

fn add_handle_marker(responses: &mut VecDeque<Message>) -> Vec<LayerId> {
	let layer_path = vec![generate_uuid()];
	responses.push_back(
		Operation::AddOverlayEllipse {
			path: layer_path.clone(),
			transform: DAffine2::IDENTITY.to_cols_array(),
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 2.0)), Some(Fill::new(Color::WHITE))),
		}
		.into(),
	);

	layer_path
}

fn add_anchor_handle_line(responses: &mut VecDeque<Message>) -> Vec<LayerId> {
	let layer_path = vec![generate_uuid()];
	responses.push_back(
		Operation::AddOverlayLine {
			path: layer_path.clone(),
			transform: DAffine2::IDENTITY.to_cols_array(),
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), Some(Fill::none())),
		}
		.into(),
	);

	layer_path
}

fn add_shape_outline(responses: &mut VecDeque<Message>) -> Vec<LayerId> {
	let layer_path = vec![generate_uuid()];
	responses.push_back(
		Operation::AddOverlayShape {
			path: layer_path.clone(),
			bez_path: BezPath::default(),
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), Some(Fill::none())),
		}
		.into(),
	);

	layer_path
}
