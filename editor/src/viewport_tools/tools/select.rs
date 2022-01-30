use crate::consts::{COLOR_ACCENT, SELECTION_DRAG_ANGLE, SELECTION_TOLERANCE, VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE};
use crate::document::transformation::{OriginalTransforms, Selected};
use crate::document::utility_types::{AlignAggregate, AlignAxis, FlipAxis};
use crate::document::DocumentMessageHandler;
use crate::frontend::utility_types::MouseCursorIcon;
use crate::input::keyboard::{Key, MouseMotion};
use crate::input::mouse::ViewportPosition;
use crate::input::InputPreprocessorMessageHandler;
use crate::message_prelude::*;
use crate::misc::{HintData, HintGroup, HintInfo, KeysGroup};
use crate::viewport_tools::snapping::SnapHandler;
use crate::viewport_tools::tool::{DocumentToolData, Fsm, ToolActionHandlerData};

use graphene::color::Color;
use graphene::document::Document;
use graphene::intersection::Quad;
use graphene::layers::style::{self, Fill, Stroke};
use graphene::Operation;

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct Select {
	fsm_state: SelectToolFsmState,
	data: SelectToolData,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Select)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum SelectMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,
	#[remain::unsorted]
	DocumentIsDirty,

	// Tool-specific messages
	Align {
		axis: AlignAxis,
		aggregate: AlignAggregate,
	},
	DragStart {
		add_to_selection: Key,
	},
	DragStop,
	FlipHorizontal,
	FlipVertical,
	MouseMove {
		snap_angle: Key,
	},
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Select {
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

	fn actions(&self) -> ActionList {
		use SelectToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(SelectMessageDiscriminant; DragStart),
			Dragging => actions!(SelectMessageDiscriminant; DragStop, MouseMove),
			DrawingBox => actions!(SelectMessageDiscriminant; DragStop, MouseMove, Abort),
			ResizingBounds => actions!(SelectMessageDiscriminant; DragStop, MouseMove, Abort),
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum SelectToolFsmState {
	Ready,
	Dragging,
	DrawingBox,
	ResizingBounds,
}

impl Default for SelectToolFsmState {
	fn default() -> Self {
		SelectToolFsmState::Ready
	}
}

#[derive(Clone, Debug, Default)]
struct SelectToolData {
	drag_start: ViewportPosition,
	drag_current: ViewportPosition,
	layers_dragging: Vec<Vec<LayerId>>, // Paths and offsets
	drag_box_overlay_layer: Option<Vec<LayerId>>,
	bounding_box_overlays: Option<BoundingBoxOverlays>,
	snap_handler: SnapHandler,
}

impl SelectToolData {
	fn selection_quad(&self) -> Quad {
		let bbox = self.selection_box();
		Quad::from_box(bbox)
	}

	fn selection_box(&self) -> [DVec2; 2] {
		if self.drag_current == self.drag_start {
			let tolerance = DVec2::splat(SELECTION_TOLERANCE);
			[self.drag_start - tolerance, self.drag_start + tolerance]
		} else {
			[self.drag_start, self.drag_current]
		}
	}
}

/// Handles the selected edges whilst dragging the layer bounds
#[derive(Clone, Debug, Default)]
struct SelectedEdges {
	original_transforms: OriginalTransforms,
	pivot: DVec2,
	bounds: [DVec2; 2],
	top: bool,
	bottom: bool,
	left: bool,
	right: bool,
}
impl SelectedEdges {
	fn new(top: bool, bottom: bool, left: bool, right: bool, bounds: [DVec2; 2]) -> Self {
		// Calculate the pivot for the operation (the opposite point to the one being dragged)
		let pivot = {
			let min = bounds[0];
			let max = bounds[1];

			let x = if left {
				max.x
			} else if right {
				min.x
			} else {
				(min.x + max.x) / 2.
			};

			let y = if top {
				max.y
			} else if bottom {
				min.y
			} else {
				(min.y + max.y) / 2.
			};

			DVec2::new(x, y)
		};

		Self {
			original_transforms: Default::default(),
			pivot,
			top,
			bottom,
			left,
			right,
			bounds,
		}
	}

	/// Calculates the required scaling to resize the bounding box
	fn pos_to_scale_transform(&self, mouse: DVec2) -> DAffine2 {
		let mut min = self.bounds[0];
		let mut max = self.bounds[1];
		if self.top {
			min.y = mouse.y;
		} else if self.bottom {
			max.y = mouse.y;
		}
		if self.left {
			min.x = mouse.x
		} else if self.right {
			max.x = mouse.x;
		}
		DAffine2::from_scale((max - min) / (self.bounds[1] - self.bounds[0]))
	}

	/// Transforms the layers to handle dragging the edge
	pub fn transform_layers(&mut self, mouse: DVec2, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		let delta = self.pos_to_scale_transform(mouse);

		let selected = document.selected_layers().map(|path| path.to_vec()).collect();
		let mut selected = Selected::new(&mut self.original_transforms, &mut self.pivot, selected, responses, &document.graphene_document);

		selected.update_transforms(delta);
	}
}

const SELECT_THRESHOLD: f64 = 20.;

fn add_bounding_box(responses: &mut Vec<Message>) -> Vec<LayerId> {
	let path = vec![generate_uuid()];

	let operation = Operation::AddOverlayRect {
		path: path.clone(),
		transform: DAffine2::ZERO.to_cols_array(),
		style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), Some(Fill::none())),
	};
	responses.push(DocumentMessage::Overlays(operation.into()).into());

	path
}

