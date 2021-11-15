use graphene::layers::style;
use graphene::layers::style::Fill;
use graphene::layers::style::Stroke;
use graphene::Operation;
use graphene::Quad;

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};

use crate::consts::COLOR_ACCENT;
use crate::input::keyboard::Key;
use crate::input::{mouse::ViewportPosition, InputPreprocessor};
use crate::tool::{DocumentToolData, Fsm, ToolActionHandlerData};
use crate::{
	consts::SELECTION_TOLERANCE,
	document::{AlignAggregate, AlignAxis, DocumentMessageHandler, FlipAxis},
	message_prelude::*,
};

#[derive(Default)]
pub struct Select {
	fsm_state: SelectToolFsmState,
	data: SelectToolData,
}

#[impl_message(Message, ToolMessage, Select)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum SelectMessage {
	DragStart { add_to_selection: Key },
	DragStop,
	MouseMove,
	Abort,
	UpdateSelectionBoundingBox,

	Align(AlignAxis, AlignAggregate),
	FlipHorizontal,
	FlipVertical,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Select {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		self.fsm_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, data.2, responses);
	}
	fn actions(&self) -> ActionList {
		use SelectToolFsmState::*;
		match self.fsm_state {
			Ready => actions!(SelectMessageDiscriminant; DragStart),
			Dragging => actions!(SelectMessageDiscriminant; DragStop, MouseMove),
			DrawingBox => actions!(SelectMessageDiscriminant; DragStop, MouseMove, Abort),
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelectToolFsmState {
	Ready,
	Dragging,
	DrawingBox,
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
	drag_box_id: Option<Vec<LayerId>>,
	bounding_box_path: Option<Vec<LayerId>>,
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

fn add_bounding_box(responses: &mut Vec<Message>) -> Vec<LayerId> {
	let path = vec![generate_uuid()];
	responses.push(
		Operation::AddOverlayRect {
			path: path.clone(),
			transform: DAffine2::ZERO.to_cols_array(),
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), Some(Fill::none())),
		}
		.into(),
	);

	path
}

fn transform_from_box(pos1: DVec2, pos2: DVec2) -> [f64; 6] {
	DAffine2::from_scale_angle_translation(pos2 - pos1, 0., pos1).to_cols_array()
}

impl Fsm for SelectToolFsmState {
	type ToolData = SelectToolData;

	fn transition(
		self,
		event: ToolMessage,
		document: &mut DocumentMessageHandler,
		_tool_data: &DocumentToolData,
		data: &mut Self::ToolData,
		input: &InputPreprocessor,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use SelectMessage::*;
		use SelectToolFsmState::*;
		if let ToolMessage::Select(event) = event {
			match (self, event) {
				(_, UpdateSelectionBoundingBox) => {
					let mut buffer = Vec::new();
					let response = match (document.selected_layers_bounding_box(), data.bounding_box_path.take()) {
						(None, Some(path)) => Operation::DeleteLayer { path }.into(),
						(Some([pos1, pos2]), path) => {
							let path = path.unwrap_or_else(|| add_bounding_box(&mut buffer));

							data.bounding_box_path = Some(path.clone());

							let half_pixel_offset = DVec2::splat(0.5);
							let pos1 = pos1 + half_pixel_offset;
							let pos2 = pos2 - half_pixel_offset;
							let transform = transform_from_box(pos1, pos2);

							Operation::SetLayerTransformInViewport { path, transform }.into()
						}
						(_, _) => Message::NoOp,
					};
					responses.push_front(response);
					buffer.into_iter().rev().for_each(|message| responses.push_front(message));
					self
				}
				(Ready, DragStart { add_to_selection }) => {
					data.drag_start = input.mouse.position;
					data.drag_current = input.mouse.position;
					let mut buffer = Vec::new();
					let mut selected: Vec<_> = document.selected_layers().map(|path| path.to_vec()).collect();
					let quad = data.selection_quad();
					let intersection = document.graphene_document.intersects_quad_root(quad);
					// If no layer is currently selected and the user clicks on a shape, select that.
					if selected.is_empty() {
						if let Some(layer) = intersection.last() {
							selected.push(layer.clone());
							buffer.push(DocumentMessage::SetSelectedLayers(selected.clone()).into());
						}
					}
					// If the user clicks on a layer that is in their current selection, go into the dragging mode.
					// Otherwise enter the box select mode
					let state = if selected.iter().any(|path| intersection.contains(path)) {
						buffer.push(DocumentMessage::StartTransaction.into());
						data.layers_dragging = selected;
						Dragging
					} else {
						if !input.keyboard.get(add_to_selection as usize) {
							buffer.push(DocumentMessage::DeselectAllLayers.into());
						}
						data.drag_box_id = Some(add_bounding_box(&mut buffer));
						DrawingBox
					};
					buffer.into_iter().rev().for_each(|message| responses.push_front(message));

					// ToDo: Clean up cloning
					let ignore_layers = match data.bounding_box_path.as_ref() {
						Some(bounding_box) => Vec::from([bounding_box.to_vec()]),
						None => Vec::new(),
					};
					document.snapping_handler.start_snap(&document.graphene_document, document.non_selected_layers_sorted(), &ignore_layers);
					state
				}
				(Dragging, MouseMove) => {
					responses.push_front(SelectMessage::UpdateSelectionBoundingBox.into());

					let mouse_delta = input.mouse.position - data.drag_current;

					let closest_move = document.snapping_handler.snap_layers(&document.graphene_document, &data.layers_dragging, mouse_delta);
					for path in data.layers_dragging.iter() {
						responses.push_front(
							Operation::TransformLayerInViewport {
								path: path.clone(),
								transform: DAffine2::from_translation(input.mouse.position - data.drag_current + closest_move).to_cols_array(),
							}
							.into(),
						);
					}
					data.drag_current = input.mouse.position + closest_move;
					Dragging
				}
				(DrawingBox, MouseMove) => {
					data.drag_current = input.mouse.position;
					let half_pixel_offset = DVec2::splat(0.5);
					let start = data.drag_start + half_pixel_offset;
					let size = data.drag_current - start + half_pixel_offset;

					responses.push_front(
						Operation::SetLayerTransformInViewport {
							path: data.drag_box_id.clone().unwrap(),
							transform: DAffine2::from_scale_angle_translation(size, 0., start).to_cols_array(),
						}
						.into(),
					);
					DrawingBox
				}
				(Dragging, DragStop) => {
					let response = match input.mouse.position.distance(data.drag_start) < 10. * f64::EPSILON {
						true => DocumentMessage::Undo,
						false => DocumentMessage::CommitTransaction,
					};
					responses.push_front(response.into());
					Ready
				}
				(DrawingBox, DragStop) => {
					let quad = data.selection_quad();
					responses.push_front(DocumentMessage::AddSelectedLayers(document.graphene_document.intersects_quad_root(quad)).into());
					responses.push_front(
						Operation::DeleteLayer {
							path: data.drag_box_id.take().unwrap(),
						}
						.into(),
					);
					Ready
				}
				(_, Abort) => {
					let mut delete = |path: &mut Option<Vec<LayerId>>| path.take().map(|path| responses.push_front(Operation::DeleteLayer { path }.into()));
					delete(&mut data.drag_box_id);
					delete(&mut data.bounding_box_path);
					Ready
				}
				(_, Align(axis, aggregate)) => {
					responses.push_back(DocumentMessage::AlignSelectedLayers(axis, aggregate).into());

					self
				}
				(_, FlipHorizontal) => {
					responses.push_back(DocumentMessage::FlipSelectedLayers(FlipAxis::X).into());

					self
				}
				(_, FlipVertical) => {
					responses.push_back(DocumentMessage::FlipSelectedLayers(FlipAxis::Y).into());

					self
				}
				_ => self,
			}
		} else {
			self
		}
	}
}