fn evaluate_transform_handle_positions((left, top): (f64, f64), (right, bottom): (f64, f64)) -> [DVec2; 8] {
	[
		DVec2::new(left, top),
		DVec2::new(left, (top + bottom) / 2.),
		DVec2::new(left, bottom),
		DVec2::new((left + right) / 2., top),
		DVec2::new((left + right) / 2., bottom),
		DVec2::new(right, top),
		DVec2::new(right, (top + bottom) / 2.),
		DVec2::new(right, bottom),
	]
}

fn add_transform_handles(responses: &mut Vec<Message>) -> [Vec<LayerId>; 8] {
	const EMPTY_VEC: Vec<LayerId> = Vec::new();
	let mut transform_handle_paths = [EMPTY_VEC; 8];

	for item in &mut transform_handle_paths {
		let current_path = vec![generate_uuid()];

		let operation = Operation::AddOverlayRect {
			path: current_path.clone(),
			transform: DAffine2::ZERO.to_cols_array(),
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 2.0)), Some(Fill::new(Color::WHITE))),
		};
		responses.push(DocumentMessage::Overlays(operation.into()).into());

		*item = current_path;
	}

	transform_handle_paths
}

fn transform_from_box(pos1: DVec2, pos2: DVec2) -> [f64; 6] {
	DAffine2::from_scale_angle_translation((pos2 - pos1).round(), 0., pos1.round() - DVec2::splat(0.5)).to_cols_array()
}

/// Contains info on the overlays for the bounding box and transform handles
#[derive(Clone, Debug, Default)]
struct BoundingBoxOverlays {
	pub bounding_box: Vec<LayerId>,
	pub transform_handles: [Vec<LayerId>; 8],
	pub bounds: [DVec2; 2],
	pub selected_edges: Option<SelectedEdges>,
}

impl BoundingBoxOverlays {
	#[must_use]
	pub fn new(buffer: &mut Vec<Message>) -> Self {
		Self {
			bounding_box: add_bounding_box(buffer),
			transform_handles: add_transform_handles(buffer),
			..Default::default()
		}
	}

	/// Update the position of the bounding box and transform handles
	pub fn transform(&mut self, buffer: &mut Vec<Message>) {
		let transform = transform_from_box(self.bounds[0], self.bounds[1]);
		let path = self.bounding_box.clone();
		buffer.push(DocumentMessage::Overlays(Operation::SetLayerTransformInViewport { path, transform }.into()).into());

		// Helps push values that end in approximately half, plus or minus some floating point imprecision, towards the same side of the round() function
		const BIAS: f64 = 0.0001;

		for (position, path) in evaluate_transform_handle_positions(self.bounds[0].into(), self.bounds[1].into())
			.into_iter()
			.zip(&self.transform_handles)
		{
			let scale = DVec2::splat(VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE);
			let translation = (position - (scale / 2.) - 0.5 + BIAS).round();
			let transform = DAffine2::from_scale_angle_translation(scale, 0., translation).to_cols_array();
			let path = path.clone();
			buffer.push(DocumentMessage::Overlays(Operation::SetLayerTransformInViewport { path, transform }.into()).into());
		}
	}

	/// Check if the user has selected the edge for dragging (returns which edge in order top, bottom, left, right)
	pub fn check_select(&mut self, cursor: DVec2) -> Option<(bool, bool, bool, bool)> {
		let min = self.bounds[0].min(self.bounds[1]);
		let max = self.bounds[0].max(self.bounds[1]);
		if min.x - cursor.x < SELECT_THRESHOLD && min.y - cursor.y < SELECT_THRESHOLD && cursor.x - max.x < SELECT_THRESHOLD && cursor.y - max.y < SELECT_THRESHOLD {
			let top = (cursor.y - min.y).abs() < SELECT_THRESHOLD;
			let bottom = (max.y - cursor.y).abs() < SELECT_THRESHOLD;
			let left = (cursor.x - min.x).abs() < SELECT_THRESHOLD;
			let right = (cursor.x - max.x).abs() < SELECT_THRESHOLD;

			if top || bottom || left || right {
				self.selected_edges = Some(SelectedEdges::new(top, bottom, left, right, self.bounds));

				return Some((top, bottom, left, right));
			}
		}

		self.selected_edges = None;
		None
	}

	/// Removes the overlays
	pub fn delete(self, buffer: &mut impl Extend<Message>) {
		buffer.extend([DocumentMessage::Overlays(Operation::DeleteLayer { path: self.bounding_box }.into()).into()]);
		buffer.extend(
			self.transform_handles
				.iter()
				.map(|path| DocumentMessage::Overlays(Operation::DeleteLayer { path: path.clone() }.into()).into()),
		);
	}
}

impl Fsm for SelectToolFsmState {
	type ToolData = SelectToolData;

	fn transition(
		self,
		event: ToolMessage,
		document: &DocumentMessageHandler,
		_tool_data: &DocumentToolData,
		data: &mut Self::ToolData,
		input: &InputPreprocessorMessageHandler,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use SelectMessage::*;
		use SelectToolFsmState::*;

		if let ToolMessage::Select(event) = event {
			match (self, event) {
				(_, DocumentIsDirty) => {
					let mut buffer = Vec::new();
					match (document.selected_visible_layers_bounding_box(), data.bounding_box_overlays.take()) {
						(None, Some(bounding_box_overlays)) => bounding_box_overlays.delete(&mut buffer),
						(Some(bounds), paths) => {
							let mut bounding_box_overlays = paths.unwrap_or_else(|| BoundingBoxOverlays::new(&mut buffer));

							bounding_box_overlays.bounds = bounds;
							bounding_box_overlays.transform(&mut buffer);

							data.bounding_box_overlays = Some(bounding_box_overlays);
						}
						(_, _) => {}
					};
					buffer.into_iter().rev().for_each(|message| responses.push_front(message));
					self
				}
				(Ready, DragStart { add_to_selection }) => {
					data.drag_start = input.mouse.position;
					data.drag_current = input.mouse.position;
					let mut buffer = Vec::new();

					let dragging_bounds = if let Some(bounding_box) = &mut data.bounding_box_overlays {
						bounding_box.check_select(input.mouse.position)
					} else {
						None
					};

					let mut selected: Vec<_> = document.selected_visible_layers().map(|path| path.to_vec()).collect();
					let quad = data.selection_quad();
					let mut intersection = document.graphene_document.intersects_quad_root(quad);
					// If the user is dragging the bounding box bounds, go into ResizingBounds mode.
					// If the user clicks on a layer that is in their current selection, go into the dragging mode.
					// If the user clicks on new shape, make that layer their new selection.
					// Otherwise enter the box select mode
					let state = if let Some(selected_edges) = dragging_bounds {
						let snap_x = selected_edges.2 || selected_edges.3;
						let snap_y = selected_edges.0 || selected_edges.1;

						data.snap_handler
							.start_snap(document, document.visible_layers().filter(|layer| !selected.iter().any(|path| path == layer)), snap_x, snap_y);

						data.layers_dragging = selected;

						ResizingBounds
					} else if selected.iter().any(|path| intersection.contains(path)) {
						buffer.push(DocumentMessage::StartTransaction.into());
						data.layers_dragging = selected;

						data.snap_handler
							.start_snap(document, document.visible_layers().filter(|layer| !data.layers_dragging.iter().any(|path| path == layer)), true, true);

						Dragging
					} else {
						if !input.keyboard.get(add_to_selection as usize) {
							buffer.push(DocumentMessage::DeselectAllLayers.into());
							data.layers_dragging.clear();
						}

						if let Some(intersection) = intersection.pop() {
							selected = vec![intersection];
							buffer.push(DocumentMessage::AddSelectedLayers { additional_layers: selected.clone() }.into());
							buffer.push(DocumentMessage::StartTransaction.into());
							data.layers_dragging.append(&mut selected);
							data.snap_handler
								.start_snap(document, document.visible_layers().filter(|layer| !data.layers_dragging.iter().any(|path| path == layer)), true, true);

							Dragging
						} else {
							data.drag_box_overlay_layer = Some(add_bounding_box(&mut buffer));
							DrawingBox
						}
					};
					buffer.into_iter().rev().for_each(|message| responses.push_front(message));

					log::info!("State {:?}", state);

					state
				}
				(Dragging, MouseMove { snap_angle }) => {
					// TODO: This is a cheat. Break out the relevant functionality from the handler above and call it from there and here.
					responses.push_front(SelectMessage::DocumentIsDirty.into());

					let mouse_position = if input.keyboard.get(snap_angle as usize) {
						let mouse_position = input.mouse.position - data.drag_start;
						let snap_resolution = SELECTION_DRAG_ANGLE.to_radians();
						let angle = -mouse_position.angle_between(DVec2::X);
						let snapped_angle = (angle / snap_resolution).round() * snap_resolution;
						DVec2::new(snapped_angle.cos(), snapped_angle.sin()) * mouse_position.length() + data.drag_start
					} else {
						input.mouse.position
					};

					let mouse_delta = mouse_position - data.drag_current;

					let closest_move = data.snap_handler.snap_layers(responses, document, &data.layers_dragging, input.viewport_bounds.size(), mouse_delta);
					// TODO: Cache the result of `shallowest_unique_layers` to avoid this heavy computation every frame of movement, see https://github.com/GraphiteEditor/Graphite/pull/481
					for path in Document::shallowest_unique_layers(data.layers_dragging.iter()) {
						responses.push_front(
							Operation::TransformLayerInViewport {
								path: path.clone(),
								transform: DAffine2::from_translation(mouse_delta + closest_move).to_cols_array(),
							}
							.into(),
						);
					}
					data.drag_current = mouse_position + closest_move;
					Dragging
				}
				(ResizingBounds, MouseMove { .. }) => {
					if let Some(bounds) = &mut data.bounding_box_overlays {
						if let Some(movement) = &mut bounds.selected_edges {
							let mouse_position = input.mouse.position;

							let snapped_mouse_position = data.snap_handler.snap_position(responses, input.viewport_bounds.size(), document, mouse_position);

							movement.transform_layers(snapped_mouse_position, document, responses);
						}
					}
					ResizingBounds
				}
				(DrawingBox, MouseMove { .. }) => {
					data.drag_current = input.mouse.position;

					responses.push_front(
						DocumentMessage::Overlays(
							Operation::SetLayerTransformInViewport {
								path: data.drag_box_overlay_layer.clone().unwrap(),
								transform: transform_from_box(data.drag_start, data.drag_current),
							}
							.into(),
						)
						.into(),
					);
					DrawingBox
				}
				(Dragging, DragStop) => {
					let response = match input.mouse.position.distance(data.drag_start) < 10. * f64::EPSILON {
						true => DocumentMessage::Undo,
						false => DocumentMessage::CommitTransaction,
					};
					data.snap_handler.cleanup(responses);
					responses.push_front(response.into());
					Ready
				}
				(ResizingBounds, DragStop) => {
					let response = match input.mouse.position.distance(data.drag_start) < 10. * f64::EPSILON {
						true => DocumentMessage::Undo,
						false => DocumentMessage::CommitTransaction,
					};
					data.snap_handler.cleanup(responses);
					responses.push_front(response.into());
					Ready
				}
				(DrawingBox, DragStop) => {
					let quad = data.selection_quad();
					responses.push_front(
						DocumentMessage::AddSelectedLayers {
							additional_layers: document.graphene_document.intersects_quad_root(quad),
						}
						.into(),
					);
					responses.push_front(
						DocumentMessage::Overlays(
							Operation::DeleteLayer {
								path: data.drag_box_overlay_layer.take().unwrap(),
							}
							.into(),
						)
						.into(),
					);
					Ready
				}
				(_, Abort) => {
					if let Some(path) = data.drag_box_overlay_layer.take() {
						responses.push_front(DocumentMessage::Overlays(Operation::DeleteLayer { path }.into()).into())
					};
					if let Some(bounding_box_overlays) = data.bounding_box_overlays.take() {
						bounding_box_overlays.delete(responses);
					}

					data.snap_handler.cleanup(responses);
					Ready
				}
				(_, Align { axis, aggregate }) => {
					responses.push_back(DocumentMessage::AlignSelectedLayers { axis, aggregate }.into());

					self
				}
				(_, FlipHorizontal) => {
					responses.push_back(DocumentMessage::FlipSelectedLayers { flip_axis: FlipAxis::X }.into());

					self
				}
				(_, FlipVertical) => {
					responses.push_back(DocumentMessage::FlipSelectedLayers { flip_axis: FlipAxis::Y }.into());

					self
				}
				_ => self,
			}
		} else {
			self
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			SelectToolFsmState::Ready => HintData(vec![
				HintGroup(vec![HintInfo {
					key_groups: vec![],
					mouse: Some(MouseMotion::LmbDrag),
					label: String::from("Drag Selected"),
					plus: false,
				}]),
				HintGroup(vec![
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyG])],
						mouse: None,
						label: String::from("Grab Selected"),
						plus: false,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyR])],
						mouse: None,
						label: String::from("Rotate Selected"),
						plus: false,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyS])],
						mouse: None,
						label: String::from("Scale Selected"),
						plus: false,
					},
				]),
				HintGroup(vec![
					HintInfo {
						key_groups: vec![],
						mouse: Some(MouseMotion::Lmb),
						label: String::from("Select Object"),
						plus: false,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyControl])],
						mouse: None,
						label: String::from("Innermost"),
						plus: true,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyShift])],
						mouse: None,
						label: String::from("Grow/Shrink Selection"),
						plus: true,
					},
				]),
				HintGroup(vec![
					HintInfo {
						key_groups: vec![],
						mouse: Some(MouseMotion::LmbDrag),
						label: String::from("Select Area"),
						plus: false,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyShift])],
						mouse: None,
						label: String::from("Grow/Shrink Selection"),
						plus: true,
					},
				]),
				HintGroup(vec![
					HintInfo {
						key_groups: vec![
							KeysGroup(vec![Key::KeyArrowUp]),
							KeysGroup(vec![Key::KeyArrowRight]),
							KeysGroup(vec![Key::KeyArrowDown]),
							KeysGroup(vec![Key::KeyArrowLeft]),
						],
						mouse: None,
						label: String::from("Nudge Selected"),
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
						key_groups: vec![KeysGroup(vec![Key::KeyAlt])],
						mouse: Some(MouseMotion::LmbDrag),
						label: String::from("Move Duplicate"),
						plus: false,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyControl, Key::KeyD])],
						mouse: None,
						label: String::from("Duplicate"),
						plus: false,
					},
				]),
			]),
			SelectToolFsmState::Dragging => HintData(vec![HintGroup(vec![
				HintInfo {
					key_groups: vec![KeysGroup(vec![Key::KeyShift])],
					mouse: None,
					label: String::from("Constrain to Axis"),
					plus: false,
				},
				HintInfo {
					key_groups: vec![KeysGroup(vec![Key::KeyControl])],
					mouse: None,
					label: String::from("Snap to Points (coming soon)"),
					plus: false,
				},
			])]),
			SelectToolFsmState::DrawingBox => HintData(vec![]),
			SelectToolFsmState::ResizingBounds => HintData(vec![]),
		};

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default }.into());
	}
}
